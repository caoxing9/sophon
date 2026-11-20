//! Integrations and plugins: what Sophon reaches, and what someone
//! else made of that, to install and to share.
//!
//! An **integration** connects one service and says nothing of what to
//! do with it: a sign-in (`setup`, `status`), what can be done there
//! (`use`, a command others call with `sophon use <id> <arguments>`),
//! and what it can watch for (`triggers`, each with a name a person
//! reads and the probe behind it, which nobody has to). It is a folder
//! with `.sophon-plugin/integration.json`, and the scripts that names.
//!
//! A **plugin** is what to do, packed to hand on: one automation or a
//! few, each a folder of its `automations` with an `automation.json`
//! (the trigger that starts it, an integration's, named
//! `integration/trigger`; the script that answers whoever caused a run;
//! what whoever sets it up may choose) beside `prompt.md`, what an
//! agent gets. With them it may bring skills, each a folder of its
//! `skills` with a `SKILL.md`: what the agents of its automations know
//! how to do, read when it is needed; and MCP servers, in `.mcp.json`,
//! which those agents can call. All three are as Claude Code and Codex
//! keep theirs, and none is listed in the manifest: being there is
//! enough. It names the integrations it counts on (`requires`), and may
//! bring some with it, each a folder under its `integrations`: there
//! while the plugin is, unless one of the same id is installed on its
//! own. It is a folder with `.sophon-plugin/plugin.json`.
//!
//! An **automation** is neither: it is the user's, kept by the
//! schedule. It names the trigger that starts it and, if it was made
//! from a plugin, the plugin's automation it runs by; the scripts are
//! read from both each time they run, so what was updated is what runs
//! next. The prompt is the automation's own, the plugin's to begin with.
//!
//! Either kind is installed from a git address (cloned) or from a
//! folder of this computer (linked, so what its author changes is what
//! runs), under `plugins/<id>` of the service's home. What one keeps (a
//! sign-in, where it read up to) is in `plugin-data/<id>`, which an
//! update leaves alone.
//!
//! Either may ask for parameters, and where one is asked says whose it
//! is. Those of the manifest itself are its own (an app's id and
//! secret): said once on this computer, over what the manifest gives,
//! so that whoever shares it fills in what is the same for everyone.
//! Those of a trigger, or of a plugin's automation, are each
//! automation's to say (which repositories, whether to approve), kept
//! with the automation. Each reaches the scripts as
//! `SOPHON_PARAM_<NAME>`, and a plugin's automation may put one in its
//! prompt: `{{name}}` (or `{{var}}`, when it gives itself another name
//! there) is its value, or the words its `prompt` gives for that value.
//!
//! A parameter has a kind, which is how a client asks for it: `text`
//! (the default), `switch` (on is `yes`, off `no`), or `multi`, some of
//! a list of choices, kept with commas between. The choices are in the
//! manifest (`options`), or said by a command when asked
//! (`optionsFrom`), a line a choice: the repositories of the account
//! signed in, say.
//!
//! A setup (a sign-in) is a command of its own, run when asked for
//! from a client: it has no terminal, so it opens what it needs to (a
//! browser) and ends when that is done. A `status` command says in a
//! line whose it is set up as, and fails while it is not.
//!
//! Some come with Sophon (a `plugins` folder beside the service's own
//! program, or the one `SOPHON_BUNDLED_PLUGINS` names): there without
//! being installed, and not to be removed. One installed under the same
//! id takes its place: a copy with the parameters filled in, to share.
//!
//! One can be turned off: it stays, with what it kept, and nothing of
//! it runs (its triggers start nothing, and nobody can use it) until
//! it is turned on again. And one can be removed, what it kept with
//! it: one installed goes from the disk, one that comes with Sophon is
//! put away, to be brought back. Which are off and which put away is
//! in `plugins.json` of the service's home.
//!
//! Its own settings can be handed on: `plugin/export` writes them as
//! one line of text (its id and each value, the secret ones too:
//! whoever asks is trusted with them), and `plugin/import` takes such
//! a line and sets them here, so that a colleague has the same app's
//! id and secret without typing either. All of one goes as a file
//! (`pack.rs`): the package, what it counts on, the settings, and an
//! automation set up with it.

use crate::host::{Shared, envelope};
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

/// What a folder is, by the manifest in it: in `.sophon-plugin`, beside
/// where Claude Code and Codex look for theirs (`.claude-plugin`,
/// `.codex-plugin`), so that one folder can be a plugin of all three.
pub const INTEGRATION: &str = ".sophon-plugin/integration.json";
pub const PLUGIN: &str = ".sophon-plugin/plugin.json";
/// Where a manifest was before, at the folder's top: said, when one is
/// found there, with where it goes.
const FORMER: [(&str, &str); 2] = [
    ("sophon-integration.json", INTEGRATION),
    ("sophon-plugin.json", PLUGIN),
];
/// What makes a folder of a plugin's `automations` one of them.
pub const AUTOMATION: &str = "automation.json";
/// The largest picture a card shows for one, read whole to be sent.
const LOGO_LIMIT: u64 = 256 * 1024;
/// How an integration's settings handed on begin, so that they are told
/// from an address to install from.
pub const EXPORTED: &str = "sophon-integration:";

fn root(home: &Path) -> PathBuf {
    home.join("plugins")
}

/// Where the integration or plugin `id` keeps what is its own.
pub fn data(home: &Path, id: &str) -> PathBuf {
    home.join("plugin-data").join(id)
}

/// A name that is a folder's and nothing else: no path in it.
pub(crate) fn named(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('.')
        && id
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '-' | '_' | '.'))
}

