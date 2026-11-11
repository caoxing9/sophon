//! The models each agent offers, read from what the agent itself keeps on
//! the computer, not from the service's built-in list (which an older
//! service lacks new models in):
//!
//! - Claude Code: its catalog cache, `<config>/cache/model-catalog/*-cc.json`,
//!   the list `/model` shows, its `main` section first.
//! - Codex: the catalog its `config.toml` names (`model_catalog_json`, a
//!   gateway's), else its own cache, `<home>/models_cache.json`: the models
//!   shown (`visibility: list`) in their order.
//!
//! Each with its reasoning levels. Other agents keep the service's list.
//!
//! A provider switcher (cc-switch) or a hand-written config points an agent
//! at another endpoint and names its models where the agent reads them:
//! Claude Code's `settings.json` `env` (`ANTHROPIC_MODEL`, the
//! `ANTHROPIC_DEFAULT_*_MODEL` three), Codex's `config.toml` `model`. Those
//! are not in either catalog, so they are added first: they are what the
//! agent runs on now.

// The phone reads no local lists yet (a remote host's come through the
// service later): its parsers wait for that.
#![cfg_attr(any(target_os = "ios", target_arch = "wasm32"), allow(dead_code))]

use serde_json::Value;

/// A model an agent offers.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LocalModel {
    pub(crate) id: String,
    pub(crate) name: String,
    /// In the picker's first section (Claude Code's `main`; every Codex one).
    pub(crate) main: bool,
    /// (id, label) of its reasoning levels.
    pub(crate) thinking: Vec<(String, String)>,
    pub(crate) default_thinking: Option<String>,
}

/// Claude Code's catalog cache: its models in order.
pub(crate) fn parse_claude(text: &str) -> Option<Vec<LocalModel>> {
    let v: Value = serde_json::from_str(text).ok()?;
    let models = v.pointer("/catalog/config/models")?.as_array()?;
    let list: Vec<LocalModel> = models
        .iter()
        .filter_map(|m| {
            let options = m.pointer("/thinking/effort_options").and_then(Value::as_array);
            let thinking = options
                .into_iter()
                .flatten()
                .filter_map(|o| Some((o.get("id")?.as_str()?.to_owned(), o.get("name")?.as_str()?.to_owned())))
                .collect();
            // The option Claude Code recommends is its default.
            let default_thinking = options.into_iter().flatten().find_map(|o| {
                (o.pointer("/badge/message").and_then(Value::as_str) == Some("Recommended"))
                    .then(|| o.get("id")?.as_str().map(str::to_owned))
                    .flatten()
            });
            Some(LocalModel {
                id: m.get("id")?.as_str()?.to_owned(),
                name: m.get("name")?.as_str()?.to_owned(),
                main: m.get("section").and_then(Value::as_str) == Some("main"),
                thinking,
                default_thinking,
            })
        })
        .collect();
    (!list.is_empty()).then_some(list)
}

/// A Codex catalog (its cache, or the one its config names): the models it
/// shows, by priority.
pub(crate) fn parse_codex(text: &str) -> Option<Vec<LocalModel>> {
    let v: Value = serde_json::from_str(text).ok()?;
    let models = v.get("models")?.as_array()?;
    let mut shown: Vec<(i64, LocalModel)> = models
        .iter()
        .filter(|m| m.get("visibility").and_then(Value::as_str).is_none_or(|v| v == "list"))
        .filter_map(|m| {
            let id = m.get("slug")?.as_str()?.to_owned();
            let thinking = m
                .get("supported_reasoning_levels")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|l| {
                    let effort = l.get("effort")?.as_str()?;
                    Some((effort.to_owned(), title(effort)))
                })
                .collect();
            let model = LocalModel {
                name: m.get("display_name").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| id.clone()),
                id,
                main: true,
                thinking,
                default_thinking: m.get("default_reasoning_level").and_then(Value::as_str).map(str::to_owned),
            };
            Some((m.get("priority").and_then(Value::as_i64).unwrap_or(i64::MAX), model))
        })
        .collect();
    shown.sort_by_key(|(p, _)| *p);
    let list: Vec<LocalModel> = shown.into_iter().map(|(_, m)| m).collect();
    (!list.is_empty()).then_some(list)
}

/// `xhigh` as `Xhigh`: a level's label where the catalog gives none.
fn title(id: &str) -> String {
    let mut s = id.to_owned();
    if let Some(first) = s.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    s
}

/// The catalog file Codex reads: `model_catalog_json` of its `config.toml`
/// when set (relative to its home), else its own cache.
pub(crate) fn codex_catalog_path(config_toml: &str, home: &std::path::Path) -> std::path::PathBuf {
    let named = config_toml.lines().find_map(|line| {
        let line = line.trim();
        let rest = line.strip_prefix("model_catalog_json")?.trim_start().strip_prefix('=')?.trim();
        Some(rest.trim_matches('"').trim_matches('\'').to_owned())
    });
    match named.filter(|p| !p.is_empty()) {
        Some(p) if p.starts_with('/') => p.into(),
        Some(p) if p.starts_with("~/") => std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default().join(&p[2..]),
        Some(p) => home.join(p),
        None => home.join("models_cache.json"),
    }
}

