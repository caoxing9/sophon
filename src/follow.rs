//! Following a conversation that goes on outside Sophon.
//!
//! A session imported from Claude Code or Codex may still be open in the
//! terminal it came from: the agent there keeps writing its record
//! (`projects/<folder>/<session>.jsonl`, `sessions/**/rollout-…-<session>.jsonl`),
//! and the service, which read the record once, shows none of it.
//!
//! So the Mac the records are on watches them. Every [`EVERY`] it looks at
//! the record of each conversation that is idle in the service; one that
//! has changed since it was last looked at was written by someone else
//! (the service writes only while the conversation runs), and the service
//! is asked to read it again (`refresh_agent_request`: for an idle
//! conversation it starts no process and takes a fraction of a second).
//! What it reads goes to every device following the conversation, so a
//! phone sees it too while this Mac's Sophon is open.
//!
//! A conversation running in the service is left alone (reading it again
//! would stop it). The first look at a record, as Sophon starts or after
//! the conversation stops, is where the comparison starts from; a record
//! written well after the service last heard of the conversation was
//! written while nobody watched, and is read again then too.

use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Mutex,
    time::{Duration, Instant, SystemTime},
};

use nsh_core::prelude::*;

use crate::{
    store,
    usage::{Source, scan::roots},
};

/// How often the records are looked at.
const EVERY: Duration = Duration::from_secs(2);
/// A record written this long after the service's last word of its
/// conversation was written by someone else (the service's own writing
/// ends with that word, give or take the time to flush).
const LATER: Duration = Duration::from_secs(5);
/// How long before a record not found is looked for again.
const LOOK_AGAIN: Duration = Duration::from_secs(60);

/// A conversation to look at: its id, agent, folder and the session's id
/// in the agent's own records.
struct Watched {
    id: String,
    provider: String,
    cwd: String,
    session: String,
    /// When the service last heard of it.
    updated: Option<SystemTime>,
}

/// A record as last seen: its length and when it was written.
#[derive(Clone, Copy, PartialEq)]
struct Stamp(u64, Option<SystemTime>);

#[derive(Default)]
struct State {
    /// Each watched conversation's record as last seen.
    seen: HashMap<String, Stamp>,
    /// Where each session's record is (`None`: not found when looked for).
    paths: HashMap<String, (Option<PathBuf>, Instant)>,
    /// A look is under way.
    busy: bool,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

/// Starts watching (desktops; nothing happens until this Mac's own
/// service is the one in use).
pub(crate) fn start() {
    if crate::ui::kit::MOBILE || cfg!(target_arch = "wasm32") {
        return;
    }
    again();
}

fn again() {
    set_timeout(EVERY, || {
        look();
        again();
    });
}

/// The record of `session` among the agent's records.
fn record(provider: &str, cwd: &str, session: &str) -> Option<PathBuf> {
    let source = match provider {
        "claude" => Source::Claude,
        "codex" => Source::Codex,
        _ => return None,
    };
    for (_, root) in roots().into_iter().filter(|(s, _)| *s == source) {
        if source == Source::Claude {
            // Claude Code names a folder's conversations by its path, each
            // record by its session.
            let folder: String = cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
            let path = root.join(folder).join(format!("{session}.jsonl"));
            if path.is_file() {
                return Some(path);
            }
        } else {
            // Codex files them by day, the session's id ending the name.
            let mut files = vec![];
            crate::usage::scan::walk(&root, source, SystemTime::UNIX_EPOCH, 0, &mut files);
            let wanted = format!("{session}.jsonl");
            if let Some(found) = files.into_iter().map(|f| f.0).find(|p| p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(&wanted))) {
                return Some(found);
            }
        }
    }
    None
}

