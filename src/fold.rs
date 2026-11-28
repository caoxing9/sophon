//! Runs of tool calls folded into one summary row, as the conversation
//! shows them: "Ran 3 commands, edited 2 files".
//!
//! Pure: items in, display rows out. The store feeds it the timeline, the
//! agent's state and what the user opened or closed; the list draws the
//! rows it returns.

use std::collections::{HashMap, HashSet};

use crate::{
    daemon::Item,
    i18n::{t, tf},
};

/// One row of the conversation's list.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Row {
    /// The item at this index. `grouped`: it shows under an open group's
    /// summary, so it is drawn indented.
    /// `turn_end`: nothing of the agent's follows it in the turn (no
    /// call, thought or more words): a reply's actions show under it.
    /// Worked out here, with the rows, so a row reads a flag rather than
    /// looking at its neighbour as each word streams in.
    Item { index: usize, grouped: bool, turn_end: bool },
    /// The summary of a run of tool calls.
    Group(Group),
    /// A turn's work, as `ToolDisplay::Stream` shows it: its calls, its
    /// thinking and what it said on the way to its reply. While the agent
    /// is at it, a small box its steps run through; done, one line ("Done ·
    /// 5 tool calls · 1 message"), pressed open to show them all.
    Work(Group),
    /// What a finished turn changed (`changes`): a card under its reply.
    Changes { start: usize, end: usize, id: String },
    /// The files a finished turn made for the user (`made`): a card under
    /// the changes.
    Made { start: usize, end: usize, id: String },
}

/// A run of two or more tool calls with nothing said between them.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Group {
    /// The run's items: `start..end`.
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// The first call's id: it stays the same as the run grows and as
    /// older pages load before it, so the list key and the user's choice
    /// follow the group.
    pub(crate) id: String,
    /// The agent is still at work in it.
    pub(crate) live: bool,
    /// Its calls show under the summary.
    pub(crate) open: bool,
}

impl Row {
    /// The list's key: an item keeps its own key whether or not a group
    /// forms over it, so a run that grows does not remount its rows.
    pub(crate) fn key(&self) -> String {
        match self {
            Row::Item { index, .. } => index.to_string(),
            Row::Group(g) => format!("g:{}", g.id),
            // Its id says what it is ("w:…"): the user's choice is kept
            // by it, apart from a run's of the same first call.
            Row::Work(g) => g.id.clone(),
            Row::Changes { id, .. } => format!("c:{id}"),
            Row::Made { id, .. } => format!("m:{id}"),
        }
    }
}

/// How the conversation shows the agent's work (`toolCalls` in the
/// settings), as DSH Go's:
/// - `Stream`: while the agent works, each step a line of
///   its own as it happens (three calls in a row or more one line, "Ran 3
///   commands"); once the turn is done, all of it one line ("Done · 5 tool
///   calls · 1 message") over its reply, pressed open to show the steps,
///   then cards of the files it changed and the files it made. A turn that
///   was stopped or failed stays open, to show where it ended.
/// - `Quiet`: while it works, one line of what it does now; done, as
///   `Stream`.
///
/// The settings' older choices read as the nearest: "auto" and "folded" as
/// `Stream`; "hidden", and none chosen, as `Quiet`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ToolDisplay {
    Stream,
    /// The default: the conversation reads as words, what the agent does
    /// now one line over the composer.
    #[default]
    Quiet,
}

impl ToolDisplay {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ToolDisplay::Stream => "stream",
            ToolDisplay::Quiet => "quiet",
        }
    }

    pub(crate) fn parse(s: &str) -> Self {
        match s {
            "stream" | "auto" | "folded" => ToolDisplay::Stream,
            _ => ToolDisplay::Quiet,
        }
    }
}

/// How many calls in a row make one line of them.
const GROUP_MIN: usize = 3;

