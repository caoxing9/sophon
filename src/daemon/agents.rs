//! The Agents settings page's data: what a provider's diagnostic says, and
//! the config patches that switch providers on and off and add or remove
//! the instances Sophon creates.
//!
//! Patches are for `set_daemon_config_request`: the daemon merges them, so
//! each one carries only the keys it changes.

use serde_json::{Map, Value, json};

use super::model::Provider;

/// The description of instances Sophon adds: Sophon removes only these.
/// (The daemon keeps only known fields, so a marker has to live in one.)
pub const SOPHON_MARK: &str = "Added by Sophon";

/// Providers the daemon has built in: their entries are never removed.
const BUILTIN: &[&str] = &["claude", "codex", "copilot", "opencode", "pi", "omp"];

/// Families whose own model ids are `provider/model`: such an id says
/// nothing about a gateway there.
const SLASHED_FAMILIES: &[&str] = &["opencode", "pi", "omp"];

// ------------------------------------------------------------- diagnostic

/// What `provider_diagnostic_request` tells about a provider. A field the
/// text does not give stays empty.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Diagnostic {
    pub version: Option<String>,
    /// The binary the daemon runs.
    pub path: Option<String>,
    pub login: Option<Login>,
    /// The diagnostic itself failed.
    pub error: Option<String>,
}

/// The account a provider is signed in with (Claude Code reports it).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Login {
    pub signed_in: bool,
    /// `claude.ai`, `api_key`, … as the CLI says it.
    pub method: Option<String>,
    pub email: Option<String>,
    /// Readable already: `Max 20x`.
    pub plan: Option<String>,
    /// The day it expires: `2026-11-29`.
    pub expires: Option<String>,
    /// The CLI's config folder, which tells the host's home.
    pub config_dir: Option<String>,
}

/// Reads a diagnostic: a title line, then `  Label: value` rows whose value
/// may run over more lines (the `Auth` JSON does).
pub fn parse_diagnostic(text: &str) -> Diagnostic {
    let mut rows: Vec<(String, String)> = Vec::new();
    for line in text.lines().skip(1) {
        match row_start(line) {
            Some((label, value)) => rows.push((label.to_owned(), value.to_owned())),
            None => {
                if let Some((_, value)) = rows.last_mut() {
                    value.push('\n');
                    value.push_str(line);
                }
            }
        }
    }
    let get = |label: &str| rows.iter().find(|(l, _)| l == label).map(|(_, v)| v.as_str());
    let first_line = |v: &str| v.lines().next().unwrap_or("").trim().to_owned();
    Diagnostic {
        version: get("Version").map(first_line).filter(|v| !v.is_empty() && v != "unknown").map(|v| clean_version(&v)),
        path: get("Resolved path").map(first_line).filter(|p| p.starts_with('/') || p.contains(":\\")),
        login: get("Auth").and_then(parse_login),
        error: get("Error").map(|e| e.trim().to_owned()).filter(|e| !e.is_empty()),
    }
}

/// `  Label: value`: two spaces, a capitalized label of words, a colon.
fn row_start(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("  ")?;
    if !rest.starts_with(|c: char| c.is_ascii_uppercase()) {
        return None;
    }
    let (label, value) = rest.split_once(':')?;
    if !label.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-') {
        return None;
    }
    Some((label, value.trim_start()))
}

/// The number out of `2.1.321 (Claude Code)`, `codex-cli 0.163.0`,
/// `opencode v2.0.25`; the text as is when there is none.
fn clean_version(v: &str) -> String {
    v.split_whitespace()
        .map(|w| w.trim_start_matches('v'))
        .find(|w| w.starts_with(|c: char| c.is_ascii_digit()) && w.contains('.'))
        .map(str::to_owned)
        .unwrap_or_else(|| v.to_owned())
}

/// The `Auth` block when it is JSON (`claude auth status --json`); other
/// shapes say nothing we can show reliably.
fn parse_login(value: &str) -> Option<Login> {
    let json: Value = serde_json::from_str(value.trim()).ok()?;
    let s = |k: &str| json.get(k).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
    let signed_in = json.get("loggedIn").and_then(Value::as_bool)?;
    Some(Login {
        signed_in,
        method: s("authMethod"),
        email: s("email").filter(|e| e.contains('@')),
        plan: s("planTier").or_else(|| s("subscriptionType")).map(|p| readable_plan(&p)),
        expires: s("loginExpiresAt").and_then(|d| day_of(&d)),
        config_dir: s("configDirectory"),
    })
}

