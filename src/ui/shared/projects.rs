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

/// How many of a workspace's conversations the list shows before "Show more".
const CONVERSATIONS_SHOWN: usize = 8;

fn row_status(status: impl Fn() -> Status + 'static) -> impl IntoElement {
    let status = computed(status);
    view()
        .w(px(15.0))
        .h(px(15.0))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .child(dynamic(move || {
            let current = status.get();
            Some(untracked(move || match current {
                // Seen and at rest: no mark, its place kept.
                Status::Idle => view().into_element(),
                // Waiting for the user: a hand raised.
                Status::NeedsYou => raised_hand(13.0).label(current.label()).into_element(),
                // Running: the mark an agent at work has in its own
                // conversation, the same here.
                Status::Running if !MOBILE => spark(13.0).label(current.label()).into_element(),
                _ => status_dot(move || current).into_element(),
            }))
        }))
}

/// A row's mark at its end, where its "…" is: the titles start flush,
/// and a row at rest has none and gives the title its whole width. One
/// with a mark keeps room for it, so a long title ends before it rather
/// than under it. Under the pointer the actions take its place; with ⌘
/// held, the row's ⌘1–9.
fn trailing_status(status: impl Fn() -> Status + 'static, menu_open: Signal<bool>) -> impl IntoElement {
    let badges = || !MOBILE && store::command_held() && store::number_shortcuts();
    let status = computed(status);
    let marked = computed(move || status.get() != Status::Idle);
    view()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .pointer_events(nsh_core::PointerEvents::None)
        // Its mark where the "…" is under the pointer.
        .max_w(move || px(if marked.get() && !badges() && !menu_open.get() { 15.0 + 4.5 } else { 0.0 }))
        .opacity(move || if menu_open.get() { 0.0_f32 } else { 1.0 })
        .group_hover(|s| s.max_w(px(0.0)).opacity(0.0))
        .transition_opacity()
        .child(view().mr(px(4.5)).child(row_status(move || status.get())))
}

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
                return view().flex_1().child(projects_heading(actions)).child(empty()).into_element();
            }
            // Two sections, as Pinned and Projects read in the sidebar: the
            // pinned first (when there are any), each under its heading.
            // "Projects" is over the projects, not over both: above the
            // pinned it named what it was not next to, and the projects
            // began under them with no name at all.
            let pinned = move || groups.with(|g| g.iter().filter(|g| g.project.is_none()).cloned().collect::<Vec<_>>());
            let projects = move || groups.with(|g| g.iter().filter(|g| g.project.is_some()).cloned().collect::<Vec<_>>());
            scroll()
                .flex_1()
                .child(
                    view()
                        .pb(sp(S2))
                        .child(each(pinned, store::Group::key, move |g, _| group(g, actions)))
                        .child(projects_heading(actions))
                        .child(each(projects, store::Group::key, move |g, _| group(g, actions))),
                )
                .into_element()
        }))
    }))
}

/// While ⌘ is held (macOS): the key, ⌘1–⌘9, that opens this row, if it
/// is among the first nine. `is` tells the row's entry.
fn number_badge(is: impl Fn(&Entry) -> bool + 'static) -> impl IntoElement {
    let number = computed(move || {
        if MOBILE || !store::command_held() || !store::number_shortcuts() {
            return None;
        }
        sidebar_entries().iter().take(9).position(&is).map(|i| i + 1)
    });
    dynamic(move || {
        let n = number.get()?;
        Some(untracked(move || {
            view()
                .h(px(18.0))
                .min_w(px(18.0))
                .px(px(5.0))
                .mr(sp(S1))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(R_SM))
                .bg(tint(FG(), 0.08))
                .child(text(format!("⌘{n}")).text_size(tx(TEXT_XS)).font_weight(FontWeight(500)).text_color(FG_MUTED()))
        }))
    })
}

/// What a row of the sidebar opens.
pub(crate) enum Entry {
    Conversation(crate::daemon::Agent),
    Workspace(String),
}

