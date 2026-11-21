//! What a conversation made: the files it wrote, and the files it named
//! (a page it built, a picture it saved) that are on this computer now,
//! as T3 Code's artifact wall finds them. Nothing is scanned: they are
//! found in what the conversation's record says, then looked for on the
//! disk.

use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

/// The kinds of file a conversation's work is kept in, by extension.
const KEPT: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "gif", "svg", "html", "htm", "pdf", "pptx", "mp4", "mov", "m4v",
    "webm", "md", "docx", "xlsx", "csv", "txt",
];

/// Folders whose files are a tool's, not a conversation's work.
const SKIPPED: &[&str] = &[
    "/node_modules/",
    "/.git/",
    "/.cache/",
    "/cache/",
    "/caches/",
    "/target/",
    "/tmp/",
    "/private/tmp/",
    "/private/var/folders/",
];

/// How a file was found, the surer first: it wrote it, or named it.
fn rank(source: &str) -> u8 {
    match source {
        "written" => 0,
        _ => 1,
    }
}

/// What a file is, for a list to show: a picture, a video, a page or a
/// document.
fn kind(extension: &str) -> &'static str {
    match extension {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => "image",
        "mp4" | "mov" | "m4v" | "webm" => "video",
        "html" | "htm" => "page",
        _ => "document",
    }
}

fn extension(path: &str) -> Option<String> {
    let name = path.rsplit('/').next()?;
    let (stem, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    (!stem.is_empty() && KEPT.contains(&extension.as_str())).then_some(extension)
}

/// The paths a text names that end in a kept extension: words split at
/// spaces, quotes and brackets (Markdown's links among them), without the
/// punctuation around them.
fn named_paths(text: &str) -> Vec<String> {
    text.split(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '"' | '\''
                    | '`'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '<'
                    | '>'
                    | '，'
                    | '。'
                    | '：'
                    | '、'
                    | '（'
                    | '）'
                    | '「'
                    | '」'
            )
    })
    .map(|word| {
        word.trim_matches(|c: char| matches!(c, ',' | ';' | ':' | '!' | '?' | '*' | '.' | '…'))
    })
    .map(|word| word.strip_prefix("file://").unwrap_or(word))
    .filter(|word| word.len() > 4 && word.len() < 400 && !word.contains("://"))
    .filter(|word| extension(word).is_some())
    .map(str::to_owned)
    .collect()
}

