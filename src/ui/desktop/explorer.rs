//! The Explorer beside a workspace (Paseo's right sidebar): its changed
//! files, live; a file opens its diff as a tab.

use crate::i18n::{t, tf};
use nsh_base::{DiffAnnotation, DiffLayout, DiffLineRange, DiffSide, button, diff_view, input};
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::{
    daemon::model::DiffFile,
    store::{self, ExplorerTab, Tab},
};

pub(crate) const EXPLORER_WIDTH: f32 = 300.0;

pub(crate) fn explorer() -> View {
    let tab = computed(store::explorer_tab);
    glass(view())
        .w(px(EXPLORER_WIDTH))
        .flex_shrink_0()
        .m(px(10.0))
        .ml(px(0.0))
        .rounded(px(R_GLASS))
        .overflow_hidden()
        .child(
            view().px(px(S3)).pt(px(S2)).pb(px(S1)).child(segmented(
                vec![
                    (ExplorerTab::Changes, t("Changes").into()),
                    (ExplorerTab::Files, t("Files").into()),
                    (ExplorerTab::Commits, t("Commits").into()),
                ],
                store::explorer_tab,
                store::set_explorer_tab,
            )),
        )
        // The three pages side by side, the row sliding to the picked one
        // with the thumb above, as a pager does; each keeps its scroll.
        .child(
            view().flex_1().overflow_hidden().child(
                view()
                    .flex_1()
                    .flex_row()
                    .w(px(EXPLORER_WIDTH * 3.0))
                    // Moved by its frame, not a transform, so the pages
                    // coming in are laid out where they show; the switch's
                    // layout spring (the thumb's) carries it.
                    .ml(move || {
                        let i = match tab.get() {
                            ExplorerTab::Changes => 0.0,
                            ExplorerTab::Files => 1.0,
                            ExplorerTab::Commits => 2.0,
                        };
                        Length::from(px(-i * EXPLORER_WIDTH))
                    })
                    .child(view().w(px(EXPLORER_WIDTH)).child(changes_view().flex_1()))
                    .child(view().w(px(EXPLORER_WIDTH)).child(files_view().flex_1()))
                    .child(view().w(px(EXPLORER_WIDTH)).child(commits_view().flex_1())),
            ),
        )
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
            .content_padding_top(px(S1))
            .content_padding_bottom(px(S2))
            .flex_1()
            .into_element()
        }))
    }))
}

fn tree_row(row: Memo<Option<store::TreeRow>>) -> impl IntoElement {
    nsh_base::context_menu()
        .popup_style(|s| {
            s.bg(POPOVER).rounded(px(R_XL)).border_1().border_color(BORDER).p(px(S1)).min_w(px(200.0)).shadow_lg()
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
        .mx(px(S2))
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(28.0))
        .pl(move || px(S2 + row.with(|r| r.as_ref().map_or(0, |r| r.depth)) as f32 * 14.0))
        .pr(px(S2))
        .rounded(px(R_LG))
        .bg(move || if selected.get() { highlight() } else { tint(SURFACE1, 0.0) })
        .hover(|s| s.bg(highlight()))
        .child(dynamic(move || {
            let (dir, open) = row.with(|r| r.as_ref().map_or((false, false), |r| (r.entry.is_dir, r.expanded)));
            Some(untracked(move || {
                let glyph = match (dir, open) {
                    (true, true) => Icon::ChevronDown,
                    (true, false) => Icon::ChevronRight,
                    _ => Icon::File,
                };
                icon(glyph, 14.0).text_color(FG_FAINT)
            }))
        }))
        .child(
            text(move || row.with(|r| r.as_ref().map(|r| r.entry.name.clone()).unwrap_or_default()))
                .text_size(px(TEXT_BASE))
                .number_of_lines(1)
                .flex_1(),
        )
}

fn changes_view() -> View {
    let changes = computed(store::changes);
    view()
        .flex_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(S2))
                .h(px(32.0))
                .px(px(S4))
                .child(
                    text(move || changes.with(|c| match c.files.len() {
                        0 => String::new(),
                        1 => t("1 file").into(),
                        n => tf("{} files", &[&n.to_string()]),
                    }))
                    .text_size(px(TEXT_SM))
                    .text_color(FG_MUTED),
                )
                .child(view().flex_1())
                .child(text(move || {
                    changes.with(|c| {
                        let (a, d) = c.files.iter().fold((0, 0), |(a, d), f| (a + f.additions, d + f.deletions));
                        if a + d == 0 { String::new() } else { format!("+{a} −{d}") }
                    })
                })
                .text_size(px(TEXT_SM))
                .font_family(|| MONO)
                .text_color(FG_MUTED)),
        )
        .child(dynamic(move || {
            let (loaded, empty, error) = changes.with(|c| (c.loaded, c.files.is_empty(), c.error.clone()));
            Some(untracked(move || {
                if let Some(e) = error.clone() {
                    return git_error(e).into_element();
                }
                if !loaded {
                    return loading_state().into_element();
                }
                if empty {
                    return empty_state(Icon::CircleCheck, t("No uncommitted changes"), t("The working tree is clean")).into_element();
                }
                let count = move || changes.with(|c| c.files.len());
                view()
                    .flex_1()
                    .child(
                        view().px(px(S2)).pt(px(S2)).child(
                            button()
                                .on_press(|| store::open_tab(Tab::Diff(None)))
                                .flex_row()
                                .items_center()
                                .gap(px(S2))
                                .h(px(30.0))
                                .px(px(S2))
                                .rounded(px(R_LG))
                                .hover(|s| s.bg(highlight()))
                                .child(icon(Icon::FileDiff, 15.0).text_color(FG_MUTED))
                                .child(text(t("All changes")).text_size(px(TEXT_BASE))),
                        ),
                    )
                    .child(
                        virtual_list(count, move |at: nsh_core::element::Position| {
                            let file = computed(move || changes.with(|c| c.files.get(at.index()).cloned()));
                            file_row(file)
                        })
                        .keys(move || changes.with(|c| c.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>()))
                        .estimated_item_size(px(30.0))
                        .content_padding_bottom(px(S2))
                        .flex_1(),
                    )
                    .into_element()
            }))
        }))
}