/// The sidebar's rows that open a conversation, top to bottom, as the
/// list shows them (folded projects and sections left out): what ⌘1–⌘9
/// open.
pub(crate) fn sidebar_entries() -> Vec<Entry> {
    let mut out = Vec::new();
    for g in store::groups() {
        match &g.project {
            None => {
                if store::is_collapsed(&g.key()) {
                    continue;
                }
                out.extend(g.conversations.iter().cloned().map(Entry::Conversation));
            }
            Some(p) if store::is_collapsed(&p.id) => continue,
            Some(_) => {}
        }
        for w in &g.workspaces {
            workspace_entries(w, &mut out);
        }
    }
    out
}

/// A workspace's rows, as `workspace_entry` draws them.
fn workspace_entries(w: &crate::daemon::Workspace, out: &mut Vec<Entry>) {
    let mut conversations: Vec<crate::daemon::Agent> =
        store::workspace_agents(&w.id).into_iter().filter(|a| !a.archived && a.parent_id.is_none() && !store::is_hidden_run(a)).collect();
    conversations.reverse();
    if !conversations.is_empty() && conversations.iter().all(|a| a.pinned) {
        return;
    }
    let unpinned: Vec<_> = conversations.into_iter().filter(|a| !a.pinned).collect();
    let bare = !w.pinned
        && w.directory.trim_end_matches('/') == w.project_root.trim_end_matches('/')
        && unpinned.is_empty()
        && store::screen() != Screen::Workspace(w.id.clone());
    if bare {
        return;
    }
    if unpinned.len() > 1 {
        out.extend(unpinned.into_iter().take(CONVERSATIONS_SHOWN).map(Entry::Conversation));
    } else {
        out.push(Entry::Workspace(w.id.clone()));
    }
}

/// The projects' heading: their name, and "Add project" (the desktop's)
/// at its right, where the rows' actions are.
pub(crate) fn projects_heading(actions: bool) -> View {
    view()
        .flex_row()
        .items_center()
        .mx(sp(S2))
        .pl(sp(S2))
        .pr(px(2.0))
        .h(px(28.0))
        .child(section_label(t("Projects")).flex_1())
        .when(actions && !MOBILE, |v| v.child(row_btn(Icon::Plus, t("Add project"), || store::open_modal(store::Modal::AddProject))))
}

fn group(g: ReadSignal<store::Group>, actions: bool) -> AnyElement {
    let rows = move || {
        each(
            move || g.with(|g| g.workspaces.clone()),
            |w: &crate::daemon::Workspace| w.id.clone(),
            move |w, _| workspace_entry(w, actions),
        )
    };
    let Some(project) = g.with_untracked(|g| g.project.clone()) else {
        let key = g.with_untracked(store::Group::key);
        let folded = computed({
            let key = key.clone();
            move || store::is_collapsed(&key)
        });
        let heading = button()
            .on_press(move || store::toggle_collapsed(&key))
            .accessibility_label(t("Pinned"))
            .w_full()
            .h(px(if MOBILE { ROW_HEIGHT } else { 28.0 }))
            .flex_row()
            .items_center()
            .gap(sp(S1))
            .px(sp(S4))
            .hover(|s| s.bg(sidebar_hover()))
            .child(
                icon(Icon::ChevronDown, 13.0)
                    .text_color(FG_FAINT())
                    .rotate(move || Radians(if folded.get() { -std::f32::consts::FRAC_PI_2 } else { 0.0 }))
                    .transition_transform(),
            )
            .child(section_label(t("Pinned")));
        let content = view()
            .child(each(
                move || g.with(|g| g.conversations.clone()),
                |a: &crate::daemon::Agent| a.id.clone(),
                move |a, _| conversation_row(a, actions),
            ))
            .child(rows());
        return view()
            // Apart from the projects under it, as a section of its own.
            .mb(sp(S3))
            .child(nsh_base::collapsible().open(move || !folded.get()).reveal(true).content(content).child(heading))
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
                .content(guided(view().child(draft_row(project.root_path.clone())).child(rows())))
                .child(project_row(header, actions)),
        )
        .into_element()
}

/// How far a project's rows sit in: under its chevron, in points from the
/// row's left edge.
const PROJECT_INDENT: f32 = S2 + S1 + 6.0;