fn text(value: &Value, key: &str) -> Option<String> {
    value[key]
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// What `list` asks to be told, each as a client asks for it and the
/// scripts are told it. `file` is the manifest it is in, to say so.
fn params(list: &Value, file: &str) -> Result<Vec<Value>, String> {
    let mut params = Vec::new();
    for param in list.as_array().into_iter().flatten() {
        let name = text(param, "name").unwrap_or_default();
        if name.is_empty()
            || !name
                .chars()
                .all(|letter| letter.is_ascii_alphanumeric() || letter == '_')
        {
            return Err(format!(
                "{file}: a parameter's name may have letters, digits and _ only"
            ));
        }
        let value = match &param["value"] {
            Value::String(value) => value.clone(),
            Value::Null => String::new(),
            Value::Bool(on) => if *on { "yes" } else { "no" }.to_owned(),
            other => other.to_string(),
        };
        let kind = param["type"]
            .as_str()
            .filter(|kind| matches!(*kind, "switch" | "multi"))
            .unwrap_or("text");
        params.push(json!({"name":name,"label":text(param, "label").unwrap_or_else(|| name.clone()),
            "hint":text(param, "hint"),"type":kind,
            "options":param["options"].as_array().map(|options| options.iter().filter_map(choice).collect::<Vec<_>>()),
            "optionsFrom":text(param, "optionsFrom"),
            // What it is called in a prompt, when that is not its own name.
            "var":text(param, "var").filter(|var| var.chars().all(|letter| letter.is_ascii_alphanumeric() || letter == '_')),
            // What a value of it puts in a prompt, where the value itself would say little.
            "prompt":param["prompt"].as_object().map(|says| says.iter()
                .filter_map(|(value, words)| Some((value.clone(), json!(words.as_str()?))))
                .collect::<serde_json::Map<String, Value>>()),
            // A switch is never missing: off is an answer.
            "secret":param["secret"] == true,"optional":param["optional"] == true || kind == "switch","value":value}));
    }
    Ok(params)
}

/// What is in `dir`, as a client lists it and an automation reads it:
/// an integration or a plugin, by the manifest there, with its fields,
/// each prompt read from its file, and where it is. One that names no
/// id has `fallback` for it.
pub(crate) fn read(dir: &Path, fallback: &str) -> Result<Value, String> {
    let (file, kind) = if dir.join(INTEGRATION).is_file() {
        (INTEGRATION, "integration")
    } else {
        (PLUGIN, "plugin")
    };
    let bytes = fs::read(dir.join(file)).map_err(|_| {
        match FORMER.iter().find(|(former, _)| dir.join(former).is_file()) {
            Some((former, now)) => format!("{former} is where a manifest was: move it to {now}"),
            None => format!("No {INTEGRATION} or {PLUGIN} in this folder"),
        }
    })?;
    let manifest: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("{file}: {error}"))?;
    let id = text(&manifest, "id").unwrap_or_else(|| fallback.to_owned());
    if !named(&id) {
        return Err(format!(
            "{file}: the id may have letters, digits, - _ and . only"
        ));
    }
    // How it is shown, in Codex's words for the same (`interface`): what
    // it is called and a line about it, when the manifest says neither
    // itself, the colour it goes by, and its picture.
    let display = &manifest["interface"];
    let mut package = json!({
        "id": id,
        "kind": kind,
        "params": params(&manifest["params"], file)?,
        "name": text(&manifest, "name").or_else(|| text(display, "displayName")).unwrap_or_else(|| id.clone()),
        "description": text(&manifest, "description").or_else(|| text(display, "shortDescription")),
        "version": text(&manifest, "version"),
        "use": text(&manifest, "use"),
        "dir": dir.display().to_string(),
        "color": text(display, "brandColor").filter(|color| {
            color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|digit| digit.is_ascii_hexdigit())
        }),
        "logo": text(display, "logo").and_then(|logo| pictured(dir, &logo)),
    });
    if kind == "integration" {
        package["setup"] = json!(text(&manifest, "setup"));
        package["setupLabel"] = json!(text(&manifest, "setupLabel"));
        package["status"] = json!(text(&manifest, "status"));
        // What it watches for, by name.
        let mut triggers = Vec::new();
        for trigger in manifest["triggers"].as_array().into_iter().flatten() {
            let (Some(key), Some(probe)) = (
                text(trigger, "id").filter(|key| named(key)),
                text(trigger, "probe"),
            ) else {
                return Err(format!("{file}: a trigger needs an id and a probe"));
            };
            triggers.push(
                json!({"id":key,"label":text(trigger, "label").unwrap_or_else(|| key.clone()),
                "description":text(trigger, "description"),"probe":probe,
                // How whoever caused an event is answered, when the automation
                // it started has no way of its own.
                "reply":text(trigger, "reply"),
                "everyMs":trigger["everyMs"].as_u64().filter(|every| *every > 0),
                "params":params(&trigger["params"], file)?}),
            );
        }
        package["triggers"] = json!(triggers);
        return Ok(package);
    }
    // One written before the two were told apart: said, not half read.
    if manifest.get("triggers").is_some()
        || manifest.get("automation").is_some()
        || manifest.get("setup").is_some()
        || manifest.get("status").is_some()
    {
        return Err(format!(
            "{file}: a sign-in, a status and triggers are an integration's ({INTEGRATION}); a plugin's automations are the folders of its `automations`"
        ));
    }
    // And one that lists them in the manifest, as they were before each
    // had a folder.
    if manifest.get("automations").is_some() {
        return Err(format!(
            "{file}: each automation is a folder of `automations` now, with an {AUTOMATION} in it"
        ));
    }
    let mut requires: Vec<String> = manifest["requires"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|other| named(other))
        .map(str::to_owned)
        .collect();
    let automations = automations(dir, &package, &mut requires)?;
    package["requires"] = json!(requires);
    package["automations"] = json!(automations);
    package["skills"] = json!(skills(dir));
    package["mcp"] = json!(servers(dir));
    Ok(package)
}

/// The picture `logo` names in `dir`, if it is one a card can show: a
/// file of the folder itself (no way out of it), of a kind every client
/// draws, and small enough to be sent whole.
fn pictured(dir: &Path, logo: &str) -> Option<String> {
    let inside = Path::new(logo.trim_start_matches("./"));
    if !inside
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    let kind = inside.extension()?.to_str()?.to_ascii_lowercase();
    if !matches!(kind.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif") {
        return None;
    }
    let file = dir.join(inside);
    fs::metadata(&file)
        .ok()
        .filter(|found| found.is_file() && found.len() <= LOGO_LIMIT)
        .map(|_| file.display().to_string())
}

/// The automations a plugin brings: each a folder of its `automations`
/// with an `automation.json` in it (what starts it, how it answers, what
/// may be chosen) beside its prompt, `prompt.md` unless it names another
/// file there or says the words itself. As with skills, being there is
/// enough: the manifest lists none. The folder's name is the id. Its
/// commands run in the plugin's own folder, as all of a plugin's do.
fn automations(
    dir: &Path,
    plugin: &Value,
    requires: &mut Vec<String>,
) -> Result<Vec<Value>, String> {
    let mut folders: Vec<PathBuf> = fs::read_dir(dir.join("automations"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|folder| folder.join(AUTOMATION).is_file())
        .collect();
    folders.sort();
    let mut found = Vec::new();
    for folder in folders {
        let key = folder
            .file_name()
            .and_then(|key| key.to_str())
            .unwrap_or_default()
            .to_owned();
        let file = format!("automations/{key}/{AUTOMATION}");
        if !named(&key) {
            return Err(format!(
                "{file}: an automation's folder may have letters, digits, - _ and . only"
            ));
        }
        let automation: Value = fs::read(folder.join(AUTOMATION))
            .map_err(|error| error.to_string())
            .and_then(|bytes| serde_json::from_slice(&bytes).map_err(|error| error.to_string()))
            .map_err(|error| format!("{file}: {error}"))?;
        // A file of its folder, or the words themselves.
        let prompt = match text(&automation, "prompt") {
            Some(prompt) if !prompt.contains('\n') && folder.join(&prompt).is_file() => {
                fs::read_to_string(folder.join(&prompt)).unwrap_or(prompt)
            }
            Some(prompt) => prompt,
            None => fs::read_to_string(folder.join("prompt.md")).unwrap_or_default(),
        };
        // What starts it: an integration's trigger, `integration/trigger`.
        let trigger = match text(&automation, "trigger") {
            None => Value::Null,
            Some(named_as) => match named_as.split_once('/') {
                Some((integration, trigger)) if named(integration) && named(trigger) => {
                    if !requires.iter().any(|known| known == integration) {
                        requires.push(integration.to_owned());
                    }
                    json!({"integration":integration,"id":trigger})
                }
                _ => {
                    return Err(format!(
                        "{file}: a trigger is named integration/trigger, not {named_as}"
                    ));
                }
            },
        };
        found.push(json!({"id":key,
            // What an automation of it is called to begin with, and a line about it.
            "name":text(&automation, "name").or_else(|| text(plugin, "name")).unwrap_or_else(|| key.clone()),
            "description":text(&automation, "description").or_else(|| text(plugin, "description")),
            "category":text(&automation, "category"),
            "trigger":trigger,
            "prompt":prompt.trim(),
            "reply":text(&automation, "reply"),
            "modeId":text(&automation, "modeId"),
            "concurrency":automation["concurrency"].as_u64().filter(|count| *count > 0),
            "params":params(&automation["params"], &file)?}));
    }
    Ok(found)
}

/// The MCP servers a plugin brings, by name: `.mcp.json` of its folder,
/// as Claude Code and Codex read it (under `mcpServers`, or the file
/// itself a server a name).
fn servers(dir: &Path) -> Vec<String> {
    let Some(said) = fs::read(dir.join(".mcp.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return Vec::new();
    };
    let listed = if said["mcpServers"].is_object() {
        &said["mcpServers"]
    } else {
        &said
    };
    listed
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, server)| server.is_object())
        .map(|(name, _)| name.clone())
        .collect()
}

