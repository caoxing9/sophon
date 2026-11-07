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
