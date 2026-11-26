//! Files a reply names, where it names them: an absolute path written as
//! code (`` `/Users/me/report.pptx` ``) shows as a chip a line high, its
//! kind (a picture by itself) and name, that opens the file in its own app
//! (⌘-press: shows it in the Finder).
//!
//! The path becomes an element (`<sophonfile path="…">`) the text view
//! reads as a custom node ([`extension`]) and the app draws ([`chip`]).

use nsh_core::{
    element::{AdaptiveColor, FontWeight},
    prelude::*,
};

use crate::{
    i18n::t,
    store,
    ui::kit::{text, *},
};

/// The custom node's name and the element that makes it.
const NODE: &str = "sophonfile";

/// What makes a file's element a custom node of the text view.
pub(crate) fn extension() -> nsh_base::MarkdownExtension {
    nsh_base::MarkdownExtension::tag(NODE, NODE)
}

/// `markdown` with each inline code span that is a file's absolute path
/// (and that `exists` says is there) made a file's element. Code blocks, and
/// code already inside a link, stay as they are.
pub(crate) fn link_paths(markdown: &str, exists: impl Fn(&str) -> bool) -> String {
    if !markdown.contains('`') {
        return markdown.to_owned();
    }
    let mut out = String::with_capacity(markdown.len() + 64);
    let mut fence: Option<&str> = None;
    for line in markdown.split_inclusive('\n') {
        let lead = line.trim_start();
        match fence {
            Some(f) => {
                if lead.starts_with(f) {
                    fence = None;
                }
                out.push_str(line);
            }
            None if lead.starts_with("```") || lead.starts_with("~~~") => {
                fence = Some(&lead[..3]);
                out.push_str(line);
            }
            None => link_line(line, &exists, &mut out),
        }
    }
    out
}

