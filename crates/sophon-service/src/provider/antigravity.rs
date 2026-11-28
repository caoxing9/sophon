//! Antigravity CLI (`agy acp`), Google's agent and Gemini CLI's successor,
//! through the Agent Client Protocol, driven as T3 Code drives it (its
//! `AntigravityAdapterV2`): the official CLI does all that touches the
//! account, and this host only starts it, speaks ACP to it and answers
//! what it asks. Nothing here reads its credentials or calls Google.
//!
//! Each provider (each account) has a folder of its own, the CLI's
//! `--gemini-dir` (under `~/.sophon/providers/antigravity/` by default): its
//! sign-in, settings and conversations, with global rules and skills linked
//! from `~/.gemini`. Auto-update is disabled (`AGY_CLI_DISABLE_AUTO_UPDATE=true`),
//! and personal Google accounts (`oauth-personal`) set `SSH_CONNECTION` so
//! `agy` 1.5.x keeps credentials per-profile instead of in one shared OS
//! keyring entry, signing in through `agy acp --login` (paste-code flow)
//! and signing out through ACP's native `logout` RPC.

use super::{Command, Emit, Event, IDLE, Outcome, Process, Start, gemini::row};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

/// The CLI's program name (`antigravity` may be its editor's).
pub(crate) const PROGRAM: &str = "agy";
/// The variable naming a provider's folder, as T3 Code sets it.
pub(crate) const HOME_VAR: &str = "GEMINI_HOME";
/// The folder of the built-in Antigravity provider, under the home folder.
pub(crate) const DEFAULT_HOME: &str = ".sophon/providers/antigravity/default";
/// Earlier releases kept the built-in provider at `~/.sophon-antigravity`.
const LEGACY_HOME: &str = ".sophon-antigravity";
/// Where in its folder the ACP server keeps its own (a relative path: the
/// CLI puts it under `--gemini-dir` whatever it is given).
const APP_DATA: &str = "antigravity-acp";
/// The token files Antigravity CLI may keep in `antigravity-acp`.
const TOKEN_FILES: [&str; 2] = ["antigravity-oauth-token", "acp_token.json"];
/// Sophon's own non-secret marker recording the last verified auth state
/// (`authenticated` / `unauthenticated`) for the active `authMethod`,
/// because `agy` 1.5.x may keep credentials in the OS keyring or internal
/// cache rather than a fixed token file.
const AUTH_STATE_FILE: &str = "sophon-auth.json";
/// What the service's environment may hold that would sign the CLI in
/// otherwise than its folder says, or change how it runs: left out, as
/// T3 Code leaves them out (`antigravityAuthSupport.ts`).
const REMOVED: &[&str] = &[
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "GOOGLE_CLOUD_QUOTA_PROJECT",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "GCLOUD_PROJECT",
    "CLOUDSDK_CORE_PROJECT",
    "AGY_ACP_CCPA_PROJECT",
    "AGY_ACP_ENABLE_OAUTH",
    "GEMINI_HOME",
    "AGY_ACP_FORCE_FILE_STORAGE",
    "AGY_CLI_DISABLE_AUTO_UPDATE",
    "ANTIGRAVITY_HARNESS_PATH",
    "BROWSER",
    "PYTHONUNBUFFERED",
    "ELECTRON_RUN_AS_NODE",
    "CHROME_CRASHPAD_PIPE_NAME",
];
/// Variables removed during `agy acp --login` so it prints the Google link
/// instead of opening a browser on the host.
const REMOVED_CODE_LOGIN: &[&str] = &[
    "GEMINI_API_KEY",
    "GOOGLE_API_KEY",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "GOOGLE_CLOUD_PROJECT",
    "GOOGLE_CLOUD_LOCATION",
    "GOOGLE_CLOUD_QUOTA_PROJECT",
    "GOOGLE_GENAI_USE_VERTEXAI",
    "GCLOUD_PROJECT",
    "CLOUDSDK_CORE_PROJECT",
    "AGY_ACP_CCPA_PROJECT",
    "AGY_ACP_ENABLE_OAUTH",
    "GEMINI_HOME",
    "AGY_ACP_FORCE_FILE_STORAGE",
    "AGY_CLI_DISABLE_AUTO_UPDATE",
    "ANTIGRAVITY_HARNESS_PATH",
    "BROWSER",
    "PYTHONUNBUFFERED",
    "ELECTRON_RUN_AS_NODE",
    "CHROME_CRASHPAD_PIPE_NAME",
    "DISPLAY",
    "WAYLAND_DISPLAY",
];
/// What the browser stand-in writes before a sign-in link: a session
/// that meets one is not signed in (the link is never opened or shown).
const LINK_MARK: &str = "__SOPHON_ANTIGRAVITY_SIGN_IN__";
/// Where `agy acp --login` sends Google; the page displays a code the user
/// pastes back.
const CODE_REDIRECT_URI: &str = "https://antigravity.google/oauth-callback";
/// How long a stop waits for the turn to end before the CLI is ended.
const STOP_WAIT: Duration = Duration::from_secs(15);
/// The sign-ins the CLI offers, as `authenticate` names them.
const METHODS: [&str; 4] = [
    "oauth-personal",
    "oauth-business",
    "gemini-api-key",
    "agent-platform",
];

fn has_token(home: &Path) -> bool {
    TOKEN_FILES
        .iter()
        .any(|name| home.join(APP_DATA).join(name).is_file())
}

fn read_auth_state(home: &Path, expected_method: &str) -> Option<String> {
    let bytes = std::fs::read(home.join(APP_DATA).join(AUTH_STATE_FILE)).ok()?;
    let value: Value = serde_json::from_slice(&bytes).ok()?;
    if value["authMethod"].as_str() != Some(expected_method) {
        return None;
    }
    value["status"].as_str().map(str::to_owned)
}

fn save_auth_state(home: &Path, method: &str, status: &str) {
    let dir = home.join(APP_DATA);
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(AUTH_STATE_FILE),
        format!("{}\n", json!({"status": status, "authMethod": method})),
    );
}

fn has_profile_data(home: &Path) -> bool {
    let app = home.join(APP_DATA);
    has_token(home)
        || app.join(AUTH_STATE_FILE).is_file()
        || app.join("sophon-models.json").is_file()
}

/// The provider's folder: the one its settings name, else the default
/// (`~/.sophon/providers/antigravity/default`, keeping an earlier
/// `~/.sophon-antigravity` login if one already lives there).
pub(crate) fn home(settings: &Value) -> Option<PathBuf> {
    if let Some(dir) = settings
        .pointer("/env/GEMINI_HOME")
        .and_then(Value::as_str)
        .filter(|dir| !dir.is_empty())
    {
        return Some(PathBuf::from(dir));
    }
    let user = PathBuf::from(std::env::var_os("HOME")?);
    let preferred = user.join(DEFAULT_HOME);
    let legacy = user.join(LEGACY_HOME);
    if !has_profile_data(&preferred) && has_profile_data(&legacy) {
        Some(legacy)
    } else {
        Some(preferred)
    }
}

/// How the provider signs in: as its settings name it, else by the key
/// they give (a Gemini API key, an Agent Platform key), else with a
/// Google account.
pub(crate) fn method(settings: &Value) -> &'static str {
    if let Some(named) = settings["authMethod"].as_str()
        && let Some(method) = METHODS.iter().find(|method| **method == named)
    {
        return method;
    }
    let has = |name: &str| {
        settings["env"][name]
            .as_str()
            .is_some_and(|value| !value.trim().is_empty())
    };
    if has("GEMINI_API_KEY") {
        "gemini-api-key"
    } else if has("GOOGLE_API_KEY") {
        "agent-platform"
    } else {
        "oauth-personal"
    }
}

/// Whether the provider is authenticated, unauthenticated, or unchecked:
/// - `authenticated`: verified by `login()` or a live ACP session (recorded
///   in `sophon-auth.json`), or has a profile token file / cached models.
/// - `unauthenticated`: explicitly signed out, rejected by ACP, or missing
///   required API key / GCP project for key-based methods.
/// - `unknown`: CLI is installed (and credentials configured if key-based),
///   but access has not been checked yet.
pub(crate) fn auth_status(settings: &Value) -> Value {
    let method = method(settings);
    let dir = home(settings);
    let non_empty = |v: &Value| v.as_str().is_some_and(|s| !s.trim().is_empty());
    let recorded = dir.as_deref().and_then(|h| read_auth_state(h, method));
    let has_models = !models(settings).is_empty();
    let token_file = dir.as_deref().is_some_and(has_token);
    let status = match method {
        "gemini-api-key" => {
            if !non_empty(&settings["env"]["GEMINI_API_KEY"]) {
                "unauthenticated"
            } else if recorded.as_deref() == Some("unauthenticated") {
                "unauthenticated"
            } else if recorded.as_deref() == Some("authenticated") || has_models {
                "authenticated"
            } else {
                "unknown"
            }
        }
        "agent-platform" => {
            if !non_empty(&settings["env"]["GOOGLE_API_KEY"])
                && !non_empty(&settings["gcp"]["project"])
            {
                "unauthenticated"
            } else if recorded.as_deref() == Some("unauthenticated") {
                "unauthenticated"
            } else if recorded.as_deref() == Some("authenticated") || has_models {
                "authenticated"
            } else {
                "unknown"
            }
        }
        _ => {
            if recorded.as_deref() == Some("unauthenticated") {
                "unauthenticated"
            } else if recorded.as_deref() == Some("authenticated") || token_file || has_models {
                "authenticated"
            } else {
                "unknown"
            }
        }
    };
    let signed = status == "authenticated";
    json!({"loggedIn":signed,"authStatus":status,"authMethod":method,
        "configDirectory":dir.map(|home| home.display().to_string())})
}

