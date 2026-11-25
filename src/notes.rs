//! Notes on a reply: words chosen in it, each with what the user says about
//! them, sent ahead of the next message.
//!
//! As sent, the agent reads them as quotes with comments; the bubble shows
//! a "N notes" chip in their place (`split`).

/// A passage of a reply and the comment on it (empty: quoted alone).
/// A draft (its card let go with words typed) waits in the composer and
/// is not sent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Note {
    pub(crate) quote: String,
    pub(crate) comment: String,
    pub(crate) draft: bool,
}

const HEAD: &str = "Notes on your reply:\n\n";
const END: &str = "\n\n(End of notes)";

/// `message` with `notes` ahead of it.
pub(crate) fn with_notes(notes: &[Note], message: &str) -> String {
    let notes: Vec<&Note> = notes.iter().filter(|n| !n.draft).collect();
    if notes.is_empty() {
        return message.to_owned();
    }
    let blocks: Vec<String> = notes
        .iter()
        .map(|n| {
            let quote: Vec<String> = n.quote.trim().lines().map(|l| if l.trim().is_empty() { ">".to_owned() } else { format!("> {l}") }).collect();
            let comment = n.comment.trim();
            if comment.is_empty() { quote.join("\n") } else { format!("{}\n{comment}", quote.join("\n")) }
        })
        .collect();
    let message = message.trim();
    let mut out = format!("{HEAD}{}{END}", blocks.join("\n\n"));
    if !message.is_empty() {
        out.push_str("\n\n");
        out.push_str(message);
    }
    out
}

/// A sent message's notes and the rest of it.
pub(crate) fn split(text: &str) -> (Vec<Note>, &str) {
    let Some(rest) = text.strip_prefix(HEAD) else { return (Vec::new(), text) };
    let Some(end) = rest.find(END) else { return (Vec::new(), text) };
    let mut notes: Vec<Note> = Vec::new();
    // A blank line before a quote's line: a new note's quote.
    let mut gap = true;
    for line in rest[..end].lines() {
        let quoted = line.strip_prefix("> ").or_else(|| (line == ">").then_some(""));
        match (quoted, notes.last_mut()) {
            (Some(q), Some(n)) if !gap && n.comment.is_empty() => {
                n.quote.push('\n');
                n.quote.push_str(q);
            }
            (Some(q), _) => notes.push(Note { quote: q.to_owned(), comment: String::new(), draft: false }),
            (None, Some(n)) => {
                if !n.comment.is_empty() || !line.is_empty() {
                    if !n.comment.is_empty() {
                        n.comment.push('\n');
                    }
                    n.comment.push_str(line);
                }
            }
            (None, None) => {}
        }
        gap = line.is_empty();
    }
    for n in &mut notes {
        n.comment = n.comment.trim().to_owned();
    }
    (notes, rest[end + END.len()..].trim_start_matches('\n'))
}

/// A message as its words: without the notes ahead of it.
pub(crate) fn body(text: &str) -> &str {
    split(text).1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(q: &str, c: &str) -> Note {
        Note { quote: q.into(), comment: c.into(), draft: false }
    }

    #[test]
    fn notes_go_ahead_and_come_back() {
        let notes = vec![note("截图占页面的 2/3", "竖长的截图放右边吧"), note("两行\n引用", ""), note("x", "多行\n\n评论")];
        let sent = with_notes(&notes, "顺便把字体也换了");
        assert!(sent.starts_with("Notes on your reply:\n\n> 截图占页面的 2/3\n竖长的截图放右边吧\n\n> 两行\n> 引用\n\n> x\n多行\n\n评论\n\n(End of notes)\n\n顺便"), "{sent}");
        let (back, body) = split(&sent);
        assert_eq!(back, notes);
        assert_eq!(body, "顺便把字体也换了");
    }

    #[test]
    fn only_notes_or_none() {
        let sent = with_notes(&[note("a", "b")], "  ");
        assert_eq!(split(&sent), (vec![note("a", "b")], ""));
        assert_eq!(split("hello"), (vec![], "hello"));
        assert_eq!(with_notes(&[], "hi"), "hi");
        // Drafts stay behind.
        let draft = Note { draft: true, ..note("d", "e") };
        assert_eq!(with_notes(&[draft.clone()], "hi"), "hi");
        assert_eq!(split(&with_notes(&[draft, note("a", "b")], "hi")), (vec![note("a", "b")], "hi"));
        assert_eq!(body("Notes on your reply:\n\nno end"), "Notes on your reply:\n\nno end");
    }
}
