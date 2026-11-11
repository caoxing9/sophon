//! The home screen: the computer and a search field on top, then the
//! conversations, the projects or settings, picked in the system tab bar
//! (Liquid Glass on iOS 26 and later) at the foot, under a round glass
//! button that starts a new conversation.

use crate::i18n::{t, tf};
use nsh_base::{button, input};
use nsh_core::{
    element::FontWeight,
    mobile::{TabBarItem, tab_bar},
    prelude::*,
};

use super::{settings, time};
use crate::daemon::{Agent, Workspace};
use crate::store::{self, Conn, Tab};
use crate::ui::kit::text;
use crate::ui::kit::*;

#[derive(Clone, Copy, Default, PartialEq)]
enum HomeTab {
    #[default]
    Chats,
    Projects,
    Usage,
    Settings,
}

impl HomeTab {
    fn index(self) -> u32 {
        self as u32
    }
    fn from_index(i: u32) -> Self {
        match i {
            1 => HomeTab::Projects,
            2 => HomeTab::Usage,
            3 => HomeTab::Settings,
            _ => HomeTab::Chats,
        }
    }
}

/// Room the floating bar and the new button take at the foot of the lists.
const BAR_ROOM: f32 = 112.0;

pub(crate) fn home() -> impl IntoElement {
    let tab = signal(HomeTab::Chats);
    let query = signal(String::new());
    view()
        .flex_1()
        .pt_safe_area()
        .child(dynamic(move || {
            let current = tab.get();
            Some(untracked(move || {
                match current {
                    HomeTab::Settings => view().flex_1().child(settings::settings()).into_element(),
                    // What the computer's agents used, and their plan
                    // limits: asked for each time the tab opens.
                    HomeTab::Usage => {
                        store::refresh_usage();
                        view()
                            .flex_1()
                            .child(large_header(
                                t("Usage"),
                                view().flex_row().items_center().gap(px(S2)).child(host_chip()).child(round_button(Icon::RotateCw, t("Refresh"), store::refresh_usage)),
                            ))
                            .child(scroll().flex_1().child(view().px(px(S4)).pt(px(S2)).pb(px(BAR_ROOM)).child(crate::ui::shared::usage::usage_sections())))
                            .into_element()
                    }
                    HomeTab::Chats => view()
                        .flex_1()
                        .child(large_header(t("Chats"), host_chip()))
                        .child(search_field(query, t("Search conversations")))
                        .child(chats(query))
                        .into_element(),
                    HomeTab::Projects => view()
                        .flex_1()
                        .child(large_header(
                            t("Projects"),
                            view().flex_row().items_center().gap(px(S2)).child(host_chip()).child(round_button(Icon::Plus, t("Add project"), || {
                                show_sheet(None, vec![
                                    SheetItem::new(t("Add a folder of the computer"), || super::push(super::Route::AddProject)).icon(Icon::FolderPlus),
                                ])
                            })),
                        ))
                        .child(search_field(query, t("Search projects")))
                        .child(projects(query, false))
                        .into_element(),
                }
            }))
        }))
        .child(foot(tab))
}

/// iOS's large title, with what the screen offers beside it.
fn large_header(title: &'static str, trailing: impl IntoElement) -> View {
    view()
        .flex_row()
        .items_center()
        .justify_between()
        .gap(px(S3))
        .px(px(S4))
        .pt(px(S2))
        .pb(px(S2))
        .child(text(title).text_size(px(30.0)).font_weight(FontWeight(700)).number_of_lines(1).flex_shrink(1.0))
        .child(trailing)
}

/// iOS's search field: a 36 pt rounded field on a light fill.
fn search_field(query: Signal<String>, placeholder: &'static str) -> View {
    view().px(px(S4)).pb(px(S3)).child(
        view()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(36.0))
            .px(px(10.0))
            .rounded(px(10.0))
            .bg(tint(FG, 0.07))
            .child(icon(Icon::Search, 16.0).text_color(FG_MUTED))
            .child(
                input()
                    .flex_1()
                    .text_size(px(TEXT_LG))
                    .text_color(FG)
                    .value(move || query.get())
                    .on_change(move |v: &str| query.set(v.to_owned()))
                    .placeholder(placeholder),
            ),
    )
}

/// A 36 pt glass circle.
fn round_button(glyph: Icon, label: &'static str, on_press: impl Fn() + 'static) -> impl IntoElement {
    liquid_glass(view()).w(px(36.0)).h(px(36.0)).rounded(px(R_FULL)).child(
        button()
            .on_press(on_press)
            .accessibility_label(label)
            .flex_1()
            .rounded(px(R_FULL))
            .items_center()
            .justify_center()
            .active(|s| s.opacity(0.6))
            .child(icon(glyph, 18.0).text_color(FG)),
    )
}

