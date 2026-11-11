//! The pictures of a user message, read from the agent's own record of the
//! conversation on this computer: Claude Code's (`projects/<folder>/*.jsonl`)
//! and Codex's (`sessions/**/rollout-*.jsonl`). The service's history has a
//! message's words alone; the agents keep the whole message, pictures too,
//! for as long as they keep the conversation. So a picture is found
//! whichever device sent it, and after anything Sophon kept of it is gone.
//!
//! Claude: the service gives a message the id it has in the record, so the
//! id finds it. Codex: by the id when the record has it, else by the
//! message's words among the messages that have pictures.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

use serde_json::Value;

use crate::usage::{Source, scan::{roots, walk}};

/// The ids looked for and not found, not to read the records again for
/// them on every opening of the conversation.
static MISSED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// How many of Codex's records, the latest first, are looked through.
const CODEX_RECORDS: usize = 300;

/// The pictures (data URIs) of the message with this id (or, for Codex,
/// these words) in the conversations of folder `cwd`. Reads files: call it
/// off the thread that draws.
pub(crate) fn find(cwd: &str, message_id: &str, text: &str) -> Vec<String> {
    let key = format!("{cwd}\n{message_id}\n{text}");
    if MISSED.lock().ok().is_some_and(|m| m.as_ref().is_some_and(|m| m.contains(&key))) {
        return vec![];
    }
    let roots = roots();
    let of = |source: Source| -> Vec<PathBuf> { roots.iter().filter(|(s, _)| *s == source).map(|(_, p)| p.clone()).collect() };
    let mut found = if is_uuid(message_id) { claude(&of(Source::Claude), cwd, message_id) } else { vec![] };
    if found.is_empty() {
        found = codex(&of(Source::Codex), cwd, message_id, text);
    }
    if found.is_empty()
        && let Ok(mut missed) = MISSED.lock()
    {
        missed.get_or_insert_with(HashSet::new).insert(key);
    }
    found
}

fn is_uuid(id: &str) -> bool {
    id.len() == 36 && id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}

/// The name Claude Code gives a folder's conversations: its path with
/// everything but letters and digits as dashes.
fn claude_folder(cwd: &str) -> String {
    cwd.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// The `.jsonl` files under `dir`, the latest first.
fn records(dir: &Path, source: Source) -> Vec<PathBuf> {
    let mut files = vec![];
    walk(dir, source, SystemTime::UNIX_EPOCH, 0, &mut files);
    files.sort_by(|a, b| b.3.cmp(&a.3));
    files.into_iter().map(|f| f.0).collect()
}

fn claude(roots: &[PathBuf], cwd: &str, id: &str) -> Vec<String> {
    let needle = format!("\"uuid\":\"{id}\"");
    for root in roots {
        // The folder's conversations; with no folder known, all of them.
        let dir = if cwd.is_empty() { root.clone() } else { root.join(claude_folder(cwd)) };
        for file in records(&dir, Source::Claude) {
            let Ok(body) = std::fs::read_to_string(&file) else { continue };
            let Some(line) = body.lines().find(|l| l.contains(&needle)) else { continue };
            return claude_pictures(line);
        }
    }
    vec![]
}

/// The pictures of one line of Claude's record: a user message's image
/// blocks.
fn claude_pictures(line: &str) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(line) else { return vec![] };
    let blocks = v.pointer("/message/content").and_then(Value::as_array);
    blocks
        .into_iter()
        .flatten()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("image"))
        .filter_map(|b| {
            let source = b.get("source")?;
            let media = source.get("media_type")?.as_str()?;
            let data = source.get("data")?.as_str()?;
            media.starts_with("image/").then(|| format!("data:{media};base64,{data}"))
        })
        .collect()
}

