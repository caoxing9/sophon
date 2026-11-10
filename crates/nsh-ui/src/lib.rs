//! A shadcn/ui-style component set for Native Shell, in Rust: the designs of
//! GPUI Component (`longbridge/gpui-kit`: variants, sizes, colors and
//! states) on `nsh-base`'s unstyled behavior, drawn from the app's theme
//! (shadcn's tokens, `nsh.json` `"theme"`). No JavaScript: Rust apps place
//! these as any element.
//!
//! ```ignore
//! use nsh_ui::prelude::*;
//! Button::new().label("Save").primary().on_press(save)
//! ```

pub mod badge;
pub mod button;
pub mod kbd;
pub mod separator;
pub mod size;
pub mod theme;

/// What an app uses.
pub mod prelude {
    pub use crate::{
        badge::{Badge, BadgeVariant},
        button::{Button, ButtonVariant},
        kbd::Kbd,
        separator::Separator,
        size::Size,
    };
}
