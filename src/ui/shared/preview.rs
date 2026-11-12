//! A file of the workspace shown as what it is, not as text: a PDF, an
//! image, a video or a sound, fetched whole from the host (the read a text
//! file takes stops at 2 MB). The system draws them: WebKit the documents
//! and images (with scripts off), AVFoundation the media. Anything else
//! offers its download (and, on the Mac, its own app).

use crate::i18n::{t, tf};
use base64::Engine as _;
use nsh_core::prelude::*;

use crate::store::{self, Preview};
use crate::ui::kit::text;
use crate::ui::kit::*;

/// How a file is previewed, by its extension.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Kind {
    /// In WebKit, from a `data:` URL: PDFs and images (SVG, HEIC, TIFF too).
    Page(&'static str),
    /// In the system's player, from a file in the temporary folder.
    Media,
}

/// The preview `path` gets, from its extension; `None` for text (shown as
/// it is) and kinds the system does not draw.
pub(crate) fn kind_of(path: &str) -> Option<Kind> {
    let ext = path.rsplit_once('.')?.1.to_ascii_lowercase();
    let mime = match ext.as_str() {
        "pdf" => "application/pdf",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "heic" => "image/heic",
        "heif" => "image/heif",
        "tif" | "tiff" => "image/tiff",
        "avif" => "image/avif",
        "mp4" | "m4v" | "mov" | "webm" | "mp3" | "m4a" | "wav" | "aac" | "flac" | "aiff" | "aif" => return Some(Kind::Media),
        _ => return None,
    };
    Some(Kind::Page(mime))
}

/// The preview of the workspace's file `path` ([`kind_of`] gave it one).
pub(crate) fn file_preview(path: String) -> impl IntoElement {
    let p = path.clone();
    let state = computed(move || store::preview(&p));
    view().flex_1().child(dynamic(move || {
        let state = state.get();
        let path = path.clone();
        Some(untracked(move || match state {
            Preview::Loading => view().flex_1().items_center().justify_center().child(loader(None)).into_element(),
            Preview::Failed(e) => fallback(path, Icon::CircleAlert, e),
            Preview::TooLarge => fallback(path, Icon::File, t("Too large to preview here").into()),
            Preview::Ready(bytes) => match kind_of(&path) {
                // The system's PDF view: WebKit's showed some PDFs blank here.
                Some(Kind::Page("application/pdf")) if cfg!(any(target_os = "macos", target_os = "ios")) => match temp_copy(&path, &bytes) {
                    Ok(file) => native(crate::platform::PDF_VIEW)
                        .prop("path", nsh_core::JsonValue::String(file.to_string_lossy().into_owned()))
                        .flex_1()
                        .into_element(),
                    Err(e) => fallback(path, Icon::CircleAlert, e),
                },
                Some(Kind::Page(mime)) => {
                    let url = format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes.as_slice()));
                    web_view(url).java_script_enabled(false).incognito(true).flex_1().into_element()
                }
                Some(Kind::Media) => match temp_copy(&path, &bytes) {
                    Ok(file) => video(file_url(&file)).flex_1().into_element(),
                    Err(e) => fallback(path, Icon::CircleAlert, e),
                },
                None => fallback(path, Icon::File, t("This file can't be shown here").into()),
            },
        }))
    }))
}

/// A file shown by its name only: why, its download, and on the Mac its own
/// app.
pub(crate) fn fallback(path: String, glyph: Icon, why: String) -> AnyElement {
    let down = path.clone();
    let mut actions = view()
        .flex_row()
        .gap(sp(S2))
        .child(btn(t("Download"), Variant::Primary, Sz::Md, Some(Icon::Download), move || store::download_file(&down)));
    if !MOBILE {
        actions = actions.child(btn(t("Open in its app"), Variant::Secondary, Sz::Md, None, move || open_in_app(path.clone())));
    }
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(sp(S3))
        .p(sp(S6))
        .child(icon(glyph, 40.0).text_color(FG_FAINT))
        .child(text(why).text_size(tx(TEXT_BASE)).text_color(FG_MUTED).text_center())
        .child(actions)
        .into_element()
}

/// The Mac: a copy in the temporary folder, opened in the app the system
/// picks for it (Preview, Word, Numbers, ...), once it has arrived.
pub(crate) fn open_in_app(path: String) {
    if store::preview(&path) == Preview::Loading {
        store::notify(tf("Fetching “{}”…", &[path.rsplit('/').next().unwrap_or(&path)]));
    }
    open_when_ready(path, 400);
}

/// Opens `path` once its bytes are here, looking every 300 ms, `tries` times.
fn open_when_ready(path: String, tries: u32) {
    match store::preview(&path) {
        Preview::Ready(bytes) => match temp_copy(&path, &bytes) {
            Ok(file) => {
                let _ = open_path(file);
            }
            Err(e) => store::notify(e),
        },
        Preview::TooLarge => store::notify(t("Too large to open here: download it").into()),
        Preview::Failed(e) => store::notify(e),
        Preview::Loading if tries > 0 => {
            set_timeout(std::time::Duration::from_millis(300), move || open_when_ready(path, tries - 1));
        }
        Preview::Loading => {}
    }
}

/// `bytes` as a file named as `path` in the temporary folder (a folder of
/// its own per path, so two files of one name do not meet).
fn temp_copy(path: &str, bytes: &[u8]) -> Result<std::path::PathBuf, String> {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    let dir = std::env::temp_dir().join("sophon-preview").join(format!("{:016x}", h.finish()));
    let file = dir.join(path.rsplit('/').next().unwrap_or(path));
    std::fs::create_dir_all(&dir)
        .and_then(|_| std::fs::write(&file, bytes))
        .map_err(|e| tf("Could not preview: {}", &[&e.to_string()]))?;
    Ok(file)
}

/// `file://` and the path, percent-encoded (spaces, CJK names).
fn file_url(file: &std::path::Path) -> String {
    let mut url = String::from("file://");
    for b in file.to_string_lossy().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'.' | b'-' | b'_' | b'~' => url.push(b as char),
            _ => url.push_str(&format!("%{b:02X}")),
        }
    }
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_get_the_preview_their_kind_takes() {
        assert_eq!(kind_of("docs/Resume.PDF"), Some(Kind::Page("application/pdf")));
        assert_eq!(kind_of("a/b/photo.jpeg"), Some(Kind::Page("image/jpeg")));
        assert_eq!(kind_of("clip.mov"), Some(Kind::Media));
        assert_eq!(kind_of("src/main.rs"), None);
        assert_eq!(kind_of("Makefile"), None);
    }

    #[test]
    fn a_file_url_encodes_spaces_and_cjk() {
        assert_eq!(file_url(std::path::Path::new("/tmp/a b/简历.pdf")), "file:///tmp/a%20b/%E7%AE%80%E5%8E%86.pdf");
    }
}
