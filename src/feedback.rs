//! Reporting an issue to Sophon's repository on GitHub: a form in the app
//! (what kind, a title, what happened) makes an issue in one shape, with
//! the versions it ran on. It is filed without a GitHub account by
//! Sophon's relay ([`file`], `relay/src/feedback.rs`), which holds the
//! token; the app keeps none. Who wants to follow it with their own
//! account can still open GitHub's new-issue page filled with it ([`url`]).

/// Where issues go: the repository releases come from.
pub(crate) const REPO: &str = "caoxing9/sophon";

/// What an issue is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Bug,
    Feature,
    Question,
}

impl Kind {
    /// The title's prefix and the label GitHub's own templates use.
    fn tag(self) -> (&'static str, &'static str) {
        match self {
            Kind::Bug => ("bug", "bug"),
            Kind::Feature => ("feat", "enhancement"),
            Kind::Question => ("question", "question"),
        }
    }
}

/// What the issue is written from.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Report {
    pub(crate) title: String,
    pub(crate) what: String,
    /// Bugs: how to make it happen again, and what was expected.
    pub(crate) steps: String,
    pub(crate) expected: String,
    /// The versions and the system, when the user leaves them in.
    pub(crate) environment: Option<Vec<(String, String)>>,
}

/// The issue's title: `<kind>: <title>`, as the repository's commits read.
pub(crate) fn title(kind: Kind, report: &Report) -> String {
    let t = report.title.trim();
    let t = if t.is_empty() { report.what.lines().next().unwrap_or("").trim() } else { t };
    let t: String = t.chars().take(100).collect();
    format!("{}: {t}", kind.tag().0)
}

/// The issue's body, in Markdown: the same sections every time.
pub(crate) fn body(kind: Kind, report: &Report) -> String {
    let or_none = |s: &str| if s.trim().is_empty() { "_None given_".to_owned() } else { s.trim().to_owned() };
    let mut out = String::new();
    match kind {
        Kind::Bug => {
            out += &format!("## What happened\n\n{}\n\n", or_none(&report.what));
            out += &format!("## Steps to reproduce\n\n{}\n\n", or_none(&report.steps));
            out += &format!("## Expected\n\n{}\n\n", or_none(&report.expected));
        }
        Kind::Feature => {
            out += &format!("## What it would do\n\n{}\n\n", or_none(&report.what));
            out += &format!("## Why\n\n{}\n\n", or_none(&report.steps));
        }
        Kind::Question => out += &format!("## Question\n\n{}\n\n", or_none(&report.what)),
    }
    if let Some(env) = &report.environment {
        out += "## Environment\n\n";
        for (k, v) in env {
            out += &format!("- {k}: {v}\n");
        }
        out += "\n";
    }
    out += "_Reported from Sophon_\n";
    out
}

/// Sophon's relay, which files reports for people without a GitHub
/// account (`relay/src/feedback.rs`).
const FILER: &str = "ws://47.119.154.134:8443/feedback";

/// The frame the relay files (`relay/src/feedback.rs`).
pub(crate) fn frame(kind: Kind, report: &Report) -> serde_json::Value {
    serde_json::json!({ "type": "report", "title": title(kind, report), "body": body(kind, report), "label": kind.tag().1 })
}

/// Files the issue through the relay: its page on GitHub, or why not
/// (`NotConfigured`, `BadRequest`, `RateLimited`, `Unreachable`). On a
/// thread of its own with its own runtime, at most 25 seconds.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn file(kind: Kind, report: &Report) -> Result<String, String> {
    let frame = frame(kind, report).to_string();
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let filed = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string()).and_then(|rt| {
            rt.block_on(async { tokio::time::timeout(std::time::Duration::from_secs(25), send(FILER, frame)).await.unwrap_or_else(|_| Err("Unreachable".into())) })
        });
        let _ = tx.send(filed);
    });
    rx.await.unwrap_or_else(|_| Err("Unreachable".into()))
}

#[cfg(not(target_arch = "wasm32"))]
async fn send(url: &str, frame: String) -> Result<String, String> {
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::tungstenite::Message;
    let (ws, _) = tokio_tungstenite::connect_async(url).await.map_err(|e| {
        log::info!("feedback: {e}");
        "Unreachable".to_owned()
    })?;
    let (mut sink, mut stream) = ws.split();
    sink.send(Message::Text(frame.into())).await.map_err(|_| "Unreachable".to_owned())?;
    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(t) = msg else { continue };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(t.as_str()) else { continue };
        if v.get("type").and_then(|t| t.as_str()) != Some("filed") {
            continue;
        }
        let field = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_owned);
        return match (v.get("ok").and_then(|x| x.as_bool()), field("url")) {
            (Some(true), Some(url)) => Ok(url),
            _ => Err(field("reason").unwrap_or_else(|| "Unreachable".into())),
        };
    }
    Err("Unreachable".into())
}

/// GitHub's new-issue page filled with the issue.
pub(crate) fn url(kind: Kind, report: &Report) -> String {
    format!(
        "https://github.com/{REPO}/issues/new?title={}&body={}&labels={}",
        encode(&title(kind, report)),
        encode(&body(kind, report)),
        encode(kind.tag().1)
    )
}

fn encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_relay_gets_the_same_issue_and_a_label_it_knows() {
        let r = Report { title: "Links".into(), what: "Pressed one".into(), ..Default::default() };
        let f = frame(Kind::Feature, &r);
        assert_eq!(f["type"], "report");
        assert_eq!(f["title"], "feat: Links");
        assert_eq!(f["label"], "enhancement");
        assert_eq!(f["body"].as_str(), Some(body(Kind::Feature, &r).as_str()));
    }

    #[test]
    fn a_bug_has_its_sections_and_environment() {
        let r = Report {
            title: " Links do not open ".into(),
            what: "Pressed a link".into(),
            steps: "1. Open a chat".into(),
            expected: String::new(),
            environment: Some(vec![("Sophon".into(), "0.2.14".into())]),
        };
        assert_eq!(title(Kind::Bug, &r), "bug: Links do not open");
        let b = body(Kind::Bug, &r);
        assert!(b.contains("## What happened\n\nPressed a link"));
        assert!(b.contains("## Expected\n\n_None given_"));
        assert!(b.contains("- Sophon: 0.2.14"));
        let u = url(Kind::Bug, &r);
        assert!(u.starts_with("https://github.com/caoxing9/sophon/issues/new?title=bug%3A%20Links%20do%20not%20open&body="));
        assert!(u.ends_with("&labels=bug"));
        // No title: the first line of what happened.
        let r = Report { what: "It hangs\nmore".into(), ..Default::default() };
        assert_eq!(title(Kind::Question, &r), "question: It hangs");
        assert!(!body(Kind::Question, &r).contains("Environment"));
    }
}