/// The display rows for `items`. `running`: the agent's turn is going on.
/// `choice`: what the user opened (true) or closed (false), by id.
pub(crate) fn fold_as(items: &[Item], running: bool, choice: &HashMap<String, bool>, display: ToolDisplay) -> Vec<Row> {
    let mut rows = Vec::with_capacity(items.len());
    // A turn at a time: from a message of the user's to the next.
    let mut start = 0;
    while start < items.len() {
        let mut end = start + 1;
        while end < items.len() && !matches!(items[end], Item::User { .. }) {
            end += 1;
        }
        turn(items, start, end, running, choice, display, &mut rows);
        start = end;
    }
    rows
}

#[cfg(test)]
pub(crate) fn fold(items: &[Item], running: bool, choice: &HashMap<String, bool>) -> Vec<Row> {
    fold_as(items, running, choice, ToolDisplay::Stream)
}

/// A turn: the user's message; its work, as `display` says; its reply,
/// the turn's last words once nothing the agent did follows them; then,
/// the turn done, the files it changed and the files it made.
fn turn(items: &[Item], start: usize, end: usize, running: bool, choice: &HashMap<String, bool>, display: ToolDisplay, rows: &mut Vec<Row>) {
    let mut body = start;
    if matches!(items[start], Item::User { .. }) {
        rows.push(Row::Item { index: start, grouped: false, turn_end: false });
        body += 1;
    }
    let work = |item: &Item| matches!(item, Item::Tool { .. } | Item::Reasoning { .. });
    let live = running_here(running, end, items.len());
    // The reply: its last words, nothing done after them (while they
    // stream, they may yet be more work's: then they join it).
    let reply = (body..end).rev().find(|&i| matches!(items[i], Item::Assistant { .. })).filter(|&r| !items[r + 1..end].iter().any(work));
    let work_end = reply.unwrap_or(end);
    let Some(first) = (body..work_end).find(|&i| work(&items[i])) else {
        return fold_range(items, body, end, running, choice, rows);
    };
    let id = match &items[first] {
        Item::Tool { call_id, .. } if !call_id.is_empty() => format!("w:{call_id}"),
        _ => format!("w:#{first}"),
    };
    // Stopped or failed: as it ran, to show where it ended.
    let broke = !live
        && items[body..end].iter().any(|i| matches!(i, Item::Error { .. }) || matches!(i, Item::Tool { status, .. } if status == "canceled"));
    if display == ToolDisplay::Stream && (live || broke) {
        fold_range(items, body, end, running, choice, rows);
    } else {
        let open = choice.get(&id).copied().unwrap_or(false);
        // What was said before the work ("Let me look at…") is the work's too.
        rows.push(Row::Work(Group { start: body, end: work_end, id: id.clone(), live, open }));
        if open {
            // Each step under it, as it happened.
            rows.extend((body..work_end).map(|index| Row::Item { index, grouped: true, turn_end: false }));
        } else {
            for i in body..work_end {
                if matches!(items[i], Item::Error { .. } | Item::Notice { .. } | Item::Retry { .. }) {
                    rows.push(Row::Item { index: i, grouped: false, turn_end: false });
                }
            }
        }
        if let Some(r) = reply {
            rows.push(Row::Item { index: r, grouped: false, turn_end: turn_end(items, r) });
            fold_range(items, r + 1, end, running, choice, rows);
        }
    }
    if !live {
        let run = &items[body..end];
        if !changes(run).is_empty() {
            rows.push(Row::Changes { start: body, end, id: id.clone() });
        }
        if !made(run).is_empty() {
            rows.push(Row::Made { start: body, end, id });
        }
    }
}

/// The agent works in the turn that ends at `end`.
fn running_here(running: bool, end: usize, len: usize) -> bool {
    running && end == len
}

