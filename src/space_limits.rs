//! A shared space's monthly spending cap. A space given to someone else
//! (its own Paseo home, `spaces.rs`) can be capped in dollars a calendar
//! month; past it, the devices that reach the space through this Mac's
//! bridge can no longer send messages or start conversations there (the
//! bridge answers them so, `bridge.rs`). Reading stays, and so does the
//! space on this Mac itself.
//!
//! The spend is the cost (at API list prices, as the usage page counts)
//! of the space's conversations this month: their sessions, named in the
//! space's agent records (`<home>/agents/**/<id>.json`,
//! `persistence.sessionId`), found in the agents' logs by the usage scan
//! (`usage::scan::month_cost_of`). It is measured every five minutes.

// A phone has no spaces: what measures and caps them is the computer's.
#![cfg_attr(any(target_os = "ios", target_os = "android", target_arch = "wasm32"), allow(dead_code))]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use serde_json::{Value, json};

/// How often the spend is measured again.
const EVERY: Duration = Duration::from_secs(5 * 60);

/// A capped space's month so far.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct Spend {
    /// USD this calendar month.
    pub(crate) spent: f64,
    /// USD a month.
    pub(crate) cap: f64,
}

impl Spend {
    pub(crate) fn over(&self) -> bool {
        self.spent >= self.cap
    }
}

/// The last measure, by space home.
static MEASURED: Mutex<Option<HashMap<String, Spend>>> = Mutex::new(None);

fn caps_path() -> PathBuf {
    crate::daemon::data_dir().join("space-limits.json")
}

/// The caps (USD a month) by space home.
pub(crate) fn caps() -> HashMap<String, f64> {
    std::fs::read_to_string(caps_path())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v.as_object().cloned())
        .into_iter()
        .flatten()
        .filter_map(|(home, cap)| Some((home, cap.as_f64().filter(|c| *c > 0.0)?)))
        .collect()
}

/// Caps space `home` at `cap` USD a month; `None` lifts it. Measured again
/// at once (on a thread of its own).
pub(crate) fn set_cap(home: &str, cap: Option<f64>) {
    let mut all = caps();
    match cap.filter(|c| c.is_finite() && *c > 0.0) {
        Some(c) => {
            all.insert(home.to_owned(), (c * 100.0).round() / 100.0);
        }
        None => {
            all.remove(home);
        }
    }
    let v: serde_json::Map<String, Value> = all.into_iter().map(|(h, c)| (h, json!(c))).collect();
    let _ = std::fs::create_dir_all(crate::daemon::data_dir());
    let _ = std::fs::write(caps_path(), Value::Object(v).to_string());
    std::thread::spawn(measure);
}

/// The month so far of capped space `home`, as last measured.
pub(crate) fn spend(home: &str) -> Option<Spend> {
    MEASURED.lock().ok()?.as_ref()?.get(home).copied()
}

/// Whether space `home` (a shared one: `None` is this Mac's first, never
/// capped) is past its cap.
pub(crate) fn over(space: Option<&str>) -> bool {
    space.and_then(spend).is_some_and(|s| s.over())
}

/// The sessions of space `home`'s conversations, as its agent records
/// name them.
pub(crate) fn session_ids(home: &Path) -> Vec<String> {
    let mut out = vec![];
    let mut stack = vec![home.join("agents")];
    while let Some(dir) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else { continue };
        for entry in read.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "json")
                && let Some(id) = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    .and_then(|v| v.pointer("/persistence/sessionId").and_then(Value::as_str).map(str::to_owned))
                    .filter(|id| id.len() >= 8)
            {
                out.push(id);
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Measures every capped space's month (blocking: it reads the logs).
pub(crate) fn measure() {
    let caps = caps();
    if caps.is_empty() {
        if let Ok(mut m) = MEASURED.lock() {
            *m = Some(HashMap::new());
        }
        return;
    }
    // The logs as they are now (the scan keeps what it read). A phone has
    // no spaces, nor logs.
    #[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
    let _ = crate::usage::scan::scan();
    let measured: HashMap<String, Spend> = caps
        .into_iter()
        .map(|(home, cap)| {
            #[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
            let spent = crate::usage::scan::month_cost_of(&session_ids(Path::new(&home)));
            #[cfg(any(target_os = "ios", target_os = "android", target_arch = "wasm32"))]
            let spent = 0.0;
            (home, Spend { spent, cap })
        })
        .collect();
    for (home, s) in &measured {
        if s.over() {
            log::info!("space {home}: ${:.2} of ${:.2} this month: capped", s.spent, s.cap);
        }
    }
    if let Ok(mut m) = MEASURED.lock() {
        *m = Some(measured);
    }
}

/// Measures now and every five minutes after, for as long as the app runs.
pub(crate) fn start() {
    std::thread::spawn(|| {
        loop {
            measure();
            std::thread::sleep(EVERY);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_space_s_sessions_are_its_agents_records() {
        let home = std::env::temp_dir().join(format!("sophon-space-{}", uuid::Uuid::new_v4()));
        let dir = home.join("agents").join("Users-me-project");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.json"), json!({ "id": "a", "persistence": { "sessionId": "b5376a2f-fb68-4d23" } }).to_string()).unwrap();
        std::fs::write(dir.join("b.json"), json!({ "id": "b", "persistence": { "sessionId": "b5376a2f-fb68-4d23" } }).to_string()).unwrap();
        std::fs::write(dir.join("c.json"), json!({ "id": "c" }).to_string()).unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        assert_eq!(session_ids(&home), vec!["b5376a2f-fb68-4d23".to_owned()]);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn a_space_is_over_from_its_cap_on() {
        assert!(!Spend { spent: 49.99, cap: 50.0 }.over());
        assert!(Spend { spent: 50.0, cap: 50.0 }.over());
        // This Mac's first space is never capped.
        assert!(!over(None));
    }
}
