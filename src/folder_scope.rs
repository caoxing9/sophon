//! A paired device kept to one folder of this Mac. The bridge stands
//! between the device and the service and reads every frame (it holds the
//! end-to-end keys), so for a device with a folder it
//!
//! - refuses the device's requests that name a path outside it: a working
//!   directory, a file, a folder to list or to add as a project
//!   ([`refuse`]: an `rpc_error` the device shows);
//! - leaves out of the service's frames what lies outside it: projects,
//!   workspaces and conversations from the lists, and any other frame
//!   about something outside it ([`scope`]).
//!
//! It keeps the device's view and requests to the folder; it is no sandbox
//! for the agents: an agent started in the folder runs as this Mac's user,
//! and its own commands can read what that user can.

use serde_json::{Value, json};

/// The keys that hold a path, in requests and in the service's frames.
const PATH_KEYS: [&str; 11] = ["cwd", "path", "paths", "directory", "dir", "rootPath", "root", "filePath", "workspaceDirectory", "projectRootPath", "target"];

/// `folder` without its trailing slash (`/` stays itself).
fn normal(folder: &str) -> &str {
    let f = folder.trim();
    if f.len() > 1 { f.trim_end_matches('/') } else { f }
}

/// Whether `path` (absolute) is `folder` or in it, with no `..` to climb
/// out.
pub(crate) fn inside(path: &str, folder: &str) -> bool {
    let (path, folder) = (path.trim(), normal(folder));
    if folder.is_empty() || path.split('/').any(|c| c == "..") {
        return false;
    }
    let path = if path.len() > 1 { path.trim_end_matches('/') } else { path };
    path == folder || folder == "/" || path.strip_prefix(folder).is_some_and(|rest| rest.starts_with('/'))
}

/// The path values of `v` (the keys of [`PATH_KEYS`], at any depth).
fn paths(v: &Value, out: &mut Vec<String>, depth: u32) {
    if depth > 12 {
        return;
    }
    match v {
        Value::Object(map) => {
            for (k, x) in map {
                if PATH_KEYS.contains(&k.as_str()) {
                    match x {
                        Value::String(s) => out.push(s.clone()),
                        Value::Array(list) => out.extend(list.iter().filter_map(Value::as_str).map(str::to_owned)),
                        _ => {}
                    }
                }
                paths(x, out, depth + 1);
            }
        }
        Value::Array(list) => list.iter().for_each(|x| paths(x, out, depth + 1)),
        _ => {}
    }
}

/// Whether a path named in a request is allowed for `folder`: an absolute
/// one inside it, or a relative one (to a working directory checked on
/// its own) that does not climb out.
fn allowed(path: &str, folder: &str) -> bool {
    let p = path.trim();
    if p.is_empty() {
        return true;
    }
    if p.starts_with('/') {
        return inside(p, folder);
    }
    // `~` and the like: not inside, as far as this can tell.
    !p.starts_with('~') && !p.split('/').any(|c| c == "..")
}

/// The answer to a device's request that names a path outside `folder`,
/// as the service's own errors come (`rpc_error`, by the request's id).
/// `None`: it passes.
pub(crate) fn refuse(text: &str, folder: &str) -> Option<String> {
    let v: Value = serde_json::from_str(text).ok()?;
    let message = v.get("message").filter(|_| v.get("type").and_then(Value::as_str) == Some("session")).unwrap_or(&v);
    let mut named = vec![];
    paths(message, &mut named, 0);
    let outside = named.iter().find(|p| !allowed(p, folder))?;
    log::info!("a device kept to {folder} asked for {outside}: refused");
    let error = format!("This device can only use {}", normal(folder));
    if v.get("type").and_then(Value::as_str) == Some("session") {
        let id = message.get("requestId").cloned().unwrap_or(Value::Null);
        Some(json!({ "type": "session", "message": { "type": "rpc_error", "payload": { "requestId": id, "error": error } } }).to_string())
    } else {
        // One of Sophon's own requests (`sophon_…`): no answer to wait on.
        Some(json!({ "type": "session", "message": { "type": "rpc_error", "payload": { "error": error } } }).to_string())
    }
}

/// The lists of the service's answers and what keeps an item in them:
/// (message type, the payload's list, the item's path).
const LISTS: [(&str, &str, &[&str]); 3] = [
    ("project.list.response", "projects", &["rootPath"]),
    ("fetch_workspaces_response", "entries", &["workspaceDirectory", "projectRootPath"]),
    ("fetch_agents_response", "entries", &["cwd"]),
];

/// The path an item of a list is kept by: the first of `keys` it has, in
/// it or in its `agent`.
fn item_path(item: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| item.get(*k).or_else(|| item.get("agent").and_then(|a| a.get(*k))).and_then(Value::as_str).map(str::to_owned))
}

