//! The pictures of the messages sent from here, kept on disk: the host's
//! history has a user message's text alone, so without these a picture's
//! chip is a bare name once the conversation is opened again (another
//! conversation in between, a restart). Under `sent-pictures` in the data
//! folder, a file per message: its id, or its text's hash for the message
//! that starts a conversation (whose id the host makes). They stay until
//! Settings' "Clear".

use crate::daemon::{data_dir, model::Item};

fn dir() -> std::path::PathBuf {
    data_dir().join("sent-pictures")
}

/// A name a file can have, whatever the id holds.
fn id_key(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(80).collect()
}

/// The same for the same words on every run (FNV-1a).
pub(crate) fn text_key(text: &str) -> String {
    let hash = text.trim().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3));
    format!("t-{hash:016x}")
}

/// Keeps `pictures` (data URIs) for the message with this id, and under
/// its words too: the host gives a message another id once it reads the
/// conversation back from the agent's own record (after a restart of the
/// agent or the service), and the words are what is left to know it by.
pub(crate) fn keep(message_id: &str, text: &str, pictures: &[String]) {
    let key = id_key(message_id);
    if key.is_empty() || pictures.is_empty() || cfg!(target_arch = "wasm32") {
        return;
    }
    if !text.trim().is_empty() {
        // The pictures once: the words' file lists the messages that had
        // these words, in the order sent.
        let dir = dir();
        let _ = std::fs::create_dir_all(&dir);
        let words = text_key(text);
        let mut ids = same_words(&words);
        if !ids.contains(&key) {
            ids.push(key.clone());
            let _ = std::fs::write(dir.join(format!("{words}.json")), serde_json::json!({ "see": ids }).to_string());
        }
    }
    write(key, pictures);
}

/// Keeps them for a message known by its words alone.
pub(crate) fn keep_for_text(text: &str, pictures: &[String]) {
    write(text_key(text), pictures);
}

fn write(key: String, pictures: &[String]) {
    // A browser has no folder (and no threads).
    if key.is_empty() || pictures.is_empty() || cfg!(target_arch = "wasm32") {
        return;
    }
    let Ok(body) = serde_json::to_vec(pictures) else { return };
    // Megabytes, at times: not on the thread that draws.
    std::thread::spawn(move || {
        let dir = dir();
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let _ = std::fs::write(dir.join(format!("{key}.json")), body);
    });
}

/// How many messages have pictures kept, and their bytes on disk.
pub(crate) fn usage() -> (usize, u64) {
    let Ok(entries) = std::fs::read_dir(dir()) else { return (0, 0) };
    entries.flatten().filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).fold((0, 0), |(n, b), m| (n + 1, b + m.len()))
}

/// Forgets them all (the Settings' "Clear").
pub(crate) fn clear() {
    let _ = std::fs::remove_dir_all(dir());
}

fn read(key: &str) -> Option<Vec<String>> {
    let body = std::fs::read(dir().join(format!("{key}.json"))).ok()?;
    match serde_json::from_slice(&body).ok()? {
        serde_json::Value::Array(pictures) => Some(pictures.into_iter().filter_map(|p| p.as_str().map(str::to_owned)).collect()),
        // The words' file of messages kept by their ids: the last one's.
        serde_json::Value::Object(_) => read(same_words(key).last()?),
        _ => None,
    }
}

/// The ids of the messages kept that had the words of key `words`, in the
/// order sent.
fn same_words(words: &str) -> Vec<String> {
    let list = || -> Option<Vec<String>> {
        let body = std::fs::read(dir().join(format!("{words}.json"))).ok()?;
        let value: serde_json::Value = serde_json::from_slice(&body).ok()?;
        Some(value.get("see")?.as_array()?.iter().filter_map(|i| i.as_str()).map(id_key).filter(|k| !k.is_empty()).collect())
    };
    list().unwrap_or_default()
}

/// The pictures kept for a message: by its id, else by its words.
pub(crate) fn of(message_id: &str, text: &str) -> Vec<String> {
    let by_id = Some(id_key(message_id)).filter(|k| !k.is_empty()).and_then(|k| read(&k));
    by_id.or_else(|| (!text.trim().is_empty()).then(|| read(&text_key(text))).flatten()).unwrap_or_default()
}

/// Whether `text` names a picture where its chip stood, so the message
/// had one.
pub(crate) fn names_a_picture(text: &str) -> bool {
    use crate::ui::shared::composer_doc::{Segment, is_image_name, segments};
    segments(text).iter().any(|s| matches!(s, Segment::Chip(n) if is_image_name(n)))
}

/// Gives the user messages among `items` that have none the pictures kept
/// for them.
pub(crate) fn restore(items: &mut [Item]) {
    if !dir().is_dir() {
        return;
    }
    // From the last: of the messages with the same words, the last here is
    // the last sent, the one before it the one sent before (the host's ids
    // for them are not the ids they were kept under).
    let mut taken: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for item in items.iter_mut().rev() {
        let Item::User { text, message_id, images } = item else { continue };
        if !images.is_empty() {
            continue;
        }
        let by_id = message_id.as_deref().map(id_key).filter(|k| !k.is_empty()).and_then(|k| read(&k));
        let found = by_id.or_else(|| {
            let words = text_key(text);
            let ids = same_words(&words);
            if ids.is_empty() {
                // A conversation's first message, kept by its words alone.
                return read(&words);
            }
            let back = taken.entry(words).or_default();
            *back += 1;
            read(ids.get(ids.len().checked_sub(*back)?)?)
        });
        if let Some(found) = found {
            *images = found;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_texts_key_is_the_same_each_time_and_ignores_the_ends() {
        assert_eq!(text_key(" [a.png] what "), text_key("[a.png] what"));
        assert_ne!(text_key("a"), text_key("b"));
        assert!(text_key("a").starts_with("t-"));
    }

    #[test]
    fn a_message_kept_by_its_id_is_found_by_its_words() {
        let dir = std::env::temp_dir().join(format!("sophon-pictures-{}", std::process::id()));
        // SAFETY: the tests of this module alone read the folder.
        unsafe { std::env::set_var("SOPHON_DATA_DIR", &dir) };
        keep("m-1", "[a.png] look", &["data:x".to_owned()]);
        // The pictures are written on a thread of their own.
        for _ in 0..200 {
            if read("m-1").is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(of("another-id", "[a.png] look"), vec!["data:x".to_owned()]);
        // The same words again, another picture: each message its own.
        keep("m-2", "[a.png] look", &["data:y".to_owned()]);
        for _ in 0..200 {
            if read("m-2").is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let message = |id: &str| Item::User { text: "[a.png] look".into(), message_id: Some(id.into()), images: vec![] };
        let mut items = vec![message("h-1"), message("h-2")];
        restore(&mut items);
        let pictures: Vec<_> = items.iter().map(|i| match i { Item::User { images, .. } => images.clone(), _ => vec![] }).collect();
        assert_eq!(pictures, vec![vec!["data:x".to_owned()], vec!["data:y".to_owned()]]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_ids_key_is_a_plain_file_name() {
        assert_eq!(id_key("../ab-12/c"), "ab-12c");
    }
}