/// The conversations whose records changed since they were last seen.
/// Reads the disk: off the thread that draws.
fn changed(watched: &[Watched]) -> Vec<String> {
    let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let state = guard.get_or_insert_with(State::default);
    // What is no longer watched (running in the service, archived, gone)
    // starts over when it is watched again.
    state.seen.retain(|id, _| watched.iter().any(|w| &w.id == id));
    let mut out = vec![];
    for w in watched {
        let known = state.paths.get(&w.session).filter(|(path, at)| path.is_some() || at.elapsed() < LOOK_AGAIN).map(|(p, _)| p.clone());
        let path = match known {
            Some(path) => path,
            None => {
                let path = record(&w.provider, &w.cwd, &w.session);
                state.paths.insert(w.session.clone(), (path.clone(), Instant::now()));
                path
            }
        };
        let Some(meta) = path.and_then(|p| std::fs::metadata(p).ok()) else { continue };
        let now = Stamp(meta.len(), meta.modified().ok());
        let written_unwatched = || matches!((now.1, w.updated), (Some(written), Some(updated)) if written > updated + LATER);
        match state.seen.insert(w.id.clone(), now) {
            Some(before) if before != now => out.push(w.id.clone()),
            None if written_unwatched() => out.push(w.id.clone()),
            _ => {}
        }
    }
    out
}

/// Whether the service may read this conversation's record again: it is
/// there, at rest, and asks nothing of its owner.
fn at_rest(a: &crate::daemon::Agent) -> bool {
    !a.archived && a.status == "idle" && a.pending.is_empty() && a.session_id.is_some()
}

fn look() {
    let connected = matches!(store::conn(), store::Conn::Connected { .. });
    if !connected || !store::on_this_mac() {
        // Another computer's records are not here.
        if let Ok(mut state) = STATE.lock() {
            *state = None;
        }
        return;
    }
    let watched: Vec<Watched> = store::agents()
        .into_iter()
        .filter(at_rest)
        .filter_map(|a| {
            let updated = chrono::DateTime::parse_from_rfc3339(&a.updated_at).ok().map(SystemTime::from);
            Some(Watched { session: a.session_id.clone()?, id: a.id, provider: a.provider, cwd: a.cwd, updated })
        })
        .collect();
    {
        let mut guard = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let state = guard.get_or_insert_with(State::default);
        if state.busy {
            return;
        }
        state.busy = true;
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Vec<String>>();
    std::thread::spawn(move || {
        let ids = changed(&watched);
        if let Some(state) = STATE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
            state.busy = false;
        }
        let _ = tx.send(ids);
    });
    store::spawn_detached(async move {
        for id in rx.recv().await.unwrap_or_default() {
            // Still at rest: one that began to run meanwhile is the
            // service's to write.
            if store::agent(&id).is_some_and(|a| at_rest(&a)) {
                store::reload_agent(&id);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watched(id: &str, session: &str) -> Watched {
        Watched { id: id.into(), provider: "claude".into(), cwd: "/x".into(), session: session.into(), updated: None }
    }

    #[test]
    fn a_record_written_since_the_last_look_is_reported_once() {
        let dir = std::env::temp_dir().join(format!("sophon-follow-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("s1.jsonl");
        std::fs::write(&file, "a\n").unwrap();
        *STATE.lock().unwrap() = Some(State::default());
        STATE.lock().unwrap().as_mut().unwrap().paths.insert("s1".into(), (Some(file.clone()), Instant::now()));
        let list = [watched("a1", "s1")];
        // The first look is where the comparison starts.
        assert!(changed(&list).is_empty());
        assert!(changed(&list).is_empty());
        std::fs::write(&file, "a\nb\n").unwrap();
        assert_eq!(changed(&list), vec!["a1".to_owned()]);
        assert!(changed(&list).is_empty());
        // Not watched for a while (it ran in the service): its writing
        // then is the service's own, and the next look starts over.
        assert!(changed(&[]).is_empty());
        std::fs::write(&file, "a\nb\nc\n").unwrap();
        assert!(changed(&list).is_empty());
        // First seen, and written long after the service last heard of
        // it: nobody watched then.
        *STATE.lock().unwrap() = Some(State::default());
        STATE.lock().unwrap().as_mut().unwrap().paths.insert("s1".into(), (Some(file.clone()), Instant::now()));
        let mut stale = watched("a1", "s1");
        stale.updated = Some(SystemTime::now() - Duration::from_secs(3600));
        assert_eq!(changed(&[stale]), vec!["a1".to_owned()]);
        let mut fresh = watched("a2", "s1");
        fresh.updated = Some(SystemTime::now());
        assert!(changed(&[fresh]).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
