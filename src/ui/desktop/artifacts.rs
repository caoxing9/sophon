//! The explorer's "Artifacts" page (as T3 Code's artifact wall): what the
//! open conversation made, or every conversation of its project. A card
//! shows the one picked (the newest picture to begin with) with what can be
//! done with it; under it the files by day, each its kind, name, folder
//! and time.

use crate::daemon::connection::Artifact;
use crate::i18n::{t, tf};
use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;
use nsh_core::{element::FontWeight, prelude::*};

/// The kinds a list can be narrowed to, in order, with their words.
const KINDS: [(&str, &str); 4] = [("image", "Images"), ("page", "Pages"), ("video", "Videos"), ("document", "Documents")];

pub(crate) fn artifacts_view(shown: Memo<bool>) -> View {
    // Asked again when the page shows, the conversation changes, or its
    // turn ends (what it made is there then).
    let agent = computed(|| store::selected_agent().map(|a| (a.id.clone(), a.is_running())));
    effect(move || {
        if shown.get() {
            agent.with(|_| ());
            untracked(store::load_artifacts);
        }
    });
    let all = computed(store::artifacts);
    let kind = computed(store::artifacts_kind);
    let listed = computed(move || {
        let wanted = kind.get();
        all.get().unwrap_or_default().into_iter().filter(|a| wanted.as_deref().is_none_or(|k| a.kind == k)).collect::<Vec<_>>()
    });
    let picked = computed(move || {
        let list = listed.get();
        let chosen = store::picked_artifact();
        chosen
            .and_then(|path| list.iter().find(|a| a.path == path).cloned())
            .or_else(|| list.iter().find(|a| a.kind == "image").cloned())
    });
    // One slim row, as the Changes page's: the scope as two words (the
    // one shown in full colour), how many, and refresh. The kinds only
    // when there are two to choose between.
    view()
        .flex_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S3))
                .h(px(32.0))
                .pl(sp(S4))
                .pr(sp(S2))
                .child(scope_word(false, t("This conversation")))
                .child(scope_word(true, t("This project")))
                .child(view().flex_1())
                .child(
                    text(move || all.with(|a| a.as_ref().filter(|l| !l.is_empty()).map(|l| l.len().to_string()).unwrap_or_default()))
                        .text_size(tx(TEXT_SM))
                        .font_family(mono_font)
                        .text_color(FG_FAINT),
                )
                .child(icon_btn(Icon::RotateCw, Sz::Sm, t("Refresh"), store::load_artifacts)),
        )
        .child(dynamic(move || {
            let list = all.get().unwrap_or_default();
            let now = kind.get();
            let kinds = KINDS.iter().filter(|(k, _)| list.iter().any(|a| a.kind == *k)).count();
            (kinds > 1).then(|| untracked(move || kind_chips(&list, now)).into_element())
        }))
        .child(view().flex_1().child(dynamic(move || {
            let Some(_) = all.get() else {
                return Some(untracked(|| view().py(sp(S8)).items_center().child(loading()).into_element()));
            };
            let list = listed.get();
            let card = picked.get();
            Some(untracked(move || {
                if list.is_empty() {
                    return super::explorer::empty_state(
                        Icon::Layers,
                        t("Nothing made yet"),
                        t("Files a conversation writes or names (pages, pictures, documents) show here once they are on the computer"),
                    )
                    .into_element();
                }
                let mut page = view().px(sp(S3)).pt(sp(S1)).pb(sp(S4)).gap(sp(S1));
                if let Some(card) = card {
                    page = page.child(preview_card(card));
                }
                let mut day = String::new();
                for artifact in list {
                    let this = day_of(artifact.modified);
                    if this != day {
                        page = page.child(text(this.clone()).text_size(tx(TEXT_XS)).text_color(FG_FAINT).px(sp(S1)).pt(sp(S3)).pb(sp(S1)));
                        day = this;
                    }
                    page = page.child(artifact_row(artifact));
                }
                scroll().flex_1().child(page).into_element()
            }))
        })))
}

