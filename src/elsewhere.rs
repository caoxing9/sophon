//! Conversations whose Claude Code session is open somewhere else.
//!
//! Claude Code runs a background session (`claude --bg`, a job) until it
//! is stopped, also between replies while nothing runs, and refuses to
//! resume a session that runs in the background: a conversation imported
//! from one cannot be sent to from Sophon meanwhile, and every message
//! fails. Claude Code lists its live sessions (`claude agents --json`,
//! about a tenth of a second), so this Mac looks every few seconds which
//! of its conversations' sessions run in the background, and the composer
//! says so before anything is sent, with a way to close it there
//! (`claude stop`, which keeps the conversation).

use std::{collections::HashSet, path::PathBuf};

use nsh_core::prelude::*;

use crate::store;

#[cfg(any(target_os = "macos", test))]
/// How often Claude Code's sessions are listed.
const EVERY: std::time::Duration = std::time::Duration::from_secs(5);

/// The sessions running in the background now, by their full ids.
#[derive(Clone, Default, PartialEq)]
struct Background(HashSet<String>);

/// Whether `session` (a Claude Code session's id) runs in the background
/// on this Mac.
pub(crate) fn in_background(session: &str) -> bool {
    global::<Background>().with(|b| b.0.contains(session))
}

/// The short id Claude Code names a background session by.
pub(crate) fn short(session: &str) -> String {
    session.chars().take(8).collect()
}

/// Claude Code's command line on this Mac.
fn claude() -> PathBuf {
    crate::bundled_cli::find("claude").unwrap_or_else(|| PathBuf::from("claude"))
}

#[cfg(any(target_os = "macos", test))]
/// The background sessions in `claude agents --json`'s answer.
fn background_of(json: &[u8]) -> HashSet<String> {
    let Ok(serde_json::Value::Array(list)) = serde_json::from_slice(json) else { return HashSet::new() };
    list.iter()
        .filter(|s| s.get("kind").and_then(serde_json::Value::as_str) == Some("background"))
        .filter_map(|s| s.get("sessionId").and_then(serde_json::Value::as_str).map(str::to_owned))
        .collect()
}

/// Starts looking (this Mac's Sophon only: the sessions are its own).
pub(crate) fn start() {
    #[cfg(target_os = "macos")]
    again();
}

#[cfg(target_os = "macos")]
fn again() {
    set_timeout(EVERY, || {
        look();
        again();
    });
}

#[cfg(target_os = "macos")]
fn look() {
    let wanted = matches!(store::conn(), store::Conn::Connected { .. })
        && store::on_this_mac()
        && store::agents().iter().any(|a| a.provider == "claude" && a.session_id.is_some());
    if !wanted {
        if global::<Background>().with_untracked(|b| !b.0.is_empty()) {
            global::<Background>().set(Background::default());
        }
        return;
    }
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Option<HashSet<String>>>();
    std::thread::spawn(move || {
        let cli = claude();
        let out = std::process::Command::new(&cli)
            .args(["agents", "--json"])
            .env("PATH", store::service_path(&cli))
            .stdin(std::process::Stdio::null())
            .output();
        let _ = tx.send(out.ok().filter(|o| o.status.success()).map(|o| background_of(&o.stdout)));
    });
    store::spawn_detached(async move {
        // No answer (an older Claude Code, none installed): nothing known.
        let found = rx.recv().await.flatten().unwrap_or_default();
        if global::<Background>().with_untracked(|b| b.0 != found) {
            global::<Background>().set(Background(found));
        }
    });
}

/// Stops background session `session` (`claude stop`): what it runs ends,
/// its conversation is kept and goes on in Sophon.
pub(crate) fn close(session: &str) {
    let id = short(session);
    let session = session.to_owned();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Result<(), String>>();
    std::thread::spawn(move || {
        let cli = claude();
        let out = std::process::Command::new(&cli)
            .args(["stop", &id])
            .env("PATH", store::service_path(&cli))
            .stdin(std::process::Stdio::null())
            .output();
        let _ = tx.send(match out {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(String::from_utf8_lossy(&o.stderr).lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("claude stop failed").trim().to_owned()),
            Err(e) => Err(e.to_string()),
        });
    });
    store::spawn_detached(async move {
        match rx.recv().await {
            Some(Ok(())) => {
                global::<Background>().update(|b| {
                    b.0.remove(&session);
                });
                store::notify(crate::i18n::t("Closed there: the conversation goes on here").to_owned());
            }
            Some(Err(e)) => store::notify(crate::i18n::tf("Could not close it there: {}", &[&e])),
            None => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_background_sessions_are_counted() {
        let json = br#"[
            {"pid":1,"cwd":"/a","kind":"interactive","sessionId":"aaaaaaaa-1","name":"x","status":"idle","startedAt":1},
            {"pid":2,"cwd":"/b","kind":"background","sessionId":"bbbbbbbb-2","name":"y","startedAt":2}
        ]"#;
        assert_eq!(background_of(json), HashSet::from(["bbbbbbbb-2".to_owned()]));
        assert!(background_of(b"not json").is_empty());
        assert_eq!(short("8bd78e07-ef2a-498d"), "8bd78e07");
    }
}
