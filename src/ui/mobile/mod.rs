//! The phone: a remote for the agents on a computer. Nothing runs on the
//! phone, so it keeps what a remote needs (the agents, their conversations,
//! answering them, the changes to read) and leaves projects, git, terminals
//! and the service to the desktop.
//!
//! With no computer paired it shows how to pair one. Then a native
//! navigation stack (the system header and swipe back): the home screen
//! (conversations, projects, settings under the system tab bar, Liquid
//! Glass on iOS 26 and later) with the conversation, the changes and a
//! file's diff pushed over it.

mod add_project;
mod changes;
mod files;
mod chat;
mod connect;
mod home;
mod settings;
mod time;

use nsh_base::{nav_page, nav_stack};
use nsh_core::prelude::*;

use crate::store;
use crate::ui::kit::*;
use crate::ui::shared::modals;

/// A screen pushed over the home screen.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum Route {
    /// Picking the workspace of a new conversation.
    NewChat,
    /// Adding a folder of the computer as a project.
    AddProject,
    Chat,
    Changes,
    Diff(String),
    /// A folder of the workspace (`.`, its root), and a file of it.
    Files(String),
    File(String),
    /// A sub-agent's timeline: (parent agent, sub-agent).
    Subagent(String, String),
}

impl Route {
    fn key(&self) -> String {
        match self {
            Route::NewChat => "new".into(),
            Route::AddProject => "add-project".into(),
            Route::Chat => "chat".into(),
            Route::Changes => "changes".into(),
            Route::Diff(p) => format!("diff:{p}"),
            Route::Files(p) => format!("files:{p}"),
            Route::File(p) => format!("file:{p}"),
            Route::Subagent(_, id) => format!("sub:{id}"),
        }
    }
}

#[derive(Clone, Default, PartialEq)]
struct Routes(Vec<Route>);

/// Pushes `route` (one of each at a time: pushing a route already open goes
/// back to it).
pub(crate) fn push(route: Route) {
    global::<Routes>().update(|r| {
        if let Some(i) = r.0.iter().position(|x| *x == route) {
            r.0.truncate(i + 1);
        } else {
            r.0.push(route);
        }
    });
}

/// The user went back from `route` (the back button or the swipe).
fn popped(route: &Route) {
    global::<Routes>().update(|r| {
        if let Some(i) = r.0.iter().position(|x| x == route) {
            r.0.truncate(i);
        }
    });
    if *route == Route::Chat {
        store::go_home();
    }
}

/// Opens workspace `id` (its last agent, or a new one) as a conversation.
pub(crate) fn open_chat(workspace: &str, tab: Option<store::Tab>) {
    store::open_workspace(workspace);
    if let Some(tab) = tab {
        store::open_tab(tab);
    }
    global::<Routes>().update(|r| r.0.clear());
    push(Route::Chat);
}

/// Goes back from the top screen, as the back button does.
pub(crate) fn back() {
    let top = global::<Routes>().with_untracked(|r| r.0.last().cloned());
    if let Some(route) = top {
        dismiss_keyboard();
        popped(&route);
    }
}

/// A pushed screen's bar, over its content: a glass back button, the title,
/// and glass actions (the system header stays hidden; the edge swipe still
/// goes back).
pub(crate) fn top_bar(title: impl IntoElement, actions: impl IntoElement) -> View {
    view().pt_safe_area().px_safe_area().child(
        view()
            .flex_row()
            .items_center()
            .gap(px(S2))
            .h(px(60.0))
            .px(px(S3))
            .child(glass_button(Icon::ChevronLeft, crate::i18n::t("Back"), back))
            .child(view().flex_1().min_w(px(0.0)).items_center().child(title))
            .child(actions),
    )
}

/// A 44 pt glass circle with an icon.
pub(crate) fn glass_button(glyph: Icon, label: &'static str, on_press: impl Fn() + 'static) -> impl IntoElement {
    liquid_glass(view()).w(px(44.0)).h(px(44.0)).rounded(px(R_FULL)).child(
        nsh_base::button()
            .on_press(on_press)
            .accessibility_label(label)
            .flex_1()
            .rounded(px(R_FULL))
            .items_center()
            .justify_center()
            .active(|s| s.opacity(0.6))
            .child(icon(glyph, 20.0).text_color(FG)),
    )
}

/// A glass capsule holding a row of 44 pt actions.
pub(crate) fn glass_group() -> View {
    liquid_glass(view()).flex_row().items_center().h(px(44.0)).px(px(2.0)).rounded(px(R_FULL))
}

pub(crate) fn shell() -> impl IntoElement {
    let paired = computed(|| store::current_host().is_some());
    view()
        .flex_1()
        .text_color(FG)
        .child(ambient())
        .child(modals::modal_layer())
        .child(dynamic(move || {
            let paired = paired.get();
            Some(untracked(move || {
                if paired {
                    stack().into_element()
                } else {
                    global::<Routes>().update(|r| r.0.clear());
                    view().flex_1().p_safe_area().child(connect::connect()).into_element()
                }
            }))
        }))
        // Last: over the pages and their tab bar.
        .child(sheet_layer())
}

fn stack() -> impl IntoElement {
    // A sub-agent opens as a screen of its own over the conversation (the
    // desktop opens it as a tab): the conversation keeps its agent.
    effect(|| {
        if let store::Tab::Subagent(parent, id) = store::tab() {
            untracked(|| {
                store::open_tab(store::Tab::Agent(parent.clone()));
                push(Route::Subagent(parent, id));
            });
        }
    });
    nav_stack()
        .flex_1()
        .child(nav_page().key("home").header_visible(false).child(home::home()))
        .child(each(
            || global::<Routes>().with(|r| r.0.clone()),
            |r| r.key(),
            |r, _| {
                let route = r.get_untracked();
                let gone = route.clone();
                let page = match &route {
                    Route::NewChat => home::new_chat_page(),
                    Route::AddProject => add_project::page(),
                    Route::Chat => chat::page(),
                    Route::Changes => changes::files_page(),
                    Route::Diff(path) => changes::diff_page(path.clone()),
                    Route::Files(dir) => files::dir_page(dir.clone()),
                    Route::File(path) => files::file_page(path.clone()),
                    Route::Subagent(parent, id) => chat::subagent_page(parent.clone(), id.clone()),
                };
                page.on_dismiss(move || popped(&gone))
            },
        ))
}
