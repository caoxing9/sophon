//! The models each agent offers, and the one it starts on, read from
//! what its CLI keeps on this host:
//!
//! - Claude Code: its catalog cache (`<config>/cache/model-catalog/*-cc.json`,
//!   the list `/model` shows) and its `settings.json` (`model`,
//!   `effortLevel`: what it starts on when told nothing).
//! - Codex: the catalog its `config.toml` names (`model_catalog_json`,
//!   a gateway's), else its own cache (`models_cache.json`), and that
//!   config's `model` and `model_reasoning_effort`.
//!
//! So a client's picker shows what the CLI's own does, and a new
//! conversation names the model and effort the CLI would have used
//! anyway. A client on this host reads the same files for itself; one
//! on a phone has only this list.

use serde_json::{Value, json};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn home() -> PathBuf {
    env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// Where `family`'s CLI keeps its files, as the provider's settings say
/// (their `env`), else the service's environment, else its usual place.
pub(crate) fn kept(settings: &Value, variable: &str, usual: &str) -> PathBuf {
    settings
        .pointer(&format!("/env/{variable}"))
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| env::var_os(variable).map(PathBuf::from))
        .unwrap_or_else(|| home().join(usual))
}

/// `xhigh` as `Xhigh`: a level's label where the catalog gives none.
fn title(id: &str) -> String {
    let mut label = id.to_owned();
    if let Some(first) = label.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    label
}

/// Claude Code's models from its catalog, with `settings.json`'s model
/// (a name the catalog stands a model for, or a model's own id) as the
/// default and its effort as that model's.
fn claude(catalog: &Value, settings: &Value) -> Vec<Value> {
    let config = &catalog["catalog"]["config"];
    let chosen = settings["model"]
        .as_str()
        .map(|model| model.trim_end_matches("[1m]"));
    let chosen = chosen.map(|model| {
        config["provider_alias_targets"][model]["default"]
            .as_str()
            .unwrap_or(model)
    });
    let effort = settings["effortLevel"].as_str();
    let mut models: Vec<Value> = config["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let id = model["id"].as_str()?;
            let options = model
                .pointer("/thinking/effort_options")
                .and_then(Value::as_array);
            let levels: Vec<&str> = options
                .into_iter()
                .flatten()
                .filter_map(|option| option["id"].as_str())
                .collect();
            // The host's own choice where this model takes it, else
            // the one Claude Code recommends.
            let level = effort.filter(|effort| levels.contains(effort)).or_else(|| {
                options
                    .into_iter()
                    .flatten()
                    .find(|option| option.pointer("/badge/message") == Some(&json!("Recommended")))
                    .and_then(|option| option["id"].as_str())
            });
            let options: Vec<Value> = options
                .into_iter()
                .flatten()
                .filter_map(|option| {
                    Some(json!({"id":option["id"].as_str()?,"label":option["name"]}))
                })
                .collect();
            Some(
                json!({"id":id,"label":model["name"],"description":model["description"],
                "isDefault":chosen == Some(id),"isSelectable":true,
                "thinkingOptions":options,"defaultThinkingOptionId":level}),
            )
        })
        .collect();
    // Nothing chosen (or something the catalog lacks): the first of the
    // list is what a picker would show first.
    if !models.iter().any(|model| model["isDefault"] == true)
        && let Some(first) = models.first_mut()
    {
        first["isDefault"] = json!(true);
    }
    models
}

/// The value of a top-level `key` of a `config.toml` (those before its
/// first table).
fn top(config: &str, key: &str) -> Option<String> {
    config
        .lines()
        .map(str::trim)
        .take_while(|line| !line.starts_with('['))
        .find_map(|line| {
            let rest = line
                .strip_prefix(key)?
                .trim_start()
                .strip_prefix('=')?
                .trim();
            let value = rest
                .split('#')
                .next()?
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            (!value.is_empty()).then(|| value.to_owned())
        })
}

