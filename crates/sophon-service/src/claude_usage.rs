//! How much of a Claude plan is used: its 5-hour session and weekly
//! windows, as Claude Code's own `/usage` shows them.
//!
//! Anthropic answers this for the account Claude Code is signed in to
//! (`/api/oauth/usage`), so it is asked with that sign-in: the access token
//! Claude Code keeps on this host (the Keychain on a Mac, else
//! `.credentials.json` in its folder). The token is read when it is needed,
//! sent to api.anthropic.com alone, and kept nowhere; it is never renewed
//! here (that is Claude Code's to do: a token renewed twice signs it out).
//!
//! A provider set up for another endpoint (a gateway, Bedrock, Vertex) has
//! no plan of Anthropic's to ask about and is left out.

use serde_json::{Value, json};
use std::{
    process::Stdio,
    sync::Mutex,
    time::{Duration, Instant},
};
use tokio::{io::AsyncWriteExt, process::Command, time::timeout};

const USAGE: &str = "https://api.anthropic.com/api/oauth/usage";
#[cfg(target_os = "macos")]
const KEYCHAIN: &str = "Claude Code-credentials";

/// A reading is good for this long: a refresh pressed twice asks once.
const FRESH: Duration = Duration::from_secs(60);

/// Claude Code's sign-in, as far as a usage query needs it.
#[derive(Debug, PartialEq)]
struct Login {
    token: String,
    /// When the token stops working (Unix milliseconds).
    expires: Option<i64>,
    plan: Option<String>,
}

/// `Max 20x` of `max` and `default_claude_max_20x`.
fn plan(subscription: Option<&str>, tier: Option<&str>) -> Option<String> {
    let subscription = subscription.filter(|name| !name.is_empty())?;
    let mut label = subscription.to_owned();
    label[..1].make_ascii_uppercase();
    match tier.and_then(|tier| tier.rsplit('_').next()).filter(|tier| tier.ends_with('x')) {
        Some(tier) => Some(format!("{label} {tier}")),
        None => Some(label),
    }
}

/// The sign-in in what Claude Code keeps (`{"claudeAiOauth": {...}}`).
fn login(kept: &str) -> Option<Login> {
    let kept: Value = serde_json::from_str(kept.trim()).ok()?;
    let oauth = &kept["claudeAiOauth"];
    let token = oauth["accessToken"].as_str()?;
    // A token goes into a header line: nothing but what a token is made of.
    if token.is_empty() || !token.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"-._~+/=".contains(&byte)) {
        return None;
    }
    Some(Login {
        token: token.to_owned(),
        expires: oauth["expiresAt"].as_i64(),
        plan: plan(oauth["subscriptionType"].as_str(), oauth["rateLimitTier"].as_str()),
    })
}

async fn output(program: &str, args: &[&str], input: Option<&str>) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .ok()?;
    if let (Some(input), Some(mut stdin)) = (input, child.stdin.take()) {
        stdin.write_all(input.as_bytes()).await.ok()?;
    }
    let done = timeout(Duration::from_secs(20), child.wait_with_output()).await.ok()?.ok()?;
    done.status.success().then(|| String::from_utf8_lossy(&done.stdout).into_owned())
}