fn link_line(line: &str, exists: &impl Fn(&str) -> bool, out: &mut String) {
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let run = rest[open..].bytes().take_while(|b| *b == b'`').count();
        if run > 1 {
            // ``code with ` in it``: left as written.
            let close = rest[open + run..].find(&rest[open..open + run]).map_or(rest.len(), |c| open + run + c + run);
            out.push_str(&rest[..close]);
            rest = &rest[close..];
            continue;
        }
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let (before, inner) = (&rest[..open], &after[..close]);
        out.push_str(before);
        if !before.ends_with('[') && is_path(inner) && exists(file_of(inner)) {
            out.push_str(&format!("<{NODE} path=\"{}\"></{NODE}>", encode(inner)));
        } else {
            out.push('`');
            out.push_str(inner);
            out.push('`');
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
}

/// An absolute path (or one under the home folder): `/a/b`, `~/a`.
fn is_path(s: &str) -> bool {
    let s = s.trim();
    s.len() < 1024
        && !s.contains('\n')
        && !s.contains("://")
        && ((s.starts_with('/') && !s.starts_with("//") && s[1..].contains('/')) || s.starts_with("~/"))
}

/// The file of `path:12` or `path:12:4` (a line, a column): the path.
fn file_of(path: &str) -> &str {
    let path = path.trim();
    let mut file = path;
    for _ in 0..2 {
        match file.rsplit_once(':') {
            Some((head, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => file = head,
            _ => break,
        }
    }
    file
}

/// Whether `path` (`~` the home folder) is a file or folder on this Mac.
pub(crate) fn exists_here(path: &str) -> bool {
    std::path::Path::new(&expand(path)).exists()
}

fn expand(path: &str) -> String {
    match (path.strip_prefix("~/"), std::env::var("HOME")) {
        (Some(rest), Ok(home)) => format!("{home}/{rest}"),
        _ => path.to_owned(),
    }
}

/// Opens `path` (`reveal`: shows it in the Finder): this Mac's directly,
/// another computer's once fetched.
fn open(path: &str, reveal: bool) {
    let file = file_of(path);
    if store::on_this_mac() {
        let file = expand(file);
        if reveal { store::reveal_path(&file) } else { store::open_artifact(&file) }
    } else if reveal {
        store::reveal_path(file);
    } else {
        super::preview::open_in_app(file.to_owned());
    }
}

/// The file a link of a reply points to, when it is a path: absolute
/// (`/a/b`, `~/a`), percent-escapes read, a line after it (`:12`, `#L12`)
/// kept as a chip's is. Codex writes the files it names so:
/// `[report](/Users/me/report.docx)`. A `file://` URL has a scheme, which
/// the text view opens itself.
fn file_of_link(url: &str) -> Option<String> {
    let path = decode(url.trim());
    // `#L12` (or `#L12C4`, `#L12-L20`): the line, as `:12`.
    let path = match path.rsplit_once("#L") {
        Some((file, line)) => {
            let n: String = line.chars().take_while(char::is_ascii_digit).collect();
            if n.is_empty() { file.to_owned() } else { format!("{file}:{n}") }
        }
        None => path,
    };
    is_path(&path).then_some(path)
}

/// A press on a reply's link: a file's opens as its chip does (⌘: shown in
/// the Finder); any other is the text view's to open.
pub(crate) fn press_link(e: &nsh_base::TextViewLinkPressEvent) {
    if let Some(path) = file_of_link(&e.url) {
        open(&path, e.modifiers.cmd);
    }
}

/// A chip's height: under the line's (24), so a line with one is no
/// taller than one without.
const CHIP_H: f32 = 20.0;

/// A file's custom node drawn: a chip a line high, its kind and name.
/// Another custom node: its text.
pub(crate) fn chip(node: &nsh_base::text_view::CustomNode) -> AnyElement {
    if node.name != NODE {
        return text(node.text.clone()).into_element();
    }
    let path = decode(node.attributes.get("path").map(String::as_str).unwrap_or_default());
    let file = file_of(&path).to_owned();
    let here = store::on_this_mac();
    let local = expand(&file);
    let name = file.trim_end_matches('/').rsplit('/').next().unwrap_or(&file).to_owned();
    let dir = file.ends_with('/') || (here && std::path::Path::new(&local).is_dir());
    let ext = name.rsplit_once('.').filter(|(stem, _)| !stem.is_empty()).map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    let kind = if dir {
        icon(Icon::Folder, 13.0).text_color(FG_MUTED()).into_element()
    } else if here && is_picture(&ext) {
        view()
            .w(px(14.0))
            .h(px(14.0))
            .rounded(px(3.0))
            .overflow_hidden()
            .child(image(SharedString::from(format!("file://{local}"))).w_full().h_full().object_fit(nsh_core::ResizeMode::Cover))
            .into_element()
    } else {
        let color = color_of(&ext);
        let label: String = if ext.is_empty() { "FILE".into() } else { ext.to_ascii_uppercase().chars().take(4).collect() };
        view()
            .h(px(15.0))
            .px(px(3.0))
            .rounded(px(3.0))
            .bg(tint(color, 0.18))
            .items_center()
            .justify_center()
            .child(text(label).text_size(px(9.0)).font_weight(FontWeight(700)).text_color(color))
            .into_element()
    };
    let tip = text(format!("{}  ·  {}", folder_of(&file), t("⌘-click to show in Finder")))
        .text_size(tx(TEXT_XS))
        .px(sp(S2))
        .py(px(3.0))
        .rounded(px(R_MD))
        .bg(SURFACE0())
        .border_1()
        .border_color(BORDER())
        .into_element();
    // An inline element sits on the baseline: moved down (drawing only)
    // so its middle meets the middle of the line's letters, ~0.36 em up
    // (CJK and capitals alike), as nsh centres inline images.
    let em = TEXT_CONTENT * crate::store::ui_scale().0;
    view()
        .tooltip(tip)
        .translate_y(px(CHIP_H / 2.0 - 0.36 * em))
        .child(
            view()
                .on_press(move |e: &nsh_core::element::PressEvent| open(&path, e.modifiers.cmd))
                .label(t("Open"))
                .flex_row()
                .items_center()
                .gap(px(5.0))
                .h(px(CHIP_H))
                .pl(px(3.0))
                .pr(px(6.0))
                .mx(px(2.0))
                .rounded(px(R_MD))
                .border_1()
                .border_color(BORDER())
                .bg(tint(FG(), 0.04))
                .hover(|s| s.bg(tint(FG(), 0.08)))
                .child(kind)
                .child(text(name).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)).text_color(FG()).number_of_lines(1).max_w(px(320.0))),
        )
        .into_element()
}

/// Where a file is, short: `~` for the home folder.
fn folder_of(file: &str) -> String {
    let parent = file.trim_end_matches('/').rsplit_once('/').map_or("/", |(p, _)| if p.is_empty() { "/" } else { p });
    match std::env::var("HOME") {
        Ok(home) if parent.starts_with(&home) => format!("~{}", &parent[home.len()..]),
        _ => parent.to_owned(),
    }
}

fn is_picture(ext: &str) -> bool {
    matches!(ext, "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "bmp" | "tiff" | "svg")
}

/// A kind of file's color, as the apps that make it use.
fn color_of(ext: &str) -> AdaptiveColor {
    match ext {
        "ppt" | "pptx" | "key" => pair(0xc4501f, 0xf08a5d),
        "doc" | "docx" | "pages" | "rtf" => pair(0x2b5fb8, 0x7aa7f0),
        "xls" | "xlsx" | "csv" | "tsv" | "numbers" => pair(0x217346, 0x5fc48d),
        "pdf" => pair(0xc0392b, 0xf07a6e),
        "md" | "txt" => pair(0x5b5b66, 0xb4b4be),
        "mp4" | "mov" | "mkv" | "webm" | "mp3" | "wav" | "m4a" => pair(0x7a3fb8, 0xb990f0),
        "zip" | "tar" | "gz" | "dmg" | "7z" | "rar" => pair(0x8a5e14, 0xd9a85c),
        "html" | "htm" => pair(0xd2551e, 0xf29a6b),
        _ => pair(0x4a5260, 0xa0a8b8),
    }
}

fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (bytes[i], bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (b, ..) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_link_to_a_file_is_the_file() {
        assert_eq!(file_of_link("/Users/me/a.tar.gz").as_deref(), Some("/Users/me/a.tar.gz"));
        assert_eq!(file_of_link("/Users/me/%E6%8A%A5%E5%91%8A.docx").as_deref(), Some("/Users/me/报告.docx"));
        assert_eq!(file_of_link("/Users/me/src/a.rs#L12").as_deref(), Some("/Users/me/src/a.rs:12"));
        assert_eq!(file_of_link("~/notes/a.md").as_deref(), Some("~/notes/a.md"));
        for not in ["https://example.com/a/b", "file:///Users/me/a.txt", "#anchor", "relative/a", "/single"] {
            assert_eq!(file_of_link(not), None, "{not}");
        }
    }

    fn all(_: &str) -> bool {
        true
    }

    #[test]
    fn a_path_in_code_becomes_a_file_element_and_back() {
        let md = "已覆盖原文件：`/Users/leo/Downloads/Teable 上手指南.pptx`。";
        let out = link_paths(md, all);
        let enc = out.split("path=\"").nth(1).and_then(|r| r.split('"').next()).expect("an element");
        assert!(out.starts_with("已覆盖原文件：<sophonfile path=\""), "{out}");
        assert!(out.ends_with("\"></sophonfile>。"), "{out}");
        assert!(!enc.contains(' '), "{enc}");
        assert_eq!(decode(enc), "/Users/leo/Downloads/Teable 上手指南.pptx");
    }

    #[test]
    fn what_is_not_a_file_stays() {
        let md = "`cargo test` `/usr` `https://a.b/c` `src/lib.rs` [`/a/b`](https://x) `~/notes.md`\n```\n`/a/b`\n```\n";
        let out = link_paths(md, all);
        assert!(out.contains("`cargo test` `/usr` `https://a.b/c` `src/lib.rs` [`/a/b`](https://x) <sophonfile path=\"~/notes.md\">"), "{out}");
        assert!(out.ends_with("```\n`/a/b`\n```\n"), "{out}");
        assert_eq!(link_paths("`/a/b`", |_| false), "`/a/b`");
    }

    #[test]
    fn a_line_number_is_not_part_of_the_file() {
        assert_eq!(file_of("/a/b.rs:12:4"), "/a/b.rs");
        assert_eq!(file_of("/a/b.rs:12"), "/a/b.rs");
        assert_eq!(file_of("/a/b.rs"), "/a/b.rs");
        let seen = std::cell::RefCell::new(Vec::new());
        link_paths("`/a/b.rs:9`", |p| {
            seen.borrow_mut().push(p.to_owned());
            true
        });
        assert_eq!(seen.into_inner(), ["/a/b.rs"]);
    }

    #[test]
    fn double_backticks_are_left_alone() {
        assert_eq!(link_paths("``a ` /x/y`` and `/x/y`", all), "``a ` /x/y`` and <sophonfile path=\"/x/y\"></sophonfile>");
    }
}
