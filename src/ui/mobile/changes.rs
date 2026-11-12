//! The open workspace's uncommitted changes, and one file's diff, unified,
//! to read: pages pushed over the conversation. Reviewing, committing and
//! pushing are the desktop's.

use crate::i18n::{t, tf};
use nsh_base::{DiffLayout, button, diff_view};
use nsh_core::{
    element::{FontWeight, Screen},
    prelude::*,
};

use super::{Route, push, top_bar};
use nsh_base::nav_page;
use crate::daemon::model::DiffFile;
use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;

pub(crate) fn files_page() -> Screen {
    nav_page().key("changes").header_visible(false).title(t("Changes")).child(
        view()
            .flex_1()
            .child(top_bar(
                text(t("Changes")).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)),
                // All the workspace's files, changed or not.
                super::glass_button(Icon::Folder, t("Browse files"), || push(Route::Files(".".into()))),
            ))
            .child(view().flex_1().px_safe_area().child(files())),
    )
}

pub(crate) fn diff_page(path: String) -> Screen {
    let name = path.rsplit('/').next().unwrap_or(&path).to_owned();
    let dir = path.rsplit_once('/').map(|(d, _)| d.to_owned()).unwrap_or_default();
    nav_page().key(format!("diff:{path}")).header_visible(false).title(name.clone()).child(
        view()
            .flex_1()
            .child(top_bar(
                view()
                    .items_center()
                    .child(text(name).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1))
                    .child(text(dir).text_size(tx(TEXT_XS)).text_color(FG_MUTED).number_of_lines(1)),
                view().w(px(44.0)),
            ))
            .child(view().flex_1().px_safe_area().pb_safe_area().child(diff(path))),
    )
}

fn files() -> impl IntoElement {
    let changes = computed(store::changes);
    view()
        .flex_1()
        .child(
            text(move || {
                changes.with(|c| {
                    let (a, d) = c.files.iter().fold((0, 0), |(a, d), f| (a + f.additions, d + f.deletions));
                    match c.files.len() {
                        0 => String::new(),
                        1 => format!("{} · +{a} −{d}", t("1 file")),
                        n => format!("{} · +{a} −{d}", tf("{} files", &[&n.to_string()])),
                    }
                })
            })
            .px(sp(S4))
            .py(sp(S2))
            .text_size(tx(TEXT_SM))
            .text_color(FG_MUTED),
        )
        .child(dynamic(move || {
            let (loaded, empty, error) = changes.with(|c| (c.loaded, c.files.is_empty(), c.error.clone()));
            Some(untracked(move || {
                let note = |glyph: Icon, message: String| {
                    view()
                        .flex_row()
                        .gap(sp(S2))
                        .p(sp(S4))
                        .child(icon(glyph, 16.0).text_color(FG_FAINT))
                        .child(text(message).text_size(tx(TEXT_BASE)).text_color(FG_MUTED).flex_1())
                        .into_element()
                };
                if let Some(e) = error.clone() {
                    return note(Icon::CircleAlert, e);
                }
                if !loaded {
                    return view().flex_1().items_center().justify_center().pb(px(96.0)).child(loader(None)).into_element();
                }
                if empty {
                    return note(Icon::CircleCheck, t("No uncommitted changes").into());
                }
                virtual_list(
                    move || changes.with(|c| c.files.len()),
                    move |at: nsh_core::element::Position| {
                        let f = computed(move || changes.with(|c| c.files.get(at.index()).cloned()));
                        file_row(f)
                    },
                )
                .keys(move || changes.with(|c| c.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>()))
                .estimated_item_size(px(56.0))
                .content_padding_bottom(sp(S4))
                .flex_1()
                .into_element()
            }))
        }))
}

fn file_row(file: Memo<Option<DiffFile>>) -> impl IntoElement {
    let split = move |f: &DiffFile| match f.path.rsplit_once('/') {
        Some((dir, name)) => (name.to_owned(), dir.to_owned()),
        None => (f.path.clone(), String::new()),
    };
    button()
        .on_press(move || {
            if let Some(f) = file.get_untracked() {
                push(Route::Diff(f.path));
            }
        })
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .min_h(px(56.0))
        .px(sp(S4))
        .active(|s| s.bg(highlight()))
        .child(
            text(move || {
                file.with(|f| match f {
                    Some(f) if f.is_new => "A",
                    Some(f) if f.is_deleted => "D",
                    Some(f) if f.old_path.is_some() => "R",
                    _ => "M",
                })
            })
            .w(px(14.0))
            .text_size(tx(TEXT_SM))
            .font_weight(FontWeight(600))
            .font_family(mono_font)
            .text_color(move || {
                file.with(|f| match f {
                    Some(f) if f.is_new => SUCCESS,
                    Some(f) if f.is_deleted => DANGER,
                    _ => WARNING,
                })
            }),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(2.0))
                .child(text(move || file.with(|f| f.as_ref().map(|f| split(f).0).unwrap_or_default())).text_size(tx(TEXT_BASE)).number_of_lines(1))
                .child(
                    text(move || file.with(|f| f.as_ref().map(|f| split(f).1).unwrap_or_default()))
                        .text_size(tx(TEXT_SM))
                        .text_color(FG_FAINT)
                        .number_of_lines(1),
                ),
        )
        .child(
            text(move || file.with(|f| f.as_ref().map(|f| format!("+{} −{}", f.additions, f.deletions)).unwrap_or_default()))
                .text_size(tx(TEXT_SM))
                .font_family(mono_font)
                .text_color(FG_MUTED),
        )
        .child(icon(Icon::ChevronRight, 16.0).text_color(FG_FAINT))
}

fn diff(path: String) -> impl IntoElement {
    let p = path.clone();
    let patch = computed(move || store::changes().files.into_iter().find(|f| f.path == p).map(|f| f.patch).unwrap_or_default());
    let language = path.rsplit_once('.').map(|(_, ext)| ext.to_owned());
    view()
        .flex_1()
        .child(dynamic(move || {
            let empty = patch.with(String::is_empty);
            let language = language.clone();
            Some(untracked(move || {
                if empty {
                    return view()
                        .flex_1()
                        .items_center()
                        .justify_center()
                        .child(text(t("No changes")).text_size(tx(TEXT_BASE)).text_color(FG_MUTED))
                        .into_element();
                }
                let mut d = diff_view()
                    .patch(move || Some(SharedString::from(patch.get())))
                    .layout(|| DiffLayout::Unified)
                    .text_size(tx(TEXT_CODE))
                    .flex_1();
                if let Some(lang) = language.clone() {
                    d = d.language(SharedString::from(lang));
                }
                d.into_element()
            }))
        }))
}
