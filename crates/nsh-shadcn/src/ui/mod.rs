//! The components, one `#[component]` each, by family. TSX imports them from
//! `nsh-shadcn`; Rust builds them with the constructors each family exports
//! (re-exported at the crate root).

pub mod button;

pub mod basics;
pub mod chat;
pub mod forms;
pub mod layout;
pub mod menus;
pub mod overlays;

use nsh_core::prelude::*;

/// Registers every component TSX imports from `nsh-shadcn`.
pub(crate) fn register(registry: &mut Registry) {
    button::register(registry);
    basics::register(registry);
    forms::register(registry);
    overlays::register(registry);
    menus::register(registry);
    layout::register(registry);
    chat::register(registry);
}
