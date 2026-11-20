//! An integration or a plugin handed on as a file.
//!
//! Sophon has no server to install from and no site to sign in at, so
//! what one computer has set up reaches another as a file, a `.sophon`:
//! a gzipped tar of
//!
//! - `sophon.json`: what is in it (its format, the package it is about,
//!   every package that one counts on, the settings of each, and one
//!   automation set up with it, if that was asked for);
//! - `packages/<id>/…`: the folder of each package that does not come
//!   with Sophon. One that does is named and not packed: whoever takes
//!   the file in has it already.
//!
//! The settings are each package's own parameters as they stand here (an
//! app's id and secret), all of them, only the ones that are no secret,
//! or none, as whoever packs it says. What a package keeps for one
//! person (a sign-in, where it read up to) is never in it: whoever takes
//! it in signs in for themselves.
//!
//! Taking one in is two steps, because a package runs its scripts on the
//! computer it is on. `unpack` reads the file into a folder of its own
//! and answers what is in it, for a person to read; nothing of it is
//! where it would run. `install` then puts each package in its place,
//! sets the settings and turns them on, and answers the automation that
//! came with it, for a client to open filled in: which project it runs
//! in is this computer's to say.
//!
//! A file is someone else's words: every path in it is checked to stay
//! under `packages/<id>/` (no `..`, nothing absolute, no links), and
//! what it unpacks to is counted and capped.

use crate::plugin;
use serde_json::{Value, json};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime},
};

/// The version of what is written in `sophon.json`.
const FORMAT: u64 = 1;
const ABOUT: &str = "sophon.json";
/// The largest file made or taken in: it is carried whole in a message.
pub const LIMIT: usize = 8 * 1024 * 1024;
/// The most a file may unpack to, and the most files in it.
const UNPACKED: u64 = 64 * 1024 * 1024;
const FILES: usize = 4000;
/// What a package's folder has that is not the package.
const LEFT_OUT: [&str; 4] = [".git", "__pycache__", "node_modules", ".DS_Store"];
/// How a folder a file was read into is named, under `plugins`: not a
/// package's name, so it is never listed as one.
const READ_INTO: &str = ".handed-";

/// Which of the settings go in the file.
#[derive(Clone, Copy, PartialEq)]
pub enum Settings {
    All,
    /// All but the secrets.
    Plain,
    None,
}

impl Settings {
    pub fn said(value: &Value) -> Self {
        match value.as_str() {
            Some("none") => Self::None,
            Some("plain") => Self::Plain,
            _ => Self::All,
        }
    }
}

/// Every file under `dir`, with its path from there: what is left out
/// and links are passed over (a link's end is not the package's).
fn files(dir: &Path, under: &Path, found: &mut Vec<(PathBuf, PathBuf)>) -> Result<(), String> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|error| format!("{}: {error}", dir.display()))?
        .flatten()
        .collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name();
        if LEFT_OUT.iter().any(|left| name == *left) {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let inside = under.join(&name);
        if kind.is_dir() {
            files(&entry.path(), &inside, found)?;
        } else if kind.is_file() {
            found.push((entry.path(), inside));
        }
    }
    Ok(())
}

/// The packages a file about `root` is about: itself, then each
/// integration it counts on that is here and can be read.
fn about(home: &Path, root: &Value) -> Vec<Value> {
    let mut all = vec![root.clone()];
    for needed in root["requires"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if all.iter().any(|known| known["id"] == needed) {
            continue;
        }
        if let Some(package) = plugin::find(home, needed).filter(|found| found["error"].is_null()) {
            all.push(package);
        }
    }
    all
}

/// The automation `schedule` as it is handed on: what it is called, what
/// starts it and what it does, with what was chosen for each. Where it
/// runs and on which model are the computer's it is on, and stay there.
fn handed(schedule: &Value) -> Value {
    let mut automation = serde_json::Map::new();
    for field in [
        "name",
        "prompt",
        "command",
        "integration",
        "trigger",
        "triggerOptions",
        "plugin",
        "template",
        "options",
        "dailyLimit",
    ] {
        let value = &schedule[field];
        if !value.is_null() && value != "" {
            automation.insert(field.to_owned(), value.clone());
        }
    }
    // When it runs, without the time zone of whoever made it.
    let cadence = &schedule["cadence"];
    match cadence["type"].as_str() {
        Some("cron") => automation.insert(
            "cadence".to_owned(),
            json!({"type":"cron","expression":cadence["expression"]}),
        ),
        Some("every") => automation.insert(
            "cadence".to_owned(),
            json!({"type":"every","everyMs":cadence["everyMs"]}),
        ),
        _ => None,
    };
    Value::Object(automation)
}

