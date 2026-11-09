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
                                .text_size(px(TEXT_BASE))
                                .font_weight(FontWeight(600))
                                .number_of_lines(1),
                        )
                        .child(
                            text(move || ws.with(|w| w.as_ref().map(|w| w.name.clone()).unwrap_or_default()))
                                .text_size(px(TEXT_XS))
                                .text_color(FG_MUTED)
                                .number_of_lines(1),
                        ),
                    glass_group()
                        .child(switcher(ws))
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
        .child(offline_banner())
        .child(dynamic(move || {
            let p = pane.get();
            Some(untracked(move || match p {
                Pane::Agent => agent::agent_pane(false).into_element(),
                Pane::Draft => draft().into_element(),
                Pane::None => view().flex_1().into_element(),
            }))
        }))
}

/// The workspace's agents in a rounded square with their count; pressed,
/// the system's sheet of them and a new one.
fn switcher(ws: Memo<Option<crate::daemon::Workspace>>) -> impl IntoElement {
    let count = computed(move || {
        ws.with(|w| w.as_ref().map(|w| store::workspace_agents(&w.id).iter().filter(|a| !a.archived).count()).unwrap_or(0))
    });
    button()
        .on_press(move || {
            let agents = ws.with_untracked(|w| w.as_ref().map(|w| store::workspace_agents(&w.id)).unwrap_or_default());
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
        })
        .accessibility_label(t("Agents"))
        .w(px(40.0))
        .h(px(44.0))
        .items_center()
        .justify_center()
        .active(|s| s.opacity(0.6))
        .child(
            view()
                .min_w(px(22.0))
                .h(px(22.0))
                .px(px(4.0))
                .rounded(px(6.0))
                .border(px(1.5))
                .border_color(FG)
                .items_center()
                .justify_center()
                .child(text(move || count.get().to_string()).text_size(px(TEXT_SM)).font_weight(FontWeight(600))),
        )
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
        .child(icon(glyph, 20.0).text_color(FG))
}

/// The open agent's actions, as the system's sheet.
fn agent_sheet() {
    let Some(a) = store::selected_agent() else { return };
    let (rename, archive, title) = (a.id.clone(), a.id.clone(), a.title.clone());
    let mut items = vec![
        SheetItem::new(t("Browse files"), || push(Route::Files(".".into()))).icon(Icon::Folder),
        SheetItem::new(t("Fork into new agent"), store::fork_selected).icon(Icon::GitCompare),
        SheetItem::new(t("Rename"), move || store::open_modal(store::Modal::RenameAgent { id: rename.clone(), name: title.clone() })).icon(Icon::Pencil),
    ];
    if a.is_running() {
        items.push(SheetItem::new(t("Stop"), store::stop_selected).icon(Icon::Square));
    }
    items.push(SheetItem::destructive(t("Archive agent"), move || store::archive_agent(&archive)).icon(Icon::Archive));
    show_sheet(Some(a.title.clone()), items);
}

/// While the computer is out of reach: a strip over the conversation, so
/// nothing looks sent that is not.
fn offline_banner() -> impl IntoElement {
    let conn = computed(store::conn);
    let down = computed(move || !matches!(conn.get(), Conn::Connected { .. }));
    when(down, move || {
        view()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(S2))
            .h(px(28.0))
            .mx(px(S3))
            .mt(px(S1))
            .rounded(px(R_FULL))
            .bg(tint(WARNING, 0.14))
            .child(icon(Icon::RotateCw, 13.0).text_color(WARNING))
            .child(
                text(move || match conn.get() {
                    Conn::Down(_) => t("Can't reach the computer · retrying").to_owned(),
                    _ => t("Reconnecting…").to_owned(),
                })
                .text_size(px(TEXT_SM))
                .text_color(WARNING),
            )
    })
}

/// A sub-agent's work, read only, over the conversation that started it.
pub(crate) fn subagent_page(parent: String, id: String) -> Screen {
    let (p2, i2) = (parent.clone(), id.clone());
    let title = computed(move || store::subagent(&p2, &i2).map(|s| s.title).unwrap_or_else(|| t("Sub-agent").into()));
    nav_page().key(format!("sub:{id}")).header_visible(false).title(t("Sub-agent")).child(
        view()
            .flex_1()
            .child(top_bar(
                text(move || title.get()).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1),
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
        _ => store::current_host().map(|h| h.name).unwrap_or_default(),
    });
    let error = computed(|| store::form().error);
    view()
        .flex_1()
        .child(view().flex_1().items_center().justify_center().child(mark(48.0, Lift::Flat)))
        .child(
            view()
                .px(px(S2))
                .pb(px(S2))
                .gap(px(S2))
                .child(when(move || error.with(Option::is_some), move || {
                    text(move || error.get().unwrap_or_default()).px(px(S3)).text_size(px(TEXT_SM)).text_color(DANGER)
                }))
                .child(picker_row(Icon::Cpu, move || host.get(), || {
                    let current = store::current_host().map(|h| h.id);
                    let items = store::hosts()
                        .into_iter()
                        .map(|h| {
                            let on = current.as_deref() == Some(h.id.as_str());
                            let id = h.id.clone();
                            SheetItem::new(h.name.clone(), move || store::switch_host(Some(id.clone()))).icon(Icon::Server).checked(on)
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
        .gap(px(S3))
        .h(px(52.0))
        .px(px(S4))
        .rounded(px(16.0))
        .bg(tint(FG, 0.06))
        .active(|s| s.opacity(0.7))
        .child(icon(glyph, 18.0).text_color(FG_MUTED))
        .child(text(value).text_size(px(TEXT_LG)).number_of_lines(1).flex_1())
        .child(icon(Icon::ChevronsUpDown, 16.0).text_color(FG_FAINT))
}
