//! The terminal panel under the workspace: the daemon's PTYs drawn by
//! `nsh/base`'s emulator. One attaches while shown (a full replay, then
//! live output) and detaches when another is picked or the panel hides.

use crate::i18n::t;
use nsh_base::{Terminal, TerminalApi, TerminalEvent, terminal};
use nsh_core::prelude::*;

use crate::ui::kit::*;
use crate::ui::kit::text;
use crate::store;

pub(crate) fn terminal_pane(id: String) -> impl IntoElement {
    let term = component_ref::<Terminal>();
    store::attach_terminal(&id);
    {
        let id = id.clone();
        cleanup(move || store::detach_terminal(&id));
    }
    {
        let id = id.clone();
        effect(move || {
            store::term_tick();
            let (reset, chunks) = untracked(|| store::drain_terminal(&id));
            if let Some(api) = term.api::<TerminalApi>() {
                if reset {
                    api.reset();
                }
                if !chunks.is_empty() {
                    api.write(chunks.concat());
                }
            }
        });
    }
    let (input_id, resize_id) = (id.clone(), id.clone());
    view().flex_1().bg(SURFACE0).p(px(S3)).child(
        terminal()
            .component_ref(&term)
            .accessibility_label(t("Terminal"))
            .font_family(SharedString::from(MONO))
            .font_size(13.0)
            .line_height(18.0)
            .scrollback(5000)
            .cursor_style(|s| s.bg(tint(FG, 0.55)))
            .on_data(move |data: &SharedString| store::terminal_input(&input_id, data))
            .on_resize(move |e: &TerminalEvent| {
                if let TerminalEvent::Resize(cols, rows) = e {
                    store::terminal_resize(&resize_id, *rows as u16, *cols as u16);
                }
            })
            .text_color(FG)
            .flex_1(),
    )
}

/// The terminal panel under the workspace, as VS Code's: a strip of the
/// workspace's terminals with + and a fold button, the one picked under
/// it, and a top edge that drags its height. ⌃` shows and hides it.
pub(crate) fn terminal_panel() -> impl IntoElement {
    let active = computed(store::active_terminal);
    view()
        .flex_shrink_0()
        .h(move || Length::from(px(store::terminal_panel_height())))
        .border_t_1()
        .border_color(BORDER)
        .bg(SURFACE0)
        .child(height_handle())
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(S1))
                .h(px(34.0))
                .pl(px(S3))
                .pr(px(S2))
                .border_b_1()
                .border_color(tint(BORDER, 0.6))
                .child(each(store::terminals, |t| t.id.clone(), move |t, _| terminal_chip(t.get_untracked(), active)))
                .child(icon_btn(Icon::Plus, Sz::Sm, t("New terminal  ⌃⇧`"), store::new_terminal))
                .child(view().flex_1())
                .child(icon_btn(Icon::ChevronDown, Sz::Sm, t("Hide terminal  ⌃`"), store::toggle_terminal_panel)),
        )
        .child(dynamic(move || {
            let id = active.get()?;
            Some(untracked(move || terminal_pane(id.clone()).into_element()))
        }))
}

/// A terminal in the panel's strip: its title, and × to end it.
fn terminal_chip(info: crate::daemon::connection::TerminalInfo, active: Memo<Option<String>>) -> impl IntoElement {
    let id = info.id.clone();
    let on = {
        let id = id.clone();
        computed(move || active.with(|a| a.as_deref() == Some(id.as_str())))
    };
    let label = info.title.clone().unwrap_or_else(|| info.name.clone());
    let (open_id, kill_id) = (id.clone(), id);
    nsh_base::button()
        .on_press(move || store::show_terminal(&open_id))
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(26.0))
        .pl(px(S2))
        .pr(px(4.0))
        .max_w(px(220.0))
        .rounded(px(R_MD))
        .bg(move || if on.get() { highlight() } else { tint(SURFACE0, 0.0) })
        .hover(|s| s.bg(highlight()))
        .child(icon(Icon::SquareTerminal, 13.0).text_color(FG_MUTED))
        .child(
            text(label)
                .text_size(px(TEXT_SM))
                .text_color(move || if on.get() { FG } else { FG_MUTED })
                .number_of_lines(1)
                .flex_shrink(1.0),
        )
        .child(
            nsh_base::button()
                .on_press(move || store::kill_terminal(&kill_id))
                .accessibility_label(t("Close terminal"))
                .w(px(18.0))
                .h(px(18.0))
                .rounded(px(R_FULL))
                .items_center()
                .justify_center()
                .hover(|s| s.bg(highlight()))
                .child(icon(Icon::X, 11.0).text_color(FG_MUTED)),
        )
}

/// The panel's top edge: dragging it sets the height.
fn height_handle() -> impl IntoElement {
    let start = signal(0.0f32);
    let hover = signal(false);
    let dragging = signal(false);
    view()
        .absolute()
        .top(px(-4.0))
        .left(px(0.0))
        .right(px(0.0))
        .h(px(8.0))
        .items_center()
        .justify_center()
        .cursor(Cursor::RowResize)
        .on_mouse_enter(move |_: &MouseEvent| hover.set(true))
        .on_mouse_leave(move |_: &MouseEvent| hover.set(false))
        .claim_touches(TouchClaim::on_start().ask(|request: &ClaimRequest| request.claim()))
        .keep_touches(true)
        .on_responder_grant(move |_: &ResponderEvent| {
            start.set(store::terminal_panel_height());
            dragging.set(true);
        })
        .on_responder_move(move |e: &ResponderEvent| store::set_terminal_panel_height(start.get_untracked() - e.gesture.delta.y.0))
        .on_responder_release(move |_: &ResponderEvent| dragging.set(false))
        .on_responder_terminate(move |_: &ResponderEvent| dragging.set(false))
        .child(
            view()
                .w_full()
                .h(px(2.0))
                .bg(move || tint(ACCENT, if dragging.get() { 0.9 } else if hover.get() { 0.5 } else { 0.0 })),
        )
}