/// The file about the package `id`: its bytes, what it is to be called,
/// and what is in it. `schedule` is an automation set up with it, to go
/// along.
pub fn pack(
    home: &Path,
    id: &str,
    settings: Settings,
    schedule: Option<&Value>,
) -> Result<Value, String> {
    let root = plugin::find(home, id)
        .filter(|found| found["error"].is_null())
        .ok_or("Unknown plugin")?;
    if let Some(schedule) = schedule
        && schedule["integration"] != id
        && schedule["plugin"] != id
    {
        return Err("That automation was not made with this".into());
    }
    let packages = about(home, &root);
    let mut listed = Vec::new();
    let mut values = serde_json::Map::new();
    let mut packed: Vec<(PathBuf, PathBuf)> = Vec::new();
    for package in &packages {
        let package_id = package["id"].as_str().unwrap_or_default();
        // One that comes with Sophon is there already; one a plugin
        // brought is in that plugin's folder, and goes with it.
        let rides = package["source"] == "plugin"
            && packages
                .iter()
                .any(|other| other["id"] == package["from"] && other["source"] != "builtin");
        let own_folder = matches!(package["source"].as_str(), Some("git" | "folder" | "file"));
        if own_folder {
            files(
                Path::new(package["dir"].as_str().unwrap_or_default()),
                &Path::new("packages").join(package_id),
                &mut packed,
            )?;
        }
        listed.push(
            json!({"id":package_id,"kind":package["kind"],"name":package["name"],
            "version":package["version"],"included":own_folder || rides}),
        );
        if settings == Settings::None {
            continue;
        }
        let set = plugin::own(home, package_id);
        let mut said = serde_json::Map::new();
        for param in package["params"].as_array().into_iter().flatten() {
            let name = param["name"].as_str().unwrap_or_default();
            if settings == Settings::Plain && param["secret"] == true {
                continue;
            }
            if let Some(value) = set
                .get(name)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            {
                said.insert(name.to_owned(), json!(value));
            }
        }
        if !said.is_empty() {
            values.insert(package_id.to_owned(), Value::Object(said));
        }
    }
    let about = json!({"format":FORMAT,"sophon":env!("CARGO_PKG_VERSION"),"root":id,
        "packages":listed,"settings":values,"automation":schedule.map(handed)});

    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut put = |inside: &Path, bytes: &[u8], runs: bool| -> Result<(), String> {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(if runs { 0o755 } else { 0o644 });
        header.set_mtime(0);
        archive
            .append_data(&mut header, inside, bytes)
            .map_err(|error| format!("{}: {error}", inside.display()))
    };
    let written = serde_json::to_vec_pretty(&about).map_err(|error| error.to_string())?;
    put(Path::new(ABOUT), &written, false)?;
    let mut total = 0u64;
    if packed.len() > FILES {
        return Err(format!(
            "More than {FILES} files: too much to hand on as one"
        ));
    }
    for (file, inside) in &packed {
        let bytes = fs::read(file).map_err(|error| format!("{}: {error}", file.display()))?;
        total += bytes.len() as u64;
        if total > UNPACKED {
            return Err("Too large to hand on as a file".into());
        }
        #[cfg(unix)]
        let runs = {
            use std::os::unix::fs::PermissionsExt;
            fs::metadata(file).is_ok_and(|found| found.permissions().mode() & 0o111 != 0)
        };
        #[cfg(not(unix))]
        let runs = false;
        put(inside, &bytes, runs)?;
    }
    let bytes = archive
        .into_inner()
        .and_then(flate2::write::GzEncoder::finish)
        .map_err(|error| error.to_string())?;
    if bytes.len() > LIMIT {
        return Err("Too large to hand on as a file".into());
    }
    use base64::Engine;
    Ok(json!({"fileName":format!("{id}.sophon"),
        "archive":base64::engine::general_purpose::STANDARD.encode(bytes),
        "about":read_about(home, &about, None)}))
}