/// The skills a plugin brings: each a folder of its `skills` with a
/// `SKILL.md` in it, as Claude Code keeps them (what it is called and
/// when to use it are at the head of that file). They need no word in
/// the manifest: being there is enough.
fn skills(dir: &Path) -> Vec<Value> {
    let mut found: Vec<Value> = fs::read_dir(dir.join("skills"))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let key = entry
                .file_name()
                .to_str()
                .filter(|key| named(key))?
                .to_owned();
            let file = entry.path().join("SKILL.md");
            let words = fs::read_to_string(&file).ok()?;
            // Its head: the lines between the first two `---`.
            let head = words.strip_prefix("---")?.split_once("\n---")?.0;
            let said = |name: &str| {
                head.lines().find_map(|line| {
                    let value = line.strip_prefix(name)?.strip_prefix(':')?.trim();
                    let value = value.trim_matches(|quote| quote == '"' || quote == '\'');
                    (!value.is_empty()).then(|| value.to_owned())
                })
            };
            Some(
                json!({"id":key,"name":said("name").unwrap_or_else(|| key.clone()),
                "description":said("description"),"file":file.display().to_string()}),
            )
        })
        .collect();
    found.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    found
}

/// `from` and all in it, copied to `to`.
fn copy(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// What a plugin brings an agent (its skills, its MCP servers) as a
/// plugin of Claude Code's own, for the agent of an automation made from
/// it (`claude --plugin-dir`). One that is Claude Code's plugin already
/// (a `.claude-plugin/plugin.json` of its own) is handed over as it is.
/// For any other, a folder in what the plugin keeps, with the manifest
/// Claude Code asks for, the skills as they are now and its `.mcp.json`,
/// copied each time (the plugin's own folder is its author's, and what
/// was updated is what the next agent has); `${CLAUDE_PLUGIN_ROOT}` in
/// that file is the plugin's own folder, where what it names is. None
/// when it brings neither.
pub fn claude_plugin(home: &Path, plugin: &Value) -> Option<PathBuf> {
    let brings = |what: &str| plugin[what].as_array().is_some_and(|all| !all.is_empty());
    if !brings("skills") && !brings("mcp") {
        return None;
    }
    let id = plugin["id"].as_str()?;
    let dir = Path::new(plugin["dir"].as_str()?);
    if dir.join(".claude-plugin/plugin.json").is_file() {
        return Some(dir.to_owned());
    }
    let made = data(home, id).join("claude-plugin");
    secluded(&made.join(".claude-plugin")).ok()?;
    let manifest =
        json!({"name":id,"description":plugin["description"],"version":plugin["version"]});
    fs::write(
        made.join(".claude-plugin/plugin.json"),
        serde_json::to_vec_pretty(&manifest).ok()?,
    )
    .ok()?;
    clear(&made.join("skills")).ok()?;
    if brings("skills") {
        copy(&dir.join("skills"), &made.join("skills")).ok()?;
    }
    clear(&made.join(".mcp.json")).ok()?;
    if brings("mcp") {
        let servers = fs::read_to_string(dir.join(".mcp.json")).ok()?;
        fs::write(
            made.join(".mcp.json"),
            servers.replace("${CLAUDE_PLUGIN_ROOT}", &dir.display().to_string()),
        )
        .ok()?;
    }
    Some(made)
}

/// A choice of a parameter: its value, and the words shown for it. In
/// a manifest or from a command, a text is both; an object says each.
fn choice(option: &Value) -> Option<Value> {
    let value = match option {
        Value::String(value) => value.trim(),
        other => other["value"].as_str()?.trim(),
    };
    (!value.is_empty() && !value.contains(',')).then(|| {
        json!({"value":value,"label":option["label"].as_str().unwrap_or(value),"hint":option["hint"].as_str()})
    })
}

/// Where the ones that come with Sophon are, if any do: the folder
/// `SOPHON_BUNDLED_PLUGINS` names, else `plugins` beside this program
/// itself (as the app's bundle has it).
fn bundled() -> Option<PathBuf> {
    std::env::var_os("SOPHON_BUNDLED_PLUGINS")
        .map(PathBuf::from)
        .or_else(|| {
            let program = fs::canonicalize(std::env::current_exe().ok()?).ok()?;
            Some(program.parent()?.join("plugins"))
        })
        .filter(|folder| folder.is_dir())
}

/// Which are turned off, and which of those that come with Sophon are
/// put away: `{"off":[ids],"away":[ids]}`.
pub(crate) fn kept(home: &Path) -> Value {
    fs::read(home.join("plugins.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}))
}

pub(crate) fn among(kept: &Value, list: &str, id: &str) -> bool {
    kept[list]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|known| known == id))
}

/// Puts `id` on `list` or takes it off, and keeps that.
pub(crate) fn mark(home: &Path, list: &str, id: &str, on: bool) -> Result<(), String> {
    let mut all = kept(home);
    let mut ids: Vec<Value> = all[list].as_array().cloned().unwrap_or_default();
    ids.retain(|known| known != id);
    if on {
        ids.push(json!(id));
    }
    all[list] = json!(ids);
    crate::state::write(
        home,
        "plugins.json",
        &serde_json::to_vec(&all).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

/// Those that come with Sophon and were put away: to bring back.
fn away(home: &Path) -> Vec<Value> {
    let kept = kept(home);
    let installed = list(home);
    bundled()
        .and_then(|folder| fs::read_dir(folder).ok())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_owned();
            if !among(&kept, "away", &name)
                || installed.iter().any(|package| package["id"] == name.as_str())
            {
                return None;
            }
            let package = read(&entry.path(), &name).ok()?;
            Some(json!({"id":name,"kind":package["kind"],"name":package["name"],"description":package["description"]}))
        })
        .collect()
}

/// The integrations and plugins there are, by name. One whose manifest
/// cannot be read is listed with why, to be removed or mended.
pub fn list(home: &Path) -> Vec<Value> {
    let kept = kept(home);
    // Those installed, then those that come with Sophon: one installed
    // under an id takes the place of the one that comes with it.
    let mut packages: Vec<Value> = Vec::new();
    for (folder, builtin) in [(Some(root(home)), false), (bundled(), true)] {
        for entry in folder
            .and_then(|folder| fs::read_dir(folder).ok())
            .into_iter()
            .flatten()
            .flatten()
        {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !named(&name) || packages.iter().any(|known| known["id"] == name.as_str()) {
                continue;
            }
            // One that comes with Sophon and was put away is not there.
            if builtin && among(&kept, "away", &name) {
                continue;
            }
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let linked =
                fs::symlink_metadata(&path).is_ok_and(|kind| kind.file_type().is_symlink());
            let dir = fs::canonicalize(&path).unwrap_or(path);
            // One that cannot be read still says which it is, by its manifest.
            let mut package = read(&dir, &name).unwrap_or_else(|error| {
                json!({"id":name,"name":name,"error":error,"dir":dir.display().to_string(),
                    "kind":if dir.join(INTEGRATION).is_file() { "integration" } else { "plugin" }})
            });
            // It is found by its folder, whatever its manifest says now.
            package["id"] = json!(name);
            package["enabled"] = json!(!among(&kept, "off", &name));
            // Where it came from: with Sophon, a folder of this computer
            // (linked), a git address (a clone, which can be updated), or
            // a file someone handed on.
            package["source"] = json!(if builtin {
                "builtin"
            } else if linked {
                "folder"
            } else if dir.join(".git").exists() {
                "git"
            } else {
                "file"
            });
            packages.push(package);
        }
    }
    // The integrations a plugin brings: there while it is, on while it
    // is on. One of the same id there on its own stands in its place.
    let mut brought: Vec<Value> = Vec::new();
    for plugin in packages
        .iter()
        .filter(|package| package["kind"] == "plugin")
    {
        let folder = Path::new(plugin["dir"].as_str().unwrap_or_default()).join("integrations");
        for entry in fs::read_dir(folder).into_iter().flatten().flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let dir = entry.path();
            if !named(&name)
                || !dir.join(INTEGRATION).is_file()
                || packages
                    .iter()
                    .chain(&brought)
                    .any(|known| known["id"] == name.as_str())
            {
                continue;
            }
            let mut integration = read(&dir, &name).unwrap_or_else(
                |error| json!({"id":name,"name":name,"kind":"integration","error":error,"dir":dir.display().to_string()}),
            );
            integration["id"] = json!(name);
            integration["enabled"] =
                json!(plugin["enabled"] == true && !among(&kept, "off", &name));
            integration["source"] = json!("plugin");
            integration["from"] = plugin["id"].clone();
            brought.push(integration);
        }
    }
    packages.extend(brought);
    packages.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    packages
}

