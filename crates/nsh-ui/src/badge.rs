//! A badge: a short status or count in a small rounded box, shadcn/ui's
//! variants (Default, Secondary, Outline, Destructive) and GPUI Component's
//! tones (Success, Warning, Info), the tones as a tinted fill with the
//! tone's text.

use nsh_core::{
    element::{AdaptiveColor, AnyElement, FontWeight, Reactive},
    prelude::*,
};

use crate::theme::{self, theme};

/// What a badge looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BadgeVariant {
    /// Filled with `primary`.
    #[default]
    Default,
    /// Filled with `secondary`.
    Secondary,
    /// An edge, no fill.
    Outline,
    /// The theme's `destructive`, tinted.
    Destructive,
    /// The theme's `success`, tinted.
    Success,
    /// The theme's `warning`, tinted.
    Warning,
    /// The theme's `info`, tinted.
    Info,
    /// Quiet: `muted`, with `muted-foreground` text.
    Muted,
}

impl BadgeVariant {
    /// (fill, edge, text).
    fn colors(self) -> (AdaptiveColor, AdaptiveColor, AdaptiveColor) {
        let t = theme();
        let tinted = |c: AdaptiveColor| (c.opacity(0.12), c.opacity(0.25), c);
        match self {
            Self::Default => (t.primary, t.primary, t.primary_foreground),
            Self::Secondary => (t.secondary, t.secondary, t.secondary_foreground),
            Self::Outline => (theme::clear(), t.border, t.foreground),
            Self::Muted => (t.muted, t.muted, t.muted_foreground),
            Self::Destructive => tinted(theme::danger()),
            Self::Success => tinted(theme::success()),
            Self::Warning => tinted(theme::warning()),
            Self::Info => tinted(theme::info()),
        }
    }
}

/// A badge, built and then placed as an element.
pub struct Badge {
    label: Reactive<SharedString>,
    variant: BadgeVariant,
}

impl Badge {
    /// A badge saying `label`.
    pub fn new(label: impl IntoReactive<SharedString>) -> Self {
        Badge { label: label.into_reactive(), variant: BadgeVariant::default() }
    }

    /// Its variant.
    pub fn variant(mut self, variant: BadgeVariant) -> Self {
        self.variant = variant;
        self
    }
}

impl IntoElement for Badge {
    fn into_element(self) -> AnyElement {
        let (fill, edge, ink) = self.variant.colors();
        let label = self.label;
        view()
            .flex_row()
            .items_center()
            .h(px(20.0))
            .px(px(8.0))
            .rounded(px((theme().radius.0 - 2.0).max(4.0)))
            .border_1()
            .border_color(edge)
            .bg(fill)
            .flex_shrink_0()
            .child(text(move || label.get()).text_size(px(12.0)).font_weight(FontWeight(500)).text_color(ink).number_of_lines(1))
            .into_element()
    }
}