/// Where the file `token` names was read into.
fn read_into(home: &Path, token: &str) -> Result<PathBuf, String> {
    if token.is_empty()
        || token.len() > 64
        || !token
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '-')
    {
        return Err("Unknown file".into());
    }
    Ok(home.join("plugins").join(format!("{READ_INTO}{token}")))
}

/// Files read and never installed, an hour on: gone.
fn forget(home: &Path) {
    for entry in fs::read_dir(home.join("plugins"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let stale = entry
            .metadata()
            .and_then(|found| found.modified())
            .ok()
            .and_then(|at| SystemTime::now().duration_since(at).ok())
            .is_some_and(|age| age > Duration::from_secs(3600));
        if stale
            && entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(READ_INTO))
        {
            let _ = plugin::clear(&entry.path());
        }
    }
}

/// Where a path of the file goes under `into`, if it may be in one: the
/// list of what is in it, or a file of a package's folder, by plain
/// names all the way (no `..`, nothing absolute, nothing of git's).
fn placed(inside: &Path, into: &Path) -> Result<PathBuf, String> {
    let refused = || format!("{} has no place in a file like this", inside.display());
    let mut parts = Vec::new();
    for part in inside.components() {
        match part {
            Component::Normal(name) => parts.push(name.to_str().ok_or_else(refused)?),
            Component::CurDir => {}
            _ => return Err(refused()),
        }
    }
    match parts.as_slice() {
        [name] if *name == ABOUT => Ok(into.join(ABOUT)),
        ["packages", id, rest @ ..]
            if !rest.is_empty()
                && plugin::named(id)
                && !rest.iter().any(|name| *name == ".git" || name.is_empty()) =>
        {
            Ok(parts
                .iter()
                .fold(into.to_owned(), |path, name| path.join(name)))
        }
        _ => Err(refused()),
    }
}

/// What is in a file, as a person is shown it before it is installed
/// and after it was made: each package (whether its folder is in the
/// file, and whether one of its id is here already), each setting by
/// name (never what it is), and the automation by name. `read` is where
/// the file was read into, for the packages that are in it.
fn read_about(home: &Path, about: &Value, read: Option<&Path>) -> Value {
    let mut packages = Vec::new();
    let mut settings = Vec::new();
    for listed in about["packages"].as_array().into_iter().flatten() {
        let Some(id) = listed["id"].as_str().filter(|id| plugin::named(id)) else {
            continue;
        };
        let here = plugin::find(home, id);
        // As the file has it, when it has it; else as it is here.
        let unpacked = read
            .map(|read| read.join("packages").join(id))
            .filter(|folder| folder.is_dir())
            .and_then(|folder| plugin::read(&folder, id).ok());
        let shown = unpacked.as_ref().or(here.as_ref());
        let name = shown
            .map(|package| package["name"].clone())
            .unwrap_or_else(|| listed["name"].clone());
        packages.push(json!({"id":id,"kind":listed["kind"],"name":name,
        "description":shown.map(|package| package["description"].clone()),
        "version":listed["version"],"included":listed["included"] == true,
        "here":match here.as_ref().map(|package| package["source"].as_str().unwrap_or_default()) {
            None => "no",
            Some("builtin") => "builtin",
            Some(_) => "installed",
        }}));
        for (param, _) in about["settings"][id].as_object().into_iter().flatten() {
            let asked = shown.and_then(|package| {
                package["params"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|asked| asked["name"] == param.as_str())
            });
            settings.push(json!({"package":id,"packageName":name,"name":param,
                "label":asked.map(|asked| asked["label"].clone()).unwrap_or_else(|| json!(param)),
                "secret":asked.is_some_and(|asked| asked["secret"] == true)}));
        }
    }
    let root = about["root"].as_str().unwrap_or_default();
    json!({"root":root,
        "name":packages.iter().find(|package| package["id"] == root).map(|package| package["name"].clone()),
        "packages":packages,"settings":settings,
        "automation":about["automation"].as_object().map(|automation| json!({"name":automation.get("name")}))})
}

