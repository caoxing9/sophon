//! A conversation, pushed over the home screen: the system header (back,
//! the agent over its workspace, the agent switcher, the changes and the
//! agent's actions), the timeline and the composer on Liquid Glass.
//!
//! The keyboard never sticks: dragging the conversation down takes it
//! along, a tap on the conversation puts it away, and so does any menu or
//! screen that opens.

use crate::i18n::t;
use nsh_base::{button, nav_page};
use nsh_core::{
    element::{FontWeight, Screen},
    prelude::*,
};

use super::{Route, glass_group, push, top_bar};
use crate::store::{self, Conn, Screen as AppScreen, Tab};
use crate::ui::kit::text;
use crate::ui::kit::*;
use crate::ui::shared::agent;

pub(crate) fn page() -> Screen {
    let ws = computed(store::current_workspace);
    let selected = computed(store::selected_agent);
    nav_page()
        .key("chat")
        .header_visible(false)
        .title(move || -> SharedString {
            selected.with(|a| a.as_ref().map(|a| a.title.clone()).unwrap_or_else(|| t("New agent").into())).into()
        })
        .child(
            view()
                .flex_1()
                .child(top_bar(
                    view()
                        .items_center()
                        .child(
                            text(move || selected.with(|a| a.as_ref().map(|a| a.title.clone()).unwrap_or_else(|| t("New agent").into())))
                                .text_size(tx(TEXT_BASE))
                                .font_weight(FontWeight(600))
                                .number_of_lines(1),
                        )
                        .child(
                            text(move || ws.with(|w| w.as_ref().map(|w| w.name.clone()).unwrap_or_default()))
                                .text_size(tx(TEXT_XS))
                                .text_color(FG_MUTED())
                                .number_of_lines(1),
                        ),
                    glass_group()
                        .child(header_button(Icon::FileDiff, t("Changes"), || {
                            dismiss_keyboard();
                            push(Route::Changes)
                        }))
                        .child(header_button(Icon::Ellipsis, t("Agent actions"), agent_sheet)),
                ))
                .child(body()),
        )
}

#[derive(Clone, Copy, PartialEq)]
enum Pane {
    Agent,
    Draft,
    None,
}

fn body() -> impl IntoElement {
    let pane = computed(|| match (store::screen(), store::tab()) {
        (AppScreen::Workspace(_), Tab::Agent(_)) => Pane::Agent,
        (AppScreen::Workspace(_), _) => Pane::Draft,
        _ => Pane::None,
    });
    view()
        .flex_1()
        .px_safe_area()
        // Over the keyboard while it shows: the composer rides just above
        // it; the home indicator's room otherwise.
        .pb(move || Length::from(px(keyboard_height().0.max(safe_area().bottom.0))))
        .child(dynamic(move || {
            let p = pane.get();
            Some(untracked(move || match p {
                Pane::Agent => agent::agent_pane(false).into_element(),
                Pane::Draft => draft().into_element(),
                Pane::None => view().flex_1().into_element(),
            }))
        }))
        // Last, so over the conversation: at its top.
        .child(crate::ui::shared::conn_banner::conn_banner(6.0))
}

/// The workspace's agents, the open one checked, and a new one: the
/// system's sheet (from the open agent's actions).
fn agents_sheet() {
    let agents = store::current_workspace().map(|w| store::workspace_agents(&w.id)).unwrap_or_default();
    let open = store::selected_id();
    let mut items: Vec<SheetItem> = agents
        .iter()
        .rev()
        .filter(|a| !a.archived)
        .map(|a| {
            let id = a.id.clone();
            let status = Status::of_agent(&a.status, a.requires_attention || !a.pending.is_empty()).label();
            let on = open.as_deref() == Some(a.id.as_str());
            SheetItem::new(format!("{} · {status}", a.title), move || store::open_tab(Tab::Agent(id.clone()))).icon(Icon::Bot).checked(on)
        })
        .collect();
    items.push(SheetItem::new(t("New agent"), || store::open_tab(Tab::Draft)).icon(Icon::SquarePen));
    show_sheet(Some(t("Agents").into()), items);
}

/// A 40 × 44 pt action of the header's glass group.
fn header_button(glyph: Icon, label: &'static str, on_press: impl Fn() + 'static) -> impl IntoElement {
    button()
        .on_press(on_press)
        .accessibility_label(label)
        .w(px(40.0))
        .h(px(44.0))
        .items_center()
        .justify_center()
        .active(|s| s.opacity(0.6))
        .child(icon(glyph, 20.0).text_color(FG()))
}