/// Items `from..to` as rows, as they happened: three calls in a row or
/// more (and the thinking between them) one line, pressed open; a call
/// still going in one shows under it all the same.
fn fold_range(items: &[Item], from: usize, to: usize, running: bool, choice: &HashMap<String, bool>, rows: &mut Vec<Row>) {
    let mut i = from;
    while i < to {
        let mut end = i;
        while end < to && matches!(items[end], Item::Tool { .. } | Item::Reasoning { .. }) {
            end += 1;
        }
        let calls = items[i..end].iter().filter(|t| matches!(t, Item::Tool { .. })).count();
        if calls < GROUP_MIN {
            rows.push(Row::Item { index: i, grouped: false, turn_end: turn_end(items, i) });
            i += 1;
            continue;
        }
        let run = &items[i..end];
        // Its first call's id: the same as thinking comes before it.
        let id = run
            .iter()
            .find_map(|t| match t {
                Item::Tool { call_id, .. } if !call_id.is_empty() => Some(call_id.clone()),
                _ => None,
            })
            .unwrap_or_else(|| format!("#{i}"));
        let live = running && (end == items.len() || run.iter().any(|t| status_of(t) == "running"));
        let open = choice.get(&id).copied().unwrap_or(false);
        rows.push(Row::Group(Group { start: i, end, id, live, open }));
        if open {
            rows.extend((i..end).map(|index| Row::Item { index, grouped: true, turn_end: false }));
        } else if live {
            rows.extend((i..end).filter(|&k| status_of(&items[k]) == "running").map(|index| Row::Item { index, grouped: true, turn_end: false }));
        }
        i = end;
    }
}

/// Whether item `i` ends what the agent did in its turn.
fn turn_end(items: &[Item], i: usize) -> bool {
    !matches!(items.get(i + 1), Some(Item::Tool { .. } | Item::Reasoning { .. } | Item::Assistant { .. }))
}

/// The row that shows item `index`: its own, or its folded group's.
pub(crate) fn row_of(rows: &[Row], index: usize) -> Option<usize> {
    // Its own row first: a turn's folded line spans words that have rows
    // of their own.
    rows.iter().position(|r| matches!(r, Row::Item { index: i, .. } if *i == index)).or_else(|| {
        rows.iter().position(|r| matches!(r, Row::Group(g) | Row::Work(g) if !g.open && (g.start..g.end).contains(&index)))
    })
}

fn status_of(item: &Item) -> &str {
    match item {
        Item::Tool { status, .. } => status,
        _ => "",
    }
}

/// A path as a line has room for: its folder and name (`src/login.rs`);
/// anything else as it is.
pub(crate) fn short_path(words: &str) -> String {
    if !(words.starts_with('/') || words.starts_with("~/")) || words.contains(' ') {
        return words.to_owned();
    }
    let parts: Vec<&str> = words.trim_end_matches('/').rsplit('/').take(2).collect();
    parts.into_iter().rev().collect::<Vec<_>>().join("/")
}

/// What a turn's done line says: its calls, the words said on the way,
/// how many failed, and whether it was stopped.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Tally {
    pub(crate) calls: usize,
    pub(crate) messages: usize,
    pub(crate) failed: usize,
    pub(crate) stopped: bool,
}

pub(crate) fn tally(run: &[Item]) -> Tally {
    let s = summarize(run);
    Tally {
        calls: run.iter().filter(|i| matches!(i, Item::Tool { .. })).count(),
        messages: run.iter().filter(|i| matches!(i, Item::Assistant { .. })).count(),
        failed: s.failed,
        stopped: run.iter().any(|i| matches!(i, Item::Tool { status, .. } if status == "canceled")),
    }
}

/// A file a turn changed: lines added and taken away, as its edits say.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FileChange {
    pub(crate) path: String,
    pub(crate) added: usize,
    pub(crate) removed: usize,
}

/// The files `run`'s calls edited or wrote, each once, in the order first
/// changed.
pub(crate) fn changes(run: &[Item]) -> Vec<FileChange> {
    let mut out: Vec<FileChange> = vec![];
    for item in run {
        let Item::Tool { kind, summary, detail, status, .. } = item else { continue };
        if !matches!(kind.as_str(), "edit" | "write") || status == "failed" {
            continue;
        }
        let field = |k: &str| detail.get(k).and_then(serde_json::Value::as_str);
        let Some(path) = field("filePath").or(Some(summary.as_str())).filter(|p| !p.trim().is_empty()) else { continue };
        let (added, removed) = if let Some(diff) = field("unifiedDiff") {
            diff.lines().fold((0, 0), |(a, r), l| {
                if l.starts_with('+') && !l.starts_with("+++") {
                    (a + 1, r)
                } else if l.starts_with('-') && !l.starts_with("---") {
                    (a, r + 1)
                } else {
                    (a, r)
                }
            })
        } else if kind == "write" {
            (field("content").map_or(0, |c| c.lines().count()), 0)
        } else {
            (field("newString").map_or(0, |c| c.lines().count()), field("oldString").map_or(0, |c| c.lines().count()))
        };
        match out.iter_mut().find(|c| c.path == path) {
            Some(c) => {
                c.added += added;
                c.removed += removed;
            }
            None => out.push(FileChange { path: path.to_owned(), added, removed }),
        }
    }
    out
}