/// The models Claude Code's `settings.json` names in its `env`, and whether
/// it also names another endpoint (`ANTHROPIC_BASE_URL`), where the
/// catalog's own models may not exist.
pub(crate) fn claude_configured(settings_json: &str) -> (Vec<LocalModel>, bool) {
    let v: Value = serde_json::from_str(settings_json).unwrap_or(Value::Null);
    let env = |key: &str| v.pointer(&format!("/env/{key}")).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty());
    let mut list: Vec<LocalModel> = Vec::new();
    // The one it starts on first, then the ones its opus, sonnet and haiku
    // names stand for.
    for key in ["ANTHROPIC_MODEL", "ANTHROPIC_DEFAULT_OPUS_MODEL", "ANTHROPIC_DEFAULT_SONNET_MODEL", "ANTHROPIC_DEFAULT_HAIKU_MODEL", "ANTHROPIC_SMALL_FAST_MODEL"] {
        if let Some(id) = env(key)
            && !list.iter().any(|m| m.id == id)
        {
            list.push(LocalModel { id: id.to_owned(), name: id.to_owned(), main: true, thinking: Vec::new(), default_thinking: None });
        }
    }
    (list, env("ANTHROPIC_BASE_URL").is_some())
}

/// The model Codex's `config.toml` starts on (its top-level `model`), with
/// the reasoning levels every Codex model takes.
pub(crate) fn codex_configured(config_toml: &str) -> Option<LocalModel> {
    let top = |key: &str| {
        // Top-level keys come before the first table.
        config_toml.lines().map(str::trim).take_while(|l| !l.starts_with('[')).find_map(|line| {
            let rest = line.strip_prefix(key)?.trim_start().strip_prefix('=')?.trim();
            let value = rest.split('#').next()?.trim().trim_matches('"').trim_matches('\'');
            (!value.is_empty()).then(|| value.to_owned())
        })
    };
    let id = top("model")?;
    Some(LocalModel {
        name: id.clone(),
        id,
        main: true,
        thinking: ["low", "medium", "high"].iter().map(|l| ((*l).to_owned(), title(l))).collect(),
        default_thinking: top("model_reasoning_effort"),
    })
}

/// `configured` first, then the `catalog`'s others; the catalog's own are
/// second when the agent talks to `another_endpoint`.
fn with_configured(configured: Vec<LocalModel>, catalog: Option<Vec<LocalModel>>, another_endpoint: bool) -> Option<Vec<LocalModel>> {
    let mut catalog = catalog.unwrap_or_default();
    let mut list: Vec<LocalModel> = Vec::new();
    for model in configured {
        // One the catalog knows keeps the catalog's name and levels.
        match catalog.iter().position(|m| m.id == model.id) {
            Some(i) => list.push(LocalModel { main: true, ..catalog.remove(i) }),
            None => list.push(model),
        }
    }
    if list.is_empty() {
        return (!catalog.is_empty()).then_some(catalog);
    }
    list.extend(catalog.into_iter().map(|m| LocalModel { main: m.main && !another_endpoint, ..m }));
    Some(list)
}

/// The models of `provider` on this computer, when it keeps a list.
#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
pub(crate) fn read_local(provider: &str) -> Option<Vec<LocalModel>> {
    let home = || std::env::var_os("HOME").map(std::path::PathBuf::from);
    match provider {
        "claude" => {
            let dir = std::env::var_os("CLAUDE_CONFIG_DIR").map(std::path::PathBuf::from).or_else(|| Some(home()?.join(".claude")))?;
            let (configured, another_endpoint) = claude_configured(&std::fs::read_to_string(dir.join("settings.json")).unwrap_or_default());
            let catalog = std::fs::read_dir(dir.join("cache").join("model-catalog"))
                .ok()
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with("-cc.json"))
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .filter_map(|text| {
                    let at = serde_json::from_str::<Value>(&text).ok()?.get("fetchedAt").and_then(Value::as_u64).unwrap_or(0);
                    Some((at, text))
                })
                .max_by_key(|(at, _)| *at)
                .and_then(|(_, text)| parse_claude(&text));
            with_configured(configured, catalog, another_endpoint)
        }
        "codex" => {
            let dir = std::env::var_os("CODEX_HOME").map(std::path::PathBuf::from).or_else(|| Some(home()?.join(".codex")))?;
            let config = std::fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
            let catalog = std::fs::read_to_string(codex_catalog_path(&config, &dir)).ok().and_then(|text| parse_codex(&text));
            with_configured(codex_configured(&config).into_iter().collect(), catalog, false)
        }
        _ => None,
    }
}