/// The computer followed, as a small chip: its status and name; pressed,
/// the computers and pairing another.
fn host_chip() -> impl IntoElement {
    let conn = computed(store::conn);
    let name = computed(move || match conn.get() {
        Conn::Connected { hostname, .. } if !hostname.is_empty() => hostname,
        _ => store::current_host().map(|h| h.label()).unwrap_or_default(),
    });
    button()
        .on_press(move || {
            let status = match conn.get_untracked() {
                Conn::Connected { .. } => t("Connected").to_owned(),
                Conn::Down(_) => t("Offline").to_owned(),
                _ => t("Connecting…").to_owned(),
            };
            let current = store::current_host().map(|h| h.id);
            let mut items: Vec<SheetItem> = store::hosts()
                .into_iter()
                .map(|h| {
                    let on = current.as_deref() == Some(h.id.as_str());
                    let id = h.id.clone();
                    SheetItem::new(h.label(), move || store::switch_host(Some(id.clone()))).icon(Icon::Server).checked(on)
                })
                .collect();
            items.push(SheetItem::new(t("Pair another computer"), || store::switch_host(None)).icon(Icon::Plus));
            if let Some(id) = current {
                items.push(SheetItem::destructive(t("Forget this computer"), move || store::remove_host(&id)).icon(Icon::Trash2));
            }
            show_sheet(Some(format!("{} · {status}", name.get_untracked())), items);
        })
        .accessibility_label(t("Computers"))
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(32.0))
        .max_w(px(180.0))
        .pl(px(10.0))
        .pr(px(S2))
        .rounded(px(R_FULL))
        .bg(tint(FG, 0.07))
        .active(|s| s.opacity(0.7))
        .child(crate::ui::shared::conn_mark())
        .child(text(move || name.get()).text_size(px(TEXT_SM)).font_weight(FontWeight(500)).number_of_lines(1).flex_shrink(1.0))
        .child(icon(Icon::ChevronDown, 12.0).text_color(FG_MUTED))
}

/// The foot: the system tab bar, icons only, with the four tabs, and
/// beside it (not over it) a round glass button that starts a conversation
/// (a screen to pick its workspace). The bar keeps its width on the tabs
/// that offer no new conversation, the button's place left empty.
fn foot(tab: Signal<HomeTab>) -> impl IntoElement {
    const NEW: f32 = 56.0;
    view()
        .absolute()
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .pb_safe_area()
        .pointer_events(nsh_core::PointerEvents::BoxNone)
        .child(
            view()
                .flex_row()
                .items_center()
                .pr(px(S4))
                .pointer_events(nsh_core::PointerEvents::BoxNone)
                .child(
                    view().flex_1().child(
                        tab_bar()
                            .items(vec![
                                TabBarItem::new("").icon("bubble.left.and.bubble.right"),
                                TabBarItem::new("").icon("folder"),
                                TabBarItem::new("").icon("chart.bar"),
                                TabBarItem::new("").icon("gearshape"),
                            ])
                            .selected_index(move || tab.get().index())
                            .on_change(move |i: &u32| {
                                dismiss_keyboard();
                                tab.set(HomeTab::from_index(*i));
                            }),
                    ),
                )
                .child(
                    // Level with the bar's pill, which sits above the
                    // middle of the bar's own box.
                    view().w(px(NEW)).h(px(NEW)).translate_y(DefiniteLength::from(px(-11.0))).pointer_events(nsh_core::PointerEvents::BoxNone).child(
                        // Only the conversations offer a new one.
                        when(move || tab.get() == HomeTab::Chats, move || {
                            liquid_glass(view()).w(px(NEW)).h(px(NEW)).rounded(px(R_FULL)).shadow_lg().child(
                                button()
                                    .on_press(|| {
                                        dismiss_keyboard();
                                        super::push(super::Route::NewChat);
                                    })
                                    .accessibility_label(t("New conversation"))
                                    .flex_1()
                                    .rounded(px(R_FULL))
                                    .items_center()
                                    .justify_center()
                                    .active(|s| s.opacity(0.6))
                                    .child(icon(Icon::SquarePen, 22.0).text_color(FG)),
                            )
                        }),
                    ),
                ),
        )
}

fn project_of(w: &Workspace) -> String {
    if w.project_name.is_empty() { crate::daemon::model::folder_name(&w.project_root) } else { w.project_name.clone() }
}

fn matches(query: &str, fields: &[&str]) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(&q))
}

/// A round glyph tinted by a status: the leading mark of a row.
fn glyph(icon_of: Icon, status: Status) -> View {
    let color = match status {
        Status::Running => ACCENT,
        Status::NeedsYou => WARNING,
        Status::Failed => DANGER,
        _ => FG_MUTED,
    };
    view()
        .w(px(36.0))
        .h(px(36.0))
        .flex_shrink_0()
        .rounded(px(R_FULL))
        .bg(tint(color, 0.13))
        .items_center()
        .justify_center()
        .child(icon(icon_of, 18.0).text_color(color))
}

