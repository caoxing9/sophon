//! The projects and their workspaces, as the sidebar lists them:
//! pinned workspaces, then each project with its workspaces. The desktop
//! sidebar and the phone's list share it; only the desktop edits.

use crate::i18n::t;
use nsh_base::button;
use nsh_core::{
    element::FontWeight,
    prelude::*,
};

use crate::store::{self, Row, Screen};
use crate::ui::kit::text;
use crate::ui::kit::*;

/// A workspace row: a touch target on the phone.
const ROW_HEIGHT: f32 = if MOBILE { 44.0 } else { 32.0 };

/// The list; `actions` adds the desktop's (new workspace, pin, rename,
/// archive), `empty` is what shows with no projects. Each project is a
/// group whose workspaces fold away: a clipped height on a native spring
/// (nsh/base's Collapsible), the groups below gliding with it. Groups and
/// rows are keyed, so what stays keeps its views while data changes.
pub(crate) fn project_list(actions: bool, empty: impl Fn() -> AnyElement + 'static) -> impl IntoElement {
    let groups = computed(store::groups);
    let none = computed(move || groups.with(Vec::is_empty));
    view().flex_1().child(dynamic(move || {
        let none = none.get();
        Some(untracked(|| {
            if none {
                return empty();
            }
            scroll()
                .flex_1()
                .child(view().pb(px(S2)).child(each(move || groups.get(), store::Group::key, move |g, _| group(g, actions))))
                .into_element()
        }))
    }))
}

fn group(g: ReadSignal<store::Group>, actions: bool) -> AnyElement {
    let rows = move || {
        each(
            move || g.with(|g| g.workspaces.clone()),
            |w: &crate::daemon::Workspace| w.id.clone(),
            move |w, _| workspace_row(computed(move || Some(Row::Workspace(w.get()))), actions),
        )
    };
    let Some(project) = g.with_untracked(|g| g.project.clone()) else {
        return view()
            .child(section_row(computed(|| Some(Row::Section("Pinned".into())))))
            .child(rows())
            .into_element();
    };
    let pid = project.id.clone();
    let header = computed(move || g.with(|g| g.project.clone().map(Row::Project)));
    view()
        // Where the group starts, for dropping a dragged project.
        .on_layout(move |b: &Bounds<Points>| store::set_project_row_top(&pid, b.origin.y.0))
        .child(
            nsh_base::collapsible()
                .open(move || !store::is_collapsed(&project.id))
                .reveal(true)
                .content(view().child(rows()))
                .child(project_row(header, actions)),
        )
        .into_element()
}

fn section_row(item: Memo<Option<Row>>) -> impl IntoElement {
    view().px(px(S4)).pt(px(S3)).pb(px(S1)).child(section_label(move || match item.get() {
        Some(Row::Section(s)) => s,
        _ => String::new(),
    }))
}