#[cfg(any(target_os = "ios", target_arch = "wasm32"))]
pub(crate) fn read_local(_provider: &str) -> Option<Vec<LocalModel>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_code_s_catalog_in_its_order_with_its_main_section_and_levels() {
        let text = r#"{"catalog":{"config":{"models":[
            {"id":"claude-opus-5-5","name":"Opus 5.5","section":"main","thinking":{"effort_options":[
                {"id":"low","name":"Low"},{"id":"medium","name":"Medium","badge":{"message":"Recommended"}},{"id":"xhigh","name":"Extra"}]}},
            {"id":"claude-fable-5-2","name":"Fable 5.2","section":"main"},
            {"id":"claude-opus-5","name":"Opus 5","section":"overflow"},
            {"name":"no id"}]}}}"#;
        let list = parse_claude(text).unwrap();
        assert_eq!(list.iter().map(|m| (m.id.as_str(), m.main)).collect::<Vec<_>>(), [("claude-opus-5-5", true), ("claude-fable-5-2", true), ("claude-opus-5", false)]);
        assert_eq!(list[0].thinking, [("low".into(), "Low".into()), ("medium".into(), "Medium".into()), ("xhigh".into(), "Extra".into())]);
        assert_eq!(list[0].default_thinking.as_deref(), Some("medium"));
        assert!(parse_claude("{}").is_none());
    }

    #[test]
    fn codex_s_shown_models_by_priority_with_their_levels() {
        let text = r#"{"models":[
            {"slug":"gpt-6-sol","display_name":"GPT-6-Sol","visibility":"list","priority":5,
             "default_reasoning_level":"medium","supported_reasoning_levels":[{"effort":"low"},{"effort":"xhigh"}]},
            {"slug":"codex-auto-review","display_name":"Codex Auto Review","visibility":"hide","priority":1},
            {"slug":"gpt-6.1-sol","display_name":"GPT-6.1-Sol","visibility":"list","priority":1}]}"#;
        let list = parse_codex(text).unwrap();
        assert_eq!(list.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["gpt-6.1-sol", "gpt-6-sol"]);
        assert_eq!(list[1].thinking, [("low".into(), "Low".into()), ("xhigh".into(), "Xhigh".into())]);
        assert_eq!(list[1].default_thinking.as_deref(), Some("medium"));
    }

    #[test]
    fn codex_reads_the_catalog_its_config_names() {
        let home = std::path::Path::new("/h/.codex");
        assert_eq!(codex_catalog_path("", home), home.join("models_cache.json"));
        assert_eq!(codex_catalog_path("model = \"x\"\nmodel_catalog_json = \"/a/b.json\"\n", home), std::path::PathBuf::from("/a/b.json"));
        assert_eq!(codex_catalog_path("model_catalog_json='cat.json'", home), home.join("cat.json"));
    }

    #[test]
    fn a_switched_claude_code_offers_the_models_its_settings_name() {
        let settings = r#"{"env":{"ANTHROPIC_BASE_URL":"https://example.test/anthropic","ANTHROPIC_AUTH_TOKEN":"k",
            "ANTHROPIC_MODEL":"glm-4.6","ANTHROPIC_DEFAULT_SONNET_MODEL":"glm-4.6","ANTHROPIC_DEFAULT_HAIKU_MODEL":"glm-4.5-air"}}"#;
        let (configured, another) = claude_configured(settings);
        assert!(another);
        assert_eq!(configured.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(), ["glm-4.6", "glm-4.5-air"]);
        let own = |id: &str| LocalModel { id: id.into(), name: id.into(), main: true, thinking: vec![("high".into(), "High".into())], default_thinking: None };
        let list = with_configured(configured, Some(vec![own("claude-opus")]), another).unwrap();
        assert_eq!(list.iter().map(|m| (m.id.as_str(), m.main)).collect::<Vec<_>>(), [("glm-4.6", true), ("glm-4.5-air", true), ("claude-opus", false)]);
        // Nothing named: the catalog as it is, and no list without one.
        assert_eq!(claude_configured(r#"{"model":"opus"}"#), (vec![], false));
        assert_eq!(with_configured(vec![], Some(vec![own("claude-opus")]), false), Some(vec![own("claude-opus")]));
        assert_eq!(with_configured(vec![], None, false), None);
        // With no catalog on disk, the named ones are the list.
        assert_eq!(with_configured(claude_configured(settings).0, None, true).map(|l| l.len()), Some(2));
    }

    #[test]
    fn codex_offers_the_model_its_config_starts_on() {
        let config = "model = \"deepseek-chat\" # switched\nmodel_provider = \"custom\"\nmodel_reasoning_effort = \"high\"\n\n[model_providers.custom]\nmodel = \"not-this\"\n";
        let m = codex_configured(config).unwrap();
        assert_eq!((m.id.as_str(), m.default_thinking.as_deref(), m.thinking.len()), ("deepseek-chat", Some("high"), 3));
        assert_eq!(codex_configured("model_provider = \"x\"\n[profiles.a]\nmodel = \"p\"\n"), None);
        // One the catalog lists keeps the catalog's name, and comes first.
        let listed = LocalModel { id: "deepseek-chat".into(), name: "DeepSeek".into(), main: false, thinking: vec![], default_thinking: None };
        let other = LocalModel { id: "gpt".into(), name: "GPT".into(), main: true, thinking: vec![], default_thinking: None };
        let list = with_configured(vec![m], Some(vec![other.clone(), listed]), false).unwrap();
        assert_eq!(list.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["DeepSeek", "GPT"]);
    }
}