/// What the CLI's settings in the provider's folder say: how it signs in
/// (so its own sign-out clears that one), and the Google Cloud project
/// and location the provider's settings give. Never a credential.
fn profile_settings(settings: &Value, method: &str) -> Value {
    let mut said = json!({"auth":{"type":method}});
    let project = settings["gcp"]["project"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let location = settings["gcp"]["location"]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if project.is_some() || location.is_some() {
        let mut gcp = serde_json::Map::new();
        if let Some(project) = project {
            gcp.insert("project".to_owned(), json!(project));
        }
        if let Some(location) = location {
            gcp.insert("location".to_owned(), json!(location));
        }
        said["gcp"] = Value::Object(gcp);
    }
    said
}

/// Keeps only the credential matching `method` in `settings["env"]`, and
/// strips managed keys so stale keys from another method never leak.
fn sanitized_settings(settings: &Value, method: &str) -> Value {
    let mut clean = settings.clone();
    if let Some(env) = clean.get_mut("env").and_then(Value::as_object_mut) {
        env.retain(|key, value| {
            let upper = key.to_ascii_uppercase();
            match upper.as_str() {
                "GEMINI_API_KEY" => {
                    method == "gemini-api-key"
                        && value.as_str().is_some_and(|v| !v.trim().is_empty())
                }
                "GOOGLE_API_KEY" => {
                    method == "agent-platform"
                        && value.as_str().is_some_and(|v| !v.trim().is_empty())
                }
                _ => !REMOVED.iter().any(|r| *r == upper.as_str()),
            }
        });
    }
    clean
}

/// Global user rules and skills linked from `~/.gemini` into an isolated
/// Antigravity profile, keeping credentials, hooks and MCP isolated.
const USER_CONFIG_LINKS: [&str; 5] = [
    "AGENTS.md",
    "GEMINI.md",
    "config/skills",
    "antigravity-cli/skills",
    "config/skills.json",
];

#[cfg(unix)]
fn link_user_config(profile: &Path, user_gemini: &Path) {
    if profile == user_gemini {
        return;
    }
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(profile), std::fs::canonicalize(user_gemini))
        && a == b
    {
        return;
    }
    for rel in USER_CONFIG_LINKS {
        let target = user_gemini.join(rel);
        let link = profile.join(rel);
        if !target.exists() {
            // Remove dangling symlinks: the harness creates these folders and
            // fails if a dangling link sits in the way.
            if link
                .symlink_metadata()
                .is_ok_and(|meta| meta.file_type().is_symlink())
            {
                let _ = std::fs::remove_file(&link);
            }
            continue;
        }
        match link.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                if std::fs::read_link(&link).ok().as_deref() == Some(target.as_path()) {
                    continue;
                }
                let _ = std::fs::remove_file(&link);
            }
            Ok(meta) if meta.is_dir() => {
                // An empty directory is the harness's placeholder; `remove_dir`
                // refuses non-empty directories, keeping profile-owned content safe.
                if std::fs::remove_dir(&link).is_err() {
                    continue;
                }
            }
            Ok(_) => continue, // Profile-owned regular file: never replace.
            Err(_) => {}
        }
        if let Some(parent) = link.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::os::unix::fs::symlink(&target, &link);
    }
}

#[cfg(not(unix))]
fn link_user_config(_profile: &Path, _user_gemini: &Path) {}

/// Removes a per-run unpack directory when dropped (even on cancellation).
struct RunDir(Option<PathBuf>);

impl Drop for RunDir {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

/// Makes the provider's folders (its own, the ACP server's and the one
/// its runs unpack into) as only this user's, writes its settings, links
/// global rules/skills from `~/.gemini`, and makes the browser stand-in:
/// what it gives the CLI as `BROWSER`.
fn prepare(home: &Path, settings: &Value, method: &str) -> io::Result<PathBuf> {
    for dir in [home.to_path_buf(), home.join(APP_DATA), home.join("runs")] {
        std::fs::create_dir_all(&dir)?;
        private(&dir, 0o700)?;
    }
    std::fs::write(
        home.join(APP_DATA).join("settings.json"),
        format!("{}\n", profile_settings(settings, method)),
    )?;
    if let Some(user_home) = std::env::var_os("HOME").map(PathBuf::from) {
        link_user_config(home, &user_home.join(".gemini"));
    }
    // Runs left by a host that ended first, a day old: each unpacks about
    // a gigabyte.
    for entry in std::fs::read_dir(home.join("runs"))?.flatten() {
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .is_ok_and(|at| {
                at.elapsed()
                    .is_ok_and(|age| age > Duration::from_secs(86_400))
            });
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    let browser = home.join("browser");
    std::fs::write(
        &browser,
        format!("#!/bin/sh\nprintf '%s %s\\n' '{LINK_MARK}' \"$1\" >&2\nexit 0\n"),
    )?;
    private(&browser, 0o700)?;
    Ok(browser)
}

#[cfg(unix)]
fn private(path: &Path, mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn private(_path: &Path, _mode: u32) -> io::Result<()> {
    Ok(())
}

/// What the CLI's environment has over the service's (from which
/// [`REMOVED`] is left out).
fn environment(home: &Path, run: &Path, browser: &Path, method: &str) -> Vec<(String, String)> {
    // The 1.5 CLI ignores `AGY_ACP_FORCE_FILE_STORAGE` and keeps Google
    // sign-ins in one shared OS keyring entry for every profile unless it
    // believes it runs over SSH; `oauth-personal` therefore sets
    // `SSH_CONNECTION` and signs in via `agy acp --login`. Other methods
    // (including Gemini Enterprise, which has no `--login`) leave it unset.
    let mut env = vec![
        (HOME_VAR.to_owned(), home.display().to_string()),
        ("AGY_ACP_FORCE_FILE_STORAGE".to_owned(), "1".to_owned()),
        ("AGY_CLI_DISABLE_AUTO_UPDATE".to_owned(), "true".to_owned()),
        ("PYTHONUNBUFFERED".to_owned(), "1".to_owned()),
        ("TMPDIR".to_owned(), run.display().to_string()),
    ];
    if method == "oauth-personal" {
        let ssh = std::env::var("SSH_CONNECTION")
            .ok()
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "127.0.0.1 0 127.0.0.1 0".to_owned());
        env.push(("SSH_CONNECTION".to_owned(), ssh));
    }
    // Python splits `BROWSER` at the path separator.
    let browser = browser.display().to_string();
    if !browser.contains(':') {
        env.push(("BROWSER".to_owned(), browser));
    }
    env
}

/// Whether the CLI's `version` is one this host drives: 1.5.1 and on (it
/// opens every model to a client by its own name) and below 2.
fn supported(version: &str) -> bool {
    let version = version.trim_start_matches("agy_acp_server_");
    let numbers: Vec<u64> = version
        .split(['-', '+'])
        .next()
        .unwrap_or_default()
        .split('.')
        .map_while(|part| part.parse().ok())
        .collect();
    match numbers.as_slice() {
        [major, minor, patch, ..] => *major == 1 && (*minor, *patch) >= (5, 1),
        _ => false,
    }
}

/// Whether an error the CLI answered means it is not signed in.
fn needs_sign_in(code: i64, said: &str) -> bool {
    code == -32000 || (code == -32603 && said.contains("No browser is available here"))
}

const NOT_SIGNED_IN: &str =
    "Antigravity is not signed in: sign it in under Settings › Agents, then send this again";

/// What the CLI says before the Google sign-in link it gives a client
/// signing it in (as T3 Code reads it).
const LINK_LINE: &str = "Open the following link to authenticate the ACP server: ";

/// A Google sign-in under way: either `agy acp --login` (`Code`, where
/// Google redirects to `https://antigravity.google/oauth-callback` and the
/// person pastes the code or page URL back) or ACP `authenticate`
/// (`Loopback`, where Google redirects to this host's `127.0.0.1:<port>/`).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Pending {
    Code { redirect: String, state: String },
    Loopback { redirect: String, state: String },
}

/// The query of `url` (after `?`), its names and values decoded.
fn query(url: &str) -> Vec<(String, String)> {
    let Some((_, query)) = url.split_once('?') else {
        return Vec::new();
    };
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(name), decode(value))
        })
        .collect()
}

/// `%xx` and `+` decoded.
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex = |b: u8| (b as char).to_digit(16);
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => match (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                (Some(high), Some(low)) => {
                    out.push((high * 16 + low) as u8);
                    i += 3;
                    continue;
                }
                _ => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            byte => out.push(byte),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The one value of `name` in `pairs`, if it has exactly one.
fn only<'a>(pairs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    let mut found = pairs.iter().filter(|(n, _)| n == name).map(|(_, v)| v.as_str());
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

fn has_userinfo(url: &str) -> bool {
    let Some((_, rest)) = url.split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    authority.contains('@')
}

/// Whether `url` is a Google sign-in link coming back to this host's
/// loopback: what it comes back to and with.
fn sign_in_link(url: &str) -> Option<Pending> {
    if url.len() > 16_384 || url.contains(char::is_whitespace) || url.contains('#') || has_userinfo(url) {
        return None;
    }
    let path = url.split(['?', '#']).next()?;
    if path != "https://accounts.google.com/o/oauth2/v2/auth" {
        return None;
    }
    let pairs = query(url);
    if only(&pairs, "response_type")? != "code" {
        return None;
    }
    let state = only(&pairs, "state").filter(|s| !s.is_empty() && s.len() <= 512 && !s.contains(char::is_whitespace))?;
    let redirect = only(&pairs, "redirect_uri")?;
    let port = redirect.strip_prefix("http://127.0.0.1:")?.strip_suffix('/')?;
    port.parse::<u16>().ok().filter(|p| *p >= 1024 && !port.starts_with('0'))?;
    Some(Pending::Loopback { redirect: redirect.to_owned(), state: state.to_owned() })
}

/// Whether `url` is `agy acp --login`'s Google sign-in link: Google's
/// `/o/oauth2/auth` coming back to `https://antigravity.google/oauth-callback`.
fn code_sign_in_link(url: &str) -> Option<Pending> {
    if url.len() > 16_384 || url.contains(char::is_whitespace) || url.contains('#') || has_userinfo(url) {
        return None;
    }
    let path = url.split(['?', '#']).next()?;
    if path != "https://accounts.google.com/o/oauth2/auth" {
        return None;
    }
    let pairs = query(url);
    if only(&pairs, "response_type")? != "code" {
        return None;
    }
    let state = only(&pairs, "state").filter(|s| !s.is_empty() && s.len() <= 512 && !s.contains(char::is_whitespace))?;
    if only(&pairs, "redirect_uri")? != CODE_REDIRECT_URI {
        return None;
    }
    Some(Pending::Code { redirect: CODE_REDIRECT_URI.to_owned(), state: state.to_owned() })
}

/// Validates the reply pasted by the user:
/// - `Pending::Code`: either the code shown by the Antigravity page
///   (`^[\w./~-]{1,2048}$`) or that page's full URL matching this sign-in's
///   `state` and carrying one `code`.
/// - `Pending::Loopback`: to this sign-in's loopback, with its state, and
///   one code or one error.
pub(crate) fn reply_for(pending: &Pending, pasted: &str) -> Result<String, String> {
    let pasted = pasted.trim();
    match pending {
        Pending::Code { redirect, state } => {
            let bad = || "Paste the code from the Antigravity page, or that page's full URL".to_owned();
            if !pasted.is_empty()
                && pasted.len() <= 2048
                && pasted
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b'~' | b'-'))
            {
                return Ok(pasted.to_owned());
            }
            if pasted.len() > 16_384 || pasted.contains(char::is_whitespace) || pasted.contains('#') || has_userinfo(pasted) {
                return Err(bad());
            }
            let path = pasted.split('?').next().unwrap_or_default();
            if path != redirect {
                return Err(bad());
            }
            let pairs = query(pasted);
            if only(&pairs, "state") != Some(state.as_str()) {
                return Err(bad());
            }
            if only(&pairs, "code").is_none_or(str::is_empty) {
                return Err(bad());
            }
            Ok(pasted.to_owned())
        }
        Pending::Loopback { redirect, state } => {
            let wrong = || "This address does not belong to this sign-in".to_owned();
            if pasted.len() > 16_384 || pasted.contains(char::is_whitespace) || pasted.contains('#') || has_userinfo(pasted) {
                return Err(wrong());
            }
            let path = pasted.split('?').next().unwrap_or_default();
            if path != redirect {
                return Err(wrong());
            }
            let pairs = query(pasted);
            if only(&pairs, "state") != Some(state.as_str()) {
                return Err(wrong());
            }
            let codes = pairs.iter().filter(|(n, v)| n == "code" && !v.is_empty()).count();
            let errors = pairs.iter().filter(|(n, v)| n == "error" && !v.is_empty()).count();
            if (codes, errors) != (1, 0) && (codes, errors) != (0, 1) {
                return Err("The address has no Google sign-in reply".to_owned());
            }
            let issuers: Vec<&str> = pairs
                .iter()
                .filter(|(n, _)| n == "iss")
                .map(|(_, v)| v.as_str())
                .collect();
            if issuers.len() > 1 || issuers.iter().any(|v| *v != "https://accounts.google.com") {
                return Err(wrong());
            }
            Ok(pasted.to_owned())
        }
    }
}

