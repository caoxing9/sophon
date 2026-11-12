//! The open workspace's files, to read and to save on the phone: a page per
//! folder, then a file's page with its text and a download into Sophon's
//! folder of the Files app. Editing them is the desktop's.

use crate::i18n::t;
use nsh_base::{button, nav_page};
use nsh_core::{
    element::{FontWeight, Position, Screen},
    prelude::*,
};

use super::{Route, glass_button, push, top_bar};
use crate::daemon::connection::DirEntry;
use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;
use crate::ui::shared::{chips::format_size, preview};
use nsh_base::text_view;

/// Text shown at most: a longer file shows its start (it downloads whole).
const SHOWN_BYTES: usize = 200_000;

/// The last part of `path`, or the workspace's name for its root.
fn title_of(path: &str) -> String {
    if path == "." {
        return store::current_workspace().map(|w| w.name).unwrap_or_else(|| t("Files").into());
    }
    path.rsplit('/').next().unwrap_or(path).to_owned()
}

pub(crate) fn dir_page(dir: String) -> Screen {
    let title = title_of(&dir);
    nav_page().key(format!("files:{dir}")).header_visible(false).title(title.clone()).child(
        view()
            .flex_1()
            .child(top_bar(
                text(title).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1),
                view().w(px(44.0)),
            ))
            .child(view().flex_1().px_safe_area().child(listing(dir))),
    )
}

fn listing(dir: String) -> impl IntoElement {
    let entries = computed(move || store::dir_entries(&dir));
    view().flex_1().child(dynamic(move || {
        let state = entries.with(|e| e.as_ref().map(Vec::is_empty));
        Some(untracked(move || match state {
            None => view().flex_1().items_center().justify_center().pb(px(96.0)).child(loader(None)).into_element(),
            Some(true) => note(Icon::Folder, t("This folder is empty").into()),
            Some(false) => virtual_list(
                move || entries.with(|e| e.as_ref().map_or(0, Vec::len)),
                move |at: Position| {
                    let entry = computed(move || entries.with(|e| e.as_ref().and_then(|e| e.get(at.index()).cloned())));
                    entry_row(entry)
                },
            )
            .keys(move || entries.with(|e| e.iter().flatten().map(|e| e.path.clone()).collect::<Vec<_>>()))
            .estimated_item_size(px(52.0))
            .content_padding_bottom(px(S4))
            .flex_1()
            .into_element(),
        }))
    }))
}

fn entry_row(entry: Memo<Option<DirEntry>>) -> impl IntoElement {
    let is_dir = move || entry.with(|e| e.as_ref().is_some_and(|e| e.is_dir));
    button()
        .on_press(move || {
            let Some(e) = entry.get_untracked() else { return };
            push(if e.is_dir { Route::Files(e.path) } else { Route::File(e.path) });
        })
        .flex_row()
        .items_center()
        .gap(px(S3))
        .min_h(px(52.0))
        .px(px(S4))
        .active(|s| s.bg(highlight()))
        .child(dynamic(move || {
            let glyph = if is_dir() { Icon::Folder } else { Icon::File };
            Some(untracked(move || icon(glyph, 18.0).text_color(FG_MUTED).into_element()))
        }))
        .child(
            text(move || entry.with(|e| e.as_ref().map(|e| e.name.clone()).unwrap_or_default()))
                .text_size(px(TEXT_BASE))
                .number_of_lines(1)
                .flex_1(),
        )
        .child(
            text(move || entry.with(|e| e.as_ref().filter(|e| !e.is_dir).map(|e| format_size(e.size as usize)).unwrap_or_default()))
                .text_size(px(TEXT_SM))
                .text_color(FG_FAINT),
        )
        .child(icon(Icon::ChevronRight, 16.0).text_color(FG_FAINT))
}

