//! A keyboard key or shortcut (`⌘K`), as shadcn/ui's `Kbd`: a small muted
//! keycap in the interface's font.

use nsh_core::{
    element::{AnyElement, FontWeight},
    prelude::*,
};

use crate::theme::theme;

/// A keycap.
pub struct Kbd {
    keys: SharedString,
}

impl Kbd {
    /// A keycap showing `keys` (`"⌘K"`, `"Esc"`).
    pub fn new(keys: impl Into<SharedString>) -> Self {
        Kbd { keys: keys.into() }
    }
}

impl IntoElement for Kbd {
    fn into_element(self) -> AnyElement {
        let t = theme();
        view()
            .h(px(20.0))
            .min_w(px(20.0))
            .px(px(4.0))
            .rounded(px(4.0))
            .items_center()
            .justify_center()
            .bg(t.muted)
            .flex_shrink_0()
            .child(text(self.keys).text_size(px(12.0)).font_weight(FontWeight(500)).text_color(t.muted_foreground))
            .into_element()
    }
}
