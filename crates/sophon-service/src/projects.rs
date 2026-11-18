use crate::{host::Shared, state::write};
use cap_std::{ambient_authority, fs::Dir};
use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

fn home_path(value: &str) -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    match value {
        "~" | "" => home,
        path if path.starts_with("~/") => home.join(&path[2..]),
        _ => PathBuf::from(value),
    }
}

pub fn suggestions(request: &Value) -> Result<Value, String> {
    let query = request["query"].as_str().unwrap_or_default();
    let root = request["cwd"].as_str().map(PathBuf::from);
    let path = if let Some(dir) = &root {
        dir.join(query)
    } else {
        home_path(query)
    };
    let (dir, prefix) =
        if path.is_dir() && (query.is_empty() || query.ends_with('/') || query == "~") {
            (path.as_path(), "")
        } else {
            (
                path.parent().unwrap_or(&path),
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default(),
            )
        };
    let mut entries = Vec::new();
    let limit = request["limit"].as_u64().unwrap_or(40).min(200) as usize;
    for entry in fs::read_dir(dir)
        .map_err(|error| error.to_string())?
        .flatten()
    {
        let name = entry.file_name();
        if !name
            .to_string_lossy()
            .to_lowercase()
            .starts_with(&prefix.to_lowercase())
        {
            continue;
        }
        let path = entry.path();
        let directory = path.is_dir();
        if directory && request["includeDirectories"] == false
            || !directory && request["includeFiles"] != true
        {
            continue;
        }
        let show = root
            .as_ref()
            .and_then(|root| path.strip_prefix(root).ok())
            .unwrap_or(&path);
        entries.push(json!({"path":show,"kind":if directory {"directory"} else {"file"}}));
    }
    entries.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    entries.truncate(limit);
    Ok(json!({"entries":entries}))
}

pub fn create_dir(request: &Value) -> Result<String, String> {
    let parent = home_path(request["parentPath"].as_str().ok_or("Missing parent")?);
    let name = request["name"].as_str().ok_or("Missing name")?;
    if Path::new(name).file_name().and_then(|value| value.to_str()) != Some(name) {
        return Err("Expected a folder name".into());
    }
    let path = parent.join(name);
    fs::create_dir(&path).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

pub fn settings(shared: &Shared, request: &Value) -> Result<Value, String> {
    let root = request["repoRoot"].as_str().ok_or("Missing root")?;
    if !shared
        .borrow()
        .state
        .projects
        .values()
        .any(|project| project["projectRootPath"] == root)
    {
        return Err("Unknown project".into());
    }
    let directory =
        Dir::open_ambient_dir(root, ambient_authority()).map_err(|error| error.to_string())?;
    let read = || -> io::Result<(Value, Value)> {
        match directory.read("sophon.json") {
            Ok(bytes) => {
                use std::hash::{DefaultHasher, Hasher};
                let mut revision = DefaultHasher::new();
                revision.write(&bytes);
                Ok((
                    serde_json::from_slice(&bytes)?,
                    json!(revision.finish().to_string()),
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok((json!({}), Value::Null)),
            Err(error) => Err(error),
        }
    };
    let (mut config, mut rev) = read().map_err(|error| error.to_string())?;
    if request["type"] == "write_project_config_request" {
        if request["expectedRevision"] != rev {
            return Err("Project settings changed on disk".into());
        }
        if !request["config"].is_object() {
            return Err("Expected a JSON object".into());
        }
        write(
            Path::new(root),
            "sophon.json",
            &serde_json::to_vec_pretty(&request["config"]).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        (config, rev) = read().map_err(|error| error.to_string())?;
    }
    Ok(json!({"ok":true,"repoRoot":root,"config":config,"revision":rev}))
}