/// Hands `reply` (an address on this host's loopback) to the CLI waiting
/// for it there, as the browser would.
pub(crate) async fn forward(reply: &str) -> Result<(), String> {
    let rest = reply.strip_prefix("http://127.0.0.1:").ok_or("Not a loopback address")?;
    let (port, target) = rest.split_once('/').ok_or("Not a loopback address")?;
    let port: u16 = port.parse().map_err(|_| "Not a loopback address")?;
    // The CLI may be a moment from listening when the link is out.
    let mut tries = 0;
    let mut stream = loop {
        match tokio::net::TcpStream::connect(("127.0.0.1", port)).await {
            Ok(stream) => break stream,
            Err(_) if tries < 15 => {
                tries += 1;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            Err(_) => return Err("Antigravity is no longer waiting for this sign-in".to_owned()),
        }
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let request = format!("GET /{target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.map_err(|e| e.to_string())?;
    // Nothing more to send: a server that reads to the end answers.
    let _ = stream.shutdown().await;
    let mut answer = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(10), stream.read_to_end(&mut answer)).await;
    Ok(())
}

/// Reads ACP responses until request `id` is answered; a sign-in link seen
/// along the way is recorded in `link`.
async fn acp_answer(
    process: &mut Option<Process>,
    id: u64,
    link: &mut Option<String>,
) -> io::Result<Result<Value, (i64, String)>> {
    loop {
        let tick = tokio::time::sleep(Duration::from_millis(250));
        tokio::select! {
            line = super::line(process) => {
                let Some(line) = line else {
                    let why = match process.take() {
                        Some(p) => p.ended().await,
                        None => String::new(),
                    };
                    return Err(io::Error::other(format!("Antigravity stopped ({why})")));
                };
                let Ok(message) = serde_json::from_str::<Value>(&line) else {
                    if let Some(url) = line.trim().strip_prefix(LINK_LINE.trim_end()) {
                        link.get_or_insert_with(|| url.trim().to_owned());
                    }
                    continue;
                };
                if message["id"].as_u64() == Some(id) && message["method"].is_null() {
                    return Ok(match message.get("error") {
                        Some(error) if !error.is_null() => Err((
                            error["code"].as_i64().unwrap_or_default(),
                            error["message"].as_str().unwrap_or("failed").to_owned(),
                        )),
                        _ => Ok(message["result"].clone()),
                    });
                }
                // A request of the CLI's meanwhile: none is answered here.
                if !message["method"].is_null() && !message["id"].is_null() {
                    let refused = json!({"jsonrpc":"2.0","id":message["id"],"error":{"code":-32601,"message":"Not supported while signing in"}});
                    if let Some(p) = process.as_mut() {
                        p.send(&refused).await?;
                    }
                }
            }
            _ = tick => {}
        }
        // The browser stand-in's line, on the error stream.
        if link.is_none()
            && let Some(p) = process.as_ref()
        {
            let said = p.errors.borrow().iter().find_map(|line| {
                line.strip_prefix(LINK_MARK)
                    .map(|url| url.trim().to_owned())
                    .filter(|url| !url.is_empty())
            });
            *link = said;
        }
    }
}

fn auth_error(code: i64, said: String, method: &str) -> String {
    if said.contains("SUBSCRIPTION_REQUIRED") {
        "Google requires an eligible Antigravity subscription for this account".to_owned()
    } else if said.to_ascii_lowercase().contains("access_denied")
        || said.to_ascii_lowercase().contains("denied access")
    {
        "Google sign-in was not approved. Start sign-in again.".to_owned()
    } else if code == -32602 && !method.starts_with("oauth") {
        "Antigravity refused the key or project in this provider's settings".to_owned()
    } else {
        said
    }
}

/// Runs ACP `initialize` -> `authenticate` -> `session/new` to verify access
/// and cache the models offered by the account into `sophon-models.json`.
async fn verify_and_cache_models(
    settings: &Value,
    home: &Path,
    browser: &Path,
    method: &str,
) -> Result<(), String> {
    let run = home.join("runs").join(crate::state::id());
    std::fs::create_dir_all(&run).map_err(|e| e.to_string())?;
    private(&run, 0o700).map_err(|e| e.to_string())?;
    let _guard = RunDir(Some(run.clone()));
    let given = environment(home, &run, browser, method);
    let env: Vec<(&str, &str)> = given.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let args: Vec<String> = [
        "acp",
        "--gemini-dir",
        &home.display().to_string(),
        "--app-data-dir",
        APP_DATA,
    ]
    .map(str::to_owned)
    .into();
    let clean = sanitized_settings(settings, method);
    let cwd = std::env::temp_dir().display().to_string();
    let mut process = Some(
        Process::spawn_without("antigravity", &clean, &cwd, &args, &env, REMOVED)
            .map_err(|e| e.to_string())?,
    );
    if let Some(p) = process.as_mut() {
        p.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,
            "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},
            "clientInfo":{"name":"sophon","title":"Sophon","version":env!("CARGO_PKG_VERSION")}}}))
            .await
            .map_err(|e| e.to_string())?;
    }
    let started = tokio::time::timeout(Duration::from_secs(120), acp_answer(&mut process, 1, &mut None))
        .await
        .map_err(|_| "Antigravity did not start in time".to_owned())?
        .map_err(|e| e.to_string())?
        .map_err(|(_, said)| said)?;
    let version = started["agentInfo"]["version"].as_str().unwrap_or_default();
    if !supported(version) {
        return Err(format!(
            "Antigravity CLI {version} is not one Sophon runs: update it to 1.5.1 or later (brew upgrade --cask antigravity-cli, or agy update)"
        ));
    }
    if let Some(p) = process.as_mut() {
        p.send(&json!({"jsonrpc":"2.0","id":2,"method":"authenticate","params":{"methodId":method}}))
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::time::timeout(Duration::from_secs(60), acp_answer(&mut process, 2, &mut None))
        .await
        .map_err(|_| "Antigravity timed out while verifying sign-in".to_owned())?
        .map_err(|e| e.to_string())?
        .map_err(|(code, said)| auth_error(code, said, method))?;
    if let Some(p) = process.as_mut() {
        p.send(&json!({"jsonrpc":"2.0","id":3,"method":"session/new","params":{"cwd":cwd,"mcpServers":[]}}))
            .await
            .map_err(|e| e.to_string())?;
    }
    let opened = tokio::time::timeout(Duration::from_secs(60), acp_answer(&mut process, 3, &mut None))
        .await
        .map_err(|_| "Antigravity timed out while loading models".to_owned())?
        .map_err(|e| e.to_string())?
        .map_err(|_| "Antigravity signed in, but could not open a session or list its models".to_owned())?;
    let models = offered_models(&opened);
    if !models.is_empty() {
        let _ = std::fs::write(
            home.join(APP_DATA).join("sophon-models.json"),
            serde_json::to_vec(&models).unwrap_or_default(),
        );
    }
    save_auth_state(home, method, "authenticated");
    drop(process);
    Ok(())
}

