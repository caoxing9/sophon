//! [shadcn/ui](https://ui.shadcn.com) for [NSH](https://nsh.dev): the same
//! components, with the same variants, sizes and look, written in Rust on
//! `nsh-base`'s behavior. Rust apps call them; TSX apps install the crate
//! with `nsh add nsh-shadcn` and import the same components.
//!
//! ```ignore
//! use nsh_shadcn as ui;
//!
//! #[component]
//! fn App() -> impl IntoElement {
//!     view()
//!         .child(ui::button("Save").on_press(|| save()))
//!         .child(ui::button("Delete").destructive())
//!         .child(ui::button("Cancel").outline().sm())
//! }
//! ```
//!
//! Each component draws with shadcn's own Tailwind classes (`src/ui/*.rs`
//! keeps them next to the shadcn source they come from), on the app's
//! theme: shadcn's tokens, which NSH's theme already is. An app restyles a
//! component as it would in shadcn, with `.app_class(..)`: its classes come
//! last, so they win.

mod cn;
pub mod icon;
mod ui;

pub use icon::{IconName, icon, labeled_icon};
/// The calendar's date type (`chrono`'s), so an app needs no `chrono`
/// dependency of its own: `NaiveDate::from_ymd_opt(2025, 6, 12)`.
pub use chrono::NaiveDate;
/// The value of a [`Calendar`]: one date or a range.
pub use nsh_base::Date;
pub use ui::button::{Button, ButtonElement, ButtonEvent, ButtonSize, ButtonVariant};
#[allow(unused_imports)]
pub use ui::{basics::*, chat::*, forms::*, layout::*, menus::*, overlays::*};

use nsh_core::prelude::*;

/// A button: `button("Save").on_press(save)`, `button("Delete").destructive()`.
/// A label that changes is given to the element: `.label(move || Some(..))`.
pub fn button(label: impl Into<SharedString>) -> ButtonElement {
    ui::button::button().label(Some(label.into()))
}

/// A button showing only `icon`, 36 points square; `label` is what screen
/// readers say: `icon_button(IconName::Plus, "Add")`.
pub fn icon_button(icon: IconName, label: impl Into<SharedString>) -> ButtonElement {
    ui::button::button()
        .icon(Some(icon))
        .size(ButtonSize::Icon)
        .accessibility_label(Some(label.into()))
}

/// What a TSX app imports from `nsh-shadcn`: every component.
#[native_module("nsh-shadcn")]
fn native(registry: &mut Registry) {
    ui::register(registry);
}