/// The open agent's actions, as the system's sheet.
fn agent_sheet() {
    let Some(a) = store::selected_agent() else { return };
    let (rename, archive, title) = (a.id.clone(), a.id.clone(), a.title.clone());
    let others = store::current_workspace()
        .map(|w| store::workspace_agents(&w.id).iter().filter(|x| !x.archived && x.id != a.id).count())
        .unwrap_or(0);
    let mut items = vec![
        // The workspace's other agents (the header's count once).
        if others > 0 {
            SheetItem::new(crate::i18n::tf("Switch agent · {}", &[&(others + 1).to_string()]), || {
                // After this sheet has gone.
                set_timeout(std::time::Duration::from_millis(350), agents_sheet);
            })
            .icon(Icon::Bot)
        } else {
            SheetItem::new(t("New agent"), || store::open_tab(Tab::Draft)).icon(Icon::SquarePen)
        },
        SheetItem::new(t("Browse files"), || push(Route::Files(".".into()))).icon(Icon::Folder),
    ];
    // The workspace's pull request, when it has one: its page here.
    if let Some((repo, number)) = store::git().pr.and_then(|pr| crate::github::of_url(&pr.url)) {
        let from = a.id.clone();
        items.insert(1, SheetItem::new(crate::i18n::tf("Pull request #{}", &[&number.to_string()]), move || {
            push(Route::PullRequest(repo.clone(), number, Some(from.clone())))
        })
        .icon(Icon::GitPullRequest));
    }
    items.extend([
        SheetItem::new(t("Fork into new agent"), store::fork_selected).icon(Icon::GitCompare),
        SheetItem::new(t("Rename"), move || store::open_modal(store::Modal::RenameAgent { id: rename.clone(), name: title.clone() })).icon(Icon::Pencil),
    ]);
    // Pinned, it heads the conversations list, as in the desktop's sidebar.
    let (pin, pinned) = (a.id.clone(), a.pinned);
    items.push(
        SheetItem::new(if pinned { t("Unpin") } else { t("Pin") }, move || store::pin_agent(&pin, !pinned))
            .icon(if pinned { Icon::PinOff } else { Icon::Pin }),
    );
    let subs = store::subagents(&a.id).len();
    if subs > 0 {
        let parent = a.id.clone();
        let label = if store::subagents_shown(&a.id) { crate::i18n::tf("Hide sub-agents · {}", &[&subs.to_string()]) } else { crate::i18n::tf("Show sub-agents · {}", &[&subs.to_string()]) };
        items.push(SheetItem::new(label, move || store::toggle_subagents(&parent)).icon(Icon::Bot));
    }
    if a.is_running() {
        items.push(SheetItem::new(t("Stop"), store::stop_selected).icon(Icon::Square));
    }
    items.push(SheetItem::destructive(t("Archive agent"), move || store::archive_agent(&archive)).icon(Icon::Archive));
    show_sheet(Some(a.title.clone()), items);
}

/// A sub-agent's work, read only, over the conversation that started it.
pub(crate) fn subagent_page(parent: String, id: String) -> Screen {
    let (p2, i2) = (parent.clone(), id.clone());
    let title = computed(move || store::subagent(&p2, &i2).map(|s| s.title).unwrap_or_else(|| t("Sub-agent").into()));
    nav_page().key(format!("sub:{id}")).header_visible(false).title(t("Sub-agent")).child(
        view()
            .flex_1()
            .child(top_bar(
                text(move || title.get()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1),
                view().w(px(44.0)),
            ))
            .child(view().flex_1().px_safe_area().pb_safe_area().child(crate::ui::shared::timeline::subagent_pane(parent, id))),
    )
}

/// A new conversation, as Lody's: an empty page, then where it runs (the
/// computer and the workspace, each a row that opens its choices) over the
/// composer.
fn draft() -> impl IntoElement {
    let ws = computed(store::current_workspace);
    let conn = computed(store::conn);
    let host = computed(move || match conn.get() {
        Conn::Connected { hostname, .. } if !hostname.is_empty() => hostname,
        _ => store::current_host().map(|h| h.label()).unwrap_or_default(),
    });
    let error = computed(|| store::form().error);
    view()
        .flex_1()
        .child(view().flex_1().items_center().justify_center().child(mark(48.0, Lift::Flat)))
        .child(
            view()
                .px(sp(S2))
                .pb(sp(S2))
                .gap(sp(S2))
                .child(when(move || error.with(Option::is_some), move || {
                    text(move || error.get().unwrap_or_default()).px(sp(S3)).text_size(tx(TEXT_SM)).text_color(DANGER)
                }))
                .child(picker_row(Icon::Cpu, move || host.get(), || {
                    let current = store::current_host().map(|h| h.id);
                    let items = store::hosts()
                        .into_iter()
                        .map(|h| {
                            let on = current.as_deref() == Some(h.id.as_str());
                            let id = h.id.clone();
                            SheetItem::new(h.label(), move || store::switch_host(Some(id.clone()))).icon(Icon::Server).checked(on)
                        })
                        .collect();
                    show_sheet(Some(t("Computer").into()), items);
                }))
                .child(picker_row(
                    Icon::Folder,
                    move || ws.with(|w| w.as_ref().map(|w| w.name.clone()).unwrap_or_default()),
                    move || {
                        let current = ws.with_untracked(|w| w.as_ref().map(|w| w.id.clone()));
                        let mut list = store::workspaces();
                        list.sort_by(|a, b| b.activity_at.cmp(&a.activity_at));
                        let items = list
                            .into_iter()
                            .take(20)
                            .map(|w| {
                                let on = current.as_deref() == Some(w.id.as_str());
                                let id = w.id.clone();
                                let label = if w.project_name.is_empty() { w.name.clone() } else { format!("{} · {}", w.name, w.project_name) };
                                SheetItem::new(label, move || super::open_chat(&id, Some(Tab::Draft)))
                                    .icon(if w.branch.is_some() { Icon::GitBranch } else { Icon::Folder })
                                    .checked(on)
                            })
                            .collect();
                        show_sheet(Some(t("Workspace").into()), items);
                    },
                ))
                .child(crate::ui::shared::composer::composer(crate::ui::shared::composer::Target::Form, "")),
        )
}

/// A row that says a choice and opens its options: an icon, the value, and
/// the up-down chevrons iOS gives such rows.
fn picker_row(glyph: Icon, value: impl Fn() -> String + 'static, on_press: impl Fn() + 'static) -> impl IntoElement {
    button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .h(px(52.0))
        .px(sp(S4))
        .rounded(px(16.0))
        .bg(tint(FG(), 0.06))
        .active(|s| s.opacity(0.7))
        .child(icon(glyph, 18.0).text_color(FG_MUTED()))
        .child(text(value).text_size(tx(TEXT_LG)).number_of_lines(1).flex_1())
        .child(icon(Icon::ChevronsUpDown, 16.0).text_color(FG_FAINT()))
}