fn codex(roots: &[PathBuf], cwd: &str, id: &str, text: &str) -> Vec<String> {
    let text = text.trim();
    if id.is_empty() && text.is_empty() {
        return vec![];
    }
    let files: Vec<PathBuf> = roots.iter().flat_map(|r| records(r, Source::Codex)).take(CODEX_RECORDS).collect();
    for file in files {
        let Ok(body) = std::fs::read_to_string(&file) else { continue };
        // Its first line says which folder the conversation is in.
        if !cwd.is_empty() {
            let meta = body.lines().next().and_then(|l| serde_json::from_str::<Value>(l).ok());
            if meta.as_ref().and_then(|m| m.pointer("/payload/cwd")).and_then(Value::as_str).is_some_and(|c| c != cwd) {
                continue;
            }
        }
        // The last message that fits: the same words again mean the later one.
        let found = body.lines().rev().filter(|l| l.contains("input_image")).find_map(|l| codex_pictures(l, id, text));
        if let Some(found) = found {
            return found;
        }
    }
    vec![]
}

/// The pictures of one line of Codex's record, if it is the user message
/// with this id or these words.
fn codex_pictures(line: &str, id: &str, text: &str) -> Option<Vec<String>> {
    let v: Value = serde_json::from_str(line).ok()?;
    let message = v.get("payload")?;
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return None;
    }
    let content = message.get("content")?.as_array()?;
    let of = |kind: &str, field: &str| -> Vec<&str> {
        content.iter().filter(|c| c.get("type").and_then(Value::as_str) == Some(kind)).filter_map(|c| c.get(field)?.as_str()).collect()
    };
    let same_id = !id.is_empty() && message.get("id").and_then(Value::as_str) == Some(id);
    let same_words = !text.is_empty() && of("input_text", "text").iter().any(|t| t.trim() == text);
    if !same_id && !same_words {
        return None;
    }
    let pictures: Vec<String> = of("input_image", "image_url").into_iter().filter(|u| u.starts_with("data:image/")).map(str::to_owned).collect();
    (!pictures.is_empty()).then_some(pictures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claudes_folder_name_is_the_path_with_dashes() {
        assert_eq!(claude_folder("/Users/a/My App/v1.2"), "-Users-a-My-App-v1-2");
    }

    #[test]
    fn a_claude_messages_image_blocks_are_its_pictures() {
        let line = r#"{"uuid":"u","type":"user","message":{"content":[{"type":"text","text":"[a.png]look"},{"type":"image","source":{"type":"base64","media_type":"image/png","data":"QUJD"}}]}}"#;
        assert_eq!(claude_pictures(line), vec!["data:image/png;base64,QUJD".to_owned()]);
        assert!(claude_pictures(r#"{"message":{"content":"plain"}}"#).is_empty());
    }

    #[test]
    fn a_codex_message_is_known_by_its_id_or_its_words() {
        let line = r#"{"type":"response_item","payload":{"type":"message","id":"msg_1","role":"user","content":[{"type":"input_text","text":"<image>"},{"type":"input_image","image_url":"data:image/png;base64,QUJD"},{"type":"input_text","text":" [a.png]look "}]}}"#;
        let pictures = Some(vec!["data:image/png;base64,QUJD".to_owned()]);
        assert_eq!(codex_pictures(line, "msg_1", ""), pictures);
        assert_eq!(codex_pictures(line, "other", "[a.png]look"), pictures);
        assert_eq!(codex_pictures(line, "other", "something else"), None);
    }

    #[test]
    fn a_record_on_disk_is_found_by_the_messages_id() {
        let root = std::env::temp_dir().join(format!("sophon-agent-pictures-{}", std::process::id()));
        let dir = root.join(claude_folder("/w/app"));
        std::fs::create_dir_all(&dir).unwrap();
        let id = "963e0eb5-b4cc-4e67-8ea8-2463e832bb99";
        std::fs::write(
            dir.join("s.jsonl"),
            format!("{{\"type\":\"assistant\"}}\n{{\"uuid\":\"{id}\",\"type\":\"user\",\"message\":{{\"content\":[{{\"type\":\"image\",\"source\":{{\"type\":\"base64\",\"media_type\":\"image/jpeg\",\"data\":\"QQ==\"}}}}]}}}}\n"),
        )
        .unwrap();
        assert_eq!(claude(&[root.clone()], "/w/app", id), vec!["data:image/jpeg;base64,QQ==".to_owned()]);
        assert!(claude(&[root.clone()], "/w/other", id).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
