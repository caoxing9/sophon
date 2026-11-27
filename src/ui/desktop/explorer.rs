//! The Explorer beside a workspace (the right sidebar): its changed
//! files, live, every one's diff in one scroll; a file opens its diff as a
//! tab.

use crate::i18n::{t, tf};
use nsh_base::{DiffAnnotation, DiffLayout, DiffLineRange, DiffSide, button, diff_view, input};
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store::{self, ExplorerTab, Tab};

/// The explorer's edge: dragging it sets the width; dragged far enough
/// out (narrower than it may be), the explorer folds away, as the sidebar
/// does (⌘E or its toggle opens it again).
pub(crate) fn resize_handle() -> impl IntoElement {
    let start = signal(0.0f32);
    let hover = signal(false);
    let dragging = signal(false);
    let fold = store::EXPLORER_MIN - 60.0;
    view()
        .w(px(10.0))
        .flex_shrink_0()
        .self_stretch()
        .items_center()
        .justify_center()
        .cursor(Cursor::ColResize)
        .window_drag(false)
        .on_mouse_enter(move |_: &MouseEvent| hover.set(true))
        .on_mouse_leave(move |_: &MouseEvent| hover.set(false))
        .claim_touches(TouchClaim::on_start().ask(|request: &ClaimRequest| request.claim()))
        .keep_touches(true)
        .on_responder_grant(move |_: &ResponderEvent| {
            start.set(store::explorer_width());
            dragging.set(true);
        })
        // Its left edge: dragging left widens it.
        .on_responder_move(move |e: &ResponderEvent| {
            let w = start.get_untracked() - e.gesture.delta.x.0;
            if w >= fold {
                store::set_explorer_width(w, false);
            }
        })
        .on_responder_release(move |e: &ResponderEvent| {
            dragging.set(false);
            if start.get_untracked() - e.gesture.delta.x.0 < fold {
                store::set_explorer_width(start.get_untracked(), false);
                store::toggle_explorer();
            } else {
                store::set_explorer_width(store::explorer_width(), true);
            }
        })
        .on_responder_terminate(move |_: &ResponderEvent| dragging.set(false))
        .child(
            view()
                .w(px(3.0))
                .h(px(48.0))
                .rounded(px(1.0))
                .bg(move || tint(ACCENT(), if dragging.get() { 0.9 } else if hover.get() { 0.5 } else { 0.0 })),
        )
}

pub(crate) fn explorer() -> View {
    let tab = computed(store::explorer_tab);
    let width = computed(store::explorer_width);
    let page = move || Length::from(px(width.get()));
    glass(view())
        .w(page)
        .flex_shrink_0()
        .m(px(10.0))
        .ml(px(0.0))
        .rounded(px(R_GLASS))
        .overflow_hidden()
        .child(
            // In the title bar's height (the panel starts there), dragging
            // the window; its own toggle at the end puts it away.
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .pl(sp(S3))
                .pr(sp(S2))
                .pt(sp(S2))
                .pb(sp(S1))
                .window_drag(true)
                .child(segmented(
                    vec![
                        (ExplorerTab::Artifacts, t("Artifacts").into()),
                        (ExplorerTab::Changes, t("Changes").into()),
                        (ExplorerTab::Files, t("Files").into()),
                    ],
                    store::explorer_tab,
                    store::set_explorer_tab,
                ))
                .child(view().flex_1())
                .child(icon_btn(Icon::PanelRight, Sz::Sm, t("Toggle explorer"), store::toggle_explorer)),
        )
        // The pages stacked, only the picked one shown, switched at once as
        // T3 Code's, Paseo's and Lody's panels are. The others stay built and
        // laid out (each keeps its state and scroll) but invisible: their
        // NSViews hidden, nothing of theirs is drawn or walked by AppKit.
        // Their frames never change, so the switch's spring moves the thumb
        // alone.
        .child({
            let hidden = move |which: ExplorerTab| move || tab.get() != which;
            view()
                .flex_1()
                .relative()
                .overflow_hidden()
                .child(page_layer(super::artifacts::artifacts_view(computed(move || tab.get() == ExplorerTab::Artifacts)), hidden(ExplorerTab::Artifacts)))
                .child(page_layer(changes_view(), hidden(ExplorerTab::Changes)))
                .child(page_layer(files_view(), hidden(ExplorerTab::Files)))
        })
}