/// `max_20x` → `Max 20x`.
fn readable_plan(p: &str) -> String {
    let words = p.replace(['_', '-'], " ");
    let mut c = words.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// The `YYYY-MM-DD` of an ISO timestamp.
fn day_of(ts: &str) -> Option<String> {
    let day = ts.get(..10)?;
    let b = day.as_bytes();
    let digits = |r: std::ops::Range<usize>| b[r].iter().all(u8::is_ascii_digit);
    (digits(0..4) && b[4] == b'-' && digits(5..7) && b[7] == b'-' && digits(8..10)).then(|| day.to_owned())
}

/// The host's home, from the folder Claude Code keeps its config in
/// (`/Users/me/.claude`).
pub fn home_from_config_dir(dir: &str) -> Option<String> {
    let home = dir.trim_end_matches('/').strip_suffix("/.claude")?;
    (!home.is_empty()).then(|| home.to_owned())
}

// ---------------------------------------------------------------- gateway

/// The built-in provider an entry extends (itself when built in).
pub fn family(id: &str, config: Option<&Value>) -> String {
    entry(config, id).and_then(|e| e.get("extends")).and_then(Value::as_str).unwrap_or(id).to_owned()
}

/// Whether some of the provider's models come through a gateway: a
/// `provider/model` id, or a description saying "via magpie".
pub fn through_gateway(p: &Provider, family: &str) -> bool {
    let slashes_count = !SLASHED_FAMILIES.contains(&family);
    p.models
        .iter()
        .any(|m| (slashes_count && m.id.contains('/')) || m.description.to_lowercase().contains("via magpie"))
}

/// Claude Code on the user's own account: the built-in one, or an account
/// instance (extends it, no endpoint of its own). It must run natively.
pub fn native_claude(id: &str, config: Option<&Value>) -> bool {
    if id == "claude" {
        return true;
    }
    let Some(e) = entry(config, id) else { return false };
    e.get("extends").and_then(Value::as_str) == Some("claude") && e.pointer("/env/ANTHROPIC_BASE_URL").is_none()
}

// ---------------------------------------------------------------- config

fn entry<'a>(config: Option<&'a Value>, id: &str) -> Option<&'a Value> {
    config?.get("providers")?.get(id)
}

/// Whether the provider is on: the config's word, else the snapshot's.
pub fn enabled(config: Option<&Value>, p: &Provider) -> bool {
    entry(config, &p.id).and_then(|e| e.get("enabled")).and_then(Value::as_bool).unwrap_or(p.enabled)
}

/// An instance Sophon added: Sophon may remove it. Built-in entries never.
pub fn created_by_sophon(config: Option<&Value>, id: &str) -> bool {
    !BUILTIN.contains(&id)
        && entry(config, id).is_some_and(|e| {
            e.get("extends").is_some() && e.get("description").and_then(Value::as_str) == Some(SOPHON_MARK)
        })
}

pub fn enable_patch(id: &str, on: bool) -> Value {
    json!({ "providers": { id: { "enabled": on } } })
}

pub fn remove_patch(id: &str) -> Value {
    json!({ "removeProviders": [id] })
}

/// What an instance runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Claude,
    Codex,
}

impl Base {
    pub fn id(self) -> &'static str {
        match self {
            Base::Claude => "claude",
            Base::Codex => "codex",
        }
    }

    /// The variable that points the CLI at its own config folder.
    pub fn home_var(self) -> &'static str {
        match self {
            Base::Claude => "CLAUDE_CONFIG_DIR",
            Base::Codex => "CODEX_HOME",
        }
    }

    /// The suggested folder for another account: `~/.claude-work`.
    pub fn default_dir(self, slug: &str) -> String {
        match self {
            Base::Claude => format!("~/.claude-{slug}"),
            Base::Codex => format!("~/.codex-{slug}"),
        }
    }
}