/// The one of `list` (triggers, automations) with the id `wanted`.
pub fn among_named<'a>(list: &'a Value, wanted: &str) -> Option<&'a Value> {
    list.as_array()
        .into_iter()
        .flatten()
        .find(|entry| entry["id"] == wanted)
}

pub fn find(home: &Path, id: &str) -> Option<Value> {
    list(home).into_iter().find(|plugin| plugin["id"] == id)
}

/// The values set on this computer for its own parameters.
pub(crate) fn own(home: &Path, id: &str) -> serde_json::Map<String, Value> {
    fs::read(data(home, id).join("params.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|values| values.as_object().cloned())
        .unwrap_or_default()
}

/// What was set on this computer for `from` under names that are
/// `to`'s own now, and `from`'s no longer, set for `to` where it has
/// nothing: settings follow an automation to the plugin it comes from.
pub fn inherit(home: &Path, from: &Value, to: &Value) {
    let asks = |package: &Value, name: &str| {
        package["params"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|param| param["name"] == name)
    };
    let (Some(old), Some(new)) = (from["id"].as_str(), to["id"].as_str()) else {
        return;
    };
    let mut values = own(home, new);
    let mut moved = false;
    for (name, value) in own(home, old) {
        if asks(to, &name) && !asks(from, &name) && !values.contains_key(&name) {
            values.insert(name, value);
            moved = true;
        }
    }
    if moved
        && secluded(&data(home, new)).is_ok()
        && let Ok(bytes) = serde_json::to_vec(&values)
    {
        let _ = crate::state::write(&data(home, new), "params.json", &bytes);
    }
}

/// What any script the service runs for an automation is told of the
/// service itself: where it is, and `sophon` on its `PATH`, by which
/// it calls an integration or a plugin (`sophon use`).
pub fn reach(home: &Path) -> Vec<(String, String)> {
    vec![
        ("SOPHON_SERVICE_HOME".to_owned(), home.display().to_string()),
        (
            "PATH".to_owned(),
            format!(
                "{}:{}",
                home.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        ),
    ]
}

/// What an integration's or a plugin's scripts are told besides what
/// they are asked: where it is, where it keeps what is its own, and
/// each of its own parameters, the value set here over the manifest's.
pub fn environment(home: &Path, plugin: &Value) -> Vec<(String, String)> {
    let id = plugin["id"].as_str().unwrap_or_default();
    let set = own(home, id);
    let mut env = reach(home);
    env.extend([
        (
            "SOPHON_PLUGIN_DIR".to_owned(),
            plugin["dir"].as_str().unwrap_or_default().to_owned(),
        ),
        (
            "SOPHON_PLUGIN_DATA".to_owned(),
            data(home, id).display().to_string(),
        ),
    ]);
    for param in plugin["params"].as_array().into_iter().flatten() {
        let name = param["name"].as_str().unwrap_or_default();
        let value = set
            .get(name)
            .and_then(Value::as_str)
            .or(param["value"].as_str())
            .unwrap_or_default();
        env.push((
            format!("SOPHON_PARAM_{}", name.to_ascii_uppercase()),
            value.to_owned(),
        ));
    }
    env
}

/// A parameter as a client asks for it: its command is the service's
/// to run, not a client's to read.
fn asked(param: &mut Value) {
    param["dynamic"] = json!(param["optionsFrom"].is_string());
    if let Some(fields) = param.as_object_mut() {
        fields.remove("optionsFrom");
        fields.remove("prompt");
        fields.remove("var");
    }
}

/// `plugin` as a client is shown it: a secret parameter is said to be
/// set or not, and never what it is.
fn shown(home: &Path, mut plugin: Value) -> Value {
    let set = own(home, plugin["id"].as_str().unwrap_or_default());
    // Its picture, whole: a client may be on another device, and has no
    // way to this folder.
    let icon = plugin["logo"].as_str().and_then(|file| {
        use base64::Engine;
        let kind = match Path::new(file).extension()?.to_str()? {
            "png" => "png",
            "webp" => "webp",
            "gif" => "gif",
            _ => "jpeg",
        };
        let bytes = fs::read(file).ok()?;
        Some(format!(
            "data:image/{kind};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    });
    plugin["icon"] = json!(icon);
    if let Some(fields) = plugin.as_object_mut() {
        fields.remove("logo");
    }
    for param in plugin["params"].as_array_mut().into_iter().flatten() {
        let name = param["name"].as_str().unwrap_or_default().to_owned();
        let changed = set.contains_key(&name);
        if let Some(value) = set.get(&name) {
            param["value"] = value.clone();
        }
        param["changed"] = json!(changed);
        asked(param);
        // One that may be left empty is never missing.
        param["filled"] = json!(
            param["optional"] == true
                || param["value"]
                    .as_str()
                    .is_some_and(|value| !value.is_empty())
        );
        if param["secret"] == true {
            param["value"] = Value::Null;
        }
    }
    // A trigger's probe is the service's to run, as a choice's command
    // is; so is how either answers. What each automation says for
    // itself is asked for with the automation.
    for list in ["triggers", "automations"] {
        for entry in plugin[list].as_array_mut().into_iter().flatten() {
            entry["replies"] = json!(entry["reply"].is_string());
            if let Some(fields) = entry.as_object_mut() {
                fields.remove("probe");
                fields.remove("reply");
            }
            for param in entry["params"].as_array_mut().into_iter().flatten() {
                asked(param);
            }
        }
    }
    plugin
}

async fn git(args: &[&str], cwd: &Path) -> Result<(), String> {
    let output = tokio::time::timeout(
        Duration::from_secs(120),
        tokio::process::Command::new("git")
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "git took longer than two minutes".to_owned())?
    .map_err(|error| format!("git: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr);
    Err(said
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("git failed")
        .trim()
        .to_owned())
}

/// Runs a command of an integration's or a plugin's in its folder, with what its scripts
/// are told, and answers what it wrote; why it failed, in its own last
/// words, when it did.
async fn run(
    home: &Path,
    plugin: &Value,
    command: &str,
    patience: Duration,
) -> Result<String, String> {
    let output = tokio::time::timeout(
        patience,
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(plugin["dir"].as_str().unwrap_or_default())
            .envs(environment(home, plugin))
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| format!("Took longer than {}s", patience.as_secs()))?
    .map_err(|error| error.to_string())?;
    let last = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or_default()
            .trim()
            .to_owned()
    };
    if output.status.success() {
        Ok(last(&output.stdout))
    } else {
        let said = last(&output.stderr);
        Err(if said.is_empty() {
            output.status.to_string()
        } else {
            said
        })
    }
}

/// What is at `path`, gone: a link taken away, a folder with all in it.
pub(crate) fn clear(path: &Path) -> Result<(), String> {
    let Ok(kind) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if kind.file_type().is_symlink() || kind.is_file() {
        fs::remove_file(path)
    } else {
        fs::remove_dir_all(path)
    }
    .map_err(|error| error.to_string())
}

pub(crate) fn secluded(path: &Path) -> Result<(), String> {
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(path).map_err(|error| error.to_string())
}

/// Installs the integration or plugin at `source`: a folder of this computer, linked;
/// anything else is a git address, cloned. One already installed under
/// its id is replaced; what it kept stays.
pub async fn add(home: &Path, source: &str) -> Result<Value, String> {
    let source = source.trim();
    if source.is_empty() || source.starts_with('-') {
        return Err("Give a git address, or a folder of this computer".into());
    }
    let plugins = root(home);
    secluded(&plugins)?;
    let local = match source.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map(|user| PathBuf::from(user).join(rest)),
        None => Some(PathBuf::from(source)),
    }
    .filter(|path| path.is_absolute() && path.is_dir());
    let id = if let Some(folder) = local {
        let folder = fs::canonicalize(folder).map_err(|error| error.to_string())?;
        let name = folder
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let plugin = read(&folder, name)?;
        let id = plugin["id"].as_str().unwrap_or_default().to_owned();
        let target = plugins.join(&id);
        clear(&target)?;
        #[cfg(unix)]
        std::os::unix::fs::symlink(&folder, &target).map_err(|error| error.to_string())?;
        #[cfg(not(unix))]
        return Err("A folder can be linked on macOS and Linux only".into());
        id
    } else {
        let incoming = plugins.join(format!(".incoming-{}", crate::state::id()));
        // Without an id of its own it has its repository's name.
        let name = source
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit(['/', ':'])
            .next()
            .unwrap_or_default();
        let cloned = git(
            &[
                "clone",
                "--depth",
                "1",
                "--",
                source,
                incoming.to_str().ok_or("Invalid service home")?,
            ],
            &plugins,
        )
        .await
        .and_then(|()| read(&incoming, name));
        let id = match cloned {
            Ok(plugin) => plugin["id"].as_str().unwrap_or_default().to_owned(),
            Err(error) => {
                let _ = clear(&incoming);
                return Err(error);
            }
        };
        let target = plugins.join(&id);
        clear(&target)?;
        fs::rename(&incoming, &target).map_err(|error| error.to_string())?;
        id
    };
    secluded(&data(home, &id))?;
    find(home, &id).ok_or_else(|| "It was not installed".into())
}

/// What a client asks of integrations and plugins, each named by its
/// id (`pluginId`) whichever it is: `plugin/list`, `plugin/add` (a
/// `source`), `plugin/update`, `plugin/remove` and the rest.
pub async fn handle(shared: &Shared, request: &Value) -> Result<Value, String> {
    let kind = request["type"].as_str().unwrap_or_default();
    let home = shared.borrow().home.clone();
    let id = request["pluginId"].as_str().unwrap_or_default();
    // What it offers, used: its own command with what was asked, run
    // as the plugin it is. Its answer is all there is to say back.
    if kind == "plugin/use" {
        let plugin = find(&home, id)
            .filter(|plugin| plugin["error"].is_null())
            .ok_or_else(|| format!("The plugin {id} is not installed"))?;
        if plugin["enabled"] == false {
            return Err(format!(
                "{} is turned off",
                plugin["name"].as_str().unwrap_or(id)
            ));
        }
        let offered = plugin["use"]
            .as_str()
            .ok_or_else(|| format!("The plugin {id} offers nothing to use"))?;
        let args: Vec<&str> = request["args"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let output = tokio::time::timeout(
            Duration::from_secs(60),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(format!("{offered} \"$@\""))
                .arg("sophon-plugin")
                .args(args)
                .current_dir(plugin["dir"].as_str().unwrap_or_default())
                .envs(environment(&home, &plugin))
                .stdin(Stdio::null())
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| "Took longer than 60s".to_owned())?
        .map_err(|error| error.to_string())?;
        return Ok(envelope(
            "plugin/use/response",
            json!({"requestId":request["requestId"],"code":output.status.code().unwrap_or(1),
                "output":String::from_utf8_lossy(&output.stdout),"errors":String::from_utf8_lossy(&output.stderr)}),
        ));
    }
    // The choices of a parameter, as its plugin says them now.
    if kind == "plugin/options" {
        let plugin = find(&home, id).ok_or("Unknown plugin")?;
        let name = request["name"].as_str().unwrap_or_default();
        // Its own, or one of a trigger's or an automation's when that is named.
        let asked_of = match (request["trigger"].as_str(), request["automation"].as_str()) {
            (Some(trigger), _) => {
                among_named(&plugin["triggers"], trigger).map(|found| &found["params"])
            }
            (_, Some(automation)) => {
                among_named(&plugin["automations"], automation).map(|found| &found["params"])
            }
            _ => Some(&plugin["params"]),
        };
        let param = asked_of
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .find(|param| param["name"] == name)
            .ok_or("Unknown parameter")?;
        let mut options: Vec<Value> = param["options"].as_array().cloned().unwrap_or_default();
        if let Some(command) = param["optionsFrom"].as_str() {
            let output = tokio::time::timeout(
                Duration::from_secs(30),
                tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .current_dir(plugin["dir"].as_str().unwrap_or_default())
                    .envs(environment(&home, &plugin))
                    .stdin(Stdio::null())
                    .kill_on_drop(true)
                    .output(),
            )
            .await
            .map_err(|_| "Took longer than 30s".to_owned())?
            .map_err(|error| error.to_string())?;
            if !output.status.success() {
                let said = String::from_utf8_lossy(&output.stderr);
                return Err(said
                    .lines()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("The plugin could not list them")
                    .trim()
                    .to_owned());
            }
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let line = line.trim();
                let said = serde_json::from_str::<Value>(line)
                    .ok()
                    .filter(Value::is_object)
                    .unwrap_or_else(|| json!(line));
                if let Some(option) = choice(&said)
                    && !options
                        .iter()
                        .any(|known| known["value"] == option["value"])
                {
                    options.push(option);
                }
            }
        }
        return Ok(envelope(
            "plugin/options/response",
            json!({"requestId":request["requestId"],"pluginId":id,"name":name,
                "trigger":request["trigger"],"automation":request["automation"],"options":options}),
        ));
    }
    let mut payload = match kind {
        "plugin/list" => json!({}),
        "plugin/add" => {
            let source = request["source"].as_str().ok_or("Missing source")?;
            json!({"plugin":shown(&home, add(&home, source).await?)})
        }
        // Its own settings as a line to hand on: the id, and each of the
        // integration's parameters as it stands here.
        "plugin/export" => {
            let plugin = find(&home, id).ok_or("Unknown plugin")?;
            let set = own(&home, id);
            let mut values = serde_json::Map::new();
            for param in plugin["params"].as_array().into_iter().flatten() {
                let name = param["name"].as_str().unwrap_or_default();
                let value = set
                    .get(name)
                    .and_then(Value::as_str)
                    .or(param["value"].as_str())
                    .unwrap_or_default();
                if !value.is_empty() {
                    values.insert(name.to_owned(), json!(value));
                }
            }
            if values.is_empty() {
                return Err("There is nothing set here to hand on".into());
            }
            use base64::Engine;
            let line = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(json!({"id":id,"params":values}).to_string());
            json!({"config":format!("{EXPORTED}{line}")})
        }
        // A line of settings handed on, taken in: found wherever it is in
        // what was pasted, a message it came in saying more around it.
        "plugin/import" => {
            use base64::Engine;
            let handed: Value = request["config"]
                .as_str()
                .and_then(|pasted| pasted.split_once(EXPORTED))
                .map(|(_, line)| {
                    line.chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                        .collect::<String>()
                })
                .and_then(|line| {
                    base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .decode(line)
                        .ok()
                })
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .ok_or("This is not an integration's settings")?;
            let wanted = handed["id"]
                .as_str()
                .filter(|wanted| named(wanted))
                .ok_or("This is not an integration's settings")?;
            // Its settings are why it is wanted: one that comes with
            // Sophon and was put away is brought back, one turned off is
            // on again.
            if among(&kept(&home), "away", wanted) {
                mark(&home, "away", wanted, false)?;
            }
            find(&home, wanted).ok_or_else(|| {
                format!("These settings are for {wanted}, which is not here: add it first")
            })?;
            mark(&home, "off", wanted, false)?;
            let plugin = find(&home, wanted).ok_or("Unknown plugin")?;
            let mut values = own(&home, wanted);
            for (name, value) in handed["params"].as_object().into_iter().flatten() {
                // Only what it asks for itself.
                let asked = plugin["params"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|param| param["name"] == name.as_str());
                if asked && let Some(value) = value.as_str().filter(|value| !value.is_empty()) {
                    values.insert(name.clone(), json!(value));
                }
            }
            secluded(&data(&home, wanted))?;
            crate::state::write(
                &data(&home, wanted),
                "params.json",
                &serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            json!({"plugin":shown(&home, plugin)})
        }
        // The same and more as a file: the package itself, those it
        // counts on, the settings of each, and an automation set up
        // with it (`pack.rs`).
        "plugin/pack" => {
            let schedule = request["scheduleId"]
                .as_str()
                .map(|schedule| {
                    shared
                        .borrow()
                        .state
                        .schedules
                        .get(schedule)
                        .cloned()
                        .ok_or("Unknown automation")
                })
                .transpose()?;
            crate::pack::pack(
                &home,
                id,
                crate::pack::Settings::said(&request["settings"]),
                schedule.as_ref(),
            )?
        }
        // Such a file read, to be shown before any of it is installed.
        "plugin/unpack" => {
            use base64::Engine;
            let bytes = request["archive"]
                .as_str()
                .filter(|archive| archive.len() <= crate::pack::LIMIT * 4 / 3 + 4)
                .and_then(|archive| {
                    base64::engine::general_purpose::STANDARD
                        .decode(archive)
                        .ok()
                })
                .ok_or("This is not a file Sophon made")?;
            crate::pack::unpack(&home, &bytes)?
        }
        // And installed, once whoever read what is in it said so.
        "plugin/install" => {
            json!({"installed":crate::pack::install(&home, request["token"].as_str().unwrap_or_default())?})
        }
        "plugin/discard" => {
            crate::pack::discard(&home, request["token"].as_str().unwrap_or_default())?;
            json!({})
        }
        "plugin/update" => {
            let plugin = find(&home, id).ok_or("Unknown plugin")?;
            // A linked folder is its author's to change.
            if plugin["source"] == "git" {
                git(
                    &["pull", "--ff-only"],
                    Path::new(plugin["dir"].as_str().unwrap_or_default()),
                )
                .await?;
            }
            json!({"plugin":find(&home, id).map(|plugin| shown(&home, plugin))})
        }
        // Its setup, for as long as a person takes to sign in.
        "plugin/setup" => {
            let plugin = find(&home, id).ok_or("Unknown plugin")?;
            let setup = plugin["setup"]
                .as_str()
                .ok_or("There is nothing to set up here")?;
            secluded(&data(&home, id))?;
            json!({"said":run(&home, &plugin, setup, Duration::from_secs(330)).await?})
        }
        // A parameter's value for this computer; an empty one goes back
        // to what the manifest gives.
        "plugin/set" => {
            let plugin = find(&home, id).ok_or("Unknown plugin")?;
            let mut values = own(&home, id);
            for (name, value) in request["values"].as_object().ok_or("Missing values")? {
                if !plugin["params"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|param| param["name"] == name.as_str())
                {
                    return Err(format!("{id} has no parameter {name} of its own"));
                }
                match value.as_str().filter(|value| !value.is_empty()) {
                    Some(value) => values.insert(name.clone(), json!(value)),
                    None => values.remove(name),
                };
            }
            secluded(&data(&home, id))?;
            crate::state::write(
                &data(&home, id),
                "params.json",
                &serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
            json!({"plugin":shown(&home, plugin)})
        }
        // Off, it stays as it is and nothing of it runs.
        "plugin/enable" => {
            find(&home, id).ok_or("Unknown plugin")?;
            mark(&home, "off", id, request["enabled"] == false)?;
            json!({})
        }
        // Gone, with what it kept: one installed from the disk, one that
        // comes with Sophon put away.
        "plugin/remove" => {
            let plugin = find(&home, id)
                .filter(|_| named(id))
                .ok_or("Unknown plugin")?;
            if plugin["source"] == "plugin" {
                return Err(format!(
                    "{} came with the plugin {}: it goes when that does",
                    plugin["name"].as_str().unwrap_or(id),
                    plugin["from"].as_str().unwrap_or_default()
                ));
            }
            // What a plugin brought goes with it.
            let with: Vec<String> = list(&home)
                .iter()
                .filter(|other| other["from"] == id)
                .filter_map(|other| other["id"].as_str().map(str::to_owned))
                .collect();
            let used = shared
                .borrow()
                .state
                .schedules
                .values()
                .filter(|schedule| {
                    std::iter::once(id)
                        .chain(with.iter().map(String::as_str))
                        .any(|gone| schedule["integration"] == gone || schedule["plugin"] == gone)
                })
                .count();
            if used > 0 {
                return Err(format!(
                    "{used} of the automations use this: delete them first"
                ));
            }
            if plugin["source"] == "builtin" {
                mark(&home, "away", id, true)?;
            } else {
                clear(&root(&home).join(id))?;
            }
            for gone in std::iter::once(id).chain(with.iter().map(String::as_str)) {
                mark(&home, "off", gone, false)?;
                clear(&data(&home, gone))?;
            }
            json!({})
        }
        // One that comes with Sophon, brought back.
        "plugin/restore" => {
            if !named(id) {
                return Err("Unknown plugin".into());
            }
            mark(&home, "away", id, false)?;
            find(&home, id).ok_or("Unknown plugin")?;
            json!({})
        }
        _ => return Err("Unknown plugin operation".into()),
    };
    // Each as it stands: whose it is set up as, and how many of the
    // schedules run it.
    let mut plugins = Vec::new();
    for plugin in list(&home) {
        let account = match plugin["status"].as_str() {
            Some(status) => run(&home, &plugin, status, Duration::from_secs(8))
                .await
                .ok(),
            None => None,
        };
        let id = plugin["id"].clone();
        let mut plugin = shown(&home, plugin);
        plugin["account"] = json!(account);
        let (mut uses, mut active) = (0, 0);
        for schedule in shared.borrow().state.schedules.values() {
            if schedule["integration"] == id || schedule["plugin"] == id {
                uses += 1;
                if schedule["status"] == "active" {
                    active += 1;
                }
            }
        }
        plugin["uses"] = json!(uses);
        plugin["active"] = json!(active);
        plugins.push(plugin);
    }
    // The integrations each plugin counts on, as they stand: there, and
    // ready to use.
    let ready: Vec<(String, String, bool)> = plugins
        .iter()
        .map(|plugin| {
            let filled = plugin["params"]
                .as_array()
                .into_iter()
                .flatten()
                .all(|param| param["filled"] == true);
            (
                plugin["id"].as_str().unwrap_or_default().to_owned(),
                plugin["name"].as_str().unwrap_or_default().to_owned(),
                plugin["error"].is_null()
                    && filled
                    && (plugin["setup"].is_null() || plugin["account"].is_string()),
            )
        })
        .collect();
    for plugin in plugins
        .iter_mut()
        .filter(|plugin| plugin["kind"] == "plugin")
    {
        let required: Vec<Value> = plugin["requires"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|other| match ready.iter().find(|(id, ..)| id == other) {
                Some((id, name, ready)) => {
                    json!({"id":id,"name":name,"installed":true,"ready":ready})
                }
                None => json!({"id":other,"name":other,"installed":false,"ready":false}),
            })
            .collect();
        plugin["requires"] = json!(required);
    }
    payload["plugins"] = json!(plugins);
    payload["away"] = json!(away(&home));
    payload["requestId"] = request["requestId"].clone();
    Ok(envelope(&format!("{kind}/response"), payload))
}

#[cfg(test)]
mod tests {
    use super::{AUTOMATION, INTEGRATION, PLUGIN, list, named, read};
    use std::{fs, path::Path};

    /// `words` as the file `inside` of `dir`, the folders on the way made.
    fn put(dir: &Path, inside: &str, words: &str) {
        let file = dir.join(inside);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, words).unwrap();
    }

    #[test]
    fn an_integration_and_a_plugin_are_told_apart_by_their_manifests() {
        let home = tempfile::tempdir().unwrap();
        // An integration: a sign-in, something to use, and what it watches for.
        let hub = home.path().join("plugins/hub");
        put(
            &hub,
            INTEGRATION,
            r##"{"id":"hub","name":"Hub","setup":"./login","status":"./whoami","use":"./do",
                "interface":{"brandColor":"#24292f","logo":"./logo.png"},
                "params":[{"name":"token","secret":true}],
                "triggers":[{"id":"asked","label":"Someone asks","probe":"./probe","everyMs":60000,
                    "params":[{"name":"repos","type":"multi","optionsFrom":"./repos"}]}]}"##,
        );
        let integration = read(&hub, "hub").unwrap();
        assert_eq!(integration["kind"], "integration");
        assert_eq!(integration["triggers"][0]["probe"], "./probe");
        assert_eq!(integration["triggers"][0]["everyMs"], 60000);
        assert_eq!(integration["triggers"][0]["params"][0]["name"], "repos");
        assert_eq!(integration["params"][0]["name"], "token");
        assert!(integration["automations"].is_null());
        // How it is shown: its colour, and its picture once that is there
        // (a client is sent it whole, and never where it is).
        assert_eq!(integration["color"], "#24292f");
        assert!(integration["logo"].is_null());
        put(&hub, "logo.png", "not much of a picture");
        let integration = read(&hub, "hub").unwrap();
        assert!(integration["logo"].as_str().unwrap().ends_with("logo.png"));
        let shown = super::shown(home.path(), integration.clone());
        assert!(
            shown["icon"]
                .as_str()
                .unwrap()
                .starts_with("data:image/png;base64,")
        );
        assert!(shown.get("logo").is_none());
        // A plugin: what to do, started by an integration's trigger. Each
        // automation is a folder of its `automations`, as each skill is one
        // of its `skills`: the manifest lists neither.
        let watch = home.path().join("plugins/watch");
        put(
            &watch,
            PLUGIN,
            r#"{"id":"watch","name":"Watch","requires":["chat"]}"#,
        );
        put(
            &watch,
            "automations/review/prompt.md",
            "Review {{url}}. {{approve}}\n",
        );
        put(
            &watch,
            &format!("automations/review/{AUTOMATION}"),
            r#"{"trigger":"hub/asked","reply":"./reply","concurrency":2,
                "params":[{"name":"approve","type":"switch","prompt":{"yes":"Approve it.","no":"Only comment."}}]}"#,
        );
        // A folder there without one is no automation.
        fs::create_dir_all(watch.join("automations/notes")).unwrap();
        let plugin = read(&watch, "watch").unwrap();
        assert_eq!(plugin["kind"], "plugin");
        assert_eq!(plugin["automations"].as_array().unwrap().len(), 1);
        let automation = &plugin["automations"][0];
        assert_eq!(automation["id"], "review", "its folder's name");
        assert_eq!(automation["prompt"], "Review {{url}}. {{approve}}");
        assert_eq!(
            automation["name"], "Watch",
            "its plugin's name, without one of its own"
        );
        assert_eq!(automation["trigger"]["integration"], "hub");
        assert_eq!(automation["trigger"]["id"], "asked");
        assert_eq!(automation["reply"], "./reply");
        assert_eq!(automation["params"][0]["prompt"]["yes"], "Approve it.");
        // It counts on what it names, and on whose trigger starts it.
        assert_eq!(plugin["requires"], serde_json::json!(["chat", "hub"]));
        assert_eq!(plugin["skills"], serde_json::json!([]));
        assert_eq!(plugin["mcp"], serde_json::json!([]));
        assert!(super::claude_plugin(home.path(), &plugin).is_none());
        // A skill is a folder of its `skills` with a SKILL.md: there, it is brought.
        let skill = watch.join("skills/pair-up");
        put(
            &skill,
            "SKILL.md",
            "---\nname: pair-up\ndescription: \"Pairs an account with a person. Use when nobody is known.\"\n---\n\nFind them.\n",
        );
        put(&skill, "scripts/find", "#!/bin/sh\n");
        fs::create_dir_all(watch.join("skills/not-one")).unwrap();
        let plugin = read(&watch, "watch").unwrap();
        assert_eq!(plugin["skills"].as_array().unwrap().len(), 1);
        assert_eq!(plugin["skills"][0]["id"], "pair-up");
        assert_eq!(
            plugin["skills"][0]["description"],
            "Pairs an account with a person. Use when nobody is known."
        );
        // For an agent of Claude Code's they are a plugin of its own kind,
        // made in what this plugin keeps and never in its own folder.
        let made = super::claude_plugin(home.path(), &plugin).unwrap();
        assert_eq!(made, home.path().join("plugin-data/watch/claude-plugin"));
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(made.join(".claude-plugin/plugin.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["name"], "watch");
        assert!(made.join("skills/pair-up/SKILL.md").is_file());
        assert!(made.join("skills/pair-up/scripts/find").is_file());
        assert!(!made.join(".mcp.json").exists());
        assert!(!watch.join(".claude-plugin").exists());
        // What was taken out of the plugin is gone from the next agent's.
        fs::remove_dir_all(skill.join("scripts")).unwrap();
        super::claude_plugin(home.path(), &read(&watch, "watch").unwrap()).unwrap();
        assert!(!made.join("skills/pair-up/scripts").exists());
        assert!(super::claude_plugin(home.path(), &integration).is_none());
        assert!(plugin["triggers"].is_null());
        // The MCP servers it names in `.mcp.json` go the same way, what
        // they run found in the plugin's own folder.
        put(
            &watch,
            ".mcp.json",
            r#"{"mcpServers":{"tracker":{"command":"${CLAUDE_PLUGIN_ROOT}/serve","args":[]}}}"#,
        );
        let plugin = read(&watch, "watch").unwrap();
        assert_eq!(plugin["mcp"], serde_json::json!(["tracker"]));
        super::claude_plugin(home.path(), &plugin).unwrap();
        let servers: serde_json::Value =
            serde_json::from_slice(&fs::read(made.join(".mcp.json")).unwrap()).unwrap();
        assert_eq!(
            servers["mcpServers"]["tracker"]["command"],
            format!("{}/serve", watch.display())
        );
        // One that is Claude Code's plugin already is handed over as it is.
        put(&watch, ".claude-plugin/plugin.json", r#"{"name":"watch"}"#);
        assert_eq!(
            super::claude_plugin(home.path(), &plugin).unwrap(),
            watch.as_path()
        );
        // One that cannot be read is listed with why.
        let broken = home.path().join("plugins/broken");
        put(&broken, PLUGIN, "{");
        let all = list(home.path());
        assert_eq!(all.len(), 3);
        assert!(
            all.iter()
                .any(|plugin| plugin["id"] == "broken" && plugin["error"].is_string())
        );
        assert!(
            all.iter()
                .any(|plugin| plugin["id"] == "watch" && plugin["error"].is_null())
        );
    }

    #[test]
    fn a_plugin_written_as_they_were_before_says_what_to_change() {
        let home = tempfile::tempdir().unwrap();
        let old = home.path().join("plugins/old");
        // Before the two were told apart.
        put(
            &old,
            PLUGIN,
            r#"{"id":"old","setup":"./login","automation":{"prompt":"Do it","probe":"./probe"}}"#,
        );
        let error = read(&old, "old").unwrap_err();
        assert!(error.contains(INTEGRATION), "{error}");
        // Before each automation had a folder.
        put(
            &old,
            PLUGIN,
            r#"{"id":"old","automations":[{"trigger":"hub/asked","prompt":"Do it"}]}"#,
        );
        let error = read(&old, "old").unwrap_err();
        assert!(error.contains(AUTOMATION), "{error}");
        // A trigger is named with its integration.
        put(&old, PLUGIN, r#"{"id":"old"}"#);
        put(
            &old,
            &format!("automations/do/{AUTOMATION}"),
            r#"{"trigger":"asked","prompt":"Do it"}"#,
        );
        let error = read(&old, "old").unwrap_err();
        assert!(error.contains("integration/trigger"), "{error}");
        assert!(error.contains("automations/do/"), "{error}");
        // And before the manifests had a folder of their own.
        let older = home.path().join("plugins/older");
        put(&older, "sophon-plugin.json", r#"{"id":"older"}"#);
        let error = read(&older, "older").unwrap_err();
        assert!(
            error.contains("sophon-plugin.json") && error.contains(PLUGIN),
            "{error}"
        );
    }

    #[test]
    fn a_plugin_brings_integrations_unless_one_of_the_id_is_there_on_its_own() {
        let home = tempfile::tempdir().unwrap();
        let pack = home.path().join("plugins/pack");
        for brought in ["mail", "chat"] {
            put(
                &pack.join("integrations").join(brought),
                INTEGRATION,
                &format!(r#"{{"name":"{brought} of the pack","use":"./send"}}"#),
            );
        }
        put(&pack, PLUGIN, r#"{"id":"pack"}"#);
        put(
            &pack,
            &format!("automations/do/{AUTOMATION}"),
            r#"{"prompt":"Do it"}"#,
        );
        let own = home.path().join("plugins/chat");
        put(&own, INTEGRATION, r#"{"name":"Chat of my own"}"#);
        let all = list(home.path());
        let named_as = |id: &str| {
            all.iter()
                .find(|package| package["id"] == id)
                .unwrap_or_else(|| panic!("no {id}"))
        };
        assert_eq!(all.len(), 3);
        assert_eq!(named_as("mail")["source"], "plugin");
        assert_eq!(named_as("mail")["from"], "pack");
        assert_eq!(named_as("mail")["kind"], "integration");
        assert_eq!(named_as("chat")["name"], "Chat of my own");
        assert!(named_as("chat")["from"].is_null());
        // Off with its plugin.
        fs::write(home.path().join("plugins.json"), r#"{"off":["pack"]}"#).unwrap();
        let all = list(home.path());
        let mail = all.iter().find(|package| package["id"] == "mail").unwrap();
        assert_eq!(mail["enabled"], false);
    }

    /// The ones in this repository's `plugins`, as they are shipped.
    #[test]
    fn what_comes_with_sophon_is_read_and_every_plugin_s_trigger_is_there() {
        let shipped = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
        let mut all = Vec::new();
        for entry in fs::read_dir(&shipped).unwrap().flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_str().unwrap().to_owned();
                all.push(
                    read(&entry.path(), &name).unwrap_or_else(|error| panic!("{name}: {error}")),
                );
            }
        }
        let integration = |id: &str| {
            all.iter()
                .find(|package| package["kind"] == "integration" && package["id"] == id)
        };
        assert!(integration("github").is_some() && integration("feishu").is_some());
        let plugins: Vec<_> = all
            .iter()
            .filter(|package| package["kind"] == "plugin")
            .collect();
        assert!(!plugins.is_empty());
        for plugin in plugins {
            for needed in plugin["requires"].as_array().unwrap() {
                assert!(integration(needed.as_str().unwrap()).is_some(), "{plugin}");
            }
            for automation in plugin["automations"].as_array().unwrap() {
                assert!(!automation["prompt"].as_str().unwrap().is_empty());
                let trigger = &automation["trigger"];
                let from = integration(trigger["integration"].as_str().unwrap()).unwrap();
                assert!(
                    super::among_named(&from["triggers"], trigger["id"].as_str().unwrap())
                        .is_some(),
                    "{trigger}"
                );
                // Each choice a prompt names is one the automation asks for.
                for param in automation["params"].as_array().unwrap() {
                    if param["prompt"].is_object() {
                        let name = format!(
                            "{{{{{}}}}}",
                            param["var"].as_str().or(param["name"].as_str()).unwrap()
                        );
                        assert!(
                            automation["prompt"].as_str().unwrap().contains(&name),
                            "{name}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn an_id_is_a_folder_s_name_and_no_path() {
        for id in ["feishu-pr-watch", "a.b_c", "X1"] {
            assert!(named(id), "{id}");
        }
        for id in ["", "..", ".hidden", "a/b", "a b", "../x"] {
            assert!(!named(id), "{id}");
        }
    }
}
