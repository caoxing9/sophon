//! The desktop shell: a pinned sidebar of projects and
//! workspaces beside the main surface on wide windows; on narrow ones, one
//! at a time. What only the desktop has (tabs, the explorer, git, terminals,
//! new workspaces, history, schedules, shortcuts) lives here.

mod explorer;
mod file_pane;
mod git;
mod history;
mod home;
mod new_workspace;
mod schedules;
mod settings;
pub(crate) mod shortcuts;
mod sidebar;
mod terminal;
mod usage;
mod workspace;

use crate::i18n::t;
use nsh_core::{
    element::desktop::{set_title_bar, title_bar},
    desktop::TitleBarStyle,
    prelude::*,
};

use crate::store::{self, Screen};
use crate::ui::{kit::*, shared::{modals, notice}};

/// The compact / wide boundary.
const SPLIT_WIDTH: f32 = 600.0;

#[derive(Clone, Copy, PartialEq)]
enum Layout {
    Split,
    Sidebar,
    Main,
}

pub(crate) fn shell() -> impl IntoElement {
    if is_desktop() {
        shortcuts::bind();
        // Claude's look: no system bar, the traffic lights sit
        // on the app's own surface.
        set_title_bar(TitleBarStyle::Hidden, Some(px(44.0)));
    }
    let layout = computed(|| {
        if window().width >= px(SPLIT_WIDTH) {
            Layout::Split
        } else if store::screen() == Screen::Home {
            Layout::Sidebar
        } else {
            Layout::Main
        }
    });
    // The safe area keeps content clear of the status bar, the Dynamic
    // Island and the home indicator; the ambient light still fills them.
    view().flex_1().p_safe_area().text_color(FG).child(ambient()).child(modals::modal_layer()).child(dynamic(move || {
        let layout = layout.get();
        Some(untracked(move || match layout {
            Layout::Split => {
                let hidden = computed(store::sidebar_hidden);
                view()
                    .flex_1()
                    .flex_row()
                    .child(
                        // Always mounted: hiding eases its width to zero,
                        // the glass inside keeps its own and slides out.
                        view()
                            .flex_shrink_0()
                            .items_end()
                            .overflow_hidden()
                            .w(move || Length::from(px(if hidden.get() { 0.0 } else { store::sidebar_width() })))
                            .opacity(move || if hidden.get() { 0.0_f32 } else { 1.0 })
                            .transition_opacity()
                            .child(
                                // Flush with the window, as a macOS sidebar: the
                                // traffic lights sit on it, not across an edge.
                                view()
                                    .child(chrome_strip())
                                    .child(sidebar::sidebar().flex_1())
                                    .flex_1()
                                    .w(move || Length::from(px(store::sidebar_width())))
                                    .flex_shrink_0()
                                    .bg(glass_fill())
                                    .backdrop_blur(px(GLASS_BLUR))
                                    .backdrop_saturate(1.7)
                                    .border_r_1()
                                    .border_color(tint(BORDER, 0.7)),
                            ),
                    )
                    .child(dynamic(move || (!hidden.get()).then(|| untracked(|| resize_handle().into_element()))))
                    .child(
                        view()
                            .flex_1()
                            .min_w(px(0.0))
                            .child(chrome_strip())
                            .child(main(false)),
                    )
                    .into_element()
            }
            Layout::Sidebar => sidebar::sidebar().bg(SIDEBAR).flex_1().into_element(),
            Layout::Main => main(true).into_element(),
        }))
    }))
    .child(sidebar_toggle())
    .child(quit_hint())
}

/// "Press ⌘Q again to quit", over the middle of the window while a second
/// press would quit (Chrome's hint).
fn quit_hint() -> impl IntoElement {
    let hint = computed(store::quit_hint);
    view()
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .items_center()
        .justify_center()
        .pointer_events(PointerEvents::None)
        .child(
            glass_strong(view())
                .px(px(S6))
                .py(px(S4))
                .rounded(px(R_LG + 4.0))
                .opacity(move || if hint.get().is_some() { 1.0_f32 } else { 0.0 })
                .transition_opacity()
                .child(
                    crate::ui::kit::text(move || hint.get().map(t).unwrap_or_default())
                        .text_size(px(TEXT_LG))
                        .font_weight(nsh_core::element::FontWeight(600))
                        .text_color(FG),
                ),
        )
}

