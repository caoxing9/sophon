//! A terminal tab: the daemon's PTY drawn by `nsh/base`'s emulator. It
//! attaches while shown (a full replay, then live output) and detaches when
//! another tab opens.

use crate::i18n::t;
use nsh_base::{Terminal, TerminalApi, TerminalEvent, terminal};
use nsh_core::prelude::*;

use crate::ui::kit::*;
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
