//! Push notifications to paired phones (`docs/push.md`), sent from here:
//! the service watches its agents whether the app is open or not.
//!
//! The app says who wants them (`push.json` in this home, which it keeps
//! as phones register over its bridge); this says what happened
//! ([`Pusher::note`]) and sends it through the gateway, and takes the
//! actions a phone presses on a notification (an approval answered, a
//! reply sent) as a client's request would be.
//!
//! What is said, and when:
//! - an approval or a question: at once;
//! - a turn that finished or failed: five seconds later, unless another
//!   turn began meanwhile (a follow-up queued) — and not while someone is
//!   at this Mac using Sophon (its window in front, the Mac used within
//!   five minutes): the app says so ([`Pusher::presence`]);
//! - a turn the user stopped, or one before this service started: nothing;
//! - several turns done together: one push for them all.

use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};

use serde_json::{Value, json};
use sophon_push::{Act, Kind, Note, ServiceFile, gateway};
use tokio::sync::mpsc;

use crate::host::Shared;

/// How long a turn's end waits before it is said: a follow-up that begins
/// meanwhile makes it no news.
const CONFIRM: Duration = Duration::from_secs(5);
/// How long the app's word that it is in front holds without another.
const PRESENT_FOR: Duration = Duration::from_secs(90);
/// How long without a key or the pointer before the Mac is left (seconds).
const AWAY_AFTER: f64 = 300.0;

pub struct Pusher {
    home: PathBuf,
    file: ServiceFile,
    read_at: Option<SystemTime>,
    /// To the gateway's socket, once it runs.
    tx: Option<mpsc::UnboundedSender<Value>>,
    /// Turns' ends waiting to be said, with when.
    waiting: Vec<(Instant, Note)>,
    /// Until when the app said it is in front.
    present_until: Option<Instant>,
}

impl Pusher {
    pub fn new(home: PathBuf) -> Self {
        let mut pusher = Self { home, file: ServiceFile::default(), read_at: None, tx: None, waiting: Vec::new(), present_until: None };
        pusher.reload();
        pusher
    }

    /// The app's `push.json`, read again when it changed.
    fn reload(&mut self) {
        let modified = std::fs::metadata(ServiceFile::path(&self.home)).and_then(|m| m.modified()).ok();
        if modified != self.read_at {
            self.read_at = modified;
            self.file = ServiceFile::load(&self.home);
        }
    }

    /// Something happened worth telling: an approval or a question goes at
    /// once, a turn's end after [`CONFIRM`].
    pub fn note(&mut self, note: Note) {
        if self.file.devices.is_empty() {
            return;
        }
        match note.kind {
            Kind::Permission | Kind::Question => self.send(&[note]),
            _ => {
                self.waiting.retain(|(_, n)| n.agent != note.agent);
                self.waiting.push((Instant::now() + CONFIRM, note));
            }
        }
    }

    /// A turn of `agent` began: its last one's end is no news.
    pub fn began(&mut self, agent: &str) {
        self.waiting.retain(|(_, n)| n.agent != agent);
    }

    /// The app is in front (`true`, said again every half minute while it
    /// is) or not.
    pub fn presence(&mut self, front: bool) {
        self.present_until = front.then(|| Instant::now() + PRESENT_FOR);
    }

    /// Someone is at this Mac using Sophon: its window in front, and the
    /// Mac used lately.
    fn in_use(&self) -> bool {
        self.present_until.is_some_and(|until| until > Instant::now()) && idle_secs().is_none_or(|idle| idle < AWAY_AFTER)
    }

    /// Each second: the app's record read again if it changed; the turns'
    /// ends that are due said, together.
    pub fn tick(&mut self) {
        self.reload();
        let now = Instant::now();
        if !self.waiting.iter().any(|(at, _)| *at <= now) {
            return;
        }
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.waiting).into_iter().partition(|(at, _)| *at <= now);
        self.waiting = later;
        if self.in_use() {
            return;
        }
        let notes: Vec<Note> = due.into_iter().map(|(_, n)| n).collect();
        self.send(&notes);
    }

    /// The plan's limit is near (the app watches the plans).
    pub fn usage(&self, label: &str, percent: u32, ends: Option<&str>) {
        let Some(tx) = &self.tx else { return };
        for wants in self.file.devices.iter().filter(|w| w.events.usage) {
            let _ = tx.send(sophon_push::usage_push(&self.file.id, label, percent, ends, wants));
        }
    }

    fn send(&self, notes: &[Note]) {
        let Some(tx) = &self.tx else { return };
        for wants in &self.file.devices {
            let wanted: Vec<Note> = notes.iter().filter(|n| wants.events.wants(n.kind)).cloned().collect();
            if let Some(push) = sophon_push::notes_push(&self.file.id, &wanted, wants) {
                let _ = tx.send(push);
            }
        }
    }
}

