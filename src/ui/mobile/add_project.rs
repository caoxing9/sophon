//! Adding a project from the phone: the computer's folders, browsed from
//! its home folder (a tap goes in, the bar goes up) or found by typing a
//! path, and the folder shown added as a project.

use crate::i18n::{t, tf};
use nsh_base::{button, input, nav_page};
use nsh_core::{
    element::{FontWeight, Screen},
    prelude::*,
};

use super::{back, top_bar};
use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;

pub(crate) fn page() -> Screen {
    let query = signal(String::new());
    store::browse(&store::host_home());
    let browse = computed(store::browse_state);
    let folder_name = computed(move || {
        browse.with(|b| b.cwd.trim_end_matches('/').rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("/").to_owned())
    });
    nav_page().key("add-project").header_visible(false).title(t("Add project")).child(
        view()
            .flex_1()
            .child(top_bar(text(t("Add project")).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)), view().w(px(44.0))))
            .child(
                view().px(px(S4)).pb(px(S2)).child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .h(px(44.0))
                        .px(px(S4))
                        .rounded(px(R_FULL))
                        .bg(SURFACE2)
                        .child(icon(Icon::Search, 18.0).text_color(FG_MUTED))
                        .child(
                            input()
                                .flex_1()
                                .text_size(px(TEXT_LG))
                                .text_color(FG)
                                .value(move || query.get())
                                .on_change(move |v: &str| query.set(v.to_owned()))
                                .placeholder(t("Find a folder on the computer")),
                        ),
                ),
            )
            .child(view().flex_1().px_safe_area().child(dynamic(move || {
                let searching = query.with(|q| !q.trim().is_empty());
                Some(untracked(move || if searching { found(query).into_element() } else { folders(browse).into_element() }))
            })))
            .child(
                view().px(px(S4)).pt(px(S2)).pb_safe_area_plus(px(S2)).child(
                    liquid_glass(view()).rounded(px(R_FULL)).child(
                        button()
                            .on_press(move || {
                                let cwd = browse.with_untracked(|b| b.cwd.clone());
                                if cwd.len() > 1 {
                                    store::add_project(&cwd);
                                    store::notify(tf("Added {}", &[&cwd]));
                                    back();
                                }
                            })
                            .h(px(52.0))
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .gap(px(S2))
                            .rounded(px(R_FULL))
                            .bg(ACCENT)
                            .active(|s| s.opacity(0.8))
                            .child(icon(Icon::FolderPlus, 18.0).text_color(ACCENT_FG))
                            .child(
                                text(move || tf("Add “{}”", &[&folder_name.get()]))
                                    .text_size(px(TEXT_LG))
                                    .font_weight(FontWeight(600))
                                    .text_color(ACCENT_FG)
                                    .number_of_lines(1),
                            ),
                    ),
                ),
            ),
    )
}

/// The browsed folder: its path (a tap on the arrow goes up) and its
/// folders.
fn folders(browse: Memo<store::Browse>) -> impl IntoElement {
    scroll().flex_1().child(
        view()
            .px(px(S4))
            .gap(px(S2))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(S2))
                    .px(px(S1))
                    .child(
                        button()
                            .on_press(move || {
                                let cwd = browse.with_untracked(|b| b.cwd.clone());
                                let parent = cwd.trim_end_matches('/').rsplit_once('/').map(|(p, _)| p).filter(|p| !p.is_empty()).unwrap_or("/").to_owned();
                                store::browse(&parent);
                            })
                            .accessibility_label(t("Up"))
                            .w(px(32.0))
                            .h(px(32.0))
                            .rounded(px(R_FULL))
                            .items_center()
                            .justify_center()
                            .bg(SURFACE2)
                            .child(icon(Icon::ArrowUp, 16.0).text_color(FG)),
                    )
                    .child(
                        text(move || browse.with(|b| b.cwd.clone()))
                            .flex_1()
                            .text_size(px(TEXT_SM))
                            .font_family(|| MONO)
                            .text_color(FG_MUTED)
                            .number_of_lines(1),
                    ),
            )
            .child(dynamic(move || {
                let b = browse.get();
                Some(untracked(move || {
                    if let Some(e) = b.error.clone() {
                        return text(e).p(px(S4)).text_size(px(TEXT_BASE)).text_color(DANGER).into_element();
                    }
                    if !b.loaded {
                        return view().pt(px(S8)).items_center().child(loader(None)).into_element();
                    }
                    if b.folders.is_empty() {
                        return text(t("No folders here")).p(px(S4)).text_size(px(TEXT_BASE)).text_color(FG_MUTED).into_element();
                    }
                    list(b.folders.clone()).into_element()
                }))
            }))
            .child(view().h(px(S4))),
    )
}

/// Folders matching what is typed (paths from the home folder).
fn found(query: Signal<String>) -> impl IntoElement {
    let results = computed(move || store::find_dirs(query.get().trim()));
    scroll().flex_1().child(view().px(px(S4)).pb(px(S4)).child(dynamic(move || {
        let list_of = results.get();
        Some(untracked(move || {
            if list_of.is_empty() {
                return text(t("No matching folders")).p(px(S4)).text_size(px(TEXT_BASE)).text_color(FG_MUTED).into_element();
            }
            list(list_of.clone()).into_element()
        }))
    })))
}

/// Folders as a card of rows; a tap goes into one.
fn list(folders: Vec<String>) -> View {
    let mut c = card();
    for (i, path) in folders.into_iter().enumerate() {
        let name = path.trim_end_matches('/').rsplit('/').next().unwrap_or(&path).to_owned();
        let open = path.clone();
        c = c.child(
            button()
                .on_press(move || store::browse(&open))
                .flex_row()
                .items_center()
                .gap(px(S3))
                .min_h(px(52.0))
                .px(px(S4))
                .when(i > 0, |v| v.border_t_1().border_color(BORDER))
                .active(|s| s.bg(highlight()))
                .child(icon(Icon::Folder, 20.0).text_color(ACCENT))
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .child(text(name).text_size(px(TEXT_LG)).number_of_lines(1))
                        .child(text(path).text_size(px(TEXT_XS)).text_color(FG_FAINT).number_of_lines(1)),
                )
                .child(icon(Icon::ChevronRight, 16.0).text_color(FG_FAINT)),
        );
    }
    c
}
