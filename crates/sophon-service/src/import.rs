use crate::state::{State, id};
use serde_json::{Value, json};
use std::{fs, io, path::Path};

fn required<'a>(entry: &'a Value, name: &str) -> io::Result<&'a str> {
    entry[name]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| io::Error::other(format!("Missing {name} in legacy registry")))
}

fn registry(home: &Path, name: &str) -> io::Result<Vec<Value>> {
    match fs::read(home.join("projects").join(name)) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

pub fn load(source: &Path) -> io::Result<State> {
    if !source.is_dir() || !(source.join("projects").is_dir() || source.join("agents").is_dir()) {
        return Err(io::Error::other(
            "Expected a legacy service home with projects or agents",
        ));
    }
    let mut state = State::default();
    for entry in registry(source, "projects.json")? {
        let key = required(&entry, "projectId")?;
        state.projects.insert(key.into(), json!({"projectId":key,"projectRootPath":required(&entry,"rootPath")?,
            "projectDisplayName":entry["customName"].as_str().or(entry["displayName"].as_str()),"projectKind":entry["kind"],
            "archivedAt":entry["archivedAt"]}));
    }
    for entry in registry(source, "workspaces.json")? {
        let key = required(&entry, "workspaceId")?;
        let project = state
            .projects
            .get(required(&entry, "projectId")?)
            .cloned()
            .unwrap_or_else(|| json!({}));
        let mut workspace = project;
        workspace["id"] = json!(key);
        workspace["workspaceKind"] = entry["kind"].clone();
        workspace["workspaceDirectory"] = json!(required(&entry, "cwd")?);
        workspace["name"] = json!(entry["title"].as_str().or(entry["displayName"].as_str()));
        workspace["archivedAt"] = entry["archivedAt"].clone();
        workspace["pinnedAt"] = entry["pinnedAt"].clone();
        state.workspaces.insert(key.into(), workspace);
    }
    let agents = source.join("agents");
    if !agents.is_dir() {
        return Ok(state);
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(agents)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            for record in fs::read_dir(entry.path())? {
                let record = record?;
                if record.file_type()?.is_file() {
                    paths.push(record.path())
                }
            }
        } else if entry.file_type()?.is_file() {
            paths.push(entry.path());
        }
    }
    paths.sort();
    for path in paths {
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let mut agent: Value = serde_json::from_slice(&fs::read(&path)?)?;
        let key = required(&agent, "id")?.to_owned();
        let cwd = required(&agent, "cwd")?.to_owned();
        if state
            .agents
            .get(&key)
            .is_some_and(|old| old["updatedAt"].as_str() >= agent["updatedAt"].as_str())
        {
            continue;
        }
        if !agent["workspaceId"]
            .as_str()
            .is_some_and(|key| state.workspaces.contains_key(key))
        {
            let project = state.projects.values().find(|entry| entry["projectRootPath"] == cwd).cloned().unwrap_or_else(|| {
                let key = id();
                let project = json!({"projectId":key,"projectRootPath":cwd,"projectDisplayName":Path::new(&cwd).file_name().and_then(|name| name.to_str()),"projectKind":"directory"});
                state.projects.insert(key,project.clone());
                project
            });
            agent["workspaceId"] = state.workspace(&project)["id"].clone();
        }
        // The old service's own (it named conversations with them).
        if agent["internal"] == true {
            continue;
        }
        agent["status"] = json!("idle");
        agent["model"] = agent.pointer("/config/model").cloned().unwrap_or_default();
        agent["thinkingOptionId"] = agent
            .pointer("/config/thinkingOptionId")
            .cloned()
            .unwrap_or_default();
        if agent["currentModeId"].is_null() {
            agent["currentModeId"] = agent["lastModeId"].clone();
        }
        state.agents.insert(key, agent);
    }
    schedules(source, &mut state)?;
    Ok(state)
}

/// The old service's schedules, a file each, with the runs each kept.
/// One that was to send into a conversation already open (this service
/// starts a new one each time) comes over paused; one whose next time
/// passed while nothing ran it gets its next time from now, not a run
/// the moment this service starts.
fn schedules(source: &Path, state: &mut State) -> io::Result<()> {
    let Ok(files) = fs::read_dir(source.join("schedules")) else {
        return Ok(());
    };
    let now = chrono::Utc::now();
    for file in files {
        let path = file?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Ok(mut schedule) = serde_json::from_slice::<Value>(&fs::read(&path)?) else {
            continue;
        };
        let Some(key) = schedule["id"].as_str().map(str::to_owned) else {
            continue;
        };
        if let Some(runs) = schedule
            .as_object_mut()
            .and_then(|fields| fields.remove("runs"))
            && let Value::Array(runs) = runs
        {
            state.runs.insert(key.clone(), runs);
        }
        if schedule["target"]["type"] != "new-agent" {
            schedule["status"] = json!("paused");
        }
        schedule["nextRunAt"] = match schedule["status"].as_str() {
            Some("active") => match crate::schedule::next(&schedule["cadence"], now) {
                Ok(when) => json!(when),
                Err(_) => {
                    schedule["status"] = json!("paused");
                    Value::Null
                }
            },
            _ => Value::Null,
        };
        state.schedules.insert(key, schedule);
    }
    Ok(())
}