fn empty_note(glyph: Icon, message: String) -> View {
    view()
        .flex_row()
        .gap(px(S2))
        .p(px(S4))
        .child(icon(glyph, 15.0).text_color(FG_FAINT))
        .child(text(message).text_size(px(TEXT_SM)).text_color(FG_MUTED).flex_1())
}

/// An empty panel's message, centered: a glyph on a soft disc, a title and
/// a line under it.
fn empty_state(glyph: Icon, title: &'static str, detail: &'static str) -> View {
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(S2))
        .px(px(S6))
        .pb(px(48.0))
        .child(
            view()
                .w(px(44.0))
                .h(px(44.0))
                .mb(px(S1))
                .rounded(px(R_FULL))
                .items_center()
                .justify_center()
                .bg(SURFACE2)
                .child(icon(glyph, 20.0).text_color(FG_MUTED)),
        )
        .child(text(title).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)).text_color(FG).text_center())
        .child(text(detail).text_size(px(TEXT_SM)).line_height(px(19.0)).text_color(FG_MUTED).text_center())
}

/// Git could not answer: a folder outside Git says so in words (its files
/// still browse), anything else shows the message.
fn git_error(e: String) -> View {
    if e.to_lowercase().contains("not a git repository") {
        return empty_state(Icon::GitBranch, t("Not a Git repository"), t("Changes and commits show for folders under Git; the files are still here"))
            .child(view().mt(px(S2)).child(btn(t("Browse files"), Variant::Secondary, Sz::Sm, Some(Icon::FolderTree), || {
                store::set_explorer_tab(ExplorerTab::Files)
            })));
    }
    view()
        .flex_1()
        .items_center()
        .justify_center()
        .gap(px(S2))
        .px(px(S6))
        .child(icon(Icon::CircleAlert, 20.0).text_color(DANGER))
        .child(text(t("Couldn't read Git")).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)).text_color(FG))
        .child(text(e).text_size(px(TEXT_SM)).text_color(FG_MUTED).text_center().selectable(true))
}

fn loading_state() -> View {
    view().flex_1().items_center().justify_center().pb(px(48.0)).child(eclipse(28.0))
}

fn split_path(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((dir, name)) => (name.to_owned(), dir.to_owned()),
        None => (path.to_owned(), String::new()),
    }
}