/// The kinds of file made for the user to open, by extension (as DSH
/// Go's): pictures, documents, sheets, slides, archives, sound, video, web
/// pages and calendar or contact cards. Code and notes are the work's own:
/// the changes card has them.
const MADE_KINDS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "svg", "heic", "pdf", "doc", "docx", "rtf", "pages", "xls", "xlsx", "csv", "numbers", "ppt", "pptx",
    "key", "zip", "tar", "gz", "tgz", "7z", "dmg", "mp3", "wav", "m4a", "aac", "flac", "mp4", "mov", "webm", "html", "htm", "ics", "vcf",
    "epub",
];

/// Whether `name` is a file of a kind made for the user.
pub(crate) fn made_kind(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && MADE_KINDS.contains(&ext.to_ascii_lowercase().as_str()))
}

/// The files `run` (a turn) made for the user, each once, as written: a
/// file of such a kind its calls wrote, or one its last words name that its
/// calls named too (a script's output: "make_chart.py → 10月趋势.png").
pub(crate) fn made(run: &[Item]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut add = |path: String| {
        if !out.contains(&path) {
            out.push(path);
        }
    };
    let mut said_by_calls = String::new();
    for item in run {
        let Item::Tool { kind, summary, detail, status, .. } = item else { continue };
        if status == "failed" {
            continue;
        }
        said_by_calls.push_str(summary);
        said_by_calls.push('\n');
        said_by_calls.push_str(&detail.to_string());
        said_by_calls.push('\n');
        if kind == "write" {
            let path = detail.get("filePath").and_then(serde_json::Value::as_str).unwrap_or(summary);
            if made_kind(path) {
                add(path.to_owned());
            }
        }
    }
    let Some(reply) = run.iter().rev().find_map(|i| match i {
        Item::Assistant { text, .. } => Some(text),
        _ => None,
    }) else {
        return out;
    };
    for word in reply.split(|c: char| c.is_whitespace() || "`()[]<>\"'*|，。；：！？、《》「」『』（）".contains(c)) {
        let word = word.trim_end_matches(|c: char| ".,;:!?".contains(c));
        if !made_kind(word) {
            continue;
        }
        // Words run on into a name where nothing parts them ("生成了报表.xlsx"):
        // the shortest end of it the calls named, from the longest down.
        let name = word.char_indices().map(|(at, _)| &word[at..]).find(|tail| {
            let file = tail.rsplit('/').next().unwrap_or(tail);
            made_kind(file) && said_by_calls.contains(file)
        });
        if let Some(name) = name {
            add(name.to_owned());
        }
    }
    out
}

/// What a run of tool calls did, by kind.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Summary {
    pub(crate) commands: usize,
    /// Distinct files, so a file edited twice counts once.
    pub(crate) edited: usize,
    pub(crate) read: usize,
    pub(crate) searches: usize,
    pub(crate) fetches: usize,
    pub(crate) agents: usize,
    /// Plans and to-do lists written.
    pub(crate) plans: usize,
    pub(crate) other: usize,
    pub(crate) running: usize,
    pub(crate) failed: usize,
}