/// One of the Explorer's pages, filling it, invisible while `hidden` holds.
fn page_layer(page: View, hidden: impl Fn() -> bool + 'static) -> View {
    view().absolute().inset_0().style_when(hidden, |s| s.invisible()).child(page.flex_1())
}

/// Every changed file's diff, one under the other, each under its header
/// (pressed, it folds); read in one scroll rather than a file at a time.
/// The whole view, split or with line comments, is a tab away.
fn changes_view() -> View {
    let changes = computed(store::changes);
    let patch = computed(move || changes.with(|c| c.files.iter().map(|f| f.patch.clone()).collect::<String>()));
    let stats = computed(move || changes.with(|c| (c.files.len(), c.files.iter().fold((0, 0), |(a, d), f| (a + f.additions, d + f.deletions)))));
    // Rebuilt only as it changes state, not on each update of the patch.
    let state = computed(move || {
        changes.with(|c| match () {
            _ if !c.loaded => 0,
            _ if c.too_large => 1,
            _ if c.files.is_empty() => 2,
            _ => 3,
        })
    });
    view().flex_1().child(dynamic(move || {
        let state = state.get();
        Some(untracked(move || match state {
            0 => loading_state().into_element(),
            1 => empty_state(Icon::FileDiff, t("Too many changes to show"), t("Commit some of them, or look in a terminal.")).into_element(),
            2 => empty_state(Icon::FileDiff, t("No changes"), t("What the agents change shows here.")).into_element(),
            _ => view()
                .flex_1()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .h(px(30.0))
                        .pl(sp(S3))
                        .pr(sp(S2))
                        .child(
                            text(move || {
                                let n = stats.get().0;
                                if n == 1 { t("1 file").to_owned() } else { tf("{} files", &[&n.to_string()]) }
                            })
                            .text_size(tx(TEXT_SM))
                            .text_color(FG_MUTED()),
                        )
                        .child(text(move || format!("+{}", stats.get().1.0)).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(SUCCESS))
                        .child(text(move || format!("\u{2212}{}", stats.get().1.1)).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(DANGER))
                        .child(view().flex_1())
                        .child(icon_btn(Icon::Maximize2, Sz::Sm, t("Open in a tab"), || store::open_tab(Tab::Diff(None)))),
                )
                .child(
                    diff_view()
                        .patch(move || Some(SharedString::from(patch.get())))
                        .layout(DiffLayout::Unified)
                        .soft_wrap(true)
                        .text_size(tx(TEXT_SM))
                        .flex_1(),
                )
                .into_element(),
        }))
    }))
}

fn files_view() -> View {
    let rows = computed(store::tree_rows);
    view().flex_1().child(dynamic(move || {
        let empty = rows.with(Vec::is_empty);
        Some(untracked(move || {
            if empty {
                return loading_state().into_element();
            }
            virtual_list(move || rows.with(Vec::len), move |at: nsh_core::element::Position| {
                let row = computed(move || rows.with(|r| r.get(at.index()).cloned()));
                tree_row(row)
            })
            .keys(move || rows.with(|r| r.iter().map(|x| x.entry.path.clone()).collect::<Vec<_>>()))
            .estimated_item_size(px(28.0))
            .content_padding_top(sp(S1))
            .content_padding_bottom(sp(S2))
            .flex_1()
            .into_element()
        }))
    }))
}

