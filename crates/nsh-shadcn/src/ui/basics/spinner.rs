//! [`Spinner`], after shadcn/ui's `spinner.tsx` (new-york-v4): Lucide's
//! `loader-circle` (`Loader2Icon`) with its classes, word for word.
//!
//! shadcn gives it `role="status"` and `aria-label="Loading"`; NSH has no
//! status role, so screen readers hear an indeterminate progress bar named
//! "Loading" (`accessibilityLabel` renames it).

use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

use super::cls;
use crate::icon::IconName;

const SPINNER: &str = "size-4 animate-spin";

/// The props of [`Spinner`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SpinnerProps {
    /// What screen readers say (default "Loading").
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// shadcn's Spinner: a 16-point circle turning, in the text color around
/// it. Size it as shadcn does, with `.app_class("size-8")`.
#[component]
pub fn Spinner(props: SpinnerProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    svg(ImageSrc::svg_markup(&IconName::LoaderCircle.svg_markup()))
        .flex_shrink_0()
        .accessible(true)
        .role(AccessibilityRole::Progressbar)
        .label(move || {
            props
                .accessibility_label
                .get()
                .unwrap_or_else(|| SharedString::from("Loading"))
        })
        .app_class(cls(SPINNER))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Spinner>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::assert_known([SPINNER]);
    }
}