pub(crate) fn summarize(run: &[Item]) -> Summary {
    let mut s = Summary::default();
    let (mut edited, mut read) = (HashSet::new(), HashSet::new());
    for (n, item) in run.iter().enumerate() {
        let Item::Tool { kind, name, status, summary, error, .. } = item else { continue };
        // A call without a path is a file of its own.
        let file = if summary.is_empty() { format!("#{n}") } else { summary.clone() };
        match kind.as_str() {
            "shell" => s.commands += 1,
            "edit" | "write" => {
                edited.insert(file);
            }
            "read" => {
                read.insert(file);
            }
            "search" => s.searches += 1,
            "fetch" => s.fetches += 1,
            "sub_agent" => s.agents += 1,
            "plan" => s.plans += 1,
            _ if name.to_lowercase().contains("todo") => s.plans += 1,
            _ => s.other += 1,
        }
        match status.as_str() {
            "running" => s.running += 1,
            "failed" => s.failed += 1,
            // An error on a call the host did not mark failed is a failure
            // all the same.
            "canceled" => {}
            _ if error.is_some() => s.failed += 1,
            _ => {}
        }
    }
    s.edited = edited.len();
    s.read = read.len();
    s
}

impl Summary {
    /// The phrases, in order, as English templates (one, many) and a count.
    fn phrases(&self) -> Vec<(&'static str, &'static str, usize)> {
        [
            ("ran 1 command", "ran {} commands", self.commands),
            ("edited 1 file", "edited {} files", self.edited),
            ("read 1 file", "read {} files", self.read),
            ("ran 1 search", "ran {} searches", self.searches),
            ("fetched 1 page", "fetched {} pages", self.fetches),
            ("ran 1 agent", "ran {} agents", self.agents),
            ("updated the plan", "updated the plan {} times", self.plans),
            ("1 other action", "{} other actions", self.other),
        ]
        .into_iter()
        .filter(|(.., n)| *n > 0)
        .collect()
    }

    /// "Ran 3 commands, edited 2 files", in the active language.
    pub(crate) fn describe(&self) -> String {
        let text = self
            .phrases()
            .into_iter()
            .map(|(one, many, n)| if n == 1 { t(one).to_owned() } else { tf(many, &[&n.to_string()]) })
            .collect::<Vec<_>>()
            .join(t(", "));
        capitalize(&text)
    }

    /// The kinds of call made and how many of each, in the order the
    /// summary names them: a folded row shows them as icons.
    pub(crate) fn counts(&self) -> Vec<(&'static str, usize)> {
        [
            ("shell", self.commands),
            ("edit", self.edited),
            ("read", self.read),
            ("search", self.searches),
            ("fetch", self.fetches),
            ("sub_agent", self.agents),
            ("plan", self.plans),
            ("other", self.other),
        ]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .collect()
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn tool(id: &str, kind: &str, status: &str, path: &str) -> Item {
        Item::Tool {
            call_id: id.into(),
            name: kind.into(),
            kind: kind.into(),
            status: status.into(),
            summary: path.into(),
            detail: Value::Null,
            error: None,
        }
    }

    fn said(text: &str) -> Item {
        Item::Assistant { text: text.into(), at: None }
    }

    fn asked(text: &str) -> Item {
        Item::User { text: text.into(), message_id: None, images: vec![], at: None }
    }

    fn keys(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::key).collect()
    }

    fn none() -> HashMap<String, bool> {
        HashMap::new()
    }

    /// Items as they happened (a live turn, a stopped one).
    fn flat(items: &[Item], running: bool, choice: &HashMap<String, bool>) -> Vec<Row> {
        let mut rows = vec![];
        fold_range(items, 0, items.len(), running, choice, &mut rows);
        rows
    }

    fn quiet(items: &[Item], running: bool, choice: &HashMap<String, bool>) -> Vec<Row> {
        fold_as(items, running, choice, ToolDisplay::Quiet)
    }

    #[test]
    fn three_calls_in_a_row_are_one_line_and_two_are_not() {
        let think = || Item::Reasoning { text: "hm".into() };
        // thinking, call, thinking, call, thinking, call, then words.
        let items = vec![think(), tool("a", "shell", "completed", ""), think(), tool("b", "shell", "completed", ""), think(), tool("c", "shell", "completed", ""), said("done")];
        let rows = flat(&items, false, &none());
        assert_eq!(keys(&rows), ["g:a", "6"]);
        assert!(matches!(&rows[0], Row::Group(g) if g.start == 0 && g.end == 6 && !g.open));
        let items = vec![said("hi"), tool("a", "shell", "completed", "ls"), tool("b", "read", "completed", "x"), said("done")];
        assert_eq!(keys(&flat(&items, false, &none())), ["0", "1", "2", "3"]);
    }

