//! Reporting an issue to Sophon's repository on GitHub: a form in the app
//! (what kind, a title, what happened) makes an issue in one shape, with
//! the versions it ran on, and the browser opens GitHub's new-issue page
//! filled with it. The user reads it there and submits it with their own
//! account; Sophon keeps no token and sends nothing itself.

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
