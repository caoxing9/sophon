use cap_std::{ambient_authority, fs::Dir};
use serde_json::{Value, json};
use std::{
    io::{self, Read},
    path::{Component, Path},
};

fn field<'a>(request: &'a Value, key: &str) -> io::Result<&'a str> {
    request[key]
        .as_str()
        .ok_or_else(|| io::Error::other(format!("Missing {key}")))
}

fn relative(path: &str) -> io::Result<&Path> {
    let value = Path::new(path);
    if value
        .components()
        .all(|part| matches!(part, Component::CurDir | Component::Normal(_)))
    {
        Ok(value)
    } else {
        Err(io::Error::other("Expected a workspace-relative path"))
    }
}

/// `path` if it names something inside the workspace: the workspace
/// itself (`.`, or nothing) is not one of its entries, and is never
/// what a rename, a delete or a write is for.
fn entry(path: &Path) -> io::Result<&Path> {
    if path
        .components()
        .any(|part| matches!(part, Component::Normal(_)))
    {
        Ok(path)
    } else {
        Err(io::Error::other(
            "Expected a file or folder inside the workspace",
        ))
    }
}

fn name(request: &Value) -> io::Result<&str> {
    let name = field(request, "name")?;
    if Path::new(name).file_name().and_then(|value| value.to_str()) == Some(name) {
        Ok(name)
    } else {
        Err(io::Error::other("Expected a file name"))
    }
}

fn stamp(metadata: &cap_std::fs::Metadata) -> io::Result<String> {
    Ok(chrono::DateTime::<chrono::Utc>::from(metadata.modified()?.into_std()).to_rfc3339())
}

pub fn execute(root: &Path, request: &Value) -> io::Result<Value> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())?;
    let value = request["path"].as_str().unwrap_or(".");
    let path = relative(value)?;
    let kind = field(request, "type")?;
    let mut response = json!({"cwd":root,"path":value,"mode":request["mode"]});
    match kind {
        "file_explorer_request" if request["mode"] == "file" => {
            // Asked before opening: opening a pipe waits for its other
            // end, on the thread everything else runs on.
            if !directory.metadata(path)?.is_file() {
                return Err(io::Error::other("Expected a regular file"));
            }
            let file = directory.open(path)?;
            let metadata = file.metadata()?;
            let limit = request["maxBytes"]
                .as_u64()
                .unwrap_or(2_000_000)
                .min(2_000_000);
            let mut bytes = Vec::new();
            file.take(limit + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > limit {
                return Err(io::Error::other("File exceeds preview limit"));
            }
            let modified = stamp(&metadata)?;
            let text = String::from_utf8(bytes)
                .ok()
                .filter(|text| !text.contains('\0'));
            response["file"] = json!({"path":value,"size":metadata.len(),"modifiedAt":modified,"revision":modified,
                "kind":if text.is_some() {"text"} else {"binary"},"encoding":text.as_ref().map(|_|"utf-8"),"content":text});
        }
        "file_explorer_request" => {
            let mut entries = Vec::new();
            for entry in directory.read_dir(path)? {
                let entry = entry?;
                let name = entry.file_name();
                let is_dir = entry.file_type()?.is_dir();
                let child = if value == "." {
                    name.clone().into()
                } else {
                    path.join(&name)
                };
                entries.push(json!({"name":name.to_string_lossy(),"path":child,"kind":if is_dir {"directory"} else {"file"},
                    "size":entry.metadata().map(|metadata|metadata.len()).unwrap_or_default()}));
            }
            entries.sort_by(|left, right| {
                (left["kind"].as_str(), left["name"].as_str())
                    .cmp(&(right["kind"].as_str(), right["name"].as_str()))
            });
            response["directory"] = json!({"entries":entries});
        }
        "fs.file.write.request" => {
            let metadata = directory.metadata(entry(path)?)?;
            let current = stamp(&metadata)?;
            if !metadata.is_file() {
                return Err(io::Error::other("Expected a regular file"));
            }
            let expected = request["expectedRevision"]
                .as_str()
                .or(request["expectedModifiedAt"].as_str());
            if expected != Some(&current) {
                response["result"] = json!({"status":"conflict"});
            } else {
                directory.write(path, field(request, "content")?)?;
                let modified = stamp(&directory.metadata(path)?)?;
                response["result"] =
                    json!({"status":"written","modifiedAt":modified,"revision":modified});
            }
        }
        "fs.entry.create.request" => {
            let parent = relative(field(request, "parentPath")?)?;
            let child = parent.join(name(request)?);
            if request["kind"] == "directory" {
                directory.create_dir(child)?
            } else {
                directory.open_with(
                    child,
                    cap_std::fs::OpenOptions::new().create_new(true).write(true),
                )?;
            }
            response["parentPath"] = json!(parent);
        }
        "fs.entry.rename.request" => {
            let parent = entry(path)?.parent().unwrap_or(Path::new(""));
            let destination = parent.join(name(request)?);
            // Taken, unless by the entry itself under its other name
            // (`readme` to `README` where case does not tell files apart).
            if let Ok(taken) = directory.symlink_metadata(&destination) {
                use cap_std::fs::MetadataExt;
                let moved = directory.symlink_metadata(path)?;
                if (taken.dev(), taken.ino()) != (moved.dev(), moved.ino()) {
                    return Err(io::Error::other("Destination exists"));
                }
            }
            directory.rename(path, &directory, destination)?;
            response["parentPath"] = json!(parent);
        }
        // A copy beside it, under the first of "name copy", "name copy 2"
        // … that is free; a folder with everything in it.
        "fs.entry.duplicate.request" => {
            let parent = entry(path)?.parent().unwrap_or(Path::new(""));
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let (stem, extension) = match name.rsplit_once('.') {
                Some((stem, extension)) if !stem.is_empty() => (stem, format!(".{extension}")),
                _ => (name, String::new()),
            };
            let copy = (1..1000)
                .map(|number| {
                    let mark = if number == 1 {
                        " copy".to_owned()
                    } else {
                        format!(" copy {number}")
                    };
                    parent.join(format!("{stem}{mark}{extension}"))
                })
                .find(|copy| directory.symlink_metadata(copy).is_err())
                .ok_or_else(|| io::Error::other("Too many copies"))?;
            fn copy_all(directory: &Dir, from: &Path, to: &Path, depth: usize) -> io::Result<()> {
                let kind = directory.symlink_metadata(from)?;
                if kind.is_dir() {
                    if depth > 64 {
                        return Err(io::Error::other("Folders nested too deep to copy"));
                    }
                    directory.create_dir(to)?;
                    for child in directory.read_dir(from)? {
                        let name = child?.file_name();
                        copy_all(directory, &from.join(&name), &to.join(&name), depth + 1)?;
                    }
                } else if kind.is_file() {
                    directory.copy(from, directory, to)?;
                }
                Ok(())
            }
            copy_all(&directory, path, &copy, 0)?;
            response["parentPath"] = json!(parent);
            response["path"] = json!(copy);
        }
        "fs.entry.delete.request" => {
            let parent = entry(path)?.parent().unwrap_or(Path::new(""));
            if directory.symlink_metadata(path)?.is_dir() {
                directory.remove_dir_all(path)?
            } else {
                directory.remove_file(path)?
            }
            response["parentPath"] = json!(parent);
        }
        _ => {
            return Err(io::Error::other(format!(
                "Unsupported file request: {kind}"
            )));
        }
    }
    Ok(response)
}
