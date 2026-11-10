//! Sophon's screens, in three layers, as Teable's front end:
//!
//! - `kit`: the design system (controls, icons, menus, tokens);
//! - `shared`: product components both platforms use (the timeline, the
//!   composer, the agent pane, the project list, dialogs, settings
//!   sections);
//! - `desktop` and `mobile`: each platform's own shell and screens. The
//!   phone is a remote for a computer's agents, so it keeps what a remote
//!   needs and leaves the rest to the desktop.

pub(crate) mod kit;

mod desktop;
mod mobile;
mod shared;

use nsh_core::prelude::*;

pub(crate) fn shell() -> AnyElement {
    if kit::MOBILE { mobile::shell().into_element() } else { desktop::shell().into_element() }
}

/// Opens a new draft (a prompt link's): in `workspace`'s draft tab, or,
/// with none, where a new chat starts (the new-workspace screen; the
/// phone's workspace list). The draft takes the link's text as it resets.
pub(crate) fn open_new_draft(workspace: Option<&str>) {
    match (kit::MOBILE, workspace) {
        (true, Some(ws)) => mobile::open_chat(ws, Some(crate::store::Tab::Draft)),
        (true, None) => mobile::push(mobile::Route::NewChat),
        (false, Some(ws)) => {
            crate::store::open_workspace(ws);
            crate::store::open_tab(crate::store::Tab::Draft);
        }
        (false, None) => crate::store::open_new_workspace(None),
    }
}