fn project_row(item: Memo<Option<Row>>, actions: bool) -> impl IntoElement {
    let project = computed(move || match item.get() {
        Some(Row::Project(p)) => Some(p),
        _ => None,
    });
    let id = move || project.with_untracked(|p| p.as_ref().map(|p| p.id.clone()).unwrap_or_default());
    let folded = computed(move || project.with(|p| p.as_ref().is_some_and(|p| store::is_collapsed(&p.id))));
    // Dragging the header moves the project: it follows the pointer, and
    // where it is let go sets its place among the projects.
    let drag = signal(0.0_f32);
    let dragging = signal(false);
    let header = view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .mx(px(S2))
        .pl(px(S1))
        .pr(px(2.0))
        .mt(px(S3))
        .mb(px(2.0))
        .h(px(28.0))
        .rounded(px(R_MD))
        .translate_y(move || DefiniteLength::from(px(drag.get())))
        .bg(move || if dragging.get() { highlight() } else { tint(SIDEBAR, 0.0) })
        // Under the pointer: the row fills, its name darkens and its
        // actions appear (out of the way otherwise, as T3's and Lody's).
        .group()
        .hover(|s| s.bg(highlight()))
        .transition_colors()
        .child(
            button()
                .on_press(move || {
                    let pid = id();
                    if !pid.is_empty() {
                        store::toggle_collapsed(&pid);
                    }
                })
                .flex_1()
                .min_w(px(0.0))
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .h(px(28.0))
                .child(
                    icon(Icon::ChevronDown, 13.0)
                        .text_color(FG_FAINT)
                        .rotate(move || Radians(if folded.get() { -std::f32::consts::FRAC_PI_2 } else { 0.0 }))
                        .transition_transform(),
                )
                .child(icon(Icon::Folder, 15.0).text_color(FG_FAINT))
                .child(
                    text(move || project.with(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default()))
                        .text_size(px(TEXT_SM))
                        .font_weight(FontWeight(500))
                        .text_color(FG_MUTED)
                        .group_hover(|s| s.text_color(FG))
                        .number_of_lines(1)
                        .flex_1(),
                ),
        );
    let header = if actions && !MOBILE {
        header
            .claim_touches(TouchClaim::on_move().ask(|r: &ClaimRequest| r.claim()))
            .on_responder_grant(move |_: &ResponderEvent| dragging.set(true))
            .on_responder_move(move |e: &ResponderEvent| drag.set(e.gesture.delta.y.0))
            .on_responder_release(move |e: &ResponderEvent| {
                let pid = id();
                store::drop_project(&pid, e.gesture.delta.y.0);
                dragging.set(false);
                drag.set(0.0);
            })
            .on_responder_terminate(move |_: &ResponderEvent| {
                dragging.set(false);
                drag.set(0.0);
            })
    } else {
        header
    };
    header
        .when(actions, |v| {
            v.child(view().flex_row().items_center().opacity(if MOBILE { 1.0 } else { 0.0 }).group_hover(|s| s.opacity(1.0)).transition_opacity().child(icon_btn(Icon::Plus, Sz::Sm, t("New workspace in project"), move || {
                if let Some(p) = project.get_untracked().filter(|p| !p.root_path.is_empty()) {
                    store::open_new_workspace(Some(p.root_path));
                }
            }))
            .child(
                dropdown(
                    view()
                        .w(px(24.0))
                        .h(px(24.0))
                        .items_center()
                        .justify_center()
                        .rounded(px(R_FULL))
                        .hover(|s| s.bg(highlight()))
                        .label(t("Project actions"))
                        .child(icon(Icon::Ellipsis, 15.0).text_color(FG_FAINT)),
                )
                .align(nsh_core::overlay::Align::End)
                .child(dynamic(move || {
                    let p = project.get()?;
                    Some(untracked(move || {
                        let (r, d, name) = (p.id.clone(), p.id.clone(), p.name.clone());
                        let (root, pname) = (p.root_path.clone(), p.name.clone());
                        view()
                            .child(menu_row(t("Project settings"), Some(Icon::Settings2), None, false, move || {
                                store::open_project_settings(&root, &pname)
                            }))
                            .child(menu_row(t("Rename"), Some(Icon::Pencil), None, false, move || {
                                store::open_modal(store::Modal::RenameProject { id: r.clone(), name: name.clone() })
                            }))
                            .child(menu_divider())
                            .child(menu_row_danger(t("Remove from Sophon"), Icon::Trash2, move || store::remove_project(&d)))
                            .into_element()
                    }))
                })),
            ))
        })
}