/// Signs the provider in to Google through Antigravity's own sign-in flow:
///
/// - For `oauth-personal` (Google account): runs `agy acp --login` in the
///   provider's folder with `SSH_CONNECTION` set and `DISPLAY` unset, so
///   the CLI prints the Google sign-in link without opening a browser and
///   stores the token in the profile instead of the shared OS keyring. The
///   user pastes the code (or page URL) from `antigravity.google/oauth-callback`,
///   which is written to stdin; once `--login` succeeds, a fresh ACP
///   process verifies access and caches the offered models.
/// - For `oauth-business` (Gemini Enterprise) and key/ADC methods: runs ACP
///   `initialize` -> `authenticate` -> `session/new` directly (capturing a
///   loopback link when the method uses browser sign-in).
pub async fn login(
    settings: &Value,
) -> io::Result<(
    Value,
    Option<Pending>,
    Option<UnboundedSender<String>>,
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>>>>,
)> {
    let settings = settings.clone();
    let home = home(&settings).ok_or_else(|| io::Error::other("No folder for Antigravity"))?;
    let method = method(&settings);
    let browser = prepare(&home, &settings, method)?;

    if method == "oauth-personal" {
        let run = home.join("runs").join(crate::state::id());
        std::fs::create_dir_all(&run)?;
        private(&run, 0o700)?;
        let mut run_guard = RunDir(Some(run.clone()));
        let given = environment(&home, &run, &browser, method);
        let env: Vec<(&str, &str)> = given.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
        let args: Vec<String> = [
            "acp",
            "--gemini-dir",
            &home.display().to_string(),
            "--app-data-dir",
            APP_DATA,
            "--login",
        ]
        .map(str::to_owned)
        .into();
        let clean = sanitized_settings(&settings, method);
        let cwd = std::env::temp_dir().display().to_string();
        let mut process = Some(Process::spawn_without(
            "antigravity",
            &clean,
            &cwd,
            &args,
            &env,
            REMOVED_CODE_LOGIN,
        )?);
        let url = tokio::time::timeout(Duration::from_secs(90), async {
            loop {
                let Some(line) = super::line(&mut process).await else {
                    let why = match process.take() {
                        Some(p) => p.ended().await,
                        None => String::new(),
                    };
                    return Err(io::Error::other(format!("Antigravity stopped ({why})")));
                };
                let trimmed = line.trim();
                let candidate = trimmed
                    .strip_prefix(LINK_LINE.trim_end())
                    .map(str::trim)
                    .unwrap_or(trimmed);
                if candidate.starts_with("https://accounts.google.com/") {
                    return Ok(candidate.to_owned());
                }
            }
        })
        .await
        .map_err(|_| io::Error::other("Antigravity did not start sign-in in time"))??;
        let pending = code_sign_in_link(&url)
            .ok_or_else(|| io::Error::other("Antigravity gave a sign-in link that is not Google's"))?;
        let (submit_tx, mut submit_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        let reply = json!({"authUrl": url, "codeLogin": true});
        let done_guard = RunDir(run_guard.0.take());
        let done = Box::pin(async move {
            let login_run = done_guard;
            let code = loop {
                tokio::select! {
                    received = submit_rx.recv() => {
                        match received {
                            Some(code) => break code,
                            None => return Err("Canceled".to_owned()),
                        }
                    }
                    line = super::line(&mut process) => {
                        if line.is_none() {
                            let why = match process.take() {
                                Some(p) => p.ended().await,
                                None => String::new(),
                            };
                            return Err(format!("Antigravity sign-in stopped unexpectedly ({why})"));
                        }
                    }
                }
            };
            if let Some(p) = process.as_mut() {
                use tokio::io::AsyncWriteExt;
                p.input
                    .write_all(format!("{code}\n").as_bytes())
                    .await
                    .map_err(|e| e.to_string())?;
                p.input.flush().await.map_err(|e| e.to_string())?;
                let _ = p.input.shutdown().await;
            }
            // Drain stdout until `--login` finishes storing the token and exits.
            tokio::time::timeout(Duration::from_secs(60), async {
                while super::line(&mut process).await.is_some() {}
            })
            .await
            .map_err(|_| "Antigravity timed out while finishing Google sign-in".to_owned())?;
            let Some(mut p) = process.take() else {
                return Err("Antigravity sign-in stopped unexpectedly".to_owned());
            };
            let status = tokio::time::timeout(Duration::from_secs(10), p.child.wait())
                .await
                .map_err(|_| "Antigravity sign-in did not exit in time".to_owned())?
                .map_err(|e| e.to_string())?;
            tokio::task::yield_now().await;
            if !status.success() {
                let said = p.errors.borrow().iter().cloned().collect::<Vec<_>>().join("\n");
                return Err(if said.contains("SUBSCRIPTION_REQUIRED") {
                    "Google requires an eligible Antigravity subscription for this account".to_owned()
                } else if said.is_empty() {
                    "Antigravity could not finish Google sign-in. Start sign-in again.".to_owned()
                } else {
                    format!("Antigravity could not finish Google sign-in ({said})")
                });
            }
            drop(login_run);
            verify_and_cache_models(&settings, &home, &browser, method).await
        });
        return Ok((reply, Some(pending), Some(submit_tx), done));
    }

    // Non-personal methods (`oauth-business`, `gemini-api-key`, `agent-platform`):
    // drive ACP `initialize` -> `authenticate` -> `session/new` directly.
    let run = home.join("runs").join(crate::state::id());
    std::fs::create_dir_all(&run)?;
    private(&run, 0o700)?;
    let mut run_guard = RunDir(Some(run.clone()));
    let given = environment(&home, &run, &browser, method);
    let env: Vec<(&str, &str)> = given.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let args: Vec<String> = [
        "acp",
        "--gemini-dir",
        &home.display().to_string(),
        "--app-data-dir",
        APP_DATA,
    ]
    .map(str::to_owned)
    .into();
    let clean = sanitized_settings(&settings, method);
    let cwd = std::env::temp_dir().display().to_string();
    let mut process = Some(Process::spawn_without(
        "antigravity",
        &clean,
        &cwd,
        &args,
        &env,
        REMOVED,
    )?);
    let mut link = None;
    if let Some(p) = process.as_mut() {
        p.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,
            "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},
            "clientInfo":{"name":"sophon","title":"Sophon","version":env!("CARGO_PKG_VERSION")}}}))
            .await?;
    }
    let started = tokio::time::timeout(Duration::from_secs(120), acp_answer(&mut process, 1, &mut link))
        .await
        .map_err(|_| io::Error::other("Antigravity did not start in time"))??
        .map_err(|(_, said)| io::Error::other(said))?;
    let version = started["agentInfo"]["version"].as_str().unwrap_or_default();
    if !supported(version) {
        return Err(io::Error::other(format!(
            "Antigravity CLI {version} is not one Sophon runs: update it to 1.5.1 or later (brew upgrade --cask antigravity-cli, or agy update)"
        )));
    }
    if let Some(p) = process.as_mut() {
        p.send(&json!({"jsonrpc":"2.0","id":2,"method":"authenticate","params":{"methodId":method}}))
            .await?;
    }
    // Until the link comes (or it is signed in already / uses API key, the answer first).
    let mut early = None;
    let waited = tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let tick = tokio::time::sleep(Duration::from_millis(250));
            tokio::select! {
                line = super::line(&mut process) => {
                    let Some(line) = line else {
                        let why = match process.take() { Some(p) => p.ended().await, None => String::new() };
                        return Err(io::Error::other(format!("Antigravity stopped ({why})")));
                    };
                    match serde_json::from_str::<Value>(&line) {
                        Ok(message) if message["id"] == 2 && message["method"].is_null() => {
                            early = Some(message);
                            return Ok(());
                        }
                        Ok(_) => {}
                        Err(_) => {
                            if let Some(url) = line.trim().strip_prefix(LINK_LINE.trim_end()) {
                                link.get_or_insert_with(|| url.trim().to_owned());
                            }
                        }
                    }
                }
                _ = tick => {}
            }
            if link.is_none()
                && let Some(p) = process.as_ref()
            {
                link = p.errors.borrow().iter().find_map(|line| {
                    line.strip_prefix(LINK_MARK)
                        .map(|url| url.trim().to_owned())
                        .filter(|url| !url.is_empty())
                });
            }
            if link.is_some() {
                return Ok(());
            }
        }
    })
    .await
    .map_err(|_| {
        io::Error::other(
            "Antigravity could not reach Google: check this computer's network, then try again",
        )
    })?;
    waited?;
    let pending = match &link {
        Some(url) => Some(
            sign_in_link(url)
                .ok_or_else(|| io::Error::other("Antigravity gave a sign-in link that is not Google's"))?,
        ),
        None => None,
    };
    let reply = json!({"authUrl": link.as_deref().unwrap_or(""), "codeLogin": false});
    let models_home = home.clone();
    let done_guard = RunDir(run_guard.0.take());
    let done = Box::pin(async move {
        let _run_guard = done_guard;
        let authenticated = match early {
            Some(message) => match message.get("error") {
                Some(error) if !error.is_null() => Err((
                    error["code"].as_i64().unwrap_or_default(),
                    error["message"].as_str().unwrap_or("failed").to_owned(),
                )),
                _ => Ok(message["result"].clone()),
            },
            None => acp_answer(&mut process, 2, &mut None)
                .await
                .map_err(|e| e.to_string())?,
        };
        authenticated.map_err(|(code, said)| auth_error(code, said, method))?;
        // Checked as T3 Code checks it: a session opens, its models kept.
        if let Some(p) = process.as_mut() {
            p.send(&json!({"jsonrpc":"2.0","id":3,"method":"session/new","params":{"cwd":cwd,"mcpServers":[]}}))
                .await
                .map_err(|e| e.to_string())?;
        }
        let opened = acp_answer(&mut process, 3, &mut None)
            .await
            .map_err(|e| e.to_string())?
            .map_err(|_| {
                "Antigravity signed in, but could not open a session or list its models".to_owned()
            })?;
        let models = offered_models(&opened);
        if !models.is_empty() {
            let _ = std::fs::write(
                models_home.join(APP_DATA).join("sophon-models.json"),
                serde_json::to_vec(&models).unwrap_or_default(),
            );
        }
        save_auth_state(&models_home, method, "authenticated");
        drop(process);
        Ok(())
    });
    Ok((reply, pending, None, done))
}

/// Signs the provider out through Antigravity's native ACP `logout` RPC
/// and clears its cached model list.
pub async fn logout(settings: &Value) -> Result<(), String> {
    let home = home(settings).ok_or("No folder for Antigravity")?;
    let method = method(settings);
    let browser = prepare(&home, settings, method).map_err(|e| e.to_string())?;
    let run = home.join("runs").join(crate::state::id());
    std::fs::create_dir_all(&run).map_err(|e| e.to_string())?;
    private(&run, 0o700).map_err(|e| e.to_string())?;
    let _guard = RunDir(Some(run.clone()));
    let given = environment(&home, &run, &browser, method);
    let env: Vec<(&str, &str)> = given.iter().map(|(n, v)| (n.as_str(), v.as_str())).collect();
    let args: Vec<String> = [
        "acp",
        "--gemini-dir",
        &home.display().to_string(),
        "--app-data-dir",
        APP_DATA,
    ]
    .map(str::to_owned)
    .into();
    let clean = sanitized_settings(settings, method);
    let cwd = std::env::temp_dir().display().to_string();
    let mut process = Some(
        Process::spawn_without("antigravity", &clean, &cwd, &args, &env, REMOVED)
            .map_err(|e| e.to_string())?,
    );
    if let Some(p) = process.as_mut() {
        p.send(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1,
            "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},
            "clientInfo":{"name":"sophon","title":"Sophon","version":env!("CARGO_PKG_VERSION")}}}))
            .await
            .map_err(|e| e.to_string())?;
    }
    let started = tokio::time::timeout(Duration::from_secs(90), acp_answer(&mut process, 1, &mut None))
        .await
        .map_err(|_| "Antigravity sign-out timed out".to_owned())?
        .map_err(|e| e.to_string())?
        .map_err(|(_, said)| said)?;
    let can_logout = !started["agentCapabilities"]["auth"]["logout"].is_null()
        && started["agentCapabilities"]["auth"]["logout"] != false;
    if !can_logout {
        return Err("This Antigravity version does not support sign-out. Update it first.".to_owned());
    }
    if let Some(p) = process.as_mut() {
        p.send(&json!({"jsonrpc":"2.0","id":2,"method":"logout","params":{}}))
            .await
            .map_err(|e| e.to_string())?;
    }
    tokio::time::timeout(Duration::from_secs(30), acp_answer(&mut process, 2, &mut None))
        .await
        .map_err(|_| "Antigravity sign-out timed out".to_owned())?
        .map_err(|e| e.to_string())?
        .map_err(|(_, said)| said)?;
    let _ = std::fs::remove_file(home.join(APP_DATA).join("sophon-models.json"));
    save_auth_state(&home, method, "unauthenticated");
    drop(process);
    Ok(())
}

/// What a request of ours was, for when its answer comes.
enum Sent {
    Prompt,
    Other,
}

