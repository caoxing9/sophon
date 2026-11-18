//! One agent's conversation, as rows.
//!
//! An agent's stream is a sequence of items, each with the next `seq`:
//! a piece of the reply being written, a tool call starting, the same
//! call finishing. A row is what those add up to: the reply so far, the
//! call as it stands. It covers the range of the stream it was made
//! from (`start..=end`), which is what a client's cursor is compared
//! with, and it is built here exactly as a client builds it from the
//! live stream, so a page fetched afterwards shows what was watched.
//!
//! Kept as a log beside the service's state, one file an agent: a row
//! is appended when it changes and the last line for a `start` is the
//! row. A reply is not written while it streams (one line a word would
//! be the whole reply again each time), only once something follows it
//! or its turn ends.

use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs, io,
    io::Write,
    path::PathBuf,
};

pub struct Row {
    pub start: u64,
    pub end: u64,
    pub item: Value,
}

pub struct Timeline {
    rows: Vec<Row>,
    seq: u64,
    path: PathBuf,
    /// The `start` of each row changed since the log was last written.
    unsaved: BTreeSet<u64>,
}

fn text_row(item: &Value) -> bool {
    matches!(
        item["type"].as_str(),
        Some("assistant_message" | "reasoning")
    )
}

impl Timeline {
    pub fn load(path: PathBuf) -> io::Result<Self> {
        let mut rows = BTreeMap::new();
        let mut lines = 0;
        match fs::read_to_string(&path) {
            Ok(log) => {
                for line in log.lines() {
                    // A line cut short by a crash is the last one, and
                    // the row it was for is in an earlier line or lost
                    // with the reply that was being written.
                    let Ok(row) = serde_json::from_str::<Value>(line) else {
                        continue;
                    };
                    let (Some(start), Some(end)) = (row["s"].as_u64(), row["e"].as_u64()) else {
                        continue;
                    };
                    lines += 1;
                    rows.insert(
                        start,
                        Row {
                            start,
                            end,
                            item: row["i"].clone(),
                        },
                    );
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let rows: Vec<Row> = rows.into_values().collect();
        let timeline = Self {
            seq: rows.iter().map(|row| row.end).max().unwrap_or(0),
            unsaved: BTreeSet::new(),
            rows,
            path,
        };
        // Mostly superseded lines: written again as the rows alone.
        if lines > 64 && lines > timeline.rows.len() * 2 {
            timeline.rewrite()?;
        }
        Ok(timeline)
    }

    fn line(row: &Row) -> String {
        format!("{}\n", json!({"s":row.start,"e":row.end,"i":row.item}))
    }

    fn rewrite(&self) -> io::Result<()> {
        let dir = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("No timeline directory"))?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        for row in &self.rows {
            file.write_all(Self::line(row).as_bytes())?;
        }
        file.as_file().sync_all()?;
        file.persist(&self.path)?;
        Ok(())
    }

    /// Writes the rows changed since the last call.
    pub fn save(&mut self) -> io::Result<()> {
        if self.unsaved.is_empty() {
            return Ok(());
        }
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut log = String::new();
        for row in self
            .rows
            .iter()
            .filter(|row| self.unsaved.contains(&row.start))
        {
            log.push_str(&Self::line(row));
        }
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(log.as_bytes())?;
        file.sync_data()?;
        self.unsaved.clear();
        Ok(())
    }

    /// Adds one item of the stream and returns its `seq`. Text joins the
    /// row being written, a tool call replaces its earlier state (found
    /// by `callId`), and anything else is a row of its own.
    pub fn push(&mut self, item: &Value) -> u64 {
        self.seq += 1;
        let seq = self.seq;
        let kind = item["type"].as_str();
        if text_row(item)
            && let Some(last) = self.rows.last_mut()
            && last.item["type"].as_str() == kind
        {
            let joined = format!(
                "{}{}",
                last.item["text"].as_str().unwrap_or_default(),
                item["text"].as_str().unwrap_or_default()
            );
            last.item["text"] = json!(joined);
            last.end = seq;
            self.unsaved.insert(last.start);
            return seq;
        }
        if kind == Some("tool_call")
            && let Some(call) = item["callId"].as_str().filter(|call| !call.is_empty())
            && let Some(row) = self
                .rows
                .iter_mut()
                .rev()
                .find(|row| row.item["type"] == "tool_call" && row.item["callId"] == call)
        {
            row.item = item.clone();
            row.end = seq;
            self.unsaved.insert(row.start);
            return seq;
        }
        self.rows.push(Row {
            start: seq,
            end: seq,
            item: item.clone(),
        });
        self.unsaved.insert(seq);
        seq
    }

    /// Its rows' items, in order.
    pub fn items(&self) -> impl Iterator<Item = &Value> {
        self.rows.iter().map(|row| &row.item)
    }

    /// How many of the person's messages there are from the one sent
    /// as `message_id` on, that one counted: none if it is not here.
    pub fn turns_from(&self, message_id: &str) -> u64 {
        let Some(at) = self.rows.iter().position(|row| {
            row.item["type"] == "user_message" && row.item["messageId"] == message_id
        }) else {
            return 0;
        };
        self.rows[at..]
            .iter()
            .filter(|row| row.item["type"] == "user_message")
            .count() as u64
    }

    /// Drops the message sent as `message_id` and everything after it.
    /// Places in the stream are not given again: what follows starts
    /// after the last one there was.
    pub fn cut(&mut self, message_id: &str) -> io::Result<()> {
        let Some(at) = self.rows.iter().position(|row| {
            row.item["type"] == "user_message" && row.item["messageId"] == message_id
        }) else {
            return Ok(());
        };
        self.rows.truncate(at);
        self.unsaved.clear();
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        self.rewrite()
    }

    /// Whether nothing has been said in it, as far as it knows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Where the message a client sent as `message_id` is, if it was
    /// taken: the same send again gets the same answer.
    pub fn sent(&self, message_id: &str) -> Option<u64> {
        self.rows
            .iter()
            .rev()
            .find(|row| row.item["type"] == "user_message" && row.item["messageId"] == message_id)
            .map(|row| row.start)
    }

    /// What the agent last said.
    pub fn last_reply(&self) -> Option<&str> {
        self.rows
            .iter()
            .rev()
            .find(|row| row.item["type"] == "assistant_message")
            .and_then(|row| row.item["text"].as_str())
    }

    /// Calls still shown as running, which nothing is running any more
    /// (their turn was stopped, or their agent's process ended): each as
    /// `status`, for the stream.
    pub fn unfinished(&self, status: &str) -> Vec<Value> {
        self.rows
            .iter()
            .filter(|row| row.item["type"] == "tool_call" && row.item["status"] == "running")
            .map(|row| {
                let mut item = row.item.clone();
                item["status"] = json!(status);
                item
            })
            .collect()
    }

    /// A page of rows: the latest, those after a cursor, or those before
    /// one. `epoch` names this conversation's stream; a cursor from
    /// another gets the latest page, marked `reset`.
    pub fn page(&self, epoch: &str, request: &Value) -> Value {
        let limit = request["limit"].as_u64().unwrap_or(40).clamp(1, 200) as usize;
        let cursor = &request["cursor"];
        let at = cursor["seq"].as_u64().filter(|_| cursor["epoch"] == epoch);
        let (rows, older, newer, reset): (Vec<&Row>, bool, bool, bool) =
            match (request["direction"].as_str(), at) {
                // Everything that changed since: a row begun before the
                // cursor and written to after it comes too, and the
                // client sees from its `seqStart` that it does not join.
                (Some("after"), Some(at)) => {
                    let mut since = self.rows.iter().filter(|row| row.end > at);
                    let rows: Vec<_> = since.by_ref().take(limit).collect();
                    let older = rows
                        .first()
                        .is_some_and(|first| first.start > self.rows[0].start);
                    (rows, older, since.next().is_some(), false)
                }
                (Some("before"), Some(at)) => {
                    let until = self.rows.partition_point(|row| row.start < at);
                    let from = until.saturating_sub(limit);
                    (
                        self.rows[from..until].iter().collect(),
                        from > 0,
                        until < self.rows.len(),
                        false,
                    )
                }
                _ => {
                    let from = self.rows.len().saturating_sub(limit);
                    (self.rows[from..].iter().collect(), from > 0, false, true)
                }
            };
        // A page that reaches the end is the conversation as of now,
        // whichever row was written to last.
        let end = if newer {
            rows.iter().map(|row| row.end).max().unwrap_or(0)
        } else {
            self.seq
        };
        let entries: Vec<_> = rows
            .iter()
            .map(|row| json!({"item":row.item,"seqStart":row.start,"seqEnd":row.end}))
            .collect();
        json!({"entries":entries,"epoch":epoch,"reset":reset,"hasOlder":older,"hasNewer":newer,
            "startCursor":{"epoch":epoch,"seq":rows.first().map_or(end, |row| row.start)},
            "endCursor":{"epoch":epoch,"seq":end}})
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline(dir: &tempfile::TempDir) -> Timeline {
        Timeline::load(dir.path().join("a.jsonl")).unwrap()
    }

    fn starts(page: &Value) -> Vec<(u64, u64)> {
        page["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| {
                (
                    entry["seqStart"].as_u64().unwrap(),
                    entry["seqEnd"].as_u64().unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn a_reply_is_one_row_and_a_call_keeps_its_place() {
        let dir = tempfile::tempdir().unwrap();
        let mut rows = timeline(&dir);
        rows.push(&json!({"type":"user_message","text":"go","messageId":"m1"}));
        rows.push(&json!({"type":"assistant_message","text":"Loo"}));
        rows.push(&json!({"type":"assistant_message","text":"king"}));
        rows.push(&json!({"type":"tool_call","callId":"c1","status":"running"}));
        rows.push(&json!({"type":"assistant_message","text":"Done"}));
        rows.push(&json!({"type":"tool_call","callId":"c1","status":"completed"}));
        let page = rows.page("a", &json!({}));
        assert_eq!(starts(&page), [(1, 1), (2, 3), (4, 6), (5, 5)]);
        assert_eq!(page["entries"][1]["item"]["text"], "Looking");
        assert_eq!(page["entries"][2]["item"]["status"], "completed");
        assert_eq!(page["endCursor"]["seq"], 6);
        assert_eq!(rows.sent("m1"), Some(1));
        assert_eq!(rows.last_reply(), Some("Done"));
    }

    #[test]
    fn what_was_saved_is_what_loads_and_a_later_line_is_the_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut rows = timeline(&dir);
        rows.push(&json!({"type":"tool_call","callId":"c1","status":"running"}));
        rows.save().unwrap();
        rows.push(&json!({"type":"tool_call","callId":"c1","status":"completed"}));
        rows.push(&json!({"type":"assistant_message","text":"written"}));
        rows.save().unwrap();
        rows.push(&json!({"type":"assistant_message","text":" and lost"}));
        let mut loaded = timeline(&dir);
        let page = loaded.page("a", &json!({}));
        assert_eq!(starts(&page), [(1, 2), (3, 3)]);
        assert_eq!(page["entries"][0]["item"]["status"], "completed");
        assert_eq!(page["entries"][1]["item"]["text"], "written");
        assert_eq!(
            loaded.push(&json!({"type":"user_message","text":"next"})),
            4
        );
    }

    #[test]
    fn pages_go_after_and_before_a_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let mut rows = timeline(&dir);
        for index in 0..6 {
            rows.push(&json!({"type":"user_message","text":index.to_string()}));
        }
        let tail = rows.page("a", &json!({"limit":2}));
        assert_eq!(starts(&tail), [(5, 5), (6, 6)]);
        assert_eq!(
            (tail["hasOlder"].clone(), tail["reset"].clone()),
            (json!(true), json!(true))
        );
        let before = rows.page(
            "a",
            &json!({"limit":2,"direction":"before","cursor":tail["startCursor"]}),
        );
        assert_eq!(starts(&before), [(3, 3), (4, 4)]);
        assert_eq!(before["reset"], false);
        let after = rows.page(
            "a",
            &json!({"direction":"after","cursor":{"epoch":"a","seq":4}}),
        );
        assert_eq!(starts(&after), [(5, 5), (6, 6)]);
        let other = rows.page(
            "a",
            &json!({"direction":"after","cursor":{"epoch":"b","seq":4}}),
        );
        assert_eq!(other["reset"], true);
        let none = rows.page(
            "a",
            &json!({"direction":"after","cursor":{"epoch":"a","seq":6}}),
        );
        assert_eq!(
            (starts(&none), none["endCursor"]["seq"].clone()),
            (vec![], json!(6))
        );
    }
}