fn workspace_row(item: Memo<Option<Row>>, actions: bool) -> impl IntoElement {
    let ws = computed(move || match item.get() {
        Some(Row::Workspace(w)) => Some(w),
        _ => None,
    });
    let selected = computed(move || {
        let id = ws.with(|w| w.as_ref().map(|w| w.id.clone()));
        id.is_some_and(|id| store::screen() == Screen::Workspace(id))
    });
    let status = move || ws.with(|w| w.as_ref().map(|w| Status::of_workspace(&w.status)).unwrap_or(Status::Idle));

    let main = button()
        .on_press(move || {
            if let Some(w) = ws.get_untracked() {
                store::open_workspace(&w.id);
            }
        })
        .flex_1()
        .min_w(px(0.0))
        .flex_row()
        .items_center()
        .gap(px(S2))
        .h(px(ROW_HEIGHT))
        .pl(px(S2))
        .child(view().w(px(15.0)).items_center().child(status_dot(status)))
        .child(
            text(move || ws.with(|w| w.as_ref().map(|w| w.name.clone()).unwrap_or_default()))
                .text_size(px(TEXT_BASE))
                .text_color(move || if selected.get() { FG } else { FG })
                .number_of_lines(1)
                .flex_1(),
        )
        .child(dynamic(move || {
            let labels = ws.with(|w| w.as_ref().map(|w| w.labels.clone()).unwrap_or_default());
            if labels.is_empty() {
                return None;
            }
            Some(untracked(move || {
                let mut row = view().flex_row().gap(px(3.0));
                for l in labels.clone() {
                    row = row.child(view().w(px(6.0)).h(px(6.0)).rounded(px(R_FULL)).bg(label_tone(&store::label_color(&l))));
                }
                row
            }))
        }))
        .child(
            text(move || {
                ws.with(|w| match w {
                    Some(w) if w.additions + w.deletions > 0 => format!("+{} −{}", w.additions, w.deletions),
                    _ => String::new(),
                })
            })
            .text_size(px(TEXT_XS))
            .font_family(|| MONO)
            .text_color(FG_FAINT),
        );

    let more = dropdown(
        view()
            .w(px(24.0))
            .h(px(24.0))
            .items_center()
            .justify_center()
            .rounded(px(R_FULL))
            .hover(|s| s.bg(highlight()))
            .child(icon(Icon::Ellipsis, 15.0).text_color(FG_FAINT)),
    )
    .align(nsh_core::overlay::Align::End)
    .child(dynamic(move || {
        let w = ws.get()?;
        Some(untracked(move || {
            let (pin_id, archive_id, rename_id) = (w.id.clone(), w.id.clone(), w.id.clone());
            let (name, name2) = (w.name.clone(), w.name.clone());
            let pinned = w.pinned;
            view()
                .child(menu_row(
                    if pinned { t("Unpin") } else { t("Pin") },
                    Some(if pinned { Icon::PinOff } else { Icon::Pin }),
                    None,
                    false,
                    move || store::pin_workspace(&pin_id, !pinned),
                ))
                .child(menu_row(t("Mark as unread"), Some(Icon::CircleDot), None, false, {
                    let id = w.id.clone();
                    move || store::mark_unread(&id)
                }))
                .child(menu_row(t("Labels…"), Some(Icon::Layers), None, false, {
                    let id = w.id.clone();
                    move || store::open_modal(store::Modal::Labels { workspace: id.clone() })
                }))
                .child(menu_row(t("Recover files"), Some(Icon::RotateCw), Some(t("If its folder went missing").into()), false, {
                    let id = w.id.clone();
                    move || store::recover_workspace(&id)
                }))
                .child(menu_row(t("Rename"), Some(Icon::Pencil), None, false, move || {
                    store::open_modal(store::Modal::RenameWorkspace { id: rename_id.clone(), name: name.clone() })
                }))
                .child(menu_divider())
                .child(menu_row_danger(t("Archive workspace"), Icon::Archive, move || {
                    store::open_modal(store::Modal::ArchiveWorkspace { id: archive_id.clone(), name: name2.clone() })
                }))
                .into_element()
        }))
    }));

    // Two layers: the chosen row's fill below, the pointer's translucent
    // one over it (so a chosen row under the pointer darkens a step too).
    view()
        .mx(px(S2))
        // A hairline between rows that fill under the pointer.
        .mb(px(2.0))
        .rounded(px(R_LG))
        .bg(move || if selected.get() { SIDEBAR_SELECTED } else { tint(SIDEBAR, 0.0) })
        .transition_colors()
        .child(
            view()
                .flex_row()
                .items_center()
                .pr(px(S1))
                .rounded(px(R_LG))
                .group()
                .hover(|s| s.bg(highlight()))
                .transition_colors()
                .child(main)
                // The actions only under the pointer (always on a phone).
                .when(actions, |v| {
                    v.child(
                        view()
                            .opacity(if MOBILE { 1.0_f32 } else { 0.0 })
                            .group_hover(|s| s.opacity(1.0))
                            .transition_opacity()
                            .child(more),
                    )
                }),
        )
}