/// The list a file read into `read` has of itself.
fn listed(read: &Path) -> Result<Value, String> {
    let about: Value = fs::read(read.join(ABOUT))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or("This is not a file Sophon made")?;
    match about["format"].as_u64() {
        Some(FORMAT) => Ok(about),
        Some(_) => Err("This file was made by a newer Sophon: update this one first".into()),
        None => Err("This is not a file Sophon made".into()),
    }
}

/// Reads the file `bytes` into a folder of its own, where nothing runs,
/// and answers what is in it and the token to install it by.
pub fn unpack(home: &Path, bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > LIMIT {
        return Err("This file is too large to be one of Sophon's".into());
    }
    forget(home);
    let token = crate::state::id();
    let into = read_into(home, &token)?;
    plugin::secluded(&into)?;
    let read = (|| {
        let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
        let (mut count, mut total) = (0usize, 0u64);
        for entry in archive
            .entries()
            .map_err(|_| "This is not a file Sophon made".to_owned())?
        {
            let mut entry = entry.map_err(|_| "This is not a file Sophon made".to_owned())?;
            let kind = entry.header().entry_type();
            if kind.is_dir() {
                continue;
            }
            let inside = entry
                .path()
                .map_err(|error| error.to_string())?
                .into_owned();
            // A link is a way out of the folder: none is taken in.
            if !kind.is_file() {
                return Err(format!("{} is not a plain file", inside.display()));
            }
            let target = placed(&inside, &into)?;
            count += 1;
            if count > FILES {
                return Err("This file holds more than a package would".into());
            }
            // Read to the cap and one byte past it, whatever the header says.
            let mut held = Vec::new();
            (&mut entry)
                .take(UNPACKED - total + 1)
                .read_to_end(&mut held)
                .map_err(|_| "This is not a file Sophon made".to_owned())?;
            total += held.len() as u64;
            if total > UNPACKED {
                return Err("This file holds more than a package would".into());
            }
            if let Some(folder) = target.parent() {
                fs::create_dir_all(folder).map_err(|error| error.to_string())?;
            }
            fs::write(&target, held).map_err(|error| error.to_string())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let runs = entry.header().mode().is_ok_and(|mode| mode & 0o111 != 0);
                fs::set_permissions(
                    &target,
                    fs::Permissions::from_mode(if runs { 0o755 } else { 0o644 }),
                )
                .map_err(|error| error.to_string())?;
            }
        }
        let about = listed(&into)?;
        // Each folder in it is the package its name says, and can be read.
        for folder in fs::read_dir(into.join("packages"))
            .into_iter()
            .flatten()
            .flatten()
        {
            let name = folder.file_name().to_str().unwrap_or_default().to_owned();
            let package = plugin::read(&folder.path(), &name)?;
            if package["id"] != name.as_str() {
                return Err(format!(
                    "The folder {name} of this file holds {}",
                    package["id"].as_str().unwrap_or_default()
                ));
            }
        }
        Ok(about)
    })();
    match read {
        Ok(about) => Ok(json!({"token":token,"about":read_about(home, &about, Some(&into))})),
        Err(error) => {
            let _ = plugin::clear(&into);
            Err(error)
        }
    }
}

/// Lets go of a file read and not wanted.
pub fn discard(home: &Path, token: &str) -> Result<(), String> {
    plugin::clear(&read_into(home, token)?)
}