pub(crate) fn file_page(path: String) -> Screen {
    // A PDF, an image, a video: fetched whole for its preview instead.
    if preview::kind_of(&path).is_none() {
        store::load_file(&path);
    }
    let name = title_of(&path);
    let dir = path.rsplit_once('/').map(|(d, _)| d.to_owned()).unwrap_or_default();
    let down = path.clone();
    let busy = {
        let p = path.clone();
        computed(move || store::downloading(&p))
    };
    nav_page().key(format!("file:{path}")).header_visible(false).title(name.clone()).child(
        view()
            .flex_1()
            .child(top_bar(
                view()
                    .items_center()
                    .child(text(name).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1))
                    .child(text(dir).text_size(px(TEXT_XS)).text_color(FG_MUTED).number_of_lines(1)),
                view()
                    .opacity(move || if busy.get() { 0.4_f32 } else { 1.0 })
                    .child(glass_button(Icon::Download, t("Download"), move || store::download_file(&down))),
            ))
            .child(view().flex_1().px_safe_area().pb_safe_area().child(content(path))),
    )
}

fn content(path: String) -> AnyElement {
    if preview::kind_of(&path).is_some() {
        return preview::file_preview(path).into_element();
    }
    let markdown = path.to_ascii_lowercase().ends_with(".md") || path.to_ascii_lowercase().ends_with(".markdown");
    let file = {
        let p = path.clone();
        computed(move || store::file(&p))
    };
    view().flex_1().child(dynamic(move || {
        let state = file.with(|f| f.as_ref().map(|f| (f.loaded, f.kind.clone(), f.error.clone())));
        let path = path.clone();
        Some(untracked(move || {
            let Some((loaded, kind, error)) = state else {
                return view().into_element();
            };
            if let Some(e) = error {
                // Too large to show, or unreadable as text: it still downloads.
                return download_note(path, e);
            }
            if !loaded {
                return view().flex_1().items_center().justify_center().pb(px(96.0)).child(loader(None)).into_element();
            }
            if kind != "text" {
                return download_note(path, t("This file can't be shown here").into());
            }
            let body = file.with_untracked(|f| f.as_ref().map(|f| f.disk.clone()).unwrap_or_default());
            let cut = body.len() > SHOWN_BYTES;
            let shown = if cut { body[..body.floor_char_boundary(SHOWN_BYTES)].to_owned() } else { body };
            // A Markdown file reads as the document it writes.
            if markdown {
                let doc = shown.clone();
                return scroll()
                    .flex_1()
                    .child(
                        view().p(px(S4)).child(
                            text_view()
                                .markdown(move || Some(SharedString::from(doc.clone())))
                                .selectable(true)
                                .gap_3()
                                .text_size(px(TEXT_CONTENT))
                                .line_height(px(24.0))
                                .text_color(FG),
                        ),
                    )
                    .into_element();
            }
            scroll()
                .flex_1()
                .child(
                    view()
                        .p(px(S4))
                        .gap(px(S3))
                        .child(
                            text(shown)
                                .text_size(px(TEXT_SM))
                                .line_height(px(19.0))
                                .font_family(mono_font)
                                .text_color(FG)
                                .selectable(true),
                        )
                        .when(cut, |v| {
                            v.child(text(t("The rest of the file is in its download")).text_size(px(TEXT_SM)).text_color(FG_MUTED))
                        }),
                )
                .into_element()
        }))
    }))
    .into_element()
}

/// A file shown by its name only: why, and its download.
fn download_note(path: String, why: String) -> AnyElement {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(S3))
        .p(px(S6))
        .pb(px(96.0))
        .child(icon(Icon::File, 40.0).text_color(FG_FAINT))
        .child(text(why).text_size(px(TEXT_BASE)).text_color(FG_MUTED).text_center())
        .child(btn(t("Download"), Variant::Primary, Sz::Md, Some(Icon::Download), move || store::download_file(&path)))
        .into_element()
}

fn note(glyph: Icon, message: String) -> AnyElement {
    view()
        .flex_row()
        .gap(px(S2))
        .p(px(S4))
        .child(icon(glyph, 16.0).text_color(FG_FAINT))
        .child(text(message).text_size(px(TEXT_BASE)).text_color(FG_MUTED).flex_1())
        .into_element()
}