/// A select option of the session (`configOptions`): its id, value and
/// choices.
#[derive(Clone, Default)]
struct Choice {
    id: String,
    current: String,
    values: Vec<(String, String)>,
}

impl Choice {
    /// The session's option of `category` (or with that id), flattening
    /// both direct options and grouped option lists.
    fn of(opened: &Value, category: &str) -> Option<Choice> {
        let option = opened["configOptions"]
            .as_array()?
            .iter()
            .find(|option| option["category"] == category || option["id"] == category)?;
        let mut values = Vec::new();
        for entry in option["options"].as_array().into_iter().flatten() {
            if let Some(value) = entry["value"].as_str() {
                let name = entry["name"].as_str().unwrap_or(value).to_owned();
                values.push((value.to_owned(), name));
            } else if let Some(group) = entry["options"].as_array() {
                for choice in group {
                    if let Some(value) = choice["value"].as_str() {
                        let name = choice["name"].as_str().unwrap_or(value).to_owned();
                        values.push((value.to_owned(), name));
                    }
                }
            }
        }
        Some(Choice {
            id: option["id"].as_str()?.to_owned(),
            current: option["currentValue"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            values,
        })
    }
}

/// The models a session offers, as the service lists models: its model
/// option's choices (else `models.availableModels`), the one it is on the
/// default, with the thinking levels it offers.
fn offered_models(opened: &Value) -> Vec<Value> {
    let level = Choice::of(opened, "thought_level");
    let thinking: Vec<Value> = level
        .iter()
        .flat_map(|choice| &choice.values)
        .map(|(id, label)| json!({"id":id,"label":label}))
        .collect();
    let level = level
        .map(|choice| choice.current)
        .filter(|current| !current.is_empty());
    let (choices, current) = match Choice::of(opened, "model") {
        Some(choice) => (choice.values, choice.current),
        None => (
            opened["models"]["availableModels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|model| {
                    let id = model["modelId"].as_str()?.to_owned();
                    let name = model["name"].as_str().unwrap_or(&id).to_owned();
                    Some((id, name))
                })
                .collect(),
            opened["models"]["currentModelId"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
        ),
    };
    choices
        .into_iter()
        .map(|(id, label)| {
            json!({"id":id,"label":label,"isDefault":id == current,"isSelectable":true,
                "thinkingOptions":thinking,"defaultThinkingOptionId":level})
        })
        .collect()
}

/// The models the provider's account offered when a session last opened
/// (kept in its folder): the service lists them for a new conversation.
pub(crate) fn models(settings: &Value) -> Vec<Value> {
    home(settings)
        .and_then(|home| std::fs::read(home.join(APP_DATA).join("sophon-models.json")).ok())
        .and_then(|bytes| serde_json::from_slice::<Vec<Value>>(&bytes).ok())
        .unwrap_or_default()
}

/// The mode Antigravity is put in for this host's `mode`: plan and auto
/// are its own (when its session has them); for the rest it asks, and
/// this host answers ([`answered`]).
fn agent_mode(mode: &str, offered: &[String]) -> String {
    let has = |id: &str| offered.iter().any(|offered| offered == id);
    match mode {
        "plan" if has("plan") => "plan".to_owned(),
        "plan" if has("architect") => "architect".to_owned(),
        "auto" if has("auto") => "auto".to_owned(),
        _ => "default".to_owned(),
    }
}

/// Whether a permission Antigravity asks is answered here without asking
/// the person, as T3 Code's policy answers it: reading, searching and
/// thinking always; with full access everything; with accepted edits an
/// edit whose files are all in the workspace.
fn answered(mode: &str, call: &Value, cwd: &str) -> bool {
    let kind = call["kind"].as_str().unwrap_or_default();
    if matches!(kind, "read" | "search" | "think") {
        return true;
    }
    match mode {
        "bypassPermissions" => true,
        "acceptEdits" if kind == "edit" => {
            let root = Path::new(cwd);
            let paths: Vec<&str> = call["locations"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|location| location["path"].as_str())
                .collect();
            !paths.is_empty()
                && paths.iter().all(|path| {
                    let path = Path::new(path);
                    path.is_absolute()
                        && !path
                            .components()
                            .any(|part| part == std::path::Component::ParentDir)
                        && path.starts_with(root)
                })
        }
        _ => false,
    }
}

struct Antigravity {
    start: Start,
    emit: Emit,
    process: Option<Process>,
    /// The folder each run of the CLI unpacks into, removed after it.
    run: Option<PathBuf>,
    session: Option<String>,
    busy: bool,
    stopping: bool,
    /// When a stop that the CLI has not answered gives up on it.
    stop_by: Option<Instant>,
    restart: bool,
    outcome: Outcome,
    next: u64,
    sent: HashMap<u64, Sent>,
    /// Prompts sent and not answered yet: one sent while a turn is
    /// underway goes into it (Antigravity ends the one before with
    /// `end_turn` and goes on), and the turn ends with the last.
    prompts: usize,
    /// Permissions asked, by the id the client knows them by: the
    /// request to answer and the options it offered.
    asks: HashMap<String, (Value, Value)>,
    calls: HashMap<String, Value>,
    message: Option<String>,
    spoke: bool,
    fresh: bool,
    /// The session's modes, and its options for the model, the thinking
    /// level and the mode.
    modes: Vec<String>,
    model: Option<Choice>,
    thinking: Option<Choice>,
    mode: Option<Choice>,
    agent_mode: String,
}

pub async fn run(start: Start, mut inbox: UnboundedReceiver<Command>, emit: Emit) {
    let mut agent = Antigravity {
        session: start.session.clone(),
        start,
        emit,
        process: None,
        run: None,
        busy: false,
        stopping: false,
        stop_by: None,
        restart: false,
        outcome: Outcome::Completed,
        next: 0,
        sent: HashMap::new(),
        prompts: 0,
        asks: HashMap::new(),
        calls: HashMap::new(),
        message: None,
        spoke: false,
        fresh: false,
        modes: Vec::new(),
        model: None,
        thinking: None,
        mode: None,
        agent_mode: String::new(),
    };
    loop {
        let stop_by = agent.stop_by;
        tokio::select! {
            command = inbox.recv() => {
                let Some(command) = command else { break };
                if let Err(error) = agent.command(command).await {
                    agent.fail(error.to_string());
                }
            }
            line = super::line(&mut agent.process) => match line {
                Some(line) => {
                    if let Err(error) = agent.read(&line).await {
                        agent.fail(error.to_string());
                    }
                }
                None => agent.stopped().await,
            },
            // A stop the CLI did not answer in time: it is ended.
            _ = async { tokio::time::sleep_until(stop_by.unwrap().into()).await }, if stop_by.is_some() => {
                agent.close();
                agent.busy = true;
                agent.outcome = Outcome::Canceled;
                agent.idle();
            }
            _ = tokio::time::sleep(IDLE), if agent.process.is_some() && !agent.busy => agent.close(),
        }
    }
    agent.close();
}

impl Antigravity {
    fn item(&mut self, item: Value) {
        (self.emit)(Event::Item(item));
    }

    fn reply(&mut self, text: &str) {
        // A reply's words after a call or a thought begin a reply of
        // their own.
        if self.fresh && self.spoke {
            self.item(json!({"type":"assistant_message","text":"\n\n"}));
        }
        self.fresh = false;
        self.spoke = true;
        self.item(json!({"type":"assistant_message","text":text}));
    }

    async fn send(&mut self, message: Value) -> io::Result<()> {
        match self.process.as_mut() {
            Some(process) => process.send(&message).await,
            None => Err(io::Error::other("Antigravity is not running")),
        }
    }

    async fn request(&mut self, method: &str, params: Value, sent: Sent) -> io::Result<u64> {
        self.next += 1;
        let id = self.next;
        self.sent.insert(id, sent);
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        Ok(id)
    }

    /// A request answered before anything else is done: its result, or
    /// the error's code and words. What comes meanwhile (the session's
    /// updates as it loads) is taken as it comes.
    async fn call(
        &mut self,
        method: &str,
        params: Value,
    ) -> io::Result<Result<Value, (i64, String)>> {
        let id = self.request(method, params, Sent::Other).await?;
        loop {
            let Some(line) = super::line(&mut self.process).await else {
                let why = match self.process.take() {
                    Some(process) => process.ended().await,
                    None => String::new(),
                };
                if why.contains(LINK_MARK) {
                    return Err(io::Error::other(NOT_SIGNED_IN));
                }
                return Err(io::Error::other(format!("Antigravity stopped ({why})")));
            };
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                // A sign-in link said to the terminal: never shown.
                if line.contains("https://accounts.google.com/") {
                    return Err(io::Error::other(NOT_SIGNED_IN));
                }
                continue;
            };
            if message["id"].as_u64() == Some(id) && message["method"].is_null() {
                self.sent.remove(&id);
                return Ok(match message.get("error") {
                    Some(error) if !error.is_null() => Err((
                        error["code"].as_i64().unwrap_or_default(),
                        error["message"].as_str().unwrap_or("failed").to_owned(),
                    )),
                    _ => Ok(message["result"].clone()),
                });
            }
            self.read(&line).await?;
        }
    }

    fn close(&mut self) {
        self.process = None;
        self.sent.clear();
        self.calls.clear();
        self.prompts = 0;
        self.restart = false;
        self.stop_by = None;
        if let Some(run) = self.run.take() {
            let _ = std::fs::remove_dir_all(run);
        }
    }