/// A service frame for a device kept to `folder`: the lists without what
/// lies outside it, any other frame about something outside it dropped
/// (`None`). A frame naming no path passes as it is.
pub(crate) fn scope(text: &str, folder: &str) -> Option<String> {
    let Ok(mut v) = serde_json::from_str::<Value>(text) else { return Some(text.to_owned()) };
    let kind = v.pointer("/message/type").and_then(Value::as_str).unwrap_or_default().to_owned();
    if let Some((_, list, keys)) = LISTS.iter().find(|(k, ..)| *k == kind) {
        if let Some(items) = v.pointer_mut(&format!("/message/payload/{list}")).and_then(Value::as_array_mut) {
            items.retain(|item| item_path(item, keys).is_some_and(|p| inside(&p, folder)));
        }
        return Some(v.to_string());
    }
    let mut named = vec![];
    paths(v.get("message").unwrap_or(&v), &mut named, 0);
    // Absolute paths only: a relative one is to a place already checked.
    if named.iter().any(|p| p.starts_with('/') && !inside(p, folder)) {
        return None;
    }
    Some(text.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const F: &str = "/Users/me/app";

    #[test]
    fn inside_is_the_folder_and_below_it_with_no_climbing() {
        assert!(inside("/Users/me/app", F));
        assert!(inside("/Users/me/app/src/main.rs", F));
        assert!(inside("/Users/me/app/", "/Users/me/app/"));
        assert!(!inside("/Users/me/apple", F), "a sibling with the same start");
        assert!(!inside("/Users/me", F));
        assert!(!inside("/Users/me/app/../secrets", F));
        assert!(inside("/anything", "/"));
    }

    #[test]
    fn a_request_outside_the_folder_is_refused_by_its_id() {
        let req = |msg: Value| json!({ "type": "session", "message": msg }).to_string();
        let create = req(json!({ "type": "create_agent_request", "requestId": "r1", "config": { "cwd": "/Users/me/other" } }));
        let answer: Value = serde_json::from_str(&refuse(&create, F).unwrap()).unwrap();
        assert_eq!((answer["message"]["type"].as_str(), answer["message"]["payload"]["requestId"].as_str()), (Some("rpc_error"), Some("r1")));
        assert!(answer["message"]["payload"]["error"].as_str().unwrap().contains(F));
        // Inside, or no path: it passes.
        assert_eq!(refuse(&req(json!({ "type": "create_agent_request", "requestId": "r2", "config": { "cwd": "/Users/me/app/web" } })), F), None);
        assert_eq!(refuse(&req(json!({ "type": "fetch_agents_request", "requestId": "r3" })), F), None);
        // A file relative to a working directory inside: passes; climbing out
        // or from home: refused.
        assert_eq!(refuse(&req(json!({ "type": "read_file_request", "cwd": F, "path": "src/a.rs" })), F), None);
        assert!(refuse(&req(json!({ "type": "read_file_request", "cwd": F, "path": "../../.ssh/id_rsa" })), F).is_some());
        assert!(refuse(&req(json!({ "type": "list_directory_request", "path": "~/Documents" })), F).is_some());
        // Sophon's own requests too.
        assert!(refuse(&json!({ "type": "sophon_pictures_request", "cwd": "/etc" }).to_string(), F).is_some());
    }

    #[test]
    fn the_service_s_lists_keep_what_is_in_the_folder() {
        let frame = |kind: &str, payload: Value| json!({ "type": "session", "message": { "type": kind, "payload": payload } }).to_string();
        let agents = frame("fetch_agents_response", json!({ "entries": [
            { "agent": { "id": "a", "cwd": "/Users/me/app" } },
            { "agent": { "id": "b", "cwd": "/Users/me/other" } },
            { "agent": { "id": "c", "cwd": "/Users/me/app/web" } },
        ] }));
        let kept: Value = serde_json::from_str(&scope(&agents, F).unwrap()).unwrap();
        let ids: Vec<&str> = kept["message"]["payload"]["entries"].as_array().unwrap().iter().map(|e| e["agent"]["id"].as_str().unwrap()).collect();
        assert_eq!(ids, ["a", "c"]);
        let projects = frame("project.list.response", json!({ "projects": [{ "rootPath": "/Users/me" }, { "rootPath": "/Users/me/app" }] }));
        let kept: Value = serde_json::from_str(&scope(&projects, F).unwrap()).unwrap();
        assert_eq!(kept["message"]["payload"]["projects"].as_array().unwrap().len(), 1);
        // An update about a conversation elsewhere: dropped; here, or of no
        // place: passes.
        assert_eq!(scope(&frame("agent_update", json!({ "agent": { "id": "b", "cwd": "/Users/me/other" } })), F), None);
        assert!(scope(&frame("agent_update", json!({ "agent": { "id": "a", "cwd": "/Users/me/app" } })), F).is_some());
        assert!(scope(&frame("pong", json!({})), F).is_some());
    }
}