    #[test]
    fn words_between_calls_split_the_runs() {
        let items = vec![
            tool("a", "shell", "completed", ""),
            tool("b", "shell", "completed", ""),
            tool("c", "shell", "completed", ""),
            said("so"),
            tool("d", "read", "completed", "x"),
            tool("e", "read", "completed", "y"),
            tool("f", "read", "completed", "z"),
        ];
        assert_eq!(keys(&flat(&items, false, &none())), ["g:a", "3", "g:d"]);
    }

    #[test]
    fn a_call_still_going_shows_under_its_line() {
        let items = vec![tool("a", "shell", "completed", ""), tool("b", "shell", "completed", ""), tool("c", "shell", "running", "")];
        let rows = flat(&items, true, &none());
        assert_eq!(keys(&rows), ["g:a", "2"]);
        assert!(matches!(&rows[0], Row::Group(g) if g.live && !g.open));
        assert!(matches!(&rows[1], Row::Item { grouped: true, .. }));
        // Opened, all of them.
        let opened = HashMap::from([("a".to_owned(), true)]);
        assert_eq!(keys(&flat(&items, true, &opened)), ["g:a", "0", "1", "2"]);
    }

    #[test]
    fn a_reply_ends_its_turn_unless_the_agent_goes_on() {
        let items = vec![asked("go"), said("looking"), tool("a", "shell", "completed", ""), said("done"), asked("next")];
        let ends: Vec<bool> = flat(&items, false, &none())
            .iter()
            .filter_map(|r| match r {
                Row::Item { turn_end, .. } => Some(*turn_end),
                _ => None,
            })
            .collect();
        assert_eq!(ends, [false, false, false, true, true]);
    }

    #[test]
    fn keys_hold_as_items_stream_in() {
        let mut items = vec![said("go"), tool("a", "shell", "running", ""), tool("b", "shell", "running", "")];
        assert_eq!(keys(&flat(&items, true, &none())), ["0", "1", "2"]);
        // A third call makes a line: it comes in over the calls, which keep
        // their keys.
        items.push(tool("c", "read", "running", "x"));
        assert_eq!(keys(&flat(&items, true, &none())), ["0", "g:a", "1", "2", "3"]);
    }

    #[test]
    fn a_call_without_an_id_keys_by_position() {
        let items = vec![tool("", "shell", "completed", ""), tool("", "shell", "completed", ""), tool("", "shell", "completed", "")];
        assert_eq!(keys(&flat(&items, false, &none())), ["g:#0"]);
    }

    #[test]
    fn a_folded_item_is_found_in_its_line() {
        let items = vec![asked("hi"), tool("a", "shell", "completed", ""), tool("b", "shell", "completed", ""), said("x")];
        let rows = fold(&items, false, &none());
        assert_eq!(keys(&rows), ["0", "w:a", "3"]);
        assert_eq!(row_of(&rows, 2), Some(1));
        assert_eq!(row_of(&rows, 3), Some(2));
        let open = fold(&items, false, &HashMap::from([("w:a".to_owned(), true)]));
        assert_eq!(row_of(&open, 2), Some(3));
    }