/// Codex's models from its catalog, those it shows, in its order; the
/// one its config starts on is the default, with that config's effort.
fn codex(catalog: &Value, config: &str) -> Vec<Value> {
    let chosen = top(config, "model");
    let effort = top(config, "model_reasoning_effort");
    let mut shown: Vec<(i64, Value)> = catalog["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|model| {
            model["visibility"]
                .as_str()
                .is_none_or(|shown| shown == "list")
        })
        .filter_map(|model| {
            let id = model["slug"].as_str()?;
            let levels: Vec<&str> = model["supported_reasoning_levels"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|level| level["effort"].as_str())
                .collect();
            let starts = chosen.as_deref() == Some(id);
            let level = effort
                .as_deref()
                .filter(|effort| starts && levels.contains(effort))
                .or(model["default_reasoning_level"].as_str());
            let options: Vec<Value> = levels
                .iter()
                .map(|level| json!({"id":level,"label":title(level)}))
                .collect();
            Some((
                model["priority"].as_i64().unwrap_or(i64::MAX),
                json!({"id":id,"label":model["display_name"].as_str().unwrap_or(id),
                    "description":model["description"],"isDefault":starts,"isSelectable":true,
                    "thinkingOptions":options,"defaultThinkingOptionId":level}),
            ))
        })
        .collect();
    shown.sort_by_key(|(priority, _)| *priority);
    let mut models: Vec<Value> = shown.into_iter().map(|(_, model)| model).collect();
    // A model its config names and its catalog does not is still what
    // it starts on.
    if let Some(id) = chosen.filter(|_| !models.iter().any(|model| model["isDefault"] == true)) {
        let options: Vec<Value> = ["low", "medium", "high"]
            .iter()
            .map(|level| json!({"id":level,"label":title(level)}))
            .collect();
        models.insert(
            0,
            json!({"id":id,"label":id,"isDefault":true,"isSelectable":true,
                "thinkingOptions":options,"defaultThinkingOptionId":effort}),
        );
    }
    if !models.iter().any(|model| model["isDefault"] == true)
        && let Some(first) = models.first_mut()
    {
        first["isDefault"] = json!(true);
    }
    models
}

/// The catalog Codex reads: the one its config names (from its home
/// when not a whole path), else its own cache.
fn codex_catalog(config: &str, home_dir: &Path) -> PathBuf {
    match top(config, "model_catalog_json") {
        Some(path) if path.starts_with('/') => path.into(),
        Some(path) if path.starts_with("~/") => home().join(&path[2..]),
        Some(path) => home_dir.join(path),
        None => home_dir.join("models_cache.json"),
    }
}

fn read(path: PathBuf) -> Value {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null)
}

/// The models of a provider of `family`, as its CLI lists them on this
/// host: none if the CLI keeps no list here yet.
pub fn list(family: &str, settings: &Value) -> Vec<Value> {
    if family == "gemini" {
        return gemini(&read(
            kept(settings, "GEMINI_CLI_HOME", ".gemini").join("settings.json"),
        ));
    }
    if family == "codex" {
        let dir = kept(settings, "CODEX_HOME", ".codex");
        let config = fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
        return codex(&read(codex_catalog(&config, &dir)), &config);
    }
    let dir = kept(settings, "CLAUDE_CONFIG_DIR", ".claude");
    // The catalog fetched last, of those kept.
    let catalog = fs::read_dir(dir.join("cache").join("model-catalog"))
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with("-cc.json"))
        .map(|entry| read(entry.path()))
        .max_by_key(|catalog| catalog["fetchedAt"].as_u64().unwrap_or(0))
        .unwrap_or(Value::Null);
    claude(&catalog, &read(dir.join("settings.json")))
}