/// One of the scope's two words: pressed, the list is that scope's.
fn scope_word(project: bool, label: &'static str) -> impl IntoElement {
    let on = computed(move || store::artifacts_project() == project);
    nsh_base::button()
        .on_press(move || store::set_artifacts_scope(project))
        .accessibility_label(label)
        .h(px(32.0))
        .justify_center()
        .child(
            text(label)
                .text_size(tx(TEXT_SM))
                .font_weight(FontWeight(500))
                .text_color(move || if on.get() { FG } else { FG_FAINT }),
        )
}

/// "All N", then each kind there is with its count; the one shown tinted.
fn kind_chips(list: &[Artifact], now: Option<String>) -> View {
    let mut row = view().flex_row().flex_wrap().gap(sp(S1)).px(sp(S3)).pb(sp(S2));
    let chip = |label: String, on: bool, pick: Option<String>| {
        nsh_base::button()
            .on_press(move || store::set_artifacts_kind(pick.clone()))
            .h(px(22.0))
            .px(sp(S2))
            .rounded(px(R_MD))
            .items_center()
            .justify_center()
            .bg(if on { tint(FG, 0.08) } else { tint(FG, 0.0) })
            .hover(|s| s.bg(tint(FG, 0.05)))
            .child(text(label).text_size(tx(TEXT_XS)).font_weight(FontWeight(500)).text_color(if on { FG } else { FG_MUTED }))
    };
    row = row.child(chip(tf("All {}", &[&list.len().to_string()]), now.is_none(), None));
    for (kind, word) in KINDS {
        let count = list.iter().filter(|a| a.kind == kind).count();
        if count > 0 {
            row = row.child(chip(format!("{} {count}", t(word)), now.as_deref() == Some(kind), Some(kind.to_owned())));
        }
    }
    row
}

/// The one picked: a picture shown, else its kind; its name, size and
/// time; and what can be done with it.
fn preview_card(artifact: Artifact) -> impl IntoElement {
    let here = store::on_this_mac();
    let media = if artifact.kind == "image" && here {
        view()
            .h(px(170.0))
            .bg(tint(FG, 0.04))
            .overflow_hidden()
            .child(image(SharedString::from(format!("file://{}", artifact.path))).w_full().h_full().object_fit(nsh_core::ResizeMode::Cover))
    } else {
        view()
            .h(px(96.0))
            .bg(tint(FG, 0.04))
            .items_center()
            .justify_center()
            .child(text(badge(&artifact)).text_size(tx(TEXT_LG)).font_weight(FontWeight(600)).text_color(FG_MUTED).font_family(mono_font))
    };
    let meta = match (artifact.width, artifact.height) {
        (Some(w), Some(h)) => format!("{w} × {h} · {}", stamp(artifact.modified)),
        _ => format!("{} · {}", size(artifact.size), stamp(artifact.modified)),
    };
    let (attach, reveal, opened) = (artifact.path.clone(), artifact.path.clone(), artifact.path.clone());
    let wide = |label: &'static str, glyph: Icon, filled: bool, press: Box<dyn Fn()>| {
        nsh_base::button()
            .on_press(move || press())
            .accessibility_label(label)
            .flex_1()
            .h(px(32.0))
            .rounded(px(R_LG))
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(6.0))
            .bg(if filled { ACCENT.into() } else { tint(FG, 0.06) })
            .hover(|s| s.opacity(0.85))
            .child(icon(glyph, 14.0).text_color(if filled { ACCENT_FG } else { FG }))
            .child(text(label).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(if filled { ACCENT_FG } else { FG }))
    };
    let mut actions = view().flex_row().gap(sp(S2));
    if here {
        actions = actions
            .child(wide(t("Attach to message"), Icon::Plus, true, Box::new(move || crate::ui::shared::composer::dropped(&[format!("file://{attach}")]))))
            .child(wide(t("Show in Finder"), Icon::FolderOpen, false, Box::new(move || store::reveal_path(&reveal))));
    }
    let page = artifact.kind == "page" && here;
    card()
        .mb(sp(S2))
        .child(media)
        .child(
            view()
                .p(sp(S3))
                .gap(sp(S2))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(text(artifact.name.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1).flex_1())
                        .when(page, |v| v.child(icon_btn(Icon::ExternalLink, Sz::Sm, t("Open"), move || store::open_artifact(&opened)))),
                )
                .child(text(meta).text_size(tx(TEXT_XS)).text_color(FG_MUTED))
                .child(actions)
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(5.0))
                        .child(icon(Icon::Info, 12.0).text_color(FG_FAINT))
                        .child(text(t("A file being there does not mean it was checked")).text_size(tx(TEXT_XS)).text_color(FG_FAINT)),
                ),
        )
}

