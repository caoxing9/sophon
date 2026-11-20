use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs, io, path::Path};

pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn write(home: &Path, name: &str, bytes: &[u8]) -> io::Result<()> {
    use io::Write;
    let mut file = tempfile::NamedTempFile::new_in(home)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(home.join(name))?;
    Ok(())
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    pub projects: BTreeMap<String, Value>,
    pub workspaces: BTreeMap<String, Value>,
    pub agents: BTreeMap<String, Value>,
    /// Conversations from when they were kept here, one row an item:
    /// read once, to give each its own log (`timeline`), and not
    /// written again.
    #[serde(default, skip_serializing)]
    pub history: BTreeMap<String, Vec<Value>>,
    #[serde(default)]
    pub creations: BTreeMap<String, String>,
    #[serde(default)]
    pub schedules: BTreeMap<String, Value>,
    #[serde(default)]
    pub runs: BTreeMap<String, Vec<Value>>,
    /// The events each automation has had, by their ids: one is acted
    /// on once.
    #[serde(default)]
    pub seen: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub labels: BTreeMap<String, Value>,
    /// The agents each agent has started, as its provider told of them.
    #[serde(default)]
    pub subagents: BTreeMap<String, Vec<Value>>,
}

impl State {
    pub fn load(home: &Path) -> io::Result<Self> {
        match fs::read(home.join("state.json")) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    pub fn save(&self, home: &Path) -> io::Result<()> {
        write(home, "state.json", &serde_json::to_vec(self)?)
    }

    pub fn project(&mut self, directory: &str) -> Result<Value, String> {
        let path = fs::canonicalize(directory).map_err(|error| error.to_string())?;
        if !path.is_dir() {
            return Err("A project must be a directory".into());
        }
        let cwd = path.to_str().ok_or("Invalid directory name")?;
        if let Some(project) = self
            .projects
            .values()
            .find(|project| project["projectRootPath"] == cwd)
        {
            return Ok(project.clone());
        }
        let project_id = id();
        let project = json!({
            "projectId": project_id, "projectRootPath": cwd,
            "projectDisplayName": path.file_name().and_then(|part| part.to_str()).unwrap_or(cwd),
            "projectKind": if path.join(".git").exists() { "git" } else { "directory" }
        });
        self.projects.insert(project_id, project.clone());
        Ok(project)
    }

    pub fn workspace(&mut self, project: &Value) -> Value {
        if let Some(workspace) = self.workspaces.values().find(|entry| {
            entry["projectId"] == project["projectId"]
                && entry["workspaceDirectory"] == project["projectRootPath"]
                && entry["archivedAt"].is_null()
        }) {
            return workspace.clone();
        }
        let workspace_id = id();
        let mut workspace = project.clone();
        workspace["id"] = json!(workspace_id);
        workspace["workspaceDirectory"] = project["projectRootPath"].clone();
        workspace["name"] = project["projectDisplayName"].clone();
        workspace["workspaceKind"] = json!("directory");
        self.workspaces.insert(workspace_id, workspace.clone());
        workspace
    }
}