/// The old service's settings this one has a place for: where phones
/// reach the host from away (`daemon.relay`), and the providers that are
/// Claude Code or Codex. `listen`: the address it was set to, kept when
/// asked (a space other than the first is found by its port).
fn settings(source: &Path, listen: bool) -> Value {
    let old: Value = fs::read(source.join("config.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null);
    let mut config = json!({});
    if old["daemon"]["relay"].is_object() {
        config["daemon"]["relay"] = old["daemon"]["relay"].clone();
    }
    if listen && old["daemon"]["listen"].is_string() {
        config["daemon"]["listen"] = old["daemon"]["listen"].clone();
    }
    for (name, provider) in old["agents"]["providers"].as_object().into_iter().flatten() {
        if matches!(
            provider["extends"].as_str().unwrap_or(name),
            "claude" | "codex"
        ) {
            config["providers"][name] = provider.clone();
        }
    }
    config
}

/// Where the old service kept what the service in `home` now keeps:
/// `~/.paseo` for the first space (`~/.sophon/service`), and
/// `~/.paseo-spaces/<name>` for `~/.sophon/spaces/<name>`. None for a
/// home anywhere else: a service set up by hand, or for a test.
fn old_home(home: &Path) -> Option<std::path::PathBuf> {
    let user = std::path::PathBuf::from(std::env::var_os("HOME")?);
    if home == user.join(".sophon/service") {
        return Some(user.join(".paseo"));
    }
    if home.parent()? == user.join(".sophon/spaces") {
        return Some(user.join(".paseo-spaces").join(home.file_name()?));
    }
    None
}

/// Stops the old service of `source` if it still runs: an app replaced
/// under it leaves it running, and its schedules would run beside the
/// ones brought over.
#[cfg(unix)]
fn retire(source: &Path) {
    let run =
        |program: &str, args: &[&str]| std::process::Command::new(program).args(args).output().ok();
    let Some(pid) = fs::read(source.join("paseo.pid"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|record| record["pid"].as_u64())
        .map(|pid| pid.to_string())
    else {
        return;
    };
    // Only if that process is still the old service: its number may be
    // another's by now.
    let named = run("ps", &["-o", "command=", "-p", &pid]).is_some_and(|out| {
        String::from_utf8_lossy(&out.stdout)
            .to_lowercase()
            .contains("paseo")
    });
    if named {
        let _ = run("kill", &["-TERM", &pid]);
        eprintln!("Stopped the old service of {}", source.display());
    }
}

#[cfg(not(unix))]
fn retire(_source: &Path) {}

/// Brings what the old service kept for `home` into it, the first time
/// this service starts there (it has no state yet): projects,
/// workspaces, conversations and schedules, and the settings above.
/// The old service's folder is read and left as it is. A conversation's
/// earlier messages are not copied: neither service keeps them, the
/// agent's CLI does, and they are read from there when it is opened
/// (`history`). For the first space, the other spaces are brought over
/// with it, each into its own folder.
pub fn adopt(home: &Path) {
    let Some(source) = old_home(home).filter(|_| !home.join("state.json").exists()) else {
        return;
    };
    let first = home.file_name().is_some_and(|name| name == "service");
    if source.is_dir() {
        retire(&source);
        match load(&source) {
            Ok(state) => {
                let config = settings(&source, !first);
                let kept = state.save(home).and_then(|()| {
                    if home.join("config.json").exists() || config == json!({}) {
                        return Ok(());
                    }
                    crate::state::write(home, "config.json", &serde_json::to_vec_pretty(&config)?)
                });
                match kept {
                    Ok(()) => eprintln!(
                        "Brought over from {}: {} projects, {} workspaces, {} conversations, {} schedules",
                        source.display(),
                        state.projects.len(),
                        state.workspaces.len(),
                        state.agents.len(),
                        state.schedules.len()
                    ),
                    Err(error) => {
                        eprintln!("Could not keep what {} had: {error}", source.display())
                    }
                }
            }
            Err(error) => eprintln!("Nothing brought over from {}: {error}", source.display()),
        }
    }
    if !first {
        return;
    }
    let Some(spaces) = source.parent().map(|user| user.join(".paseo-spaces")) else {
        return;
    };
    for space in fs::read_dir(spaces).into_iter().flatten().flatten() {
        let Some(target) = home
            .parent()
            .map(|data| data.join("spaces").join(space.file_name()))
        else {
            continue;
        };
        if space.path().is_dir() && !target.exists() && fs::create_dir_all(&target).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&target, fs::Permissions::from_mode(0o700));
            }
            adopt(&target);
        }
    }
}