/// A file of the list: a thumbnail (a picture's) or its kind, its name and
/// folder, its time; pressed, it is the card's.
fn artifact_row(artifact: Artifact) -> impl IntoElement {
    let path = artifact.path.clone();
    let on = {
        let path = path.clone();
        computed(move || store::picked_artifact().as_deref() == Some(path.as_str()))
    };
    let lead = if artifact.kind == "image" && store::on_this_mac() {
        view()
            .w(px(40.0))
            .h(px(28.0))
            .rounded(px(R_SM))
            .overflow_hidden()
            .bg(tint(FG, 0.05))
            .child(image(SharedString::from(format!("file://{}", artifact.path))).w_full().h_full().object_fit(nsh_core::ResizeMode::Cover))
    } else {
        view()
            .w(px(40.0))
            .h(px(28.0))
            .rounded(px(R_SM))
            .bg(tint(ACCENT, 0.10))
            .items_center()
            .justify_center()
            .child(text(badge(&artifact)).text_size(px(9.0)).font_weight(FontWeight(700)).text_color(ACCENT))
    };
    nsh_base::button()
        .on_press(move || store::pick_artifact(Some(path.clone())))
        .accessibility_label(artifact.name.clone())
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .px(sp(S1))
        .py(px(5.0))
        .rounded(px(R_LG))
        .bg(move || if on.get() { highlight() } else { tint(FG, 0.0) })
        .hover(|s| s.bg(highlight()))
        .child(lead)
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(1.0))
                .child(text(artifact.name.clone()).text_size(tx(TEXT_SM)).text_color(FG).number_of_lines(1))
                .when(!artifact.folder.is_empty(), |v| v.child(text(artifact.folder.clone()).text_size(tx(TEXT_XS)).text_color(FG_FAINT).number_of_lines(1))),
        )
        .child(text(clock(artifact.modified)).text_size(tx(TEXT_XS)).font_family(mono_font).text_color(FG_FAINT))
}

/// A file's kind in a few letters: its extension, as T3 Code's badge.
fn badge(artifact: &Artifact) -> String {
    artifact.extension.to_ascii_uppercase().chars().take(4).collect()
}

/// Local (seconds since the epoch, the day and the time of day) of Unix ms.
fn local(ms: i64) -> (i64, i64) {
    let utc = ms.div_euclid(1000);
    let local = utc + crate::usage::local_offset(utc);
    (local.div_euclid(86_400), local.rem_euclid(86_400))
}

/// "Today", "Yesterday" or the date, for the day a file changed.
fn day_of(ms: i64) -> String {
    let (day, _) = local(ms);
    let (today, _) = local(crate::usage::now() * 1000);
    match today - day {
        0 => t("Today").to_owned(),
        1 => t("Yesterday").to_owned(),
        _ => {
            let (year, month, date) = crate::usage::civil(day);
            format!("{year}-{month:02}-{date:02}")
        }
    }
}

fn clock(ms: i64) -> String {
    let (_, of_day) = local(ms);
    format!("{:02}:{:02}", of_day / 3600, of_day % 3600 / 60)
}

fn stamp(ms: i64) -> String {
    let (day, _) = local(ms);
    let (_, month, date) = crate::usage::civil(day);
    format!("{month}/{date} {}", clock(ms))
}

fn size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else {
        format!("{} KB", bytes.div_ceil(1024).max(1))
    }
}