/// The rows one level down, set in under their project: the indent alone
/// says what is under what (the hairline down their left is gone).
fn guided(rows: View) -> View {
    view().child(rows.ml(px(if MOBILE { S2 } else { PROJECT_INDENT })))
}

/// A workspace in the list: its conversations each a row of their own,
/// newest first, when it has several (sessions imported together, a second
/// chat in the same folder); the workspace's one row otherwise. There are
/// no tabs to find them under: the list is where every conversation is.
fn workspace_entry(w: ReadSignal<crate::daemon::Workspace>, actions: bool) -> impl IntoElement {
    let id = w.with_untracked(|w| w.id.clone());
    let conversations = computed(move || {
        let mut list: Vec<crate::daemon::Agent> =
            store::workspace_agents(&id).into_iter().filter(|a| !a.archived && a.parent_id.is_none() && !store::is_hidden_run(a)).collect();
        list.reverse();
        list
    });
    // Those pinned on their own are under "Pinned"; a workspace whose
    // conversations all are shows no row of its own.
    let unpinned = computed(move || conversations.with(|c| c.iter().filter(|a| !a.pinned).cloned().collect::<Vec<_>>()));
    let all_pinned = computed(move || conversations.with(|c| !c.is_empty() && c.iter().all(|a| a.pinned)));
    let conversations = unpinned;
    let several = computed(move || conversations.with(|c| c.len() > 1));
    // The newest few; the rest behind "Show more", a screenful at a time
    // (a folder may hold dozens of imported sessions).
    let limit = signal(CONVERSATIONS_SHOWN);
    let more = computed(move || conversations.with(|c| c.len()).saturating_sub(limit.get()));
    // The project's own folder with no conversation in it (every project
    // has it from the moment it is added, for the phone to start one in)
    // says nothing its project's row does not, whose + starts the first:
    // no row, unless it is the one open.
    let bare = computed(move || {
        let w = w.get();
        !w.pinned
            && w.directory.trim_end_matches('/') == w.project_root.trim_end_matches('/')
            && conversations.with(Vec::is_empty)
            && store::screen() != Screen::Workspace(w.id)
    });
    view().child(dynamic(move || {
        let several = several.get();
        if all_pinned.get() || bare.get() {
            return None;
        }
        Some(untracked(move || {
            if several {
                view()
                    .child(each(
                        move || conversations.with(|c| c.iter().take(limit.get()).cloned().collect::<Vec<_>>()),
                        |a: &crate::daemon::Agent| a.id.clone(),
                        move |a, _| conversation_row(a, actions),
                    ))
                    .child(when(move || more.get() > 0, move || {
                        button()
                            .on_press(move || limit.update(|l| *l += 20))
                            .mx(sp(S2))
                            .h(px(ROW_HEIGHT - 4.0))
                            .pl(px(S2 + 15.0 + S2))
                            .justify_center()
                            .rounded(px(R_LG))
                            .hover(|s| s.bg(sidebar_hover()))
                            .child(text(move || crate::i18n::tf("Show {} more", &[&more.get().to_string()])).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
                    }))
                    .into_element()
            } else {
                workspace_row(computed(move || Some(Row::Workspace(w.get()))), actions).into_element()
            }
        }))
    }))
}

/// One conversation of a workspace that has several: its state, its title,
/// and (the desktop's) archiving it.
/// The new conversation being written in project `root` (its + or ⌘T),
/// first in its list until its first message makes it.
fn draft_row(root: String) -> impl IntoElement {
    let here = computed(move || {
        store::screen() == Screen::NewWorkspace && store::form().scoped && store::form().path.trim_end_matches('/') == root.trim_end_matches('/')
    });
    when(here, || {
        view()
            .mx(sp(S2))
            .mb(px(2.0))
            .rounded(px(R_LG))
            .bg(SIDEBAR_SELECTED())
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .h(sp(ROW_HEIGHT))
            .pl(sp(S2))
            .pr(px(S1 + 4.5 + 4.0))
            .child(text(t("New conversation")).text_size(tx(TEXT_BASE)).text_color(FG()).number_of_lines(1).flex_1())
            .child(view().w(px(15.0)).items_center().child(icon(Icon::SquarePen, 13.0).text_color(FG_MUTED())))
    })
}

fn conversation_row(agent: ReadSignal<crate::daemon::Agent>, actions: bool) -> impl IntoElement {
    let id = agent.with_untracked(|a| a.id.clone());
    let selected = {
        let id = id.clone();
        computed(move || matches!(store::screen(), Screen::Workspace(_)) && store::tab() == store::Tab::Agent(id.clone()))
    };
    let main = button()
        .on_press(move || store::open_session(&agent.get_untracked()))
        .flex_1()
        .min_w(px(0.0))
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .h(sp(ROW_HEIGHT))
        .pl(sp(S2))
        .when(MOBILE || !actions, |v| v.child(row_status(move || agent.with(Status::of_agent_row))))
        .child(text(move || agent.with(store::conversation_title)).text_size(tx(TEXT_BASE)).text_color(FG()).number_of_lines(1).flex_1())
        .child({
            let id = id.clone();
            number_badge(move |e| matches!(e, Entry::Conversation(a) if a.id == id))
        });
    let menu_open = signal(false);
    let more = dropdown(
        view()
            .w(px(ROW_BTN))
            .h(px(ROW_BTN))
            .items_center()
            .justify_center()
            .rounded(px(R_MD))
            .hover(|s| s.bg(highlight()))
            .child(icon(Icon::Ellipsis, 14.0).text_color(FG_FAINT())),
    )
    .align(nsh_core::overlay::Align::End)
    .on_open_change(move |open: &bool| menu_open.set(*open))
    .child(
        view()
            .child({
                let id = id.clone();
                menu_row(t("Rename"), Some(Icon::Pencil), None, false, move || {
                    let name = agent.with_untracked(store::conversation_title);
                    store::open_modal(store::Modal::RenameAgent { id: id.clone(), name });
                })
            })
            .child(dynamic({
                let id = id.clone();
                move || {
                    let pinned = agent.with(|a| a.pinned);
                    let id = id.clone();
                    Some(untracked(move || {
                        menu_row(
                            if pinned { t("Unpin") } else { t("Pin") },
                            Some(if pinned { Icon::PinOff } else { Icon::Pin }),
                            None,
                            false,
                            move || store::pin_agent(&id, !pinned),
                        )
                    }))
                }
            }))
            .child(menu_row(t("Archive"), Some(Icon::Archive), None, false, {
                let id = id.clone();
                move || store::archive_agent(&id)
            })),
    );
    let quick = {
        let (pin_id, archive_id) = (id.clone(), id.clone());
        hover_actions(
            computed(move || agent.with(|a| a.pinned)),
            move |pinned| store::pin_agent(&pin_id, pinned),
            move || store::archive_agent(&archive_id),
        )
    };
    view()
        .mx(sp(S2))
        .mb(px(2.0))
        .rounded(px(R_LG))
        .bg(move || if selected.get() { SIDEBAR_SELECTED() } else { tint(SIDEBAR(), 0.0) })
        .transition_colors()
        .child(
            view()
                .flex_row()
                .items_center()
                .pr(sp(S1))
                .rounded(px(R_LG))
                .group()
                .hover(|s| s.bg(sidebar_hover()))
                .transition_colors()
                .child(main)
                .when(actions, |v| {
                    v.child(row_actions(menu_open).when(!MOBILE, |v| v.child(quick)).child(more))
                })
                .when(actions && !MOBILE, |v| v.child(trailing_status(move || agent.with(Status::of_agent_row), menu_open))),
        )
}

/// Where a row's actions go: shown under the pointer (always on a phone),
/// and out of the way while ⌘ is held, so the row's ⌘1–9 sits at its end
/// as on the rows without them.
fn row_actions(menu_open: Signal<bool>) -> View {
    let badges = || !MOBILE && store::command_held() && store::number_shortcuts();
    // On the desktop they take no room until the pointer is on the row: the
    // title has the row's whole width, cut short only while they show. They
    // stay while the row's "…" menu is open: the pointer is on the menu
    // then, and the row giving the room back moved the menu under it.
    let shown = move || MOBILE || menu_open.get();
    view()
        .flex_row()
        .items_center()
        .flex_shrink_0()
        .max_w(move || px(if badges() && !menu_open.get() { 0.0 } else if shown() { 200.0 } else { 0.0 }))
        .overflow_hidden()
        .opacity(move || if shown() { 1.0_f32 } else { 0.0 })
        .group_hover(|s| s.opacity(1.0).max_w(px(200.0)))
        .transition_opacity()
}

/// A row's pin and archive at hand, under the pointer before its "…"
/// (the desktop's; the phone has them in the menu).
fn hover_actions(pinned: Memo<bool>, pin: impl Fn(bool) + 'static, archive: impl Fn() + 'static) -> impl IntoElement {
    let pin = std::rc::Rc::new(pin);
    view()
        .flex_row()
        .items_center()
        .child(dynamic(move || {
            let on = pinned.get();
            let pin = pin.clone();
            Some(untracked(move || {
                row_btn(if on { Icon::PinOff } else { Icon::Pin }, if on { t("Unpin") } else { t("Pin") }, move || pin(!on))
            }))
        }))
        .child(row_btn(Icon::Archive, t("Archive"), archive))
}

/// How far the pointer goes before a press on a project's header is a drag.
const DRAG_SLOP: f32 = 5.0;

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
        .gap(sp(S2))
        .mx(sp(S2))
        .pl(sp(S1))
        .pr(px(2.0))
        // Projects sit as close as the rows above them: folded, they read
        // as one list.
        .mt(px(if MOBILE { S3 } else { 2.0 }))
        .mb(px(2.0))
        // As tall and as round as the conversations' rows under it.
        .h(sp(ROW_HEIGHT))
        .rounded(px(R_LG))
        .translate_y(move || DefiniteLength::from(px(drag.get())))
        .bg(move || if dragging.get() { sidebar_hover() } else { tint(SIDEBAR(), 0.0) })
        // Under the pointer: the row fills, its name darkens and its
        // actions appear (out of the way otherwise, as T3's and Lody's).
        .group()
        .hover(|s| s.bg(sidebar_hover()))
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
                .h(sp(ROW_HEIGHT))
                .child(
                    icon(Icon::ChevronDown, 13.0)
                        .text_color(FG_FAINT())
                        .rotate(move || Radians(if folded.get() { -std::f32::consts::FRAC_PI_2 } else { 0.0 }))
                        .transition_transform(),
                )
                .child(icon(Icon::Folder, 15.0).text_color(FG_FAINT()))
                .child(
                    text(move || project.with(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default()))
                        .text_size(tx(TEXT_SM))
                        .font_weight(FontWeight(500))
                        .text_color(FG_MUTED())
                        .group_hover(|s| s.text_color(FG()))
                        .number_of_lines(1)
                        .flex_1(),
                ),
        );
    let header = if actions && !MOBILE {
        header
            // A drag once the pointer has gone some way: a press moves a
            // point or two between down and up, and claimed at the first
            // of them it was a drag of nothing, and no press (the folder
            // did not open).
            .claim_touches(TouchClaim::on_move().ask(|r: &ClaimRequest| {
                if r.event.gesture.delta.y.0.abs() >= DRAG_SLOP {
                    r.claim();
                }
            }))
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
            v.child(view().flex_row().items_center().opacity(if MOBILE { 1.0 } else { 0.0 }).group_hover(|s| s.opacity(1.0)).transition_opacity().child(row_btn(Icon::Plus, t("New workspace in project"), move || {
                if let Some(p) = project.get_untracked().filter(|p| !p.root_path.is_empty()) {
                    store::new_chat_in(p.root_path);
                }
            }))
            .child(
                dropdown(
                    view()
                        .w(px(ROW_BTN))
                        .h(px(ROW_BTN))
                        .items_center()
                        .justify_center()
                        .rounded(px(R_MD))
                        .hover(|s| s.bg(highlight()))
                        .label(t("Project actions"))
                        .child(icon(Icon::Ellipsis, 14.0).text_color(FG_FAINT())),
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
    let display_name = computed(move || {
        let workspace = ws.get();
        let Some(workspace) = workspace else { return String::new() };
        store::workspace_agents(&workspace.id)
            .into_iter()
            .filter(|a| !a.archived && a.parent_id.is_none() && !store::is_hidden_run(a))
            .max_by(|a, b| a.updated_at.cmp(&b.updated_at))
            .map(|a| store::conversation_title(&a))
            .unwrap_or(workspace.name)
    });
    // Its conversations' marks, the most urgent: as each conversation's
    // row says it, not the service's summary (which marks a seen, idle
    // workspace done).
    let status = move || {
        let Some(id) = ws.with(|w| w.as_ref().map(|w| w.id.clone())) else { return Status::Idle };
        Status::most_urgent(
            store::workspace_agents(&id)
                .iter()
                .filter(|a| !a.archived && a.parent_id.is_none() && !store::is_hidden_run(a))
                .map(Status::of_agent_row),
        )
    };

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
        .gap(sp(S2))
        .h(sp(ROW_HEIGHT))
        .pl(sp(S2))
        .when(MOBILE || !actions, |v| v.child(row_status(status)))
        .child(
            text(move || display_name.get())
                .text_size(tx(TEXT_BASE))
                .text_color(move || if selected.get() { FG() } else { FG() })
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
        .child(number_badge(move |e| matches!(e, Entry::Workspace(id) if ws.with_untracked(|w| w.as_ref().is_some_and(|w| &w.id == id)))))
;

    let menu_open = signal(false);
    let more = dropdown(
        view()
            .w(px(ROW_BTN))
            .h(px(ROW_BTN))
            .items_center()
            .justify_center()
            .rounded(px(R_MD))
            .hover(|s| s.bg(highlight()))
            .child(icon(Icon::Ellipsis, 14.0).text_color(FG_FAINT())),
    )
    .align(nsh_core::overlay::Align::End)
    .on_open_change(move |open: &bool| menu_open.set(*open))
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
                // The row shows its newest conversation, by that one's title:
                // a rename names it (kept over what is said in it later); a
                // workspace with none is renamed itself.
                .child(menu_row(t("Rename"), Some(Icon::Pencil), None, false, move || {
                    let shown = store::workspace_agents(&rename_id)
                        .into_iter()
                        .filter(|a| !a.archived && a.parent_id.is_none() && !store::is_hidden_run(a))
                        .max_by(|a, b| a.updated_at.cmp(&b.updated_at));
                    match shown {
                        Some(a) => store::open_modal(store::Modal::RenameAgent { id: a.id.clone(), name: store::conversation_title(&a) }),
                        None => store::open_modal(store::Modal::RenameWorkspace { id: rename_id.clone(), name: name.clone() }),
                    }
                }))
                .child(menu_divider())
                .child(menu_row_danger(t("Archive workspace"), Icon::Archive, move || {
                    store::open_modal(store::Modal::ArchiveWorkspace { id: archive_id.clone(), name: name2.clone() })
                }))
                .into_element()
        }))
    }));

    let quick = hover_actions(
        computed(move || ws.with(|w| w.as_ref().is_some_and(|w| w.pinned))),
        move |pinned| {
            if let Some(w) = ws.get_untracked() {
                store::pin_workspace(&w.id, pinned);
            }
        },
        move || {
            if let Some(w) = ws.get_untracked() {
                store::open_modal(store::Modal::ArchiveWorkspace { id: w.id, name: w.name });
            }
        },
    );

    // Two layers: the chosen row's fill below, the pointer's translucent
    // one over it (so a chosen row under the pointer darkens a step too).
    view()
        .mx(sp(S2))
        // A hairline between rows that fill under the pointer.
        .mb(px(2.0))
        .rounded(px(R_LG))
        .bg(move || if selected.get() { SIDEBAR_SELECTED() } else { tint(SIDEBAR(), 0.0) })
        .transition_colors()
        .child(
            view()
                .flex_row()
                .items_center()
                .pr(sp(S1))
                .rounded(px(R_LG))
                .group()
                .hover(|s| s.bg(sidebar_hover()))
                .transition_colors()
                .child(main)
                // The actions only under the pointer (always on a phone).
                .when(actions, |v| {
                    v.child(row_actions(menu_open).when(!MOBILE, |v| v.child(quick)).child(more))
                })
                .when(actions && !MOBILE, |v| v.child(trailing_status(status, menu_open))),
        )
}

