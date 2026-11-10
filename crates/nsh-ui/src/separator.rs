//! A one-point hairline in the theme's `border`, across or down.

use nsh_core::{element::AnyElement, prelude::*};

use crate::theme::theme;

/// A separator.
pub struct Separator {
    vertical: bool,
}

impl Separator {
    /// A line across.
    pub fn horizontal() -> Self {
        Separator { vertical: false }
    }

    /// A line down.
    pub fn vertical() -> Self {
        Separator { vertical: true }
    }
}

impl IntoElement for Separator {
    fn into_element(self) -> AnyElement {
        let line = view().bg(theme().border).flex_shrink_0();
        if self.vertical { line.w(px(1.0)).self_stretch() } else { line.h(px(1.0)).w_full() }.into_element()
    }
}