    /// Starts the CLI in the provider's folder and opens the conversation's
    /// session in it: the one it had (resumed), else a new one.
    async fn open(&mut self) -> io::Result<()> {
        if self.process.is_some() {
            return Ok(());
        }
        let settings = self.start.settings.clone();
        let home = home(&settings).ok_or_else(|| io::Error::other("No folder for Antigravity"))?;
        let method = method(&settings);
        let browser = prepare(&home, &settings, method)?;
        let run = home.join("runs").join(crate::state::id());
        std::fs::create_dir_all(&run)?;
        private(&run, 0o700)?;
        let given = environment(&home, &run, &browser, method);
        let env: Vec<(&str, &str)> = given
            .iter()
            .chain(self.start.env.iter())
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let args: Vec<String> = [
            "acp",
            "--gemini-dir",
            &home.display().to_string(),
            "--app-data-dir",
            APP_DATA,
        ]
        .map(str::to_owned)
        .into();
        let clean = sanitized_settings(&settings, method);
        self.process = Some(Process::spawn_without(
            "antigravity",
            &clean,
            &self.start.cwd,
            &args,
            &env,
            REMOVED,
        )?);
        self.run = Some(run);
        let cwd = self.start.cwd.clone();
        let wanted = self.session.clone();
        let opening = async {
            let started = self
                .call(
                    "initialize",
                    json!({"protocolVersion":1,
                        "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},
                        "clientInfo":{"name":"sophon","title":"Sophon","version":env!("CARGO_PKG_VERSION")}}),
                )
                .await?
                .map_err(|(_, said)| io::Error::other(said))?;
            let version = started["agentInfo"]["version"].as_str().unwrap_or_default();
            if !supported(version) {
                return Err(io::Error::other(format!(
                    "Antigravity CLI {version} is not one Sophon runs: update it to 1.5.1 or later (brew upgrade --cask antigravity-cli, or agy update)"
                )));
            }
            let resumes = !started["agentCapabilities"]["sessionCapabilities"]["resume"].is_null();
            // Signed in as the provider's folder says, before any session.
            if let Err((code, said)) = self
                .call("authenticate", json!({"methodId":method}))
                .await?
            {
                return Err(io::Error::other(if needs_sign_in(code, &said) {
                    let _ = std::fs::remove_file(home.join(APP_DATA).join("sophon-models.json"));
                    save_auth_state(&home, method, "unauthenticated");
                    NOT_SIGNED_IN.to_owned()
                } else {
                    said
                }));
            }
            let fresh = json!({"cwd":cwd,"mcpServers":[]});
            let opened = match wanted.as_deref().filter(|_| resumes) {
                Some(session) => {
                    let resumed = self
                        .call(
                            "session/resume",
                            json!({"sessionId":session,"cwd":cwd,"mcpServers":[]}),
                        )
                        .await?;
                    match resumed {
                        Ok(result) => Ok((session.to_owned(), result)),
                        // A session it no longer has: a new one.
                        Err((code, said)) if !needs_sign_in(code, &said) => {
                            eprintln!(
                                "antigravity: session {session} could not be resumed ({said}): a new one"
                            );
                            self.call("session/new", fresh).await?.map(|result| {
                                (
                                    result["sessionId"].as_str().unwrap_or_default().to_owned(),
                                    result,
                                )
                            })
                        }
                        Err(error) => Err(error),
                    }
                }
                None => self.call("session/new", fresh).await?.map(|result| {
                    (
                        result["sessionId"].as_str().unwrap_or_default().to_owned(),
                        result,
                    )
                }),
            };
            opened.map_err(|(code, said)| {
                io::Error::other(if needs_sign_in(code, &said) {
                    let _ = std::fs::remove_file(home.join(APP_DATA).join("sophon-models.json"));
                    save_auth_state(&home, method, "unauthenticated");
                    NOT_SIGNED_IN.to_owned()
                } else {
                    said
                })
            })
        };
        // Each start unpacks its runtime, which may take a minute.
        let (session, opened) = tokio::time::timeout(Duration::from_secs(120), opening)
            .await
            .map_err(|_| io::Error::other("Antigravity did not start in time"))??;
        if session.is_empty() {
            return Err(io::Error::other("Antigravity opened no session"));
        }
        if self.start.session.as_deref() != Some(session.as_str()) {
            self.start.session = Some(session.clone());
            (self.emit)(Event::Session(session.clone()));
        }
        self.session = Some(session);
        self.modes = opened["modes"]["availableModes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|mode| mode["id"].as_str().map(str::to_owned))
            .collect();
        self.agent_mode = opened["modes"]["currentModeId"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        self.model = Choice::of(&opened, "model");
        self.thinking = Choice::of(&opened, "thought_level");
        self.mode = Choice::of(&opened, "mode");
        if let Some(mode) = &self.mode {
            self.modes = mode.values.iter().map(|(id, _)| id.clone()).collect();
            self.agent_mode = mode.current.clone();
        }
        // What the account offers, for a new conversation's choice.
        let models = offered_models(&opened);
        if !models.is_empty() {
            let _ = std::fs::write(
                home.join(APP_DATA).join("sophon-models.json"),
                serde_json::to_vec(&models).unwrap_or_default(),
            );
        }
        save_auth_state(&home, method, "authenticated");
        self.choose().await
    }

    /// Puts the session on the conversation's model, thinking level and
    /// mode, each only when it is not on it already.
    async fn choose(&mut self) -> io::Result<()> {
        let session = self.session.clone();
        let wanted = [
            (self.model.clone(), self.start.model.clone()),
            (self.thinking.clone(), self.start.thinking.clone()),
        ];
        for (choice, value) in wanted {
            let (Some(mut choice), Some(value)) = (choice, value.filter(|value| !value.is_empty()))
            else {
                continue;
            };
            if choice.current == value || !choice.values.iter().any(|(id, _)| *id == value) {
                continue;
            }
            let set = self
                .call(
                    "session/set_config_option",
                    json!({"sessionId":session,"configId":choice.id,"value":value}),
                )
                .await?;
            match set {
                Ok(_) => {
                    choice.current = value;
                    if self
                        .model
                        .as_ref()
                        .is_some_and(|model| model.id == choice.id)
                    {
                        self.model = Some(choice);
                    } else {
                        self.thinking = Some(choice);
                    }
                }
                Err((_, said)) => self.item(json!({"type":"notification",
                    "message":format!("Antigravity kept its choice: {said}")})),
            }
        }
        self.set_mode().await
    }

    async fn set_mode(&mut self) -> io::Result<()> {
        if self.session.is_none() {
            return Ok(());
        }
        let wanted = agent_mode(&self.start.mode, &self.modes);
        if wanted == self.agent_mode || !self.modes.contains(&wanted) {
            return Ok(());
        }
        let params = match &self.mode {
            Some(option) => json!({"sessionId":self.session,"configId":option.id,"value":wanted}),
            None => json!({"sessionId":self.session,"modeId":wanted}),
        };
        let method = if self.mode.is_some() {
            "session/set_config_option"
        } else {
            "session/set_mode"
        };
        self.agent_mode = wanted;
        self.request(method, params, Sent::Other).await.map(drop)
    }

    fn begin(&mut self) {
        self.busy = true;
        self.stopping = false;
        self.outcome = Outcome::Completed;
        self.message = None;
        (self.emit)(Event::Busy);
    }

    fn idle(&mut self) {
        if !self.busy {
            return;
        }
        self.busy = false;
        self.spoke = false;
        self.stop_by = None;
        for id in std::mem::take(&mut self.asks).into_keys() {
            (self.emit)(Event::Settled(id));
        }
        (self.emit)(Event::Idle(std::mem::replace(
            &mut self.outcome,
            Outcome::Completed,
        )));
        if self.restart {
            self.close();
        }
    }

    fn fail(&mut self, error: String) {
        self.close();
        self.busy = true;
        self.outcome = Outcome::Failed(error);
        self.idle();
    }

    async fn stopped(&mut self) {
        let Some(process) = self.process.take() else {
            return;
        };
        let why = process.ended().await;
        self.close();
        if self.busy {
            self.outcome = if self.stopping {
                Outcome::Canceled
            } else if why.contains(LINK_MARK) {
                Outcome::Failed(NOT_SIGNED_IN.to_owned())
            } else {
                Outcome::Failed(format!("Antigravity stopped ({why})"))
            };
            self.idle();
        }
    }

    /// `prompt` to the session: a turn of its own, or into the one
    /// underway (Antigravity ends that prompt and goes on with this).
    async fn prompt(&mut self, prompt: Value) -> io::Result<()> {
        self.open().await?;
        if !self.busy {
            // A model or thinking level chosen during the last turn.
            self.choose().await?;
            self.begin();
        }
        self.prompts += 1;
        let params = json!({"sessionId":self.session,"prompt":prompt});
        self.request("session/prompt", params, Sent::Prompt)
            .await
            .map(drop)
    }

    async fn command(&mut self, command: Command) -> io::Result<()> {
        match command {
            Command::Prompt { text, images, .. } => {
                // Words and pictures in the order they were written.
                let mut prompt = Vec::new();
                for piece in super::pieces(&text, &images) {
                    prompt.push(match piece {
                        super::Piece::Text(words) => json!({"type":"text","text":words}),
                        super::Piece::Image(image) => {
                            let (mime, _, data) = super::image(image)?;
                            json!({"type":"image","data":data,"mimeType":mime})
                        }
                    });
                }
                if prompt.is_empty() {
                    prompt.push(json!({"type":"text","text":text}));
                }
                self.prompt(json!(prompt)).await
            }
            Command::Interrupt => {
                if !self.busy {
                    return Ok(());
                }
                self.stopping = true;
                for (id, (request, _)) in std::mem::take(&mut self.asks) {
                    (self.emit)(Event::Settled(id));
                    self.send(json!({"jsonrpc":"2.0","id":request,
                        "result":{"outcome":{"outcome":"cancelled"}}}))
                        .await?;
                }
                let session = self.session.clone();
                self.stop_by = Some(Instant::now() + STOP_WAIT);
                self.send(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session}}))
                    .await
            }
            Command::Answer { id, response } => {
                let Some((request, options)) = self.asks.remove(&id) else {
                    return Ok(());
                };
                let options: Vec<&Value> = options.as_array().into_iter().flatten().collect();
                let picked = response["selectedActionId"].as_str();
                // A question's answer is its option, picked by its id.
                let asked = options
                    .iter()
                    .find(|option| option["optionId"].as_str() == picked && picked.is_some())
                    .and_then(|option| option["optionId"].as_str());
                let chosen = match (asked, response["behavior"] == "allow") {
                    (Some(option), true) => Some(option),
                    (_, allowed) => {
                        let wanted = match (allowed, picked) {
                            (false, _) => ["reject_once", "reject_always"],
                            (true, Some("allow_always")) => ["allow_always", "allow_once"],
                            (true, _) => ["allow_once", "allow_always"],
                        };
                        wanted
                            .iter()
                            .find_map(|kind| options.iter().find(|option| option["kind"] == *kind))
                            .and_then(|option| option["optionId"].as_str())
                    }
                };
                let outcome = match chosen {
                    Some(option) => json!({"outcome":"selected","optionId":option}),
                    None => json!({"outcome":"cancelled"}),
                };
                (self.emit)(Event::Settled(id));
                self.send(json!({"jsonrpc":"2.0","id":request,"result":{"outcome":outcome}}))
                    .await
            }
            Command::Mode(mode) => {
                self.start.mode = mode.clone();
                (self.emit)(Event::Mode(mode));
                if self.process.is_some() {
                    self.set_mode().await?;
                }
                Ok(())
            }
            Command::Model(model) => {
                self.start.model = model;
                if self.process.is_some() && !self.busy {
                    self.choose().await?;
                }
                Ok(())
            }
            Command::Thinking(thinking) => {
                self.start.thinking = thinking;
                if self.process.is_some() && !self.busy {
                    self.choose().await?;
                }
                Ok(())
            }
            Command::StopTask(_) => Ok(()),
            Command::Rewind { .. } => {
                (self.emit)(Event::Rewound(Err(
                    "Antigravity cannot go back in a conversation from here".into(),
                )));
                Ok(())
            }
        }
    }

    async fn read(&mut self, line: &str) -> io::Result<()> {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            // A sign-in link said to the terminal: the session is not
            // signed in (the link is never shown or opened).
            if line.contains("https://accounts.google.com/") {
                return Err(io::Error::other(NOT_SIGNED_IN));
            }
            return Ok(());
        };
        let params = &message["params"];
        match (message["method"].as_str(), message["id"].is_null()) {
            (Some(method), false) => self.asked(method, &message["id"], params).await,
            (Some("session/update"), true) => {
                self.update(&params["update"]);
                Ok(())
            }
            (Some(_), true) => Ok(()),
            (None, _) => {
                let Some(sent) = message["id"].as_u64().and_then(|id| self.sent.remove(&id)) else {
                    return Ok(());
                };
                let Sent::Prompt = sent else { return Ok(()) };
                self.prompts = self.prompts.saturating_sub(1);
                let error = message["error"]["message"].as_str();
                let code = message["error"]["code"].as_i64().unwrap_or_default();
                match (error, message["result"]["stopReason"].as_str()) {
                    (Some(error), _) if needs_sign_in(code, error) => {
                        self.outcome = Outcome::Failed(NOT_SIGNED_IN.to_owned());
                    }
                    (Some(error), _) => self.outcome = Outcome::Failed(error.to_owned()),
                    (None, Some("cancelled")) => self.outcome = Outcome::Canceled,
                    (None, Some("refusal")) => {
                        self.outcome = Outcome::Failed(
                            "Antigravity did not go on: the account's quota may be used up".into(),
                        );
                    }
                    _ if self.stopping => self.outcome = Outcome::Canceled,
                    _ => {}
                }
                // A prompt sent into this turn is still underway.
                if self.prompts == 0 {
                    self.idle();
                }
                Ok(())
            }
        }
    }

    /// A request of Antigravity's: a permission (answered here as the mode
    /// says, else shown), or a question (its `interaction_` calls, shown
    /// with their options); anything else is refused, so it does not wait.
    async fn asked(&mut self, method: &str, request: &Value, params: &Value) -> io::Result<()> {
        if method != "session/request_permission" {
            return self
                .send(json!({"jsonrpc":"2.0","id":request,
                    "error":{"code":-32601,"message":"Not supported by this host"}}))
                .await;
        }
        let call = &params["toolCall"];
        let id = call["toolCallId"].as_str().unwrap_or_default();
        let mut known = self.calls.get(id).cloned().unwrap_or_else(|| json!({}));
        if let (Some(known), Some(update)) = (known.as_object_mut(), call.as_object()) {
            for (key, value) in update {
                if !value.is_null() {
                    known.insert(key.clone(), value.clone());
                }
            }
        }
        let options = params["options"].clone();
        let question = id.starts_with("interaction_");
        if !question && answered(&self.start.mode, &known, &self.start.cwd) {
            let pick = ["allow_once", "allow_always"].iter().find_map(|kind| {
                options
                    .as_array()?
                    .iter()
                    .find(|option| option["kind"] == *kind)?["optionId"]
                    .as_str()
            });
            let outcome = match pick {
                Some(option) => json!({"outcome":"selected","optionId":option}),
                None => json!({"outcome":"cancelled"}),
            };
            return self
                .send(json!({"jsonrpc":"2.0","id":request,"result":{"outcome":outcome}}))
                .await;
        }
        let shown = row(id, &known);
        let input = &known["rawInput"];
        let asked = crate::state::id();
        // What Antigravity warns of with its options.
        let warning = options
            .as_array()
            .into_iter()
            .flatten()
            .find_map(|option| option["_meta"]["agy.security.warning"].as_str())
            .map(str::to_owned);
        let actions: Vec<Value> = if question {
            options
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|option| {
                    let id = option["optionId"].as_str()?;
                    Some(json!({"id":id,"label":option["name"].as_str().unwrap_or(id),"behavior":"allow"}))
                })
                .collect()
        } else {
            let always = options
                .as_array()
                .into_iter()
                .flatten()
                .any(|option| option["kind"] == "allow_always");
            let mut actions = vec![json!({"id":"allow","label":"Allow","behavior":"allow"})];
            if always {
                actions
                    .push(json!({"id":"allow_always","label":"Always allow","behavior":"allow"}));
            }
            actions.push(json!({"id":"deny","label":"Deny","behavior":"deny"}));
            actions
        };
        let mut description = known["title"].as_str().unwrap_or_default().to_owned();
        if let Some(warning) = warning {
            description = format!("{description}\n\n⚠ {warning}");
        }
        (self.emit)(Event::Asked(
            json!({"id":asked,"provider":"antigravity","name":shown["name"],
            "kind":if question { "question" } else { "tool" },
            "input":if input.is_object() && !question { input.clone() } else { json!({"description":description}) },
            "detail":shown["detail"],"actions":actions}),
        ));
        self.asks.insert(asked, (request.clone(), options));
        Ok(())
    }

    fn update(&mut self, update: &Value) {
        match update["sessionUpdate"].as_str() {
            Some("agent_message_chunk") => {
                let message = update["messageId"].as_str().map(str::to_owned);
                if message.is_some() && message != self.message {
                    self.fresh = true;
                    self.message = message;
                }
                if let Some(text) = update["content"]["text"].as_str() {
                    self.reply(text);
                }
            }
            Some("agent_thought_chunk") => {
                let Some(text) = update["content"]["text"]
                    .as_str()
                    .filter(|text| !text.is_empty())
                else {
                    return;
                };
                self.fresh = true;
                self.item(json!({"type":"reasoning","text":text}));
            }
            Some(kind @ ("tool_call" | "tool_call_update")) => {
                let Some(id) = update["toolCallId"].as_str().map(str::to_owned) else {
                    return;
                };
                let call = self.calls.entry(id.clone()).or_insert_with(|| json!({}));
                if kind == "tool_call" {
                    *call = json!({});
                }
                if let (Some(call), Some(update)) = (call.as_object_mut(), update.as_object()) {
                    for (key, value) in update {
                        if !value.is_null() && key != "sessionUpdate" {
                            call.insert(key.clone(), value.clone());
                        }
                    }
                }
                // Its checklist comes as an `update_steps` call, not a
                // plan: shown as one.
                let call = &self.calls[&id];
                let title = call["title"].as_str().unwrap_or_default();
                if title.ends_with("update_steps") && call["rawInput"]["steps"].is_array() {
                    let items: Vec<_> = call["rawInput"]["steps"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|step| json!({"text":step["title"],
                            "completed":matches!(step["status"].as_str(), Some("done" | "completed"))}))
                        .collect();
                    self.item(json!({"type":"todo","items":items}));
                    return;
                }
                let shown = row(&id, call);
                self.fresh = true;
                self.item(shown);
            }
            Some("plan") => {
                let items: Vec<_> = update["entries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|entry| json!({"text":entry["content"],"completed":entry["status"] == "completed"}))
                    .collect();
                self.item(json!({"type":"todo","items":items}));
            }
            // Its own mode, which this host's stands over.
            Some("current_mode_update") => {
                if let Some(mode) = update["currentModeId"].as_str() {
                    self.agent_mode = mode.to_owned();
                }
            }
            Some("usage_update") => {
                (self.emit)(Event::Usage(
                    json!({"contextWindowUsedTokens":update["used"],"contextWindowMaxTokens":update["size"]}),
                ));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sign_in_link_is_google_s_coming_back_to_this_host() {
        let link = "https://accounts.google.com/o/oauth2/v2/auth?response_type=code&client_id=x&state=s%201x&redirect_uri=http%3A%2F%2F127.0.0.1%3A47123%2F&scope=a+b";
        assert_eq!(sign_in_link(link), None, "a state with a space");
        let link = link.replace("s%201x", "s1");
        let pending = sign_in_link(&link).unwrap();
        assert_eq!(
            pending,
            Pending::Loopback { redirect: "http://127.0.0.1:47123/".into(), state: "s1".into() }
        );
        for wrong in [
            link.replace("accounts.google.com", "accounts.example.com"),
            link.replace("accounts.google.com", "u@accounts.google.com"),
            link.replace("response_type=code", "response_type=token"),
            link.replace("127.0.0.1%3A47123", "evil.example%3A47123"),
            link.replace("47123", "80"),
            format!("{link}&state=s2"),
            format!("{link}#x"),
        ] {
            assert_eq!(sign_in_link(&wrong), None, "{wrong}");
        }
    }

    #[test]
    fn a_code_sign_in_link_matches_agy_login_and_accepts_code_or_url() {
        let link = "https://accounts.google.com/o/oauth2/auth?access_type=offline&code_challenge_method=S256&redirect_uri=https%3A%2F%2Fantigravity.google%2Foauth-callback&response_type=code&state=login-state";
        let pending = code_sign_in_link(link).unwrap();
        assert_eq!(
            pending,
            Pending::Code {
                redirect: "https://antigravity.google/oauth-callback".into(),
                state: "login-state".into(),
            }
        );
        assert_eq!(sign_in_link(link), None, "code link is not loopback");
        for wrong in [
            link.replace("antigravity.google", "antigravity.google.example.invalid"),
            link.replace("/o/oauth2/auth", "/o/oauth2/v2/auth"),
            link.replace("accounts.google.com", "user@accounts.google.com"),
            format!("{link}#frag"),
        ] {
            assert_eq!(code_sign_in_link(&wrong), None, "{wrong}");
        }
        let page = "https://antigravity.google/oauth-callback?state=login-state&code=4%2F0Ab-c";
        assert_eq!(
            reply_for(&pending, " 4/0Ab-code_1.x~ "),
            Ok("4/0Ab-code_1.x~".to_owned())
        );
        assert_eq!(reply_for(&pending, page), Ok(page.to_owned()));
        for invalid in [
            "",
            "4/0Ab code",
            "4/0Ab\nsecond line",
            &page.replace("login-state", "other-state"),
            &page.replace("antigravity.google", "evil.example"),
            &page.replace("&code=4%2F0Ab-c", ""),
            &format!("{page}&code=again"),
        ] {
            assert!(reply_for(&pending, invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn a_pasted_reply_is_this_sign_in_s() {
        let pending = Pending::Loopback { redirect: "http://127.0.0.1:47123/".into(), state: "s1".into() };
        let reply = "http://127.0.0.1:47123/?state=s1&code=4%2F0abc&scope=x&iss=https%3A%2F%2Faccounts.google.com";
        assert_eq!(reply_for(&pending, &format!(" {reply} ")), Ok(reply.to_owned()));
        assert!(reply_for(&pending, "http://127.0.0.1:47123/?state=s1&error=access_denied").is_ok());
        for wrong in [
            "http://127.0.0.1:47123/?state=s2&code=c",
            "http://127.0.0.1:47124/?state=s1&code=c",
            "http://127.0.0.1:47123/x?state=s1&code=c",
            "http://127.0.0.1:47123/?state=s1",
            "http://127.0.0.1:47123/?state=s1&code=c&error=e",
            "http://127.0.0.1:47123/?state=s1&code=c&iss=https%3A%2F%2Fevil.example",
            "http://127.0.0.1:47123/?state=s1&code=c&iss=https%3A%2F%2Faccounts.google.com&iss=https%3A%2F%2Faccounts.google.com",
            "https://example.com/?state=s1&code=c",
        ] {
            assert!(reply_for(&pending, wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn only_versions_from_1_5_1_below_2_are_driven() {
        for version in ["1.5.1", "1.5.14", "agy_acp_server_1.6.0", "1.9.0-rc1"] {
            assert!(supported(version), "{version}");
        }
        for version in ["1.5.0", "1.4.9", "2.0.0", "", "unknown"] {
            assert!(!supported(version), "{version}");
        }
    }

    #[test]
    fn the_sign_in_is_named_by_the_settings_else_by_the_key_they_give() {
        assert_eq!(method(&json!({})), "oauth-personal");
        assert_eq!(
            method(&json!({"env":{"GEMINI_API_KEY":"k"}})),
            "gemini-api-key"
        );
        assert_eq!(
            method(&json!({"env":{"GOOGLE_API_KEY":"k"}})),
            "agent-platform"
        );
        assert_eq!(
            method(&json!({"authMethod":"oauth-business","env":{"GEMINI_API_KEY":"k"}})),
            "oauth-business"
        );
        assert_eq!(method(&json!({"authMethod":"made-up"})), "oauth-personal");
    }

    #[test]
    fn its_settings_say_how_it_signs_in_and_never_a_credential() {
        let said = profile_settings(
            &json!({"env":{"GEMINI_API_KEY":"secret"},"gcp":{"project":"p","location":"l"}}),
            "gemini-api-key",
        );
        assert_eq!(
            said,
            json!({"auth":{"type":"gemini-api-key"},"gcp":{"project":"p","location":"l"}})
        );
        assert!(!said.to_string().contains("secret"));
    }

    #[test]
    fn oauth_personal_sets_ssh_connection_and_disables_auto_update() {
        let personal = environment(
            Path::new("/h"),
            Path::new("/h/runs/1"),
            Path::new("/h/browser"),
            "oauth-personal",
        );
        fn get<'a>(env: &'a [(String, String)], name: &str) -> Option<&'a str> {
            env.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_str())
        }
        assert_eq!(get(&personal, "GEMINI_HOME"), Some("/h"));
        assert_eq!(get(&personal, "AGY_ACP_FORCE_FILE_STORAGE"), Some("1"));
        assert_eq!(get(&personal, "AGY_CLI_DISABLE_AUTO_UPDATE"), Some("true"));
        assert_eq!(get(&personal, "TMPDIR"), Some("/h/runs/1"));
        assert_eq!(get(&personal, "BROWSER"), Some("/h/browser"));
        assert!(get(&personal, "SSH_CONNECTION").is_some());

        // Only `oauth-personal` has `--login` and isolates via `SSH_CONNECTION`.
        for other in ["oauth-business", "gemini-api-key", "agent-platform"] {
            let env = environment(
                Path::new("/h"),
                Path::new("/h/runs/1"),
                Path::new("/h/browser"),
                other,
            );
            assert_eq!(get(&env, "SSH_CONNECTION"), None, "{other}");
            assert_eq!(get(&env, "AGY_CLI_DISABLE_AUTO_UPDATE"), Some("true"), "{other}");
        }
    }

    #[test]
    fn sanitized_settings_keeps_only_the_active_methods_credential() {
        let raw = json!({
            "env": {
                "GEMINI_HOME": "/h",
                "GEMINI_API_KEY": "g-key",
                "GOOGLE_API_KEY": "v-key",
                "CUSTOM": "keep"
            }
        });
        assert_eq!(
            sanitized_settings(&raw, "oauth-personal")["env"],
            json!({"CUSTOM": "keep"})
        );
        assert_eq!(
            sanitized_settings(&raw, "gemini-api-key")["env"],
            json!({"GEMINI_API_KEY": "g-key", "CUSTOM": "keep"})
        );
        assert_eq!(
            sanitized_settings(&raw, "agent-platform")["env"],
            json!({"GOOGLE_API_KEY": "v-key", "CUSTOM": "keep"})
        );
    }

    #[test]
    fn auth_status_tracks_verified_unchecked_and_signed_out_per_method() {
        let dir = tempfile::tempdir().unwrap();
        let settings = json!({"env":{"GEMINI_HOME":dir.path()}});
        let initial = auth_status(&settings);
        assert_eq!((&initial["loggedIn"], &initial["authStatus"]), (&json!(false), &json!("unknown")));
        let app = dir.path().join(APP_DATA);
        std::fs::create_dir_all(&app).unwrap();
        // Token files (both 1.5 and legacy) count as authenticated.
        std::fs::write(app.join("antigravity-oauth-token"), "{}").unwrap();
        assert_eq!(auth_status(&settings)["authStatus"], "authenticated");
        std::fs::remove_file(app.join("antigravity-oauth-token")).unwrap();
        std::fs::write(app.join("acp_token.json"), "{}").unwrap();
        assert_eq!(auth_status(&settings)["authStatus"], "authenticated");
        std::fs::remove_file(app.join("acp_token.json")).unwrap();
        // Verified marker (e.g. macOS Keychain storage with no token file) counts as authenticated.
        save_auth_state(dir.path(), "oauth-personal", "authenticated");
        assert_eq!((&auth_status(&settings)["loggedIn"], &auth_status(&settings)["authStatus"]), (&json!(true), &json!("authenticated")));
        // Explicit sign-out wins until re-authenticated.
        save_auth_state(dir.path(), "oauth-personal", "unauthenticated");
        assert_eq!((&auth_status(&settings)["loggedIn"], &auth_status(&settings)["authStatus"]), (&json!(false), &json!("unauthenticated")));
        // Switching authMethod ignores another method's marker.
        let key_missing = json!({"authMethod":"gemini-api-key","env":{"GEMINI_HOME":dir.path()}});
        assert_eq!(auth_status(&key_missing)["authStatus"], "unauthenticated");
        let key_present = json!({"authMethod":"gemini-api-key","env":{"GEMINI_HOME":dir.path(),"GEMINI_API_KEY":"k"}});
        assert_eq!(auth_status(&key_present)["authStatus"], "unknown");
        save_auth_state(dir.path(), "gemini-api-key", "authenticated");
        assert_eq!(auth_status(&key_present)["authStatus"], "authenticated");
    }

    #[cfg(unix)]
    #[test]
    fn global_gemini_rules_and_skills_link_into_profile_without_replacing_own() {
        let root = tempfile::tempdir().unwrap();
        let user_gemini = root.path().join("home/.gemini");
        let profile = root.path().join("profile");
        std::fs::create_dir_all(user_gemini.join("config/skills/review")).unwrap();
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(user_gemini.join("AGENTS.md"), "global").unwrap();
        std::fs::write(user_gemini.join("GEMINI.md"), "global-g").unwrap();
        std::fs::write(user_gemini.join("config/skills.json"), "{}").unwrap();
        // Profile owns its own GEMINI.md; empty skills placeholder is replaced.
        std::fs::write(profile.join("GEMINI.md"), "own").unwrap();
        std::fs::create_dir_all(profile.join("config/skills")).unwrap();

        link_user_config(&profile, &user_gemini);
        assert_eq!(
            std::fs::read_link(profile.join("AGENTS.md")).unwrap(),
            user_gemini.join("AGENTS.md")
        );
        assert_eq!(std::fs::read_to_string(profile.join("GEMINI.md")).unwrap(), "own");
        assert_eq!(
            std::fs::read_link(profile.join("config/skills")).unwrap(),
            user_gemini.join("config/skills")
        );
        assert_eq!(
            std::fs::read_link(profile.join("config/skills.json")).unwrap(),
            user_gemini.join("config/skills.json")
        );
        // Removing target cleans up the dangling symlink on next prepare.
        std::fs::remove_file(user_gemini.join("AGENTS.md")).unwrap();
        link_user_config(&profile, &user_gemini);
        assert!(profile.join("AGENTS.md").symlink_metadata().is_err());
    }

    #[test]
    fn a_mode_it_lacks_is_its_default() {
        let offered = ["default".to_owned(), "plan".to_owned()];
        assert_eq!(agent_mode("plan", &offered), "plan");
        assert_eq!(agent_mode("auto", &offered), "default");
        assert_eq!(agent_mode("bypassPermissions", &offered), "default");
        assert_eq!(agent_mode("plan", &["architect".to_owned()]), "architect");
    }

    #[test]
    fn what_is_answered_here_is_what_the_mode_lets_through() {
        let read = json!({"kind":"read"});
        let edit_in = json!({"kind":"edit","locations":[{"path":"/w/a.rs"}]});
        let edit_out = json!({"kind":"edit","locations":[{"path":"/etc/hosts"}]});
        let sneaking = json!({"kind":"edit","locations":[{"path":"/w/../etc/hosts"}]});
        let command = json!({"kind":"execute"});
        assert!(answered("default", &read, "/w"));
        assert!(!answered("default", &edit_in, "/w"));
        assert!(answered("acceptEdits", &edit_in, "/w"));
        assert!(!answered("acceptEdits", &edit_out, "/w"));
        assert!(!answered("acceptEdits", &sneaking, "/w"));
        assert!(!answered("acceptEdits", &command, "/w"));
        assert!(answered("bypassPermissions", &command, "/w"));
        assert!(!answered("auto", &command, "/w"));
    }

    #[test]
    fn the_models_are_the_session_s_model_option_with_its_thinking_levels() {
        let opened = json!({"configOptions":[
            {"id":"model","category":"model","type":"select","currentValue":"gemini-3-pro",
             "options":[{"value":"gemini-3-pro","name":"Gemini 3 Pro"},{"value":"claude-sonnet","name":"Claude Sonnet"}]},
            {"id":"effort","category":"thought_level","type":"select","currentValue":"high",
             "options":[{"value":"low","name":"Low"},{"value":"high","name":"High"}]}]});
        let models = offered_models(&opened);
        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["isDefault"], true);
        assert_eq!(models[1]["label"], "Claude Sonnet");
        assert_eq!(models[1]["thinkingOptions"][1]["id"], "high");
        assert_eq!(models[0]["defaultThinkingOptionId"], "high");
        let listed = json!({"models":{"currentModelId":"m2","availableModels":[{"modelId":"m1","name":"One"},{"modelId":"m2"}]}});
        let models = offered_models(&listed);
        assert_eq!(
            (models[1]["id"].as_str(), models[1]["isDefault"].as_bool()),
            (Some("m2"), Some(true))
        );
    }
}
