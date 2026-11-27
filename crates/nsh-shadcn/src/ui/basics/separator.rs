//! [`Separator`], after shadcn/ui's `separator.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word; the
//! `data-[orientation=horizontal]:…` / `data-[orientation=vertical]:…`
//! pairs follow the `orientation` prop (NSH has no valued data selectors).
//! Radix gives a separator that is not `decorative` the `separator` role;
//! NSH has no such role, so it is left in the accessibility tree without one.

use nsh_core::prelude::*;

use super::cls;
use crate::cn::cn;

const BASE: &str = "shrink-0 bg-border";
/// `data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full`.
const HORIZONTAL: &str = "h-px w-full";
/// `data-[orientation=vertical]:h-full data-[orientation=vertical]:w-px`.
const VERTICAL: &str = "h-full w-px";

/// Which way a separator runs: shadcn's `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SeparatorOrientation {
    /// A line across, between stacked content.
    #[default]
    Horizontal,
    /// A line down, between content side by side.
    Vertical,
}

pub(crate) fn separator_classes(orientation: SeparatorOrientation) -> SharedString {
    cn([
        BASE,
        match orientation {
            SeparatorOrientation::Horizontal => HORIZONTAL,
            SeparatorOrientation::Vertical => VERTICAL,
        },
    ])
}

/// The props of [`Separator`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SeparatorProps {
    /// `horizontal` or `vertical`.
    #[prop(default)]
    pub orientation: Prop<SeparatorOrientation>,
    /// Purely visual (the default): hidden from screen readers.
    #[prop(default = true)]
    pub decorative: Prop<bool>,
}

/// shadcn's Separator: a one-point line in the border color.
#[component]
pub fn Separator(props: SeparatorProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .accessibility_elements_hidden(props.decorative)
        .app_class(move || cls(separator_classes(props.orientation.get())))
}

impl SeparatorElement {
    /// `.orientation(SeparatorOrientation::Vertical)`.
    pub fn vertical(self) -> Self {
        self.orientation(SeparatorOrientation::Vertical)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Separator>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let all = [
            separator_classes(SeparatorOrientation::Horizontal),
            separator_classes(SeparatorOrientation::Vertical),
        ];
        super::super::assert_known(all.iter().map(|s| s.as_ref()));
    }
}