/// The top of the window, where the system title bar was: it drags the
/// window.
fn chrome_strip() -> impl IntoElement {
    let bar = computed(title_bar);
    view().flex_shrink_0().h(move || Length::from(px(bar.get().top.0))).window_drag(true)
}

/// The sidebar toggle, beside the traffic lights over everything: it stays
/// put while the sidebar slides under it.
fn sidebar_toggle() -> impl IntoElement {
    let bar = computed(title_bar);
    let shown = computed(move || bar.get().top.0 > 0.0 && window().width >= px(SPLIT_WIDTH));
    when(shown, move || {
        view()
            .absolute()
            .top(px(0.0))
            .left(move || Length::from(px(bar.get().left.0 + 2.0)))
            .h(move || Length::from(px(bar.get().top.0)))
            .justify_center()
            .child(icon_btn(Icon::PanelLeft, Sz::Sm, t("Toggle sidebar  ⌘B"), store::toggle_sidebar))
    })
}

/// The sidebar's edge: dragging it sets the width; dragging it far
/// enough in folds the sidebar away (⌘B or the toggle opens it again).
fn resize_handle() -> impl IntoElement {
    let start = signal(0.0f32);
    let hover = signal(false);
    let dragging = signal(false);
    view()
        .w(px(8.0))
        .ml(px(-5.0))
        .mr(px(-3.0))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .cursor(Cursor::ColResize)
        .window_drag(false)
        .on_mouse_enter(move |_: &MouseEvent| hover.set(true))
        .on_mouse_leave(move |_: &MouseEvent| hover.set(false))
        .claim_touches(TouchClaim::on_start().ask(|request: &ClaimRequest| request.claim()))
        .keep_touches(true)
        .on_responder_grant(move |_: &ResponderEvent| {
            start.set(store::sidebar_width());
            dragging.set(true);
        })
        .on_responder_move(move |e: &ResponderEvent| {
            let w = start.get_untracked() + e.gesture.delta.x.0;
            if w >= store::SIDEBAR_MIN - 60.0 {
                store::set_sidebar_width(w, false);
            }
        })
        .on_responder_release(move |e: &ResponderEvent| {
            dragging.set(false);
            if start.get_untracked() + e.gesture.delta.x.0 < store::SIDEBAR_MIN - 60.0 {
                store::set_sidebar_width(start.get_untracked(), false);
                store::toggle_sidebar();
            } else {
                store::set_sidebar_width(store::sidebar_width(), true);
            }
        })
        .on_responder_terminate(move |_: &ResponderEvent| dragging.set(false))
        .child(
            view()
                .w(px(2.0))
                .h(px(36.0))
                .rounded(px(1.0))
                .bg(move || tint(ACCENT, if dragging.get() { 0.9 } else if hover.get() { 0.5 } else { 0.0 })),
        )
}

#[derive(Clone, Copy, PartialEq)]
enum Main {
    Home,
    New,
    Settings,
    History,
    Schedules,
    Usage,
    Workspace,
}

/// The main surface: the open screen, and the notice over its foot.
fn main(compact: bool) -> impl IntoElement {
    let which = computed(|| match store::screen() {
        Screen::Home => Main::Home,
        Screen::NewWorkspace => Main::New,
        Screen::Settings => Main::Settings,
        Screen::History => Main::History,
        Screen::Schedules => Main::Schedules,
        Screen::Usage => Main::Usage,
        Screen::Workspace(_) => Main::Workspace,
    });
    view()
        .flex_1()
        .min_w(px(0.0))
        .child(dynamic(move || {
            let which = which.get();
            Some(untracked(move || match which {
                Main::Home => home::home().into_element(),
                Main::New => new_workspace::new_workspace(compact).into_element(),
                Main::Settings => settings::settings(compact).into_element(),
                Main::History => history::history(compact).into_element(),
                Main::Schedules => schedules::schedules(compact).into_element(),
                Main::Usage => usage::usage_page(compact).into_element(),
                Main::Workspace => workspace::workspace(compact).into_element(),
            }))
        }))
        .child(notice::notice_bar())
}

