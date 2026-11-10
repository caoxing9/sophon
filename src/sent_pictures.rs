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

/// Keeps `pictures` (data URIs) for the message with this id.
pub(crate) fn keep(message_id: &str, pictures: &[String]) {
    write(id_key(message_id), pictures);
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
    serde_json::from_slice(&body).ok()
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
    for item in items.iter_mut() {
        let Item::User { text, message_id, images } = item else { continue };
        if !images.is_empty() {
            continue;
        }
        let found = message_id.as_deref().map(id_key).filter(|k| !k.is_empty()).and_then(|k| read(&k)).or_else(|| read(&text_key(text)));
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
    fn an_ids_key_is_a_plain_file_name() {
        assert_eq!(id_key("../ab-12/c"), "ab-12c");
    }
}
