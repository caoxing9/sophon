//! [`AspectRatio`], after shadcn/ui's `aspect-ratio.tsx` (new-york-v4),
//! which is Radix's: a box as wide as its container whose height follows
//! `ratio`. Radix draws it with a padded wrapper around the app's box; NSH
//! lays out an aspect ratio natively, so it is one view, `relative w-full`
//! (the wrapper's `position: relative; width: 100%`) at `ratio`, with the
//! app's classes.

use nsh_core::prelude::*;

use super::cls;

/// Radix's wrapper: `position: relative; width: 100%`.
const ROOT: &str = "relative w-full";

/// The props of [`AspectRatio`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AspectRatioProps {
    /// Width over height: `16.0 / 9.0` (default 1, a square).
    #[prop(default = 1.0)]
    pub ratio: Prop<f32>,
    /// The content, laid out over the box (`absolute inset-0` or `size-full`).
    pub children: Prop<Children>,
}

/// shadcn's AspectRatio: content at a width-to-height ratio.
#[component]
pub fn AspectRatio(props: AspectRatioProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .app_class(cls(ROOT))
        .aspect_ratio(props.ratio)
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<AspectRatio>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::assert_known([ROOT]);
    }
}