fn tree_row(row: Memo<Option<store::TreeRow>>) -> impl IntoElement {
    nsh_base::context_menu()
        .popup_style(|s| {
            s.bg(POPOVER()).rounded(px(R_XL)).border_1().border_color(BORDER()).p(sp(S1)).min_w(px(200.0))
        })
        .trigger(tree_row_body(row))
        .child(dynamic(move || {
            let r = row.get()?;
            Some(untracked(move || {
                let e = r.entry.clone();
                let dir = if e.is_dir { e.path.clone() } else { e.path.rsplit_once('/').map(|(d, _)| d.to_owned()).unwrap_or_else(|| ".".into()) };
                let (d1, d2, p1, p2, p3, n1) = (dir.clone(), dir, e.path.clone(), e.path.clone(), e.path.clone(), e.name.clone());
                view()
                    .child(menu_row(t("New file…"), Some(Icon::File), None, false, move || {
                        store::open_modal(store::Modal::FileName { op: "create-file", path: d1.clone(), name: String::new() })
                    }))
                    .child(menu_row(t("New folder…"), Some(Icon::FolderPlus), None, false, move || {
                        store::open_modal(store::Modal::FileName { op: "create-dir", path: d2.clone(), name: String::new() })
                    }))
                    .child(menu_divider())
                    .child(menu_row(t("Rename…"), Some(Icon::Pencil), None, false, move || {
                        store::open_modal(store::Modal::FileName { op: "rename", path: p1.clone(), name: n1.clone() })
                    }))
                    .child(menu_row(t("Duplicate"), Some(Icon::Copy), None, false, move || store::file_op("duplicate", &p2, "")))
                    // A copy here, of a file only: the host's, which may be
                    // another computer.
                    .when(!e.is_dir, |v| {
                        let p4 = e.path.clone();
                        v.child(menu_row(t("Download…"), Some(Icon::Download), None, false, move || store::download_file(&p4)))
                    })
                    .child(menu_divider())
                    .child(menu_row_danger(t("Delete"), Icon::Trash2, move || store::file_op("delete", &p3, "")))
                    .into_element()
            }))
        }))
}

fn tree_row_body(row: Memo<Option<store::TreeRow>>) -> impl IntoElement {
    let selected = computed(move || {
        let path = row.with(|r| r.as_ref().map(|r| r.entry.path.clone()));
        path.is_some_and(|p| store::tab() == Tab::File(p))
    });
    button()
        .on_press(move || {
            if let Some(r) = row.get_untracked() {
                if r.entry.is_dir {
                    store::toggle_dir(&r.entry.path);
                } else {
                    store::open_file(&r.entry.path);
                }
            }
        })
        .mx(sp(S2))
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(28.0))
        .pl(move || px(S2 + row.with(|r| r.as_ref().map_or(0, |r| r.depth)) as f32 * 14.0))
        .pr(sp(S2))
        .rounded(px(R_LG))
        .bg(move || if selected.get() { highlight() } else { tint(SURFACE1(), 0.0) })
        .hover(|s| s.bg(highlight()))
        .child(dynamic(move || {
            let (dir, open) = row.with(|r| r.as_ref().map_or((false, false), |r| (r.entry.is_dir, r.expanded)));
            // As an editor's tree: a folder's chevron and its folder, a
            // file's room for a chevron and its page, the names in one
            // column whichever it is.
            Some(untracked(move || {
                let chevron = match (dir, open) {
                    (true, true) => Some(Icon::ChevronDown),
                    (true, false) => Some(Icon::ChevronRight),
                    _ => None,
                };
                let kind = match (dir, open) {
                    (true, true) => Icon::FolderOpen,
                    (true, false) => Icon::Folder,
                    _ => Icon::FileText,
                };
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(4.0))
                    .child(match chevron {
                        Some(c) => icon(c, 12.0).text_color(FG_FAINT()).into_element(),
                        None => view().w(px(12.0)).into_element(),
                    })
                    .child(icon(kind, 15.0).text_color(FG_MUTED()))
                    .into_element()
            }))
        }))
        .child(
            text(move || row.with(|r| r.as_ref().map(|r| r.entry.name.clone()).unwrap_or_default()))
                .text_size(tx(TEXT_BASE))
                .number_of_lines(1)
                .flex_1(),
        )
}

