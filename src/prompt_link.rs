//! Prompt links: `sophon://prompt?text=<urlencoded>&workspace=<id>`.
//!
//! A link only fills a new draft. It never sends, and it carries no mode,
//! model or branch, so opening one cannot change how an agent runs.

/// The most characters a link puts in a draft. A prompt longer than this
/// is a document, not a link. The encoded link must also fit the system's
/// URL limit, which non-ASCII text reaches sooner.
pub(crate) const MAX_PROMPT_CHARS: usize = 4000;

/// The longest link the app is handed (`nsh`'s URL limit).
const MAX_LINK_BYTES: usize = 16 * 1024;

const PREFIX: &str = "sophon://prompt";

/// A parsed prompt link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PromptLink {
    pub(crate) text: String,
    pub(crate) workspace: Option<String>,
}

impl PromptLink {
    /// Reads `url`. `None` when it is not a prompt link or has no text.
    pub(crate) fn parse(url: &str) -> Option<Self> {
        let rest = url.trim().strip_prefix(PREFIX)?;
        // `sophon://prompt/?…` is the same link; `sophon://prompts` is not.
        let rest = rest.strip_prefix('/').unwrap_or(rest);
        let query = match rest.strip_prefix('?') {
            Some(q) => q,
            None if rest.is_empty() || rest.starts_with('#') => "",
            None => return None,
        };
        // An unencoded `#` ends the query; text with `#` in it is encoded.
        let query = query.split('#').next().unwrap_or("");
        let (mut text, mut workspace) = (None, None);
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            match key {
                "text" if text.is_none() => text = decode(value),
                "workspace" if workspace.is_none() => workspace = decode(value),
                _ => {}
            }
        }
        let text = clean(&text?);
        if text.trim().is_empty() {
            return None;
        }
        let workspace = workspace.map(|w| w.trim().to_owned()).filter(|w| !w.is_empty());
        Some(Self { text, workspace })
    }

    /// The link for `text` (and `workspace`). `None` when there is no text
    /// or it is longer than a link carries.
    pub(crate) fn format(text: &str, workspace: Option<&str>) -> Option<String> {
        let text = text.replace("\r\n", "\n");
        if text.trim().is_empty() || text.chars().count() > MAX_PROMPT_CHARS {
            return None;
        }
        let mut link = format!("{PREFIX}?text={}", encode(&text));
        if let Some(ws) = workspace.filter(|w| !w.is_empty()) {
            link.push_str("&workspace=");
            link.push_str(&encode(ws));
        }
        (link.len() <= MAX_LINK_BYTES).then_some(link)
    }
}

/// Percent-encodes everything but RFC 3986's unreserved characters, so
/// spaces, `&`, `#`, `+` and newlines survive as text.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Percent-decodes `s`. A `+` is a space, as a web form encodes it.
/// `None` for a broken escape or bytes that are not UTF-8.
fn decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = s.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// The text as a draft takes it: plain lines, no control characters but
/// tabs, at most [`MAX_PROMPT_CHARS`].
fn clean(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .take(MAX_PROMPT_CHARS)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(text: &str, ws: Option<&str>) {
        let link = PromptLink::format(text, ws).expect("a link");
        assert!(!link.chars().any(|c| c.is_whitespace() || c.is_control()), "{link}");
        let parsed = PromptLink::parse(&link).expect("parses");
        assert_eq!(parsed.text, text);
        assert_eq!(parsed.workspace.as_deref(), ws);
    }

    #[test]
    fn round_trips() {
        round_trip("Fix the failing test", None);
        round_trip("Fix the failing test", Some("ws-123"));
        round_trip("修复登录页的 bug 🐛，然后跑测试", Some("工作区"));
        round_trip("a & b = c #1 + 50% ?x=y\nnext line\twith tab", None);
        round_trip("@src/main.rs /review", Some("a&b#c"));
    }

    #[test]
    fn reads_hand_written_links() {
        let p = PromptLink::parse("sophon://prompt?text=hello+world&workspace=w1").unwrap();
        assert_eq!(p, PromptLink { text: "hello world".into(), workspace: Some("w1".into()) });
        let p = PromptLink::parse("sophon://prompt/?workspace=w1&text=hi%20there").unwrap();
        assert_eq!(p.text, "hi there");
        // A raw `#` ends the query.
        let p = PromptLink::parse("sophon://prompt?text=issue#42").unwrap();
        assert_eq!(p.text, "issue");
    }

    #[test]
    fn missing_or_broken_params() {
        assert_eq!(PromptLink::parse("sophon://prompt"), None);
        assert_eq!(PromptLink::parse("sophon://prompt?workspace=w1"), None);
        assert_eq!(PromptLink::parse("sophon://prompt?text="), None);
        assert_eq!(PromptLink::parse("sophon://prompt?text=%20%0A"), None);
        assert_eq!(PromptLink::parse("sophon://prompt?text=%E4%B8"), None);
        assert_eq!(PromptLink::parse("sophon://prompt?text=%zz"), None);
        assert_eq!(PromptLink::parse("sophon://prompts?text=hi"), None);
        assert_eq!(PromptLink::parse("sophon://pair?text=hi"), None);
        let p = PromptLink::parse("sophon://prompt?text=hi&workspace=").unwrap();
        assert_eq!(p.workspace, None);
    }

    #[test]
    fn text_is_plain_and_capped() {
        let p = PromptLink::parse("sophon://prompt?text=a%00b%1Bc%0D%0Ad").unwrap();
        assert_eq!(p.text, "abc\nd");
        let long = "字".repeat(MAX_PROMPT_CHARS + 10);
        let p = PromptLink::parse(&format!("sophon://prompt?text={}", encode(&long))).unwrap();
        assert_eq!(p.text.chars().count(), MAX_PROMPT_CHARS);
        assert_eq!(PromptLink::format(&long, None), None);
        // ASCII up to the cap fits a link; CJK escapes to 9 bytes a
        // character and hits the URL limit first.
        assert!(PromptLink::format(&"a".repeat(MAX_PROMPT_CHARS), None).is_some());
        assert!(PromptLink::format(&"字".repeat(1500), None).is_some());
        assert_eq!(PromptLink::format(&"字".repeat(2000), None), None);
        assert_eq!(PromptLink::format("  \n", None), None);
    }
}