/// The files the conversation's rows say it wrote or named, each with how
/// it was found, as the rows say them (a path may be relative).
pub fn candidates<'a>(items: impl Iterator<Item = &'a Value>) -> Vec<(String, &'static str)> {
    let mut found: Vec<(String, &'static str)> = Vec::new();
    for item in items {
        match item["type"].as_str() {
            Some("tool_call") => {
                let detail = &item["detail"];
                match detail["type"].as_str() {
                    Some("write" | "edit") => {
                        if let Some(path) = detail["filePath"]
                            .as_str()
                            .filter(|p| extension(p).is_some())
                        {
                            found.push((path.to_owned(), "written"));
                        }
                    }
                    Some("shell") => {
                        for field in ["command", "output"] {
                            if let Some(text) = detail[field].as_str() {
                                found.extend(named_paths(text).into_iter().map(|p| (p, "named")));
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some("assistant_message") => {
                if let Some(text) = item["text"].as_str() {
                    found.extend(named_paths(text).into_iter().map(|p| (p, "named")));
                }
            }
            _ => {}
        }
    }
    found
}

/// `path` where it is: as it is when absolute, from the home folder for
/// `~/`, else in `cwd`.
fn resolve(path: &str, cwd: &str) -> Option<PathBuf> {
    if let Some(rest) = path.strip_prefix("~/") {
        return Some(PathBuf::from(std::env::var_os("HOME")?).join(rest));
    }
    let path = Path::new(path);
    if path.is_absolute() {
        Some(path.to_path_buf())
    } else if cwd.is_empty() {
        None
    } else {
        Some(Path::new(cwd).join(path))
    }
}

/// A picture's size from its first bytes: PNG, GIF and JPEG.
fn dimensions(path: &Path) -> Option<(u32, u32)> {
    let mut head = vec![0u8; 64 * 1024];
    let mut file = fs::File::open(path).ok()?;
    let read = file.read(&mut head).ok()?;
    let head = &head[..read];
    if head.starts_with(b"\x89PNG\r\n\x1a\n") && head.len() >= 24 {
        let at = |i: usize| u32::from_be_bytes([head[i], head[i + 1], head[i + 2], head[i + 3]]);
        return Some((at(16), at(20)));
    }
    if head.starts_with(b"GIF8") && head.len() >= 10 {
        return Some((
            u16::from_le_bytes([head[6], head[7]]) as u32,
            u16::from_le_bytes([head[8], head[9]]) as u32,
        ));
    }
    if head.starts_with(&[0xFF, 0xD8]) {
        let mut i = 2;
        while i + 9 < head.len() {
            if head[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = head[i + 1];
            let length = u16::from_be_bytes([head[i + 2], head[i + 3]]) as usize;
            if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
                let height = u16::from_be_bytes([head[i + 5], head[i + 6]]) as u32;
                let width = u16::from_be_bytes([head[i + 7], head[i + 8]]) as u32;
                return Some((width, height));
            }
            i += 2 + length;
        }
    }
    None
}

/// The files of `found` on this computer now, once each (the surer way it
/// was found kept), newest first, each said as a list shows it. A named
/// one only if it changed since the conversation began (`since`, Unix
/// milliseconds, less five minutes): a file it only mentioned is not its
/// work.
pub fn existing(
    found: &[(String, &'static str)],
    cwd: &str,
    since: Option<i64>,
    agent: &str,
) -> Vec<Value> {
    let mut best: HashMap<PathBuf, &'static str> = HashMap::new();
    for (path, source) in found {
        let Some(path) = resolve(path, cwd) else {
            continue;
        };
        let kept = best.entry(path).or_insert(source);
        if rank(source) < rank(kept) {
            *kept = source;
        }
    }
    let root = fs::canonicalize(cwd)
        .ok()
        .map(|root| root.display().to_string());
    let mut out = Vec::new();
    for (path, source) in best {
        let Ok(real) = fs::canonicalize(&path) else {
            continue;
        };
        let shown = real.display().to_string();
        // Inside the conversation's own folder only its tools' folders are
        // left out (a project kept under /tmp is still a project).
        let judged = match root.as_deref().and_then(|root| shown.strip_prefix(root)) {
            Some(inside) => inside,
            None => shown.as_str(),
        };
        if SKIPPED.iter().any(|skip| judged.contains(skip)) {
            continue;
        }
        let Ok(meta) = fs::metadata(&real) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let modified = meta
            .modified()
            .ok()
            .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
            .map(|at| at.as_millis() as i64)
            .unwrap_or(0);
        if source == "named" && since.is_some_and(|since| modified < since - 5 * 60 * 1000) {
            continue;
        }
        let Some(extension) = extension(&shown) else {
            continue;
        };
        let kind = kind(&extension);
        let size = if kind == "image" {
            dimensions(&real)
        } else {
            None
        };
        out.push(json!({
            "path":shown,
            "name":real.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            "folder":real.parent().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()),
            "extension":extension,
            "kind":kind,
            "source":source,
            "sizeBytes":meta.len(),
            "modifiedAt":modified,
            "width":size.map(|s| s.0),
            "height":size.map(|s| s.1),
            "agentId":agent,
        }));
    }
    out.sort_by_key(|artifact| std::cmp::Reverse(artifact["modifiedAt"].as_i64().unwrap_or(0)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_conversation_s_files_are_those_it_wrote_and_named() {
        let rows = [
            json!({"type":"tool_call","detail":{"type":"write","filePath":"/w/out/page.html"}}),
            json!({"type":"tool_call","detail":{"type":"edit","filePath":"/w/src/main.rs"}}),
            json!({"type":"tool_call","detail":{"type":"shell","command":"open shots/a.png","output":"saved to `report.pdf`."}}),
            json!({"type":"assistant_message","text":"Done: [preview](design/v9-preview.html)，见 ~/Desktop/x.png。"}),
            json!({"type":"user_message","text":"look at /w/mine.png"}),
        ];
        let found = candidates(rows.iter());
        let paths: Vec<_> = found.iter().map(|(p, s)| (p.as_str(), *s)).collect();
        assert_eq!(
            paths,
            [
                ("/w/out/page.html", "written"),
                ("shots/a.png", "named"),
                ("report.pdf", "named"),
                ("design/v9-preview.html", "named"),
                ("~/Desktop/x.png", "named"),
            ]
        );
    }

    #[test]
    fn only_files_there_now_are_kept_with_their_size_and_kind() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_str().unwrap().to_owned();
        // A 3×2 PNG's header, and a page.
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(3u32.to_be_bytes());
        png.extend(2u32.to_be_bytes());
        png.extend([8, 6, 0, 0, 0]);
        fs::write(dir.path().join("shot.png"), &png).unwrap();
        fs::create_dir(dir.path().join("site")).unwrap();
        fs::write(dir.path().join("site/index.html"), "<p>hi</p>").unwrap();
        let found = vec![
            ("shot.png".to_owned(), "named"),
            ("shot.png".to_owned(), "written"),
            (format!("{cwd}/site/index.html"), "named"),
            ("gone.html".to_owned(), "written"),
        ];
        let kept = existing(&found, &cwd, Some(0), "a1");
        assert_eq!(kept.len(), 2, "{kept:?}");
        let shot = kept.iter().find(|a| a["name"] == "shot.png").unwrap();
        assert_eq!(
            (
                &shot["kind"],
                &shot["source"],
                &shot["width"],
                &shot["height"]
            ),
            (&json!("image"), &json!("written"), &json!(3), &json!(2))
        );
        let page = kept.iter().find(|a| a["name"] == "index.html").unwrap();
        assert_eq!(
            (&page["kind"], &page["folder"]),
            (&json!("page"), &json!("site"))
        );
        // Named before the conversation began: not its work.
        let later = i64::MAX / 2;
        assert!(existing(&found[2..3], &cwd, Some(later), "a1").is_empty());
    }
}
