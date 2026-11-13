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
    Item { index: usize, grouped: bool },
    /// The summary of a run of tool calls.
    Group(Group),
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
        }
    }
}

/// The display rows for `items`. `running`: the agent's turn is going on.
/// `choice`: groups the user opened (true) or closed (false), by id; the
/// rest are open while live and folded otherwise.
pub(crate) fn fold(items: &[Item], running: bool, choice: &HashMap<String, bool>) -> Vec<Row> {
    let mut rows = Vec::with_capacity(items.len());
    let mut i = 0;
    while i < items.len() {
        // A run is the agent at work between what it says: its calls, and
        // its thinking between them (as Claude Code's app shows a turn's
        // steps under one summary), two calls at least.
        let mut end = i;
        while end < items.len() && matches!(items[end], Item::Tool { .. } | Item::Reasoning { .. }) {
            end += 1;
        }
        let calls = items[i..end].iter().filter(|t| matches!(t, Item::Tool { .. })).count();
        if calls < 2 {
            rows.push(Row::Item { index: i, grouped: false });
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
        // The run the agent is in (the last one), or one with a call still
        // going, stays open so its progress shows.
        let live = running && (end == items.len() || run.iter().any(|t| status_of(t) == "running"));
        let open = choice.get(&id).copied().unwrap_or(live);
        rows.push(Row::Group(Group { start: i, end, id, live, open }));
        if open {
            rows.extend((i..end).map(|index| Row::Item { index, grouped: true }));
        }
        i = end;
    }
    rows
}

/// The row that shows item `index`: its own, or its folded group's.
pub(crate) fn row_of(rows: &[Row], index: usize) -> Option<usize> {
    rows.iter().position(|r| match r {
        Row::Item { index: i, .. } => *i == index,
        Row::Group(g) => !g.open && (g.start..g.end).contains(&index),
    })
}

fn status_of(item: &Item) -> &str {
    match item {
        Item::Tool { status, .. } => status,
        _ => "",
    }
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
    pub(crate) other: usize,
    pub(crate) running: usize,
    pub(crate) failed: usize,
}

pub(crate) fn summarize(run: &[Item]) -> Summary {
    let mut s = Summary::default();
    let (mut edited, mut read) = (HashSet::new(), HashSet::new());
    for (n, item) in run.iter().enumerate() {
        let Item::Tool { kind, status, summary, error, .. } = item else { continue };
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

    /// "1 failed", or nothing.
    pub(crate) fn failures(&self) -> Option<String> {
        (self.failed > 0).then(|| tf("{} failed", &[&self.failed.to_string()]))
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    #[test]
    fn thinking_between_calls_folds_with_them() {
        let think = || Item::Reasoning { text: "hm".into() };
        let call = |id: &str| Item::Tool { call_id: id.into(), name: "Bash".into(), kind: "shell".into(), status: "completed".into(), summary: String::new(), detail: serde_json::Value::Null, error: None };
        let say = || Item::Assistant { text: "done".into() };
        // thinking, call, thinking, call, thinking, call, then words.
        let items = vec![think(), call("a"), think(), call("b"), think(), call("c"), say()];
        let rows = fold(&items, false, &HashMap::new());
        assert_eq!(rows.len(), 2);
        assert!(matches!(&rows[0], Row::Group(g) if g.start == 0 && g.end == 6 && g.id == "a" && !g.open));
        assert_eq!(summarize(&items[0..6]).commands, 3);
        // Thinking with one call stays as it is.
        let items = vec![think(), call("a"), say()];
        assert_eq!(fold(&items, false, &HashMap::new()).len(), 3);
    }

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
        Item::Assistant { text: text.into() }
    }

    fn keys(rows: &[Row]) -> Vec<String> {
        rows.iter().map(Row::key).collect()
    }

    fn none() -> HashMap<String, bool> {
        HashMap::new()
    }

    #[test]
    fn a_run_folds_into_one_row() {
        let items = vec![said("hi"), tool("a", "shell", "completed", "ls"), tool("b", "read", "completed", "x"), said("done")];
        let rows = fold(&items, false, &none());
        assert_eq!(keys(&rows), ["0", "g:a", "3"]);
        assert!(matches!(&rows[1], Row::Group(g) if g.start == 1 && g.end == 3 && !g.open && !g.live));
    }

    #[test]
    fn a_single_tool_stays_a_row() {
        let items = vec![said("hi"), tool("a", "shell", "completed", "ls"), said("ok"), tool("b", "edit", "completed", "f")];
        let rows = fold(&items, false, &none());
        assert_eq!(keys(&rows), ["0", "1", "2", "3"]);
    }

    #[test]
    fn messages_between_tools_split_the_runs() {
        let items = vec![
            tool("a", "shell", "completed", ""),
            tool("b", "shell", "completed", ""),
            // What the agent says splits runs; its thinking does not
            // (`thinking_between_calls_folds_with_them`).
            Item::Assistant { text: "so".into() },
            tool("c", "read", "completed", "x"),
            tool("d", "read", "completed", "y"),
            tool("e", "read", "completed", "z"),
        ];
        let rows = fold(&items, false, &none());
        assert_eq!(keys(&rows), ["g:a", "2", "g:c"]);
    }

    #[test]
    fn the_live_run_stays_open_until_the_turn_ends() {
        let items = vec![
            tool("a", "shell", "completed", ""),
            tool("b", "shell", "completed", ""),
            said("next"),
            tool("c", "shell", "completed", ""),
            tool("d", "shell", "running", ""),
        ];
        let rows = fold(&items, true, &none());
        // The earlier run folds; the one at the end shows its calls.
        assert_eq!(keys(&rows), ["g:a", "2", "g:c", "3", "4"]);
        assert!(matches!(&rows[2], Row::Group(g) if g.live && g.open));
        assert!(matches!(&rows[3], Row::Item { grouped: true, .. }));
        let rows = fold(&items, false, &none());
        assert_eq!(keys(&rows), ["g:a", "2", "g:c"]);
    }

    #[test]
    fn the_users_choice_wins() {
        let items = vec![tool("a", "shell", "completed", ""), tool("b", "shell", "running", "")];
        let closed = HashMap::from([("a".to_owned(), false)]);
        assert_eq!(keys(&fold(&items, true, &closed)), ["g:a"]);
        let opened = HashMap::from([("a".to_owned(), true)]);
        assert_eq!(keys(&fold(&items, false, &opened)), ["g:a", "0", "1"]);
    }

    #[test]
    fn keys_hold_as_items_stream_in() {
        let mut items = vec![said("go"), tool("a", "shell", "running", "")];
        let before = keys(&fold(&items, true, &none()));
        assert_eq!(before, ["0", "1"]);
        // A second call makes a group: its summary comes in over the first
        // call, which keeps its key.
        items.push(tool("b", "shell", "running", ""));
        let after = keys(&fold(&items, true, &none()));
        assert_eq!(after, ["0", "g:a", "1", "2"]);
        items.push(tool("c", "read", "running", "x"));
        assert_eq!(keys(&fold(&items, true, &none())), ["0", "g:a", "1", "2", "3"]);
    }

    #[test]
    fn a_call_without_an_id_keys_by_position() {
        let items = vec![tool("", "shell", "completed", ""), tool("", "shell", "completed", "")];
        assert_eq!(keys(&fold(&items, false, &none())), ["g:#0"]);
    }

    #[test]
    fn a_folded_item_is_found_in_its_group() {
        let items = vec![said("hi"), tool("a", "shell", "completed", ""), tool("b", "shell", "completed", ""), said("x")];
        let rows = fold(&items, false, &none());
        assert_eq!(row_of(&rows, 0), Some(0));
        assert_eq!(row_of(&rows, 2), Some(1));
        assert_eq!(row_of(&rows, 3), Some(2));
        let open = fold(&items, false, &HashMap::from([("a".to_owned(), true)]));
        assert_eq!(row_of(&open, 2), Some(3));
        assert_eq!(row_of(&open, 3), Some(4));
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
        ];
        let s = summarize(&run);
        assert_eq!((s.commands, s.edited, s.read, s.searches, s.fetches, s.other), (2, 2, 1, 1, 0, 1));
        assert_eq!(
            s.phrases(),
            vec![
                ("ran 1 command", "ran {} commands", 2),
                ("edited 1 file", "edited {} files", 2),
                ("read 1 file", "read {} files", 1),
                ("ran 1 search", "ran {} searches", 1),
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
        assert_eq!(summarize(&run[1..2]).failures(), None);
    }

    #[test]
    fn capitalizes_the_first_phrase_only() {
        assert_eq!(capitalize("ran 2 commands, read 1 file"), "Ran 2 commands, read 1 file");
        assert_eq!(capitalize("运行了 2 条命令"), "运行了 2 条命令");
    }
}
