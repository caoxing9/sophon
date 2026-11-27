//! [`DirectionProvider`] and [`use_direction`], after shadcn/ui's
//! `direction.tsx` (new-york-v4), Radix's `Direction` provider.
//!
//! Radix's provider only tells its components which way to lay out; the
//! page's `dir` attribute lays the content out. NSH lays a subtree out in
//! a direction of its own (a view's `direction` style), and its `rtl:` and
//! `ltr:` classes and logical properties follow it, so the provider does
//! both: it sets the direction of its content and tells the components
//! inside ([`use_direction`]).

use nsh_core::prelude::*;

/// A layout direction: shadcn's `dir` (`"ltr" | "rtl"`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum Direction {
    /// Left to right.
    #[default]
    Ltr,
    /// Right to left.
    Rtl,
}

/// The direction a provider gives.
#[derive(Clone, Copy)]
struct DirectionContext(Prop<Direction>);

/// The props of [`DirectionProvider`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DirectionProviderProps {
    /// `ltr` or `rtl` (shadcn's `direction`, or Radix's `dir`).
    #[prop(default)]
    pub direction: Prop<Direction>,
    /// The content laid out in that direction.
    pub children: Prop<Children>,
}

/// Lays its content out in `direction` and tells the components inside
/// ([`use_direction`]). The content is a column filling the provider.
#[component]
pub fn DirectionProvider(props: DirectionProviderProps) -> impl IntoElement {
    provide_context(DirectionContext(props.direction));
    view()
        .direction(move || match props.direction.get() {
            Direction::Ltr => LayoutDirection::Ltr,
            Direction::Rtl => LayoutDirection::Rtl,
        })
        .child(props.children)
}

/// The direction of the nearest [`DirectionProvider`] around the component
/// being made; without one, the app's (the user's language's). Radix's
/// `useDirection`.
pub fn use_direction() -> Direction {
    match use_context::<DirectionContext>() {
        Some(DirectionContext(direction)) => direction.get(),
        None if nsh_core::element::layout_rtl() => Direction::Rtl,
        None => Direction::Ltr,
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<DirectionProvider>();
}