/// Seconds since a key or the pointer was last used on this Mac.
#[cfg(target_os = "macos")]
fn idle_secs() -> Option<f64> {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    }
    // The combined session's state, any input event.
    // SAFETY: a plain C call with no pointers.
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) };
    secs.is_finite().then_some(secs)
}

#[cfg(not(target_os = "macos"))]
fn idle_secs() -> Option<f64> {
    None
}

/// The gateway's socket, for as long as the service runs: what the app's
/// record names, read again at each connection; actions taken as a
/// client's requests.
pub fn start(shared: &Shared) {
    let (tx, rx) = mpsc::unbounded_channel();
    shared.borrow_mut().push.tx = Some(tx);
    let (s1, s2, s3, s4, s5) = (shared.clone(), shared.clone(), shared.clone(), shared.clone(), shared.clone());
    tokio::task::spawn_local(async move {
        gateway::computer_loop(
            move || {
                let host = s1.borrow();
                let f = &host.push.file;
                (!f.gateway.is_empty() && !f.id.is_empty() && !f.devices.is_empty()).then(|| gateway::Target { endpoint: f.gateway.clone(), tls: f.tls, id: f.id.clone() })
            },
            move |gw, key| sophon_push::trust_gateway(&s2.borrow().home, gw, key),
            move || s3.borrow().push.file.secret,
            move || s4.borrow().push.file.tokens.clone(),
            rx,
            move |token, act| take(&s5, token, act),
        )
        .await;
    });
}

/// An action pressed on a notification, by the device with `token`.
fn take(shared: &Shared, token: &str, act: Act) {
    let request = match act {
        Act::Permission { agent, permission, allow } => {
            // Answered already, here or on another device: nothing to do.
            let waiting = shared.borrow().state.agents.get(&agent).is_some_and(|record| {
                record["pendingPermissions"].as_array().is_some_and(|asked| asked.iter().any(|p| p["id"] == permission.as_str()))
            });
            if !waiting {
                return;
            }
            let mut response = json!({ "behavior": if allow { "allow" } else { "deny" } });
            if !allow {
                response["message"] = json!("Dismissed by user");
            }
            json!({ "type": "agent_permission_response", "agentId": agent, "requestId": permission, "response": response })
        }
        Act::Reply { agent, text } => {
            if !shared.borrow().state.agents.contains_key(&agent) {
                return;
            }
            json!({ "type": "send_agent_message_request", "agentId": agent, "text": text.trim(), "messageId": crate::state::id(), "requestId": crate::state::id() })
        }
        Act::Forget { device } => {
            // Off the list here at once; the app takes it off its record and
            // revokes the device's pairing when it next looks.
            let mut host = shared.borrow_mut();
            let home = host.home.clone();
            sophon_push::note_forgotten(&home, &device, token);
            host.push.file.devices.retain(|w| w.pairing != token);
            return;
        }
    };
    if let Err(error) = crate::host::dispatch(shared, &request) {
        eprintln!("A notification's action: {error}");
    }
}

/// What an agent's record says of it for a notification.
pub fn note_of(state: &crate::state::State, agent: &str, kind: Kind, detail: String, permission: Option<String>) -> Option<Note> {
    let record = state.agents.get(agent)?;
    let project = record["workspaceId"]
        .as_str()
        .and_then(|ws| state.workspaces.get(ws))
        .and_then(|ws| ws["name"].as_str().or(ws["projectDisplayName"].as_str()))
        .unwrap_or_default()
        .to_owned();
    Some(Note { kind, agent: agent.to_owned(), title: record["title"].as_str().unwrap_or_default().to_owned(), project, detail, permission })
}

/// An approval's or a question's words: the question asked, else what it
/// is about (the command, the file, what is searched or fetched; a tool's
/// asking has no title of its own: a client says it from its detail), else
/// its title or the tool's name.
pub fn asked_words(request: &Value) -> (Kind, String) {
    let text = |path: &str| request.pointer(path).and_then(Value::as_str).filter(|s| !s.trim().is_empty());
    if request["kind"] == "question" {
        let q = text("/input/questions/0/question").or(text("/title")).unwrap_or_default();
        return (Kind::Question, q.to_owned());
    }
    let what = ["/detail/command", "/input/command", "/detail/filePath", "/input/file_path", "/detail/query", "/detail/url", "/title", "/description", "/name"]
        .into_iter()
        .find_map(text)
        .unwrap_or_default();
    (Kind::Permission, what.to_owned())
}