    #[test]
    fn counts_by_kind_and_files_once() {
        let run = vec![
            tool("1", "shell", "completed", "ls"),
            tool("2", "shell", "completed", "pwd"),
            tool("3", "edit", "completed", "a.rs"),
            tool("4", "edit", "completed", "a.rs"),
            tool("5", "write", "completed", "b.rs"),
            tool("6", "read", "completed", "a.rs"),
            tool("7", "search", "completed", "fn"),
            tool("8", "sub_agent", "completed", ""),
            tool("9", "plan", "completed", ""),
            tool("10", "mcp", "completed", ""),
        ];
        let s = summarize(&run);
        assert_eq!((s.commands, s.edited, s.read, s.searches, s.fetches, s.agents, s.plans, s.other), (2, 2, 1, 1, 0, 1, 1, 1));
        assert_eq!(
            s.phrases(),
            vec![
                ("ran 1 command", "ran {} commands", 2),
                ("edited 1 file", "edited {} files", 2),
                ("read 1 file", "read {} files", 1),
                ("ran 1 search", "ran {} searches", 1),
                ("ran 1 agent", "ran {} agents", 1),
                ("updated the plan", "updated the plan {} times", 1),
                ("1 other action", "{} other actions", 1),
            ]
        );
    }

    #[test]
    fn failures_are_counted() {
        let mut errored = tool("3", "read", "completed", "x");
        if let Item::Tool { error, .. } = &mut errored {
            *error = Some("no such file".into());
        }
        let run = vec![tool("1", "shell", "failed", ""), tool("2", "shell", "running", ""), errored, tool("4", "shell", "canceled", "")];
        let s = summarize(&run);
        assert_eq!((s.failed, s.running), (2, 1));
    }

    #[test]
    fn capitalizes_the_first_phrase_only() {
        assert_eq!(capitalize("ran 2 commands, read 1 file"), "Ran 2 commands, read 1 file");
        assert_eq!(capitalize("运行了 2 条命令"), "运行了 2 条命令");
    }

    fn edit(id: &str, path: &str, diff: &str) -> Item {
        Item::Tool {
            call_id: id.into(),
            name: "Edit".into(),
            kind: "edit".into(),
            status: "completed".into(),
            summary: path.into(),
            detail: serde_json::json!({ "type": "edit", "filePath": path, "unifiedDiff": diff }),
            error: None,
        }
    }

    /// Done, a turn is the user's message, one line for all its work (what
    /// it said on the way too), its reply, and the files it changed.
    #[test]
    fn a_done_turn_is_one_line_its_reply_and_its_changes() {
        let items = vec![
            asked("fix it"),
            said("Let me look."),
            tool("a", "read", "completed", "src/a.rs"),
            Item::Reasoning { text: "hm".into() },
            edit("b", "src/a.rs", "@@\n-old\n+new\n+more"),
            said("Fixed."),
        ];
        for rows in [fold(&items, false, &none()), quiet(&items, false, &none())] {
            assert_eq!(keys(&rows), ["0", "w:a", "5", "c:w:a"]);
            assert!(matches!(&rows[1], Row::Work(g) if g.start == 1 && g.end == 5 && !g.live && !g.open));
            assert!(matches!(rows[2], Row::Item { index: 5, turn_end: true, .. }), "the reply ends the turn");
            assert!(matches!(&rows[3], Row::Changes { start: 1, end: 6, .. }));
        }
        // Opened: its steps, all of them, under its line.
        let open = HashMap::from([("w:a".to_owned(), true)]);
        assert_eq!(keys(&fold(&items, false, &open)), ["0", "w:a", "1", "2", "3", "4", "5", "c:w:a"]);
    }

    /// While the agent works, its steps show one by one; quietly, one line
    /// of what it does now, the last words streaming under it.
    #[test]
    fn a_live_turn_shows_its_steps_or_one_line() {
        let mut items = vec![asked("go"), said("Looking"), tool("a", "shell", "running", "ls")];
        assert_eq!(keys(&fold(&items, true, &none())), ["0", "1", "2"]);
        assert_eq!(keys(&quiet(&items, true, &none())), ["0", "w:a"]);
        assert!(matches!(&quiet(&items, true, &none())[1], Row::Work(g) if g.live && g.end == 3));
        items.push(said("Now the tests"));
        assert_eq!(keys(&quiet(&items, true, &none())), ["0", "w:a", "3"]);
        items.push(tool("b", "shell", "running", "cargo test"));
        assert_eq!(keys(&quiet(&items, true, &none())), ["0", "w:a"], "the words joined the work");
        // An earlier turn is done while a later one runs.
        items.extend([said("ok"), asked("again"), tool("c", "shell", "running", "ls")]);
        let rows = fold(&items, true, &none());
        assert_eq!(keys(&rows), ["0", "w:a", "5", "6", "7"]);
        assert!(matches!(&rows[1], Row::Work(g) if !g.live));
    }