/// A group's header: its chevron, name and count; pressed, it folds.
fn group_header(project_id: String, name: String, count: usize) -> impl IntoElement {
    let pid = project_id.clone();
    let folded = computed(move || store::is_collapsed(&pid));
    button()
        .on_press(move || store::toggle_collapsed(&project_id))
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(32.0))
        .px(px(S1))
        .active(|s| s.opacity(0.6))
        .child(
            icon(Icon::ChevronDown, 13.0)
                .text_color(FG_FAINT)
                .rotate(move || Radians(if folded.get() { -std::f32::consts::FRAC_PI_2 } else { 0.0 }))
                .transition_transform(),
        )
        .child(text(name).text_size(px(TEXT_SM)).font_weight(FontWeight(600)).text_color(FG_MUTED).number_of_lines(1).flex_1())
        .child(text(count.to_string()).text_size(px(TEXT_SM)).text_color(FG_FAINT))
}

/// A folding group: its header always, its card revealed (a clipped
/// height springing open and shut, natively: nsh/base's Collapsible). The
/// card is made once, so folding re-creates nothing and nothing flashes.
fn group(project_id: String, name: String, rows: Vec<AnyElement>) -> impl IntoElement {
    let pid = project_id.clone();
    let count = rows.len();
    nsh_base::collapsible()
        .open(move || !store::is_collapsed(&pid))
        .reveal(true)
        .gap(px(S1))
        .content(group_card(rows))
        .child(group_header(project_id, name, count))
}

/// Grouped rows, iOS's inset style: a rounded card whose dividers start at
/// the rows' text.
fn group_card(rows: Vec<AnyElement>) -> View {
    let mut c = view().rounded(px(14.0)).bg(SURFACE0).border_1().border_color(tint(BORDER, 0.7)).overflow_hidden();
    for r in rows {
        c = c.child(r);
    }
    c
}

/// A row: its glyph, then title and line, then what trails; the divider
/// above it (but the first) starts where its text does.
fn row_shell(lead: View, title: String, line: String, trailing: View, divider: bool, on_press: impl Fn() + 'static) -> AnyElement {
    button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap(px(S3))
        .pl(px(S4))
        .active(|s| s.bg(highlight()))
        .child(lead)
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .flex_row()
                .items_center()
                .gap(px(S2))
                .min_h(px(60.0))
                .py(px(10.0))
                .pr(px(S4))
                .when(divider, |v| v.border_t_1().border_color(tint(BORDER, 0.8)))
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap(px(2.0))
                        .child(text(title).text_size(px(16.0)).font_weight(FontWeight(600)).number_of_lines(1))
                        .child(text(line).text_size(px(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1)),
                )
                .child(trailing),
        )
        .into_element()
}

/// Every conversation, the latest first.
fn chats(query: Signal<String>) -> impl IntoElement {
    let list = computed(move || {
        let q = query.get();
        let mut agents: Vec<(Agent, Option<Workspace>)> = store::agents()
            .into_iter()
            .filter(|a| !a.archived && a.parent_id.is_none() && !store::is_hidden_run(a))
            .map(|a| {
                let w = a.workspace_id.as_deref().and_then(store::workspace);
                (a, w)
            })
            .filter(|(a, w)| matches(&q, &[&a.title, w.as_ref().map(|w| w.name.as_str()).unwrap_or("")]))
            .collect();
        agents.sort_by(|a, b| b.0.updated_at.cmp(&a.0.updated_at));
        agents
    });
    scroll().flex_1().child(dynamic(move || {
        let items = list.get();
        let waiting = !matches!(store::conn(), Conn::Connected { .. });
        Some(untracked(move || {
            if items.is_empty() {
                // Nothing kept of this computer and it has not answered yet.
                if waiting {
                    return connecting().into_element();
                }
                return empty(t("No conversations yet")).into_element();
            }
            // One list, the latest first, as Messages' (the projects tab
            // groups by project).
            let rows = items.iter().cloned().enumerate().map(|(i, (a, w))| chat_row(a, w, i > 0)).collect();
            let col = view().px(px(S4)).pb(px(BAR_ROOM)).child(group_card(rows));
            col.into_element()
        }))
    }))
}