fn file_row(file: Memo<Option<DiffFile>>) -> impl IntoElement {
    let selected = computed(move || {
        let path = file.with(|f| f.as_ref().map(|f| f.path.clone()));
        path.is_some() && store::tab() == Tab::Diff(path)
    });
    button()
        .on_press(move || {
            if let Some(f) = file.get_untracked() {
                store::open_tab(Tab::Diff(Some(f.path)));
            }
        })
        .mx(px(S2))
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(30.0))
        .px(px(S2))
        .rounded(px(R_LG))
        .bg(move || if selected.get() { highlight() } else { tint(SURFACE1, 0.0) })
        .hover(|s| s.bg(highlight()))
        .child(
            text(move || {
                file.with(|f| match f {
                    Some(f) if f.is_new => "A",
                    Some(f) if f.is_deleted => "D",
                    Some(f) if f.old_path.is_some() => "R",
                    _ => "M",
                })
            })
            .w(px(12.0))
            .text_size(px(TEXT_XS))
            .font_weight(FontWeight(600))
            .font_family(|| MONO)
            .text_color(move || {
                file.with(|f| match f {
                    Some(f) if f.is_new => SUCCESS,
                    Some(f) if f.is_deleted => DANGER,
                    _ => WARNING,
                })
            }),
        )
        .child(
            text(move || file.with(|f| f.as_ref().map(|f| split_path(&f.path).0).unwrap_or_default()))
                .text_size(px(TEXT_BASE))
                .number_of_lines(1)
                .flex_shrink_0()
                .max_w(px(170.0)),
        )
        .child(
            text(move || file.with(|f| f.as_ref().map(|f| split_path(&f.path).1).unwrap_or_default()))
                .text_size(px(TEXT_SM))
                .text_color(FG_FAINT)
                .number_of_lines(1)
                .flex_1(),
        )
        .child(
            text(move || file.with(|f| f.as_ref().map(|f| format!("+{}", f.additions)).unwrap_or_default()))
                .text_size(px(TEXT_XS))
                .font_family(|| MONO)
                .text_color(SUCCESS),
        )
        .child(
            text(move || file.with(|f| f.as_ref().map(|f| format!("−{}", f.deletions)).unwrap_or_default()))
                .text_size(px(TEXT_XS))
                .font_family(|| MONO)
                .text_color(DANGER),
        )
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
        .gap(px(S2))
        .h(px(44.0))
        .pl(px(S4))
        .pr(px(S2))
        .border_b_1()
        .border_color(tint(BORDER, 0.7))
        .child(icon(Icon::FileDiff, 15.0).text_color(FG_MUTED))
        .child(text(name).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1).flex_shrink(1.0))
        .child(dir.map(|d| text(d).text_size(px(TEXT_SM)).text_color(FG_FAINT).number_of_lines(1).flex_shrink(1.0)))
        .child(view().flex_1())
        .child(
            view()
                .flex_row()
                .gap(px(6.0))
                .child(text(move || format!("+{}", stats.get().0)).text_size(px(TEXT_SM)).font_family(|| MONO).text_color(SUCCESS))
                .child(text(move || format!("\u{2212}{}", stats.get().1)).text_size(px(TEXT_SM)).font_family(|| MONO).text_color(DANGER)),
        )
        .child(dynamic(move || {
            (!one_sided.get()).then(|| untracked(move || {
                view()
                    .ml(px(S2))
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
                    .child(text(t("No changes")).text_size(px(TEXT_BASE)).text_color(FG_MUTED))
                    .into_element();
            }
            let mut d = diff_view()
                .patch(move || Some(SharedString::from(patch.get())))
                .layout(move || if one_sided.get() || prefs.with(|p| p.unified) { DiffLayout::Unified } else { DiffLayout::Split })
                .text_size(px(TEXT_CODE))
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
                        .bg(ACCENT)
                        .shadow_sm()
                        .child(icon(Icon::Plus, 12.0).text_color(ACCENT_FG))
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
        .gap(px(S2))
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH))
        .self_center()
        .h(px(48.0))
        .pl(px(S4))
        .pr(px(6.0))
        .rounded(px(R_FULL))
        .child(icon(Icon::Pencil, 16.0).text_color(FG_MUTED))
        .child(
            input()
                .flex_1()
                .text_size(px(TEXT_BASE))
                .text_color(FG)
                .caret_color(|| color(ACCENT))
                .selection_color(|| color(tint(ACCENT, 0.3)))
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
                .bg(move || if empty_note.get() { highlight() } else { ACCENT })
                .transition_colors()
                .child(icon(Icon::ArrowUp, 17.0).text_color(move || if empty_note.get() { FG_FAINT } else { ACCENT_FG })),
        );
    view()
        .flex_1()
        .px(px(S4))
        .pt(px(S1))
        .pb(px(S3))
        .gap(px(S3))
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
        .gap(px(S2))
        .child(text(label).text_size(px(TEXT_XS)).text_color(FG_MUTED).font_weight(FontWeight(600)))
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
                .gap(px(S2))
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
        .gap(px(S2))
        .child(
            view()
                .w(px(22.0))
                .h(px(22.0))
                .rounded(px(R_FULL))
                .bg(tint(ACCENT, 0.15))
                .items_center()
                .justify_center()
                .child(icon(Icon::Pencil, 11.0).text_color(ACCENT)),
        )
        .child(
            view()
                .flex_1()
                .gap(px(2.0))
                .child(text(lines_label(c.range)).text_size(px(TEXT_XS)).text_color(FG_MUTED).font_weight(FontWeight(600)))
                .child(text(c.text).text_size(px(TEXT_SM)).text_color(FG)),
        )
        .child(icon_btn(Icon::Trash2, Sz::Sm, t("Delete comment"), remove))
}

fn comment_shell() -> View {
    view()
        .mx(px(S3))
        .my(px(S2))
        .p(px(S3))
        .max_w(px(640.0))
        .rounded(px(R_LG))
        .bg(glass_fill_strong())
        .border_1()
        .border_color(glass_edge())
        .shadow_sm()
}

fn commits_view() -> View {
    let data = computed(store::commits);
    view().flex_1().child(dynamic(move || {
        let state = data.get();
        Some(untracked(move || match state.clone() {
            None => loading_state().into_element(),
            Some((_, Some(e))) => git_error(e).into_element(),
            Some((list, None)) if list.is_empty() => empty_state(Icon::GitCommitHorizontal, t("No commits yet"), t("Commits show here once there are some")).into_element(),
            Some((list, None)) => {
                let mut col = view().gap(px(2.0)).py(px(S1));
                for c in list {
                    let sha = c.sha.clone();
                    let mut files = view().gap(px(1.0)).pt(px(4.0));
                    for (path, add, del) in c.files.clone() {
                        let (sha, p) = (sha.clone(), path.clone());
                        files = files.child(
                            nsh_base::button()
                                .on_press(move || store::open_tab(Tab::CommitDiff(sha.clone(), p.clone())))
                                .flex_row()
                                .items_center()
                                .gap(px(6.0))
                                .h(px(24.0))
                                .px(px(S1))
                                .rounded(px(R_MD))
                                .hover(|s| s.bg(highlight()))
                                .child(text(split_path(&path).0).text_size(px(TEXT_SM)).number_of_lines(1).flex_1())
                                .child(text(format!("+{add}")).text_size(px(TEXT_XS)).font_family(|| MONO).text_color(SUCCESS))
                                .child(text(format!("−{del}")).text_size(px(TEXT_XS)).font_family(|| MONO).text_color(DANGER)),
                        );
                    }
                    col = col.child(
                        view()
                            .mx(px(S2))
                            .px(px(S2))
                            .py(px(6.0))
                            .rounded(px(R_LG))
                            .hover(|s| s.bg(highlight()))
                            .gap(px(2.0))
                            .child(text(c.subject.clone()).text_size(px(TEXT_BASE)).number_of_lines(2))
                            .child(
                                view()
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .child(text(c.short.clone()).text_size(px(TEXT_XS)).font_family(|| MONO).text_color(FG_MUTED))
                                    .child(
                                        text(format!("{} · {}", c.author, c.date.get(..10).unwrap_or_default()))
                                            .text_size(px(TEXT_XS))
                                            .text_color(FG_FAINT)
                                            .number_of_lines(1)
                                            .flex_1(),
                                    )
                                    .when(!c.pushed, |v| v.child(badge("local", WARNING))),
                            )
                            .child(files),
                    );
                }
                scroll().flex_1().child(col).into_element()
            }
        }))
    }))
}

/// One file of a commit, as a diff tab.
pub(crate) fn commit_diff_pane(sha: String, path: String) -> impl IntoElement {
    let data = computed(store::commit_diff);
    let title = format!("{} · {}", sha.get(..7).unwrap_or(&sha), path);
    let language = path.rsplit_once('.').map(|(_, e)| e.to_owned());
    view()
        .flex_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(S2))
                .h(px(40.0))
                .px(px(S4))
                .border_b_1()
                .border_color(BORDER)
                .child(icon(Icon::GitCommitHorizontal, 15.0).text_color(FG_MUTED))
                .child(text(title).text_size(px(TEXT_BASE)).font_family(|| MONO).number_of_lines(1).flex_1()),
        )
        .child(dynamic(move || {
            let d = data.get();
            let language = language.clone();
            Some(untracked(move || match d.clone() {
                None => empty_note(Icon::RotateCw, t("Loading…").into()).into_element(),
                Some(Err(e)) => empty_note(Icon::Info, e).into_element(),
                Some(Ok(f)) => {
                    let mut v = diff_view().patch(SharedString::from(f.patch)).layout(DiffLayout::Unified).text_size(px(TEXT_CODE)).flex_1();
                    if let Some(l) = language.clone() {
                        v = v.language(SharedString::from(l));
                    }
                    v.into_element()
                }
            }))
        }))
}