/// Installs the file `token` names, read before: each package of it in
/// its place (one of its id there already replaced, what that kept
/// left), the settings set, all of it turned on. Answers the package it
/// is about and the automation that came with it.
pub fn install(home: &Path, token: &str) -> Result<Value, String> {
    let into = read_into(home, token)?;
    if !into.is_dir() {
        return Err("That file is no longer here: choose it again".into());
    }
    let about = listed(&into)?;
    let plugins = home.join("plugins");
    for folder in fs::read_dir(into.join("packages"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = folder.file_name().to_str().unwrap_or_default().to_owned();
        if !plugin::named(&name) {
            continue;
        }
        let target = plugins.join(&name);
        plugin::clear(&target)?;
        fs::rename(folder.path(), &target).map_err(|error| error.to_string())?;
        plugin::secluded(&plugin::data(home, &name))?;
    }
    plugin::clear(&into)?;
    for listed in about["packages"].as_array().into_iter().flatten() {
        let Some(id) = listed["id"].as_str().filter(|id| plugin::named(id)) else {
            continue;
        };
        // It is wanted: one put away is brought back, one off is on.
        if plugin::among(&plugin::kept(home), "away", id) {
            plugin::mark(home, "away", id, false)?;
        }
        let Some(package) = plugin::find(home, id) else {
            continue;
        };
        plugin::mark(home, "off", id, false)?;
        let mut values = plugin::own(home, id);
        let mut set = false;
        for (name, value) in about["settings"][id].as_object().into_iter().flatten() {
            // Only what it asks for itself.
            let asked = package["params"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|param| param["name"] == name.as_str());
            if asked && let Some(value) = value.as_str().filter(|value| !value.is_empty()) {
                values.insert(name.clone(), json!(value));
                set = true;
            }
        }
        if set {
            plugin::secluded(&plugin::data(home, id))?;
            crate::state::write(
                &plugin::data(home, id),
                "params.json",
                &serde_json::to_vec(&values).map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        }
    }
    Ok(json!({"root":about["root"],"automation":about["automation"]}))
}

#[cfg(test)]
mod tests {
    use super::{ABOUT, Settings, install, pack, unpack};
    use crate::plugin::{AUTOMATION, INTEGRATION, PLUGIN};
    use base64::Engine;
    use serde_json::{Value, json};
    use std::{fs, path::Path};

    fn put(dir: &Path, inside: &str, words: &str) {
        let file = dir.join(inside);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, words).unwrap();
    }

    /// A home with an integration (`chat`, an app's id and secret set
    /// here) and a plugin that counts on it, each a folder of its own.
    fn home() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let chat = home.path().join("plugins/chat");
        put(
            &chat,
            INTEGRATION,
            r#"{"id":"chat","name":"Chat","use":"./say",
                "params":[{"name":"app_id","label":"App ID"},{"name":"app_secret","label":"App Secret","secret":true},{"name":"unset","optional":true}]}"#,
        );
        put(&chat, "say", "#!/bin/sh\necho said\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(chat.join("say"), fs::Permissions::from_mode(0o755)).unwrap();
        }
        // What is git's, and what one person keeps, stay behind.
        put(&chat, ".git/config", "[core]\n");
        put(
            home.path(),
            "plugin-data/chat/params.json",
            r#"{"app_id":"cli_1","app_secret":"s3cret","stray":"x"}"#,
        );
        put(home.path(), "plugin-data/chat/token", "mine alone");
        let watch = home.path().join("plugins/watch");
        put(
            &watch,
            PLUGIN,
            r#"{"id":"watch","name":"Watch","version":"1.2.0","requires":["chat"],"params":[{"name":"people"}]}"#,
        );
        put(
            &watch,
            &format!("automations/review/{AUTOMATION}"),
            r#"{"prompt":"Review {{url}}","params":[{"name":"approve","type":"switch"}]}"#,
        );
        put(
            &watch,
            "skills/pair-up/SKILL.md",
            "---\nname: pair-up\n---\n",
        );
        put(
            home.path(),
            "plugin-data/watch/params.json",
            r#"{"people":"ada=ada@x.io"}"#,
        );
        home
    }

    fn bytes(packed: &Value) -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(packed["archive"].as_str().unwrap())
            .unwrap()
    }

    #[test]
    fn a_plugin_goes_to_another_computer_as_one_file() {
        let here = home();
        let schedule = json!({"id":"s1","name":"Watch mine","prompt":"Review {{url}} well","plugin":"watch","template":"review",
            "options":{"approve":"yes"},"cadence":{"type":"cron","expression":"0 9 * * 1-5","timezone":"Asia/Singapore"},
            "target":{"type":"new-agent","config":{"cwd":"/home/me/project","model":"m"}},"status":"active"});
        let packed = pack(here.path(), "watch", Settings::All, Some(&schedule)).unwrap();
        assert_eq!(packed["fileName"], "watch.sophon");
        // What it says of itself: both packages in it, and each setting
        // by name.
        let about = &packed["about"];
        assert_eq!(about["root"], "watch");
        assert_eq!(about["packages"].as_array().unwrap().len(), 2);
        assert!(
            about["packages"]
                .as_array()
                .unwrap()
                .iter()
                .all(|package| package["included"] == true)
        );
        let names: Vec<&str> = about["settings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|setting| setting["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["people", "app_id", "app_secret"]);
        assert!(!packed["about"].to_string().contains("s3cret"));
        assert_eq!(about["automation"]["name"], "Watch mine");

        // On a computer with neither.
        let there = tempfile::tempdir().unwrap();
        let read = unpack(there.path(), &bytes(&packed)).unwrap();
        let token = read["token"].as_str().unwrap();
        // Read, and nothing of it where it would run yet.
        assert_eq!(read["about"]["packages"][0]["here"], "no");
        assert_eq!(read["about"]["packages"][0]["name"], "Watch");
        assert_eq!(read["about"]["settings"][2]["secret"], true);
        assert_eq!(read["about"]["settings"][2]["label"], "App Secret");
        assert!(crate::plugin::list(there.path()).is_empty());
        assert!(!there.path().join("plugin-data").exists());

        let installed = install(there.path(), token).unwrap();
        assert_eq!(installed["root"], "watch");
        let all = crate::plugin::list(there.path());
        assert_eq!(all.len(), 2);
        let chat = all.iter().find(|package| package["id"] == "chat").unwrap();
        assert!(chat["error"].is_null(), "{chat}");
        assert_eq!(chat["source"], "file");
        let folder = there.path().join("plugins/chat");
        assert!(!folder.join(".git").exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |name: &str| {
                fs::metadata(folder.join(name))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777
            };
            assert_eq!(mode("say"), 0o755);
            assert_eq!(mode(INTEGRATION), 0o644);
        }
        assert!(
            there
                .path()
                .join("plugins/watch/skills/pair-up/SKILL.md")
                .is_file()
        );
        // The settings came, only what each asks for; what one person
        // keeps did not.
        let set = |id: &str| -> Value {
            serde_json::from_slice(
                &fs::read(
                    there
                        .path()
                        .join("plugin-data")
                        .join(id)
                        .join("params.json"),
                )
                .unwrap(),
            )
            .unwrap()
        };
        assert_eq!(set("chat"), json!({"app_id":"cli_1","app_secret":"s3cret"}));
        assert_eq!(set("watch"), json!({"people":"ada=ada@x.io"}));
        assert!(!there.path().join("plugin-data/chat/token").exists());
        // The automation came as it was set up, without where it ran.
        let automation = &installed["automation"];
        assert_eq!(automation["prompt"], "Review {{url}} well");
        assert_eq!(automation["options"]["approve"], "yes");
        assert_eq!(
            automation["cadence"],
            json!({"type":"cron","expression":"0 9 * * 1-5"})
        );
        assert!(automation["target"].is_null() && automation["id"].is_null());
        // The folder it was read into is gone, and so is its token.
        assert!(install(there.path(), token).is_err());
        assert!(
            fs::read_dir(there.path().join("plugins"))
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_str().unwrap().starts_with('.'))
        );

        // Taken in again over what is there: said so, and what was set
        // here since stays where the file says nothing.
        put(
            there.path(),
            "plugin-data/watch/params.json",
            r#"{"people":"bob=bob@x.io"}"#,
        );
        let plain = pack(here.path(), "watch", Settings::Plain, None).unwrap();
        assert!(
            !plain["about"]["settings"]
                .to_string()
                .contains("app_secret")
        );
        assert!(plain["about"]["automation"].is_null());
        let none = pack(here.path(), "watch", Settings::None, None).unwrap();
        assert_eq!(none["about"]["settings"], json!([]));
        let read = unpack(there.path(), &bytes(&none)).unwrap();
        assert_eq!(read["about"]["packages"][0]["here"], "installed");
        install(there.path(), read["token"].as_str().unwrap()).unwrap();
        assert_eq!(set("watch"), json!({"people":"bob=bob@x.io"}));
        assert_eq!(set("chat")["app_secret"], "s3cret");
    }

    #[test]
    fn an_automation_goes_only_with_what_it_was_made_with() {
        let here = home();
        let other = json!({"id":"s2","name":"Other","plugin":"elsewhere"});
        assert!(pack(here.path(), "watch", Settings::All, Some(&other)).is_err());
        assert!(pack(here.path(), "nowhere", Settings::All, None).is_err());
    }

    /// A file made by hand, each entry a path, what it holds, and how
    /// its header says what it is.
    fn made(entries: &[(&str, &[u8], tar::EntryType)]) -> Vec<u8> {
        let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        ));
        for (path, held, kind) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(*kind);
            header.set_size(held.len() as u64);
            header.set_mode(0o644);
            // Written as it is: the library would refuse a path that
            // climbs, and a file from elsewhere need not have been made
            // by it.
            let name = &mut header.as_old_mut().name;
            name[..path.len()].copy_from_slice(path.as_bytes());
            if *kind == tar::EntryType::Symlink {
                header.set_link_name("/etc/passwd").unwrap();
            }
            header.set_cksum();
            archive.append(&header, *held).unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    #[test]
    fn a_file_that_reaches_outside_its_packages_is_refused_whole() {
        let about =
            br#"{"format":1,"root":"x","packages":[{"id":"x","kind":"plugin","included":true}]}"#;
        let manifest = br#"{"id":"x"}"#;
        let file = tar::EntryType::Regular;
        let refused = |entries: &[(&str, &[u8], tar::EntryType)]| {
            let home = tempfile::tempdir().unwrap();
            let outside = home.path().join("outside");
            fs::create_dir_all(home.path().join("service")).unwrap();
            let error = unpack(&home.path().join("service"), &made(entries)).unwrap_err();
            // Nothing of it is left, and nothing was written beside it.
            assert!(!outside.exists(), "{error}");
            assert!(
                fs::read_dir(home.path().join("service/plugins"))
                    .unwrap()
                    .next()
                    .is_none(),
                "{error}"
            );
            assert!(!home.path().join("service/plugin-data").exists());
            error
        };
        // Up and out of the folder it is read into.
        refused(&[
            (ABOUT, about, file),
            ("packages/x/../../../../outside", b"x", file),
        ]);
        refused(&[(ABOUT, about, file), ("../outside", b"x", file)]);
        refused(&[(ABOUT, about, file), ("/tmp/sophon-outside", b"x", file)]);
        // Beside the packages, or straight under them.
        refused(&[(ABOUT, about, file), ("plugins.json", b"{}", file)]);
        refused(&[(ABOUT, about, file), ("packages/x", b"x", file)]);
        refused(&[(ABOUT, about, file), ("packages/.hidden/a", b"x", file)]);
        // A link, and what is git's.
        refused(&[
            (ABOUT, about, file),
            ("packages/x/passwd", b"", tar::EntryType::Symlink),
        ]);
        refused(&[
            (ABOUT, about, file),
            ("packages/x/.git/config", b"[core]", file),
        ]);
        // No list of what is in it, one from a later format, and not a
        // file of this kind at all.
        refused(&[("packages/x/.sophon-plugin/plugin.json", manifest, file)]);
        let error = refused(&[(ABOUT, br#"{"format":2}"#, file)]);
        assert!(error.contains("newer"), "{error}");
        let home = tempfile::tempdir().unwrap();
        assert!(unpack(home.path(), b"not a file of this kind").is_err());
        // A folder that is not the package its name says.
        refused(&[
            (ABOUT, about, file),
            (
                "packages/x/.sophon-plugin/plugin.json",
                br#"{"id":"y"}"#,
                file,
            ),
        ]);
        // And one that is all it should be is read.
        let home = tempfile::tempdir().unwrap();
        let read = unpack(
            home.path(),
            &made(&[
                (ABOUT, about, file),
                ("packages/x/.sophon-plugin/plugin.json", manifest, file),
            ]),
        )
        .unwrap();
        assert_eq!(read["about"]["packages"][0]["id"], "x");
    }
}