fn chat_row(a: Agent, w: Option<Workspace>, divider: bool) -> AnyElement {
    let needs_you = a.requires_attention || !a.pending.is_empty();
    let status = Status::of_agent(&a.status, needs_you);
    let line = match &w {
        Some(w) => format!("{} · {}", w.name, project_of(w)),
        None => crate::daemon::model::folder_name(&a.cwd),
    };
    let trailing = view()
        .items_end()
        .gap(px(4.0))
        .child(text(time::ago(&a.updated_at)).text_size(px(TEXT_XS)).text_color(FG_FAINT))
        .child(if needs_you {
            view()
                .min_w(px(18.0))
                .h(px(18.0))
                .px(px(5.0))
                .rounded(px(R_FULL))
                .bg(ACCENT)
                .items_center()
                .justify_center()
                .child(text(a.pending.len().max(1).to_string()).text_size(px(11.0)).font_weight(FontWeight(600)).text_color(ACCENT_FG))
        } else {
            view().h(px(18.0))
        });
    let (id, ws) = (a.id.clone(), w.as_ref().map(|w| w.id.clone()));
    row_shell(glyph(Icon::Bot, status), a.title.clone(), line, trailing, divider, move || {
        match &ws {
            Some(ws) => super::open_chat(ws, Some(Tab::Agent(id.clone()))),
            // Its workspace was archived under it: restored, then shown.
            None => store::open_run_agent(&id, None),
        }
    })
}

/// The projects, each a folding group of its workspaces; a workspace
/// opens its last conversation, or, with `new_agent`, a new one.
pub(crate) fn projects(query: Signal<String>, new_agent: bool) -> impl IntoElement {
    let groups = computed(move || {
        let q = query.get();
        let mut by_project: Vec<(String, String, Vec<Workspace>)> = Vec::new();
        let mut list = store::workspaces();
        list.sort_by(|a, b| b.activity_at.cmp(&a.activity_at));
        for w in list {
            if !matches(&q, &[&w.name, &project_of(&w), &w.project_root]) {
                continue;
            }
            match by_project.iter_mut().find(|g| g.0 == w.project_id) {
                Some(g) => g.2.push(w),
                None => by_project.push((w.project_id.clone(), project_of(&w), vec![w])),
            }
        }
        by_project
    });
    scroll().flex_1().child(dynamic(move || {
        let groups = groups.get();
        let waiting = !matches!(store::conn(), Conn::Connected { .. });
        Some(untracked(move || {
            if groups.is_empty() {
                if waiting {
                    return connecting().into_element();
                }
                return empty(t("No workspaces yet. Start one on the computer.")).into_element();
            }
            let mut col = view().px(px(S4)).pb(px(BAR_ROOM)).gap(px(S3));
            for (pid, name, workspaces) in groups.clone() {
                let rows = workspaces.into_iter().enumerate().map(|(i, w)| workspace_row(w, new_agent, i > 0)).collect();
                col = col.child(group(pid, name, rows));
            }
            col.into_element()
        }))
    }))
}

fn workspace_row(w: Workspace, new_agent: bool, divider: bool) -> AnyElement {
    let id = w.id.clone();
    let agents = store::workspace_agents(&w.id).iter().filter(|a| !a.archived).count();
    let line = if agents == 1 { t("1 agent").to_owned() } else { tf("{} agents", &[&agents.to_string()]) };
    let trailing = view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .child(text(time::ago(&w.activity_at)).text_size(px(TEXT_XS)).text_color(FG_FAINT))
        .child(icon(Icon::ChevronRight, 14.0).text_color(FG_FAINT));
    let kind = if w.branch.is_some() { Icon::GitBranch } else { Icon::Folder };
    row_shell(glyph(kind, Status::of_workspace(&w.status)), w.name.clone(), line, trailing, divider, move || {
        super::open_chat(&id, new_agent.then_some(Tab::Draft))
    })
}

/// In an empty list's place while the computer is being reached: the
/// loader, and what it waits for.
fn connecting() -> impl IntoElement {
    view().items_center().pt(px(S8 * 2.0)).child(loader(None)).child(
        text(|| match store::conn() {
            Conn::Down(_) => t("Can't reach the computer · retrying").to_owned(),
            _ => t("Connecting…").to_owned(),
        })
        .mt(px(S3))
        .text_size(px(TEXT_SM))
        .text_color(FG_MUTED),
    )
}

fn empty(message: &'static str) -> impl IntoElement {
    view()
        .items_center()
        .px(px(S8))
        .pt(px(S8 * 2.0))
        .child(text(message).text_size(px(TEXT_BASE)).line_height(px(20.0)).text_color(FG_MUTED).text_center())
}

/// A new conversation: the workspaces to start it in, searchable, under
/// the screen's bar.
pub(crate) fn new_chat_page() -> nsh_core::element::Screen {
    let query = signal(String::new());
    nsh_base::nav_page().key("new").header_visible(false).title(t("New conversation")).child(
        view()
            .flex_1()
            .child(super::top_bar(
                text(t("New conversation")).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)),
                view().w(px(44.0)),
            ))
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
                                .placeholder(t("Search workspaces")),
                        ),
                ),
            )
            .child(view().flex_1().px_safe_area().child(projects(query, true))),
    )
}