/// Whether an agent's turns are anybody's news: not an automation's run
/// that finished as it should (a failed one is), not an agent another
/// started.
pub fn quiet(record: &Value, kind: Kind) -> bool {
    let scheduled = record.pointer("/labels/sophon.schedule-id").is_some_and(|v| !v.is_null());
    !record["parentAgentId"].is_null() || (scheduled && kind == Kind::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(agent: &str, kind: Kind) -> Note {
        Note { kind, agent: agent.into(), title: "t".into(), project: "p".into(), detail: String::new(), permission: None }
    }

    fn pusher() -> (Pusher, mpsc::UnboundedReceiver<Value>) {
        let mut p = Pusher::new(std::env::temp_dir().join("sophon-push-none"));
        p.file = ServiceFile { id: "mac1".into(), devices: vec![sophon_push::Wants { token: "t".into(), ..Default::default() }], ..Default::default() };
        // No file there: nothing to read again over this.
        p.read_at = None;
        let (tx, rx) = mpsc::unbounded_channel();
        p.tx = Some(tx);
        (p, rx)
    }

    #[test]
    fn an_approval_goes_at_once_and_a_turn_s_end_after_a_moment() {
        let (mut p, mut rx) = pusher();
        p.note(note("a", Kind::Permission));
        assert_eq!(rx.try_recv().unwrap()["category"], "PERMISSION");
        p.note(note("a", Kind::Finished));
        p.tick();
        assert!(rx.try_recv().is_err(), "not yet");
        p.waiting[0].0 = Instant::now();
        p.tick();
        assert_eq!(rx.try_recv().unwrap()["category"], "FINISHED");
    }

    #[test]
    fn a_follow_up_makes_a_turn_s_end_no_news() {
        let (mut p, mut rx) = pusher();
        p.note(note("a", Kind::Finished));
        p.began("a");
        for (at, _) in &mut p.waiting {
            *at = Instant::now();
        }
        p.tick();
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn turns_done_together_are_one_push() {
        let (mut p, mut rx) = pusher();
        p.note(note("a", Kind::Finished));
        p.note(note("b", Kind::Failed));
        for (at, _) in &mut p.waiting {
            *at = Instant::now();
        }
        p.tick();
        let push = rx.try_recv().unwrap();
        assert_eq!(push["thread"], "finished");
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn nothing_for_a_device_that_turned_it_off_nor_with_nobody_to_tell() {
        let (mut p, mut rx) = pusher();
        p.file.devices[0].events.finished = false;
        p.note(note("a", Kind::Finished));
        p.waiting[0].0 = Instant::now();
        p.tick();
        assert!(rx.try_recv().is_err());
        p.file.devices.clear();
        p.note(note("a", Kind::Permission));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn an_automation_s_run_is_news_only_when_it_failed() {
        let scheduled = json!({ "labels": { "sophon.schedule-id": "s1" }, "parentAgentId": null });
        assert!(quiet(&scheduled, Kind::Finished));
        assert!(!quiet(&scheduled, Kind::Failed));
        assert!(quiet(&json!({ "parentAgentId": "p" }), Kind::Permission));
        assert!(!quiet(&json!({ "parentAgentId": null }), Kind::Finished));
    }

    #[test]
    fn a_question_says_what_it_asks() {
        let q = json!({ "kind": "question", "title": "Question", "input": { "questions": [{ "question": "Which branch?" }] } });
        assert_eq!(asked_words(&q), (Kind::Question, "Which branch?".to_owned()));
        assert_eq!(asked_words(&json!({ "kind": "tool", "title": "npm test" })), (Kind::Permission, "npm test".to_owned()));
        // A tool's asking says what it runs, from its detail.
        let bash = json!({ "kind": "tool", "title": null, "name": "Bash", "detail": { "type": "shell", "command": "npm test" } });
        assert_eq!(asked_words(&bash), (Kind::Permission, "npm test".to_owned()));
        assert_eq!(asked_words(&json!({ "kind": "tool", "name": "Edit", "detail": { "filePath": "/a/b.rs" } })).1, "/a/b.rs");
    }
}