/// An empty panel's message, centered: a glyph on a soft disc, a title and
/// a line under it.
pub(super) fn empty_state(glyph: Icon, title: &'static str, detail: &'static str) -> View {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(sp(S2))
        .px(sp(S6))
        .pb(px(48.0))
        .child(
            view()
                .w(px(44.0))
                .h(px(44.0))
                .mb(sp(S1))
                .rounded(px(R_FULL))
                .items_center()
                .justify_center()
                .bg(SURFACE2())
                .child(icon(glyph, 20.0).text_color(FG_MUTED())),
        )
        .child(text(title).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)).text_color(FG()).text_center())
        .child(text(detail).text_size(tx(TEXT_SM)).line_height(px(19.0)).text_color(FG_MUTED()).text_center())
}

fn loading_state() -> View {
    view().flex_1().items_center().justify_center().pb(px(48.0)).child(spinner(28.0))
}

#[derive(Clone, Default)]
struct DiffPrefs {
    unified: bool,
}

/// A diff tab: one file's changes, or all of them.
pub(crate) fn diff_pane(path: Option<String>) -> impl IntoElement {
    let prefs = global::<DiffPrefs>();
    let note = signal(String::new());
    let review_path = path.clone();
    let send_path = path.clone();
    let path2 = path.clone();
    let files = computed(move || {
        store::changes().files.into_iter().filter(|f| path2.as_ref().is_none_or(|p| *p == f.path)).collect::<Vec<_>>()
    });
    let patch = computed(move || files.with(|fs| fs.iter().map(|f| f.patch.clone()).collect::<String>()));
    let stats = computed(move || files.with(|fs| fs.iter().fold((0, 0), |(a, d), f| (a + f.additions, d + f.deletions))));
    // Split only means something with both sides: a new or deleted file
    // shows one column.
    let one_sided = computed(move || files.with(|fs| !fs.is_empty() && fs.iter().all(|f| f.is_new || f.is_deleted || f.deletions == 0 || f.additions == 0)));
    let (name, dir) = match path.as_deref() {
        Some(p) => match p.rsplit_once('/') {
            Some((d, n)) => (n.to_owned(), Some(d.to_owned())),
            None => (p.to_owned(), None),
        },
        None => (t("All changes").to_owned(), None),
    };
    let language = path.as_deref().and_then(|p| p.rsplit_once('.')).map(|(_, ext)| ext.to_owned());
    // Line comments, as VS Code's and GitHub's reviews: the + beside a
    // line (or a selection of lines) opens a draft under it; the review
    // sends them all, with the note.
    let comments = signal(Vec::<LineComment>::new());
    let draft = signal(None::<DiffLineRange>);
    let draft_text = signal(String::new());
    let next_id = signal(0u32);
    let annotations = computed(move || {
        let mut list: Vec<DiffAnnotation> = comments.with(|c| c.iter().map(|c| annotation(&c.id, c.range)).collect());
        if let Some(r) = draft.get() {
            list.push(annotation("draft", r));
        }
        list
    });
    let send = move || {
        let mut text = String::new();
        for c in comments.get_untracked() {
            let side = if c.range.side == DiffSide::Old { " (old)" } else { "" };
            let lines = if c.range.start == c.range.end {
                format!("line {}", c.range.start + 1)
            } else {
                format!("lines {}–{}", c.range.start + 1, c.range.end + 1)
            };
            text.push_str(&format!("- {lines}{side}: {}\n", c.text.trim()));
        }
        let general = note.get_untracked();
        if !general.trim().is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(general.trim());
        }
        if !text.trim().is_empty() && store::send_review(send_path.as_deref(), &text) {
            note.set(String::new());
            comments.set(vec![]);
            draft.set(None);
        }
    };
    let send2 = send.clone();
    let header = view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .h(px(44.0))
        .pl(sp(S4))
        .pr(sp(S2))
        .border_b_1()
        .border_color(tint(BORDER(), 0.7))
        .child(icon(Icon::FileDiff, 15.0).text_color(FG_MUTED()))
        .child(text(name).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1).flex_shrink(1.0))
        .child(dir.map(|d| text(d).text_size(tx(TEXT_SM)).text_color(FG_FAINT()).number_of_lines(1).flex_shrink(1.0)))
        .child(view().flex_1())
        .child(
            view()
                .flex_row()
                .gap(px(6.0))
                .child(text(move || format!("+{}", stats.get().0)).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(SUCCESS))
                .child(text(move || format!("\u{2212}{}", stats.get().1)).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(DANGER)),
        )
        .child(dynamic(move || {
            (!one_sided.get()).then(|| untracked(move || {
                view()
                    .ml(sp(S2))
                    .child(segmented(
                        vec![(true, t("Unified").into()), (false, t("Split").into())],
                        move || prefs.with(|p| p.unified),
                        move |u| prefs.update(|p| p.unified = u),
                    ))
                    .into_element()
            }))
        }));
    let body = dynamic(move || {
        let empty = patch.with(|p| p.is_empty());
        let language = language.clone();
        Some(untracked(move || {
            if empty {
                return view()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .child(text(t("No changes")).text_size(tx(TEXT_BASE)).text_color(FG_MUTED()))
                    .into_element();
            }
            let mut d = diff_view()
                .patch(move || Some(SharedString::from(patch.get())))
                .layout(move || if one_sided.get() || prefs.with(|p| p.unified) { DiffLayout::Unified } else { DiffLayout::Split })
                .text_size(tx(TEXT_CODE))
                .annotations(move || annotations.get())
                .render_add_annotation(move |r: &DiffLineRange| {
                    let r = *r;
                    button()
                        .on_press(move || {
                            draft_text.set(String::new());
                            draft.set(Some(r));
                        })
                        .accessibility_label(t("Comment on these lines"))
                        .w(px(18.0))
                        .h(px(18.0))
                        .rounded(px(5.0))
                        .items_center()
                        .justify_center()
                        .bg(ACCENT())
                        .child(icon(Icon::Plus, 12.0).text_color(ACCENT_FG()))
                })
                .render_annotation(move |a: &DiffAnnotation| {
                    if a.id.as_ref() == "draft" {
                        comment_draft(draft, draft_text, move |text| {
                            let Some(range) = draft.get_untracked() else { return };
                            let id = next_id.get_untracked();
                            next_id.set(id + 1);
                            comments.update(|c| c.push(LineComment { id: format!("c{id}"), range, text }));
                            draft.set(None);
                        })
                        .into_element()
                    } else {
                        let id = a.id.to_string();
                        let c = comments.with_untracked(|c| c.iter().find(|c| c.id == id).cloned());
                        match c {
                            Some(c) => comment_card(c, move || comments.update(|list| list.retain(|x| x.id != id))).into_element(),
                            None => view().into_element(),
                        }
                    }
                })
                .flex_1();
            if let Some(lang) = language.clone() {
                d = d.language(SharedString::from(lang));
            }
            d.into_element()
        }))
    });
    let _ = review_path;
    let empty_note = computed(move || note.with(|n| n.trim().is_empty()) && comments.with(Vec::is_empty));
    let review = glass_strong(view())
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH()))
        .self_center()
        .h(px(48.0))
        .pl(sp(S4))
        .pr(px(6.0))
        .rounded(px(R_FULL))
        .child(icon(Icon::Pencil, 16.0).text_color(FG_MUTED()))
        .child(
            input()
                .flex_1()
                .text_size(tx(TEXT_BASE))
                .text_color(FG())
                .caret_color(|| color(ACCENT()))
                .selection_color(|| color(tint(ACCENT(), 0.3)))
                .value(move || note.get())
                .on_change(move |v: &str| note.set(v.to_owned()))
                .on_submit(move |_: &str| send())
                .placeholder(move || {
                    match comments.with(Vec::len) {
                        0 => t("Ask the agent about these changes, or + on a line to comment…").to_owned(),
                        1 => t("Send 1 line comment, with a note…").to_owned(),
                        n => tf("Send {} line comments, with a note…", &[&n.to_string()]),
                    }
                }),
        )
        .child(
            button()
                .on_press(move || send2())
                .accessibility_label(t("Send review"))
                .w(px(36.0))
                .h(px(36.0))
                .rounded(px(R_FULL))
                .items_center()
                .justify_center()
                .bg(move || if empty_note.get() { highlight() } else { ACCENT() })
                .transition_colors()
                .child(icon(Icon::ArrowUp, 17.0).text_color(move || if empty_note.get() { FG_FAINT() } else { ACCENT_FG() })),
        );
    view()
        .flex_1()
        .px(sp(S4))
        .pt(sp(S1))
        .pb(sp(S3))
        .gap(sp(S3))
        .child(glass(view()).flex_1().rounded(px(R_GLASS)).overflow_hidden().child(header).child(body))
        .child(review)
}