/// The Keychain item Claude Code keeps its sign-in in: its usual one, or
/// for a folder of its own (`CLAUDE_CONFIG_DIR`) that name with the first
/// eight hex digits of the folder's SHA-256.
#[cfg(target_os = "macos")]
async fn keychain(settings: &Value) -> Option<Login> {
    let folder = settings
        .pointer("/env/CLAUDE_CONFIG_DIR")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| std::env::var("CLAUDE_CONFIG_DIR").ok());
    let service = match folder {
        Some(folder) => {
            let digest = output("/usr/bin/shasum", &["-a", "256"], Some(&folder)).await?;
            format!("{KEYCHAIN}-{}", digest.get(..8)?)
        }
        None => KEYCHAIN.to_owned(),
    };
    let user = std::env::var("USER").unwrap_or_default();
    let named = user.bytes().all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte));
    let account = if named && !user.is_empty() { user.as_str() } else { "claude-code-user" };
    for args in [
        vec!["find-generic-password", "-a", account, "-w", "-s", service.as_str()],
        vec!["find-generic-password", "-w", "-s", service.as_str()],
    ] {
        if let Some(found) = output("/usr/bin/security", &args, None).await.as_deref().and_then(login) {
            return Some(found);
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
async fn keychain(_settings: &Value) -> Option<Login> {
    None
}

/// Claude Code's sign-in on this host, for the provider with `settings`.
async fn signed_in(settings: &Value) -> Option<Login> {
    if let Some(found) = keychain(settings).await {
        return Some(found);
    }
    let file = crate::models::kept(settings, "CLAUDE_CONFIG_DIR", ".claude").join(".credentials.json");
    login(&tokio::fs::read_to_string(file).await.ok()?)
}

/// Whether the provider runs on Anthropic's own plan: not on another
/// endpoint, Bedrock or Vertex.
fn on_a_plan(settings: &Value) -> bool {
    let set = |name: &str| settings.pointer(&format!("/env/{name}")).and_then(Value::as_str).filter(|value| !value.is_empty());
    let on = |name: &str| set(name).is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"));
    let elsewhere = set("ANTHROPIC_BASE_URL").is_some_and(|url| url.trim_end_matches('/') != "https://api.anthropic.com");
    !(elsewhere || on("CLAUDE_CODE_USE_BEDROCK") || on("CLAUDE_CODE_USE_VERTEX"))
}

fn window(id: &str, label: &str, used: &Value, resets: &Value) -> Option<Value> {
    let used = used.as_f64().or_else(|| used.as_str()?.parse().ok())?;
    Some(json!({"id":id,"label":label,"usedPct":used,"resetsAt":resets.as_str()}))
}

/// A name as an id's letters: `Fable Pro` as `fable_pro`.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_ascii_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    out.trim_end_matches('_').to_owned()
}

/// The windows in Anthropic's answer: the session and the week for every
/// model, then each week that is one model's or one surface's own (named
/// in `limits`, or by an older answer's `seven_day_<model>`).
fn windows(answer: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    for (field, id, label) in [("five_hour", "five_hour", "Session"), ("seven_day", "weekly", "Weekly")] {
        out.extend(window(id, label, &answer[field]["utilization"], &answer[field]["resets_at"]));
    }
    let mut scoped: Vec<(String, String, Value, Value)> = Vec::new();
    for limit in answer["limits"].as_array().into_iter().flatten() {
        // The same two for every model, where only `limits` names them
        // (the answer is moving there from its fields).
        if limit["scope"].is_null() {
            let known = match limit["group"].as_str() {
                Some("weekly") => Some(("weekly", "Weekly")),
                Some("session" | "five_hour") => Some(("five_hour", "Session")),
                _ => None,
            };
            if let Some((id, label)) = known.filter(|(id, _)| !out.iter().any(|window| window["id"] == *id)) {
                out.extend(window(id, label, &limit["percent"], &limit["resets_at"]));
            }
            continue;
        }
        if limit["kind"] != "weekly_scoped" {
            continue;
        }
        for dimension in ["model", "surface"] {
            let scope = &limit["scope"][dimension];
            let id = scope["id"].as_str().map(str::trim).filter(|id| !id.is_empty());
            let Some(name) = scope["display_name"].as_str().map(str::trim).filter(|name| !name.is_empty()).or(id) else {
                continue;
            };
            scoped.push((
                format!("weekly_{dimension}_{}", id.map_or_else(|| slug(name), str::to_owned)),
                name.to_owned(),
                limit["percent"].clone(),
                limit["resets_at"].clone(),
            ));
            break;
        }
    }
    for (field, name) in [("seven_day_opus", "Opus"), ("seven_day_omelette", "Omelette")] {
        let id = format!("weekly_model_{}", slug(name));
        if answer[field].is_object() && !scoped.iter().any(|(known, ..)| *known == id) {
            scoped.push((id, name.to_owned(), answer[field]["utilization"].clone(), answer[field]["resets_at"].clone()));
        }
    }
    for (id, name, used, resets) in scoped {
        if !out.iter().any(|known| known["id"] == id.as_str()) {
            out.extend(window(&id, &format!("Weekly · {name}"), &used, &resets));
        }
    }
    out
}

/// What Anthropic answered a sign-in, and until when it asked not to be
/// asked again.
#[derive(Default)]
struct Kept {
    read: Option<(Instant, Vec<Value>)>,
    wait_until: Option<Instant>,
}

static KEPT: Mutex<Vec<(String, Kept)>> = Mutex::new(Vec::new());

fn kept<T>(key: &str, with: impl FnOnce(&mut Kept) -> T) -> T {
    let mut all = KEPT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !all.iter().any(|(known, _)| known == key) {
        all.push((key.to_owned(), Kept::default()));
    }
    with(&mut all.iter_mut().find(|(known, _)| known == key).expect("just added").1)
}

/// Asks Anthropic with `token`: the answer, or what to tell the person.
async fn ask(token: &str) -> Result<Value, (String, Option<Duration>)> {
    // The token goes to curl on its input, not among its arguments, which
    // any process on the host can read.
    let config = format!(
        "url = \"{USAGE}\"\nheader = \"Authorization: Bearer {token}\"\nheader = \"Accept: application/json\"\nheader = \"anthropic-beta: oauth-2025-04-20\"\nsilent\nmax-time = 15\nwrite-out = \"\\n%{{http_code}} %header{{retry-after}}\"\n"
    );
    let plain = |text: &str| (text.to_owned(), None);
    let out = output("curl", &["--config", "-"], Some(&config))
        .await
        .ok_or_else(|| plain("Could not reach Anthropic for Claude's usage"))?;
    let (body, status) = out.rsplit_once('\n').ok_or_else(|| plain("Anthropic's answer could not be read"))?;
    let mut status = status.split_whitespace();
    match status.next() {
        Some("200") => serde_json::from_str(body).map_err(|_| plain("Anthropic's answer could not be read")),
        Some("401" | "403") => Err(plain("Claude Code's sign-in was refused: sign in again in Claude Code")),
        Some("429") => {
            let wait = status.next().and_then(|seconds| seconds.parse::<u64>().ok()).map(Duration::from_secs);
            Err(("Anthropic limits how often usage is read: try again later".to_owned(), wait))
        }
        Some(code) => Err(plain(&format!("Anthropic answered {code} for Claude's usage"))),
        None => Err(plain("Anthropic's answer could not be read")),
    }
}

/// The usage of the Claude provider `name` (its settings in the service's
/// `config`), as an entry of `provider.usage.list.response`; none when it
/// is on no plan of Anthropic's or Claude Code is not signed in.
pub async fn read(name: &str, settings: &Value) -> Option<Value> {
    if settings["enabled"] == false || !on_a_plan(settings) {
        return None;
    }
    let login = signed_in(settings).await?;
    let label = settings["label"].as_str().filter(|label| !label.is_empty()).unwrap_or("Claude");
    let entry = |windows: Vec<Value>, error: Option<&str>| {
        json!({"providerId":name,"displayName":label,"planLabel":login.plan,"windows":windows,"error":error})
    };
    let now = Instant::now();
    let (recent, waiting) = kept(name, |kept| {
        (
            kept.read.clone().filter(|(at, _)| now.duration_since(*at) < FRESH).map(|(_, windows)| windows),
            kept.wait_until.is_some_and(|until| until > now),
        )
    });
    if let Some(windows) = recent {
        return Some(entry(windows, None));
    }
    let last = || kept(name, |kept| kept.read.clone().map(|(_, windows)| windows));
    if waiting {
        // Asked not to ask yet: the last reading, which still holds roughly.
        return Some(match last() {
            Some(windows) => entry(windows, None),
            None => entry(Vec::new(), Some("Anthropic limits how often usage is read: try again later")),
        });
    }
    let expired = login.expires.is_some_and(|at| at <= chrono::Utc::now().timestamp_millis());
    if expired {
        return Some(entry(
            last().unwrap_or_default(),
            Some("Claude Code's sign-in has run out: open Claude Code once to renew it"),
        ));
    }
    match ask(&login.token).await {
        Ok(answer) => {
            let windows = windows(&answer);
            kept(name, |kept| {
                kept.read = Some((Instant::now(), windows.clone()));
                kept.wait_until = None;
            });
            Some(entry(windows, None))
        }
        Err((error, wait)) => {
            if let Some(wait) = wait {
                kept(name, |kept| kept.wait_until = Some(Instant::now() + wait));
            }
            // A reading from before is shown on, with what went wrong.
            Some(entry(last().unwrap_or_default(), Some(&error)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sign_in_is_read_from_what_claude_code_keeps() {
        let kept = r#"{"claudeAiOauth":{"accessToken":"sk-ant-oat01-abc_DEF-123","expiresAt":1794999999000,
            "subscriptionType":"max","rateLimitTier":"default_claude_max_20x"}}"#;
        assert_eq!(
            login(kept),
            Some(Login { token: "sk-ant-oat01-abc_DEF-123".into(), expires: Some(1_794_999_999_000), plan: Some("Max 20x".into()) })
        );
        assert_eq!(plan(Some("pro"), None), Some("Pro".into()));
        assert_eq!(plan(None, Some("default_claude_max_5x")), None);
        // Not a token's letters: nothing that could end the header's line.
        assert_eq!(login(r#"{"claudeAiOauth":{"accessToken":"a\"\nurl = \"http://elsewhere"}}"#), None);
        assert_eq!(login(r#"{"claudeAiOauth":{}}"#), None);
        assert_eq!(login("not json"), None);
    }

    #[test]
    fn the_answer_s_windows_are_the_session_the_week_and_each_model_s_own() {
        let answer = json!({
            "five_hour": {"utilization": 42.0, "resets_at": "2026-11-18T15:00:00Z"},
            "seven_day": {"utilization": "17.5", "resets_at": "2026-11-22T08:00:00Z"},
            "seven_day_opus": {"utilization": 3, "resets_at": null},
            "seven_day_omelette": null,
            "limits": [
                {"kind": "weekly_scoped", "percent": 9, "resets_at": "2026-11-22T08:00:00Z",
                    "scope": {"model": {"id": null, "display_name": "Fable Pro"}}},
                {"kind": "weekly_scoped", "percent": 4, "scope": {"model": {"display_name": "Opus"}}},
                {"kind": "session", "percent": 42},
                "not a limit"
            ],
            "extra_usage": {"is_enabled": false}
        });
        let windows = windows(&answer);
        let read: Vec<(&str, &str, f64)> = windows
            .iter()
            .map(|window| (window["id"].as_str().unwrap(), window["label"].as_str().unwrap(), window["usedPct"].as_f64().unwrap()))
            .collect();
        assert_eq!(
            read,
            [
                ("five_hour", "Session", 42.0),
                ("weekly", "Weekly", 17.5),
                ("weekly_model_fable_pro", "Weekly · Fable Pro", 9.0),
                // Named both ways: the newer naming's reading, once.
                ("weekly_model_opus", "Weekly · Opus", 4.0),
            ]
        );
        assert_eq!(windows[0]["resetsAt"], "2026-11-18T15:00:00Z");
        assert!(super::windows(&json!({})).is_empty());
        // As Anthropic answers today: no session window, the week in both
        // places (read once), one model's week in `limits` alone.
        let today = json!({
            "five_hour": null,
            "seven_day": {"utilization": 21.0, "resets_at": "2026-11-23T14:00:00+00:00"},
            "limits": [
                {"group": "weekly", "kind": "weekly_all", "percent": 21, "resets_at": "2026-11-23T14:00:00+00:00", "scope": null},
                {"group": "weekly", "kind": "weekly_scoped", "percent": 29, "resets_at": "2026-11-23T14:00:00+00:00",
                    "scope": {"model": {"display_name": "Fable", "id": null}, "surface": null}}
            ]
        });
        let ids: Vec<_> = super::windows(&today).iter().map(|window| window["id"].as_str().unwrap().to_owned()).collect();
        assert_eq!(ids, ["weekly", "weekly_model_fable"]);
        // Named in `limits` alone: still read.
        let only_limits = json!({"limits": [
            {"group": "session", "kind": "session", "percent": 12, "scope": null},
            {"group": "weekly", "kind": "weekly_all", "percent": 30, "scope": null}
        ]});
        let ids: Vec<_> = super::windows(&only_limits).iter().map(|window| window["id"].as_str().unwrap().to_owned()).collect();
        assert_eq!(ids, ["five_hour", "weekly"]);
    }

    /// Asks Anthropic with this host's own sign-in: run by hand
    /// (`cargo test -p sophon-service live_claude_plan -- --ignored --nocapture`).
    #[tokio::test]
    #[ignore = "reads this host's Claude Code sign-in and asks Anthropic"]
    async fn live_claude_plan_is_read() {
        let entry = read("claude", &json!({})).await.expect("Claude Code is signed in on this host");
        assert!(entry["error"].is_null(), "{}", entry["error"]);
        let windows = entry["windows"].as_array().unwrap();
        for window in windows {
            println!("{} | {} | {}% | resets {}", window["id"], window["label"], window["usedPct"], window["resetsAt"]);
        }
        println!("plan: {}", entry["planLabel"]);
        assert!(windows.iter().any(|window| window["id"] == "five_hour" || window["id"] == "weekly"), "{entry}");
    }

    #[test]
    fn a_provider_on_another_endpoint_has_no_plan_to_read() {
        assert!(on_a_plan(&json!({})));
        assert!(on_a_plan(&json!({"env":{"ANTHROPIC_BASE_URL":"https://api.anthropic.com/"}})));
        assert!(!on_a_plan(&json!({"env":{"ANTHROPIC_BASE_URL":"https://gateway.example.com"}})));
        assert!(!on_a_plan(&json!({"env":{"CLAUDE_CODE_USE_BEDROCK":"1"}})));
    }
}