/// A provider id from a name: lowercase letters, digits and hyphens, a
/// letter first (`^[a-z][a-z0-9-]*$`), and not one in `taken`.
pub fn new_id(name: &str, fallback: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    for c in name.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let mut slug = slug.trim_end_matches('-').chars().take(32).collect::<String>().trim_end_matches('-').to_owned();
    if !slug.starts_with(|c: char| c.is_ascii_lowercase()) {
        slug = if slug.is_empty() { fallback.to_owned() } else { format!("{fallback}-{slug}") };
    }
    let free = |id: &str| !BUILTIN.contains(&id) && !taken.iter().any(|t| t == id);
    if free(&slug) {
        return slug;
    }
    (2..).map(|n| format!("{slug}-{n}")).find(|id| free(id)).unwrap_or(slug)
}

/// `~/x` on the host whose home is `home`; a full path as is. `None` when
/// it cannot be made absolute (the CLI does not expand `~`).
pub fn absolute_dir(dir: &str, home: Option<&str>) -> Option<String> {
    let dir = dir.trim();
    if dir.starts_with('/') {
        return Some(dir.trim_end_matches('/').to_owned()).filter(|d| !d.is_empty());
    }
    let rest = dir.strip_prefix("~/")?.trim_end_matches('/');
    let home = home?.trim_end_matches('/');
    (!rest.is_empty() && !home.is_empty()).then(|| format!("{home}/{rest}"))
}

/// Another account of a CLI: its own config folder, so its login is its own.
pub fn account_patch(id: &str, base: Base, label: &str, dir: &str) -> Value {
    json!({ "providers": { id: {
        "extends": base.id(),
        "label": label.trim(),
        "description": SOPHON_MARK,
        "env": { base.home_var(): dir },
    } } })
}

/// The command that signs the account in, run on the host.
pub fn login_command(base: Base, dir: &str) -> String {
    let dir = shell_quote(dir);
    match base {
        Base::Claude => format!("CLAUDE_CONFIG_DIR={dir} claude auth login"),
        // Codex refuses a CODEX_HOME that does not exist.
        Base::Codex => format!("mkdir -p {dir} && CODEX_HOME={dir} codex login"),
    }
}

fn shell_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-~+".contains(c)) {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// An endpoint that takes an API key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub base: Base,
    pub base_url: &'static str,
    /// (id, label); the first is the default.
    pub models: &'static [(&'static str, &'static str)],
    pub extra_env: &'static [(&'static str, &'static str)],
}

/// The endpoints Paseo documents (`docs/custom-providers.md`), as written
/// there. Others go through Custom.
pub const PRESETS: &[Preset] = &[
    Preset {
        id: "zai",
        label: "Z.AI (GLM)",
        base: Base::Claude,
        base_url: "https://api.z.ai/api/anthropic",
        models: &[("glm-5-turbo", "GLM 5 Turbo"), ("glm-5.1", "GLM 5.1"), ("glm-4.5-air", "GLM 4.5 Air")],
        extra_env: &[("API_TIMEOUT_MS", "3000000")],
    },
    Preset {
        id: "qwen",
        label: "Qwen (Alibaba)",
        base: Base::Claude,
        base_url: "https://coding-intl.dashscope.aliyuncs.com/apps/anthropic",
        models: &[("qwen3.5-plus", "Qwen 3.5 Plus"), ("qwen3-coder-next", "Qwen 3 Coder Next"), ("kimi-k2.5", "Kimi K2.5")],
        extra_env: &[],
    },
];

/// An instance on an API key.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyInstance {
    pub label: String,
    pub base: Base,
    pub base_url: String,
    pub key: String,
    /// (id, label); the first is the default.
    pub models: Vec<(String, String)>,
    pub extra_env: Vec<(String, String)>,
    /// Claude Code's config folder for it: empty of any subscription login,
    /// and of the user's settings.json `env`, which could reroute it.
    pub claude_dir: Option<String>,
}

/// Model ids typed as a list: commas, spaces or lines between them.
pub fn parse_models(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for id in text.split(|c: char| c == ',' || c.is_whitespace()).filter(|s| !s.is_empty()) {
        if !out.iter().any(|(m, _)| m == id) {
            out.push((id.to_owned(), id.to_owned()));
        }
    }
    out
}