#[derive(Clone, PartialEq)]
struct LineComment {
    id: String,
    range: DiffLineRange,
    text: String,
}

fn annotation(id: &str, range: DiffLineRange) -> DiffAnnotation {
    let mut a = DiffAnnotation::default();
    a.id = id.to_owned().into();
    a.side = range.side;
    a.line = range.end;
    a
}

fn lines_label(r: DiffLineRange) -> String {
    if r.start == r.end { tf("Line {}", &[&(r.start + 1).to_string()]) } else { tf("Lines {}–{}", &[&(r.start + 1).to_string(), &(r.end + 1).to_string()]) }
}

/// A comment being written under its lines.
fn comment_draft(draft: Signal<Option<DiffLineRange>>, text_sig: Signal<String>, save: impl Fn(String) + Clone + 'static) -> View {
    let label = draft.get_untracked().map(lines_label).unwrap_or_default();
    let save2 = save.clone();
    let empty = computed(move || text_sig.with(|t| t.trim().is_empty()));
    comment_shell()
        .gap(sp(S2))
        .child(text(label).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).font_weight(FontWeight(600)))
        .child(
            field()
                .auto_focus(true)
                .value(move || text_sig.get())
                .on_change(move |v: &str| text_sig.set(v.to_owned()))
                .on_submit(move |_: &str| {
                    let t = text_sig.get_untracked();
                    if !t.trim().is_empty() {
                        save(t);
                    }
                })
                .placeholder(t("Leave a comment for the agent")),
        )
        .child(
            view()
                .flex_row()
                .justify_end()
                .gap(sp(S2))
                .child(btn(t("Cancel"), Variant::Ghost, Sz::Sm, None, move || draft.set(None)))
                .child(btn(t("Comment"), Variant::Primary, Sz::Sm, None, move || {
                    let t = text_sig.get_untracked();
                    if !t.trim().is_empty() {
                        save2(t);
                    }
                })
                .opacity(move || if empty.get() { 0.5_f32 } else { 1.0 })),
        )
}

/// A comment kept for the review.
fn comment_card(c: LineComment, remove: impl Fn() + 'static) -> View {
    comment_shell()
        .flex_row()
        .items_start()
        .gap(sp(S2))
        .child(
            view()
                .w(px(22.0))
                .h(px(22.0))
                .rounded(px(R_FULL))
                .bg(tint(ACCENT(), 0.15))
                .items_center()
                .justify_center()
                .child(icon(Icon::Pencil, 11.0).text_color(ACCENT())),
        )
        .child(
            view()
                .flex_1()
                .gap(px(2.0))
                .child(text(lines_label(c.range)).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).font_weight(FontWeight(600)))
                .child(text(c.text).text_size(tx(TEXT_SM)).text_color(FG())),
        )
        .child(icon_btn(Icon::Trash2, Sz::Sm, t("Delete comment"), remove))
}

fn comment_shell() -> View {
    view()
        .mx(sp(S3))
        .my(sp(S2))
        .p(sp(S3))
        .max_w(px(640.0))
        .rounded(px(R_LG))
        .bg(glass_fill_strong())
        .border_1()
        .border_color(glass_edge())
}

