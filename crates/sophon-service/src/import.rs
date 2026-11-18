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
        agent["status"] = json!("idle");
        agent["model"] = agent.pointer("/config/model").cloned().unwrap_or_default();
        state.agents.insert(key, agent);
    }
    Ok(state)
}
