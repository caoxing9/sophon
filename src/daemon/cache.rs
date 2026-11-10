//! What the host last said, kept on this device: its lists (projects,
//! workspaces, agents) and each opened conversation's last page, as the
//! host sent them. Shown at once on the next start or switch, before the
//! connection is up (through a relay that takes seconds), then brought up
//! to date: the lists are asked again whole, a conversation only for what
//! came after the page kept (`connection::timeline_requests`).
//!
//! Under `cache/<host>` in the data folder; the host's folder is named by
//! a hash of its address, which holds a pairing's key.

use serde_json::Value;
use std::path::PathBuf;
use std::sync::Mutex;

static HOST: Mutex<String> = Mutex::new(String::new());

/// The same for the same words on every run (FNV-1a).
fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// The folder name of a host: this computer's service, or a saved one.
pub fn host_key(url: Option<&str>) -> String {
    match url {
        None => "local".into(),
        Some(url) => format!("h-{:016x}", hash(url.trim())),
    }
}

/// The host whose answers are kept and read from now on.
pub fn set_host(key: String) {
    *HOST.lock().unwrap_or_else(|e| e.into_inner()) = key;
}

fn root() -> PathBuf {
    super::data_dir().join("cache")
}

fn file(name: &str) -> Option<PathBuf> {
    let host = HOST.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if host.is_empty() {
        return None;
    }
    let name: String = name.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').take(120).collect();
    (!name.is_empty()).then(|| root().join(host).join(format!("{name}.json")))
}

/// The file a conversation's last page is kept in.
pub fn timeline_name(agent_id: &str) -> String {
    format!("timeline-{agent_id}")
}

/// Keeps `payload` as the current host's `name`. Not in tests: they would
/// write into the user's folder.
pub fn save(name: &str, payload: &Value) {
    if cfg!(test) || cfg!(target_arch = "wasm32") {
        return;
    }
    let (Some(path), Ok(body)) = (file(name), serde_json::to_vec(payload)) else { return };
    // Off the connection's task: a list is hundreds of kilobytes.
    std::thread::spawn(move || {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Whole or not at all: a reader never sees half a file.
        let part = path.with_extension("part");
        if std::fs::write(&part, body).is_ok() {
            let _ = std::fs::rename(&part, &path);
        }
    });
}

/// What was kept as the current host's `name`.
pub fn load(name: &str) -> Option<Value> {
    if cfg!(test) || cfg!(target_arch = "wasm32") {
        return None;
    }
    serde_json::from_slice(&std::fs::read(file(name)?).ok()?).ok()
}

/// The files kept, over all hosts, and their bytes.
pub fn usage() -> (usize, u64) {
    fn walk(dir: &std::path::Path, total: &mut (usize, u64)) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            match entry.metadata() {
                Ok(m) if m.is_dir() => walk(&entry.path(), total),
                Ok(m) => *total = (total.0 + 1, total.1 + m.len()),
                Err(_) => {}
            }
        }
    }
    let mut total = (0, 0);
    walk(&root(), &mut total);
    total
}

/// Forgets everything kept, for every host.
pub fn clear() {
    let _ = std::fs::remove_dir_all(root());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hosts_folder_does_not_spell_its_address() {
        let key = host_key(Some("sophon://pair?key=secret"));
        assert!(key.starts_with("h-") && !key.contains("secret"));
        assert_eq!(key, host_key(Some(" sophon://pair?key=secret ")));
        assert_eq!(host_key(None), "local");
    }
}