    /// A stopped or failed turn stays as it ran; quietly it folds as well.
    #[test]
    fn a_turn_that_broke_stays_open() {
        let items = vec![asked("run"), tool("a", "shell", "failed", "x"), Item::Error { message: "boom".into() }];
        assert_eq!(keys(&fold(&items, false, &none())), ["0", "1", "2"]);
        assert_eq!(keys(&quiet(&items, false, &none())), ["0", "w:a", "2"]);
        let stopped = vec![asked("run"), tool("a", "shell", "canceled", "x")];
        assert_eq!(keys(&fold(&stopped, false, &none())), ["0", "1"]);
        // A turn that only talks is as it was.
        assert_eq!(keys(&fold(&[asked("hi"), said("hello")], false, &none())), ["0", "1"]);
    }

    #[test]
    fn older_choices_read_as_the_nearest() {
        for (old, now) in [("auto", ToolDisplay::Stream), ("folded", ToolDisplay::Stream), ("stream", ToolDisplay::Stream), ("quiet", ToolDisplay::Quiet), ("hidden", ToolDisplay::Quiet)] {
            assert_eq!(ToolDisplay::parse(old), now);
        }
        assert_eq!(ToolDisplay::parse(""), ToolDisplay::Quiet, "none chosen: the default");
        assert_eq!(ToolDisplay::default(), ToolDisplay::Quiet);
    }

    fn wrote(id: &str, path: &str) -> Item {
        Item::Tool {
            call_id: id.into(),
            name: "Write".into(),
            kind: "write".into(),
            status: "completed".into(),
            summary: path.into(),
            detail: serde_json::json!({ "filePath": path, "content": "x" }),
            error: None,
        }
    }

    /// Made: what its calls wrote of a kind for the user, and what its
    /// reply names that its calls named; not code, nor what it only talks of.
    #[test]
    fn the_files_a_turn_made() {
        let items = vec![
            asked("chart my spending"),
            wrote("a", "/w/make_chart.py"),
            wrote("b", "/w/notes.md"),
            wrote("c", "/w/out/summary.pdf"),
            tool("d", "shell", "completed", "python3 make_chart.py 账单-10月.csv -o 10月趋势.png"),
            said("做好了：生成了10月趋势.png，也写了 `/w/out/summary.pdf`。参考了 report.xlsx。"),
        ];
        assert_eq!(made(&items), ["/w/out/summary.pdf", "10月趋势.png"]);
        let rows = fold(&items, false, &none());
        assert_eq!(keys(&rows), ["0", "w:a", "5", "c:w:a", "m:w:a"]);
        // Live, no cards yet.
        assert!(!keys(&fold(&items, true, &none())).iter().any(|k| k.starts_with("m:")));
    }

    #[test]
    fn changes_count_lines_per_file_once() {
        let items = vec![
            edit("a", "src/a.rs", "--- a\n+++ b\n@@\n-x\n+y\n+z"),
            edit("b", "src/a.rs", "@@\n-q"),
            Item::Tool {
                call_id: "c".into(),
                name: "Write".into(),
                kind: "write".into(),
                status: "completed".into(),
                summary: "new.txt".into(),
                detail: serde_json::json!({ "filePath": "new.txt", "content": "1\n2\n3" }),
                error: None,
            },
            tool("d", "edit", "failed", "nope.rs"),
        ];
        assert_eq!(
            changes(&items),
            vec![
                FileChange { path: "src/a.rs".into(), added: 2, removed: 2 },
                FileChange { path: "new.txt".into(), added: 3, removed: 0 },
            ]
        );
    }

    #[test]
    fn a_path_is_named_by_its_folder_and_name() {
        assert_eq!(short_path("/tmp/x/proj/src/login.rs"), "src/login.rs");
        assert_eq!(short_path("cargo test /a/b"), "cargo test /a/b", "a command as it is");
    }
}