/// Gemini CLI's models, as its sessions offer them (it lists them only to
/// a session it has opened): "Auto" lets it choose. The one its settings
/// name (`model.name`) is the default, and is listed if it is not here.
fn gemini(settings: &Value) -> Vec<Value> {
    let named = settings
        .pointer("/model/name")
        .or_else(|| settings.get("model").filter(|model| model.is_string()))
        .and_then(Value::as_str)
        .filter(|name| !name.is_empty());
    let mut models: Vec<(String, String)> = [
        ("auto", "Auto"),
        ("gemini-3.1-pro-preview", "Gemini 3.1 Pro"),
        ("gemini-3.8-flash", "Gemini 3.8 Flash"),
        ("gemini-3-flash-preview", "Gemini 3 Flash"),
        ("gemini-2.5-pro", "Gemini 2.5 Pro"),
        ("gemini-3.5-flash-lite", "Gemini 3.5 Flash-Lite"),
    ]
    .into_iter()
    .map(|(id, label)| (id.to_owned(), label.to_owned()))
    .collect();
    if let Some(named) = named
        && !models.iter().any(|(id, _)| id == named)
    {
        models.insert(1, (named.to_owned(), named.to_owned()));
    }
    let default = named.unwrap_or("auto");
    models
        .into_iter()
        .map(|(id, label)| json!({"id":id,"label":label,"isDefault":id == default,"thinkingOptions":[]}))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(models: &[Value], key: &str) -> Vec<Value> {
        models.iter().map(|model| model[key].clone()).collect()
    }

    #[test]
    fn gemini_starts_on_the_model_its_settings_name() {
        let models = gemini(&json!({"model":{"name":"gemini-2.5-pro"}}));
        let default: Vec<_> = models
            .iter()
            .filter(|m| m["isDefault"] == true)
            .map(|m| m["id"].clone())
            .collect();
        assert_eq!(default, [json!("gemini-2.5-pro")]);
        // One it does not know is listed, second after Auto.
        let named = gemini(&json!({"model":{"name":"gemini-9-ultra"}}));
        assert_eq!(named[1]["id"], "gemini-9-ultra");
        // None named: Auto.
        assert_eq!(gemini(&Value::Null)[0]["isDefault"], true);
    }

    #[test]
    fn claude_code_starts_on_what_its_settings_name() {
        let catalog = json!({"catalog":{"config":{
            "provider_alias_targets":{"fable":{"default":"claude-fable-5-2"}},
            "models":[
                {"id":"claude-opus-5-5","name":"Opus 5.5","thinking":{"effort_options":[
                    {"id":"low","name":"Low"},{"id":"medium","name":"Medium","badge":{"message":"Recommended"}}]}},
                {"id":"claude-fable-5-2","name":"Fable 5.2","thinking":{"effort_options":[
                    {"id":"medium","name":"Medium","badge":{"message":"Recommended"}},{"id":"high","name":"High"}]}},
                {"id":"claude-haiku-4-5","name":"Haiku 4.5","thinking":{}}]}}});
        let models = claude(&catalog, &json!({"model":"fable","effortLevel":"high"}));
        assert_eq!(ids(&models, "isDefault"), [false, true, false]);
        // Its effort where the model takes it, the recommended one where not.
        assert_eq!(
            ids(&models, "defaultThinkingOptionId"),
            [json!("medium"), json!("high"), Value::Null]
        );
        assert_eq!(models[1]["label"], "Fable 5.2");
        assert_eq!(
            models[0]["thinkingOptions"][1],
            json!({"id":"medium","label":"Medium"})
        );
        // Told nothing, or a model the catalog lacks: the first.
        assert_eq!(
            ids(&claude(&catalog, &Value::Null), "isDefault"),
            [true, false, false]
        );
        assert_eq!(
            ids(
                &claude(&catalog, &json!({"model":"claude-opus-5-5[1m]"})),
                "isDefault"
            ),
            [true, false, false]
        );
        assert!(claude(&Value::Null, &Value::Null).is_empty());
    }

    #[test]
    fn codex_starts_on_what_its_config_names() {
        let catalog = json!({"models":[
            {"slug":"b","display_name":"B","priority":2,"default_reasoning_level":"medium",
                "supported_reasoning_levels":[{"effort":"low"},{"effort":"medium"},{"effort":"xhigh"}]},
            {"slug":"hidden","visibility":"hide","priority":0},
            {"slug":"a","priority":1,"default_reasoning_level":"low","supported_reasoning_levels":[{"effort":"low"}]}]});
        let config = "model = \"b\" # chosen\nmodel_reasoning_effort = \"xhigh\"\n[profiles.x]\nmodel = \"other\"\n";
        let models = codex(&catalog, config);
        assert_eq!(ids(&models, "id"), ["a", "b"]);
        assert_eq!(ids(&models, "isDefault"), [false, true]);
        assert_eq!(ids(&models, "defaultThinkingOptionId"), ["low", "xhigh"]);
        assert_eq!(
            models[1]["thinkingOptions"][2],
            json!({"id":"xhigh","label":"Xhigh"})
        );
        // One its catalog lacks is listed first, as what it starts on.
        let elsewhere = codex(&catalog, "model = \"deepseek\"\n");
        assert_eq!(
            (
                elsewhere[0]["id"].clone(),
                elsewhere[0]["isDefault"].clone()
            ),
            (json!("deepseek"), json!(true))
        );
        assert_eq!(ids(&codex(&catalog, ""), "isDefault"), [true, false]);
        assert_eq!(
            codex_catalog("model_catalog_json = \"mine.json\"", Path::new("/h")),
            Path::new("/h/mine.json")
        );
        assert_eq!(
            codex_catalog("", Path::new("/h")),
            Path::new("/h/models_cache.json")
        );
    }
}