/// Why the instance cannot be added yet, if so.
pub fn key_instance_problem(k: &KeyInstance) -> Option<&'static str> {
    if k.label.trim().is_empty() {
        Some("Give it a name")
    } else if !(k.base_url.starts_with("https://") || k.base_url.starts_with("http://")) {
        Some("The endpoint is a URL starting with https://")
    } else if k.key.trim().is_empty() {
        Some("Paste the API key")
    } else if k.models.is_empty() {
        Some("List at least one model")
    } else if k.base == Base::Claude && k.claude_dir.is_none() {
        Some("Sophon does not know that computer's home folder yet")
    } else {
        None
    }
}

/// The instance's entry: the endpoint and key in its environment, its
/// models listed (the endpoint's ids, not Claude's or OpenAI's), never the
/// subscription login of the CLI it runs on.
pub fn key_instance_patch(id: &str, k: &KeyInstance) -> Value {
    let mut env = Map::new();
    match k.base {
        Base::Claude => {
            env.insert("ANTHROPIC_BASE_URL".into(), k.base_url.trim().into());
            env.insert("ANTHROPIC_AUTH_TOKEN".into(), k.key.trim().into());
            if let Some(dir) = &k.claude_dir {
                env.insert("CLAUDE_CONFIG_DIR".into(), dir.clone().into());
            }
        }
        Base::Codex => {
            // Paseo then routes Codex to the endpoint and turns its
            // ChatGPT login off (`requires_openai_auth = false`).
            env.insert("OPENAI_BASE_URL".into(), k.base_url.trim().into());
            env.insert("OPENAI_API_KEY".into(), k.key.trim().into());
        }
    }
    for (name, value) in &k.extra_env {
        env.insert(name.clone(), value.clone().into());
    }
    let models: Vec<Value> = k
        .models
        .iter()
        .enumerate()
        .map(|(i, (id, label))| {
            let mut m = json!({ "id": id, "label": label });
            if i == 0 {
                m["isDefault"] = true.into();
            }
            m
        })
        .collect();
    let mut provider = json!({
        "extends": k.base.id(),
        "label": k.label.trim(),
        "description": SOPHON_MARK,
        "env": env,
        "models": models,
    });
    if k.base == Base::Claude {
        // Anthropic's server-side web search does not exist elsewhere.
        provider["disallowedTools"] = json!(["WebSearch"]);
    }
    json!({ "providers": { id: provider } })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::model::Model;

    const CLAUDE: &str = r#"Claude Code
  Command source: default
  Configured command: claude
  Daemon PATH: /usr/bin:/bin
  PATH matches: /Users/me/.local/bin/claude
  which -a claude: /Users/me/.local/bin/claude
  zsh -lc type -a claude: claude is /Users/me/.local/bin/claude
  Binary: claude
  Resolved path: /Users/me/.local/bin/claude
  Version: 2.1.321 (Claude Code)
  Auth: {
  "loggedIn": true,
  "authMethod": "claude.ai",
  "apiProvider": "firstParty",
  "configDirectory": "/Users/me/.claude",
  "loginExpiresAt": "2026-11-29T12:25:01.053Z",
  "email": "someone@example.com",
  "orgName": "Org: with colon",
  "subscriptionType": "max",
  "planTier": "max_20x"
}
  Models: 19
  Status: Ready"#;

    #[test]
    fn reads_a_claude_diagnostic() {
        let d = parse_diagnostic(CLAUDE);
        assert_eq!(d.version.as_deref(), Some("2.1.321"));
        assert_eq!(d.path.as_deref(), Some("/Users/me/.local/bin/claude"));
        let l = d.login.unwrap();
        assert!(l.signed_in);
        assert_eq!(l.method.as_deref(), Some("claude.ai"));
        assert_eq!(l.email.as_deref(), Some("someone@example.com"));
        assert_eq!(l.plan.as_deref(), Some("Max 20x"));
        assert_eq!(l.expires.as_deref(), Some("2026-11-29"));
        assert_eq!(home_from_config_dir(&l.config_dir.unwrap()).as_deref(), Some("/Users/me"));
        assert_eq!(d.error, None);
    }

    #[test]
    fn missing_or_odd_fields_stay_empty() {
        let copilot = "Copilot\n  Binary: copilot\n  Resolved path: not found\n  Version: unknown\n  Models: —\n  Status: Unavailable";
        assert_eq!(parse_diagnostic(copilot), Diagnostic::default());

        let codex = "Codex\n  which -a codex: /opt/homebrew/bin/codex\n/opt/homebrew/bin/codex\n  Resolved path: /opt/homebrew/bin/codex\n  Version: codex-cli 0.163.0";
        let d = parse_diagnostic(codex);
        assert_eq!(d.version.as_deref(), Some("0.163.0"));
        assert_eq!(d.path.as_deref(), Some("/opt/homebrew/bin/codex"));
        assert_eq!(d.login, None);

        // OpenCode's Auth is a table, not JSON: nothing shown.
        let opencode = "OpenCode\n  Version: opencode v2.0.25\n  Auth: \n    OpenAI  OAuth   stored\n  Status: Unavailable";
        let d = parse_diagnostic(opencode);
        assert_eq!(d.version.as_deref(), Some("2.0.25"));
        assert_eq!(d.login, None);

        // Broken JSON, a signed-out login, junk values.
        assert_eq!(parse_diagnostic("X\n  Auth: { \"loggedIn\": tru").login, None);
        let out = parse_diagnostic("X\n  Auth: {\"loggedIn\": false, \"email\": \"nope\", \"loginExpiresAt\": \"soon\"}").login.unwrap();
        assert_eq!(out, Login { signed_in: false, ..Default::default() });

        assert_eq!(parse_diagnostic("Claude Code\n  Error: boom").error.as_deref(), Some("boom"));
        assert_eq!(parse_diagnostic(""), Diagnostic::default());
    }

    fn provider(models: &[(&str, &str)]) -> Provider {
        Provider {
            id: "p".into(),
            models: models.iter().map(|(id, d)| Model { id: (*id).into(), description: (*d).into(), ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn spots_gateway_models() {
        assert!(!through_gateway(&provider(&[("claude-opus-5", "Opus 5"), ("fable", "From settings.json")]), "claude"));
        assert!(through_gateway(&provider(&[("teable/gpt-5.6-sol", "")]), "codex"));
        assert!(through_gateway(&provider(&[("gpt", "GPT · Copilot via Magpie")]), "codex"));
        // OpenCode's own ids are provider/model.
        assert!(!through_gateway(&provider(&[("anthropic/claude", "")]), "opencode"));
    }

    #[test]
    fn native_claude_is_the_builtin_or_an_account() {
        let cfg = json!({ "providers": {
            "work": { "extends": "claude", "env": { "CLAUDE_CONFIG_DIR": "/h/.claude-work" } },
            "zai": { "extends": "claude", "env": { "ANTHROPIC_BASE_URL": "https://api.z.ai/api/anthropic" } },
        } });
        assert!(native_claude("claude", None));
        assert!(native_claude("work", Some(&cfg)));
        assert!(!native_claude("zai", Some(&cfg)));
        assert!(!native_claude("codex", Some(&cfg)));
        assert_eq!(family("zai", Some(&cfg)), "claude");
        assert_eq!(family("codex", Some(&cfg)), "codex");
    }

    #[test]
    fn enable_and_remove_patch_only_their_keys() {
        assert_eq!(enable_patch("copilot", false), json!({ "providers": { "copilot": { "enabled": false } } }));
        assert_eq!(remove_patch("work"), json!({ "removeProviders": ["work"] }));
        let p = Provider { id: "omp".into(), enabled: false, ..Default::default() };
        assert!(!enabled(None, &p));
        assert!(enabled(Some(&json!({ "providers": { "omp": { "enabled": true } } })), &p));
    }

    #[test]
    fn only_sophon_instances_are_removable() {
        let cfg = json!({ "providers": {
            "claude": { "extends": "claude", "description": SOPHON_MARK },
            "mine": { "extends": "claude", "description": SOPHON_MARK },
            "theirs": { "extends": "claude", "label": "Theirs" },
            "copilot": { "enabled": false },
        } });
        assert!(created_by_sophon(Some(&cfg), "mine"));
        assert!(!created_by_sophon(Some(&cfg), "theirs"));
        assert!(!created_by_sophon(Some(&cfg), "claude"));
        assert!(!created_by_sophon(Some(&cfg), "copilot"));
        assert!(!created_by_sophon(None, "mine"));
    }

    #[test]
    fn ids_fit_the_daemon_pattern() {
        let taken = vec!["work".to_owned()];
        assert_eq!(new_id("Work", "claude", &taken), "work-2");
        assert_eq!(new_id("  My Team!  ", "claude", &[]), "my-team");
        assert_eq!(new_id("工作", "acct", &[]), "acct");
        assert_eq!(new_id("2nd", "codex", &[]), "codex-2nd");
        assert_eq!(new_id("Claude", "claude", &[]), "claude-2");
        for id in ["work-2", "my-team", "acct", "codex-2nd", "claude-2"] {
            assert!(id.starts_with(|c: char| c.is_ascii_lowercase()));
            assert!(id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
        }
    }

    #[test]
    fn folders_become_absolute_on_the_host() {
        assert_eq!(absolute_dir("~/.claude-work", Some("/Users/me")).as_deref(), Some("/Users/me/.claude-work"));
        assert_eq!(absolute_dir("/srv/claude/", None).as_deref(), Some("/srv/claude"));
        assert_eq!(absolute_dir("~/.claude-work", None), None);
        assert_eq!(absolute_dir("relative", Some("/Users/me")), None);
        assert_eq!(absolute_dir("~/", Some("/Users/me")), None);
    }

    #[test]
    fn an_account_gets_its_own_config_folder() {
        assert_eq!(
            account_patch("work", Base::Claude, " Work ", "/Users/me/.claude-work"),
            json!({ "providers": { "work": {
                "extends": "claude", "label": "Work", "description": SOPHON_MARK,
                "env": { "CLAUDE_CONFIG_DIR": "/Users/me/.claude-work" },
            } } })
        );
        assert_eq!(account_patch("cx", Base::Codex, "Cx", "/h/.codex-cx")["providers"]["cx"]["env"], json!({ "CODEX_HOME": "/h/.codex-cx" }));
        assert_eq!(login_command(Base::Claude, "/Users/me/.claude-work"), "CLAUDE_CONFIG_DIR=/Users/me/.claude-work claude auth login");
        assert_eq!(login_command(Base::Codex, "/a b"), "mkdir -p '/a b' && CODEX_HOME='/a b' codex login");
    }

    #[test]
    fn a_key_instance_never_uses_the_subscription_login() {
        let zai = PRESETS[0];
        let k = KeyInstance {
            label: "ZAI".into(),
            base: zai.base,
            base_url: zai.base_url.into(),
            key: " secret ".into(),
            models: zai.models.iter().map(|(i, l)| ((*i).into(), (*l).into())).collect(),
            extra_env: zai.extra_env.iter().map(|(n, v)| ((*n).into(), (*v).into())).collect(),
            claude_dir: Some("/Users/me/.claude-zai".into()),
        };
        assert_eq!(key_instance_problem(&k), None);
        let p = &key_instance_patch("zai", &k)["providers"]["zai"];
        assert_eq!(p["extends"], "claude");
        assert_eq!(p["description"], SOPHON_MARK);
        assert_eq!(
            p["env"],
            json!({
                "ANTHROPIC_BASE_URL": "https://api.z.ai/api/anthropic",
                "ANTHROPIC_AUTH_TOKEN": "secret",
                "CLAUDE_CONFIG_DIR": "/Users/me/.claude-zai",
                "API_TIMEOUT_MS": "3000000",
            })
        );
        assert_eq!(p["disallowedTools"], json!(["WebSearch"]));
        assert_eq!(p["models"][0], json!({ "id": "glm-5-turbo", "label": "GLM 5 Turbo", "isDefault": true }));
        assert_eq!(p["models"][1].get("isDefault"), None);
        // An instance without its own config folder is refused.
        assert!(key_instance_problem(&KeyInstance { claude_dir: None, ..k.clone() }).is_some());

        let cx = KeyInstance {
            label: "Relay".into(),
            base: Base::Codex,
            base_url: "https://relay.example.com".into(),
            key: "sk".into(),
            models: parse_models("a/b, c\nc d"),
            extra_env: vec![],
            claude_dir: None,
        };
        assert_eq!(key_instance_problem(&cx), None);
        let p = &key_instance_patch("relay", &cx)["providers"]["relay"];
        assert_eq!(p["env"], json!({ "OPENAI_BASE_URL": "https://relay.example.com", "OPENAI_API_KEY": "sk" }));
        assert_eq!(p.get("disallowedTools"), None);
        assert_eq!(p["models"].as_array().unwrap().len(), 3);
        assert_eq!(key_instance_problem(&KeyInstance { base_url: "relay".into(), ..cx }), Some("The endpoint is a URL starting with https://"));
    }
}
