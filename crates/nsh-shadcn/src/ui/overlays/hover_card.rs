//! [`HoverCard`] and [`HoverCardContent`], after shadcn/ui's
//! `hover-card.tsx` (new-york-v4).
//!
//! Behavior is `nsh_base::HoverCard`'s: the pointer resting on the
//! `trigger` slot (shadcn's `HoverCardTrigger asChild`) for `open_delay`
//! opens the card next to it, which stays while the pointer moves between
//! the two and closes `close_delay` after it left both (Radix's defaults:
//! 700 and 300 ms); Escape and a press outside close it; on touch screens a
//! press on the trigger toggles it.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border` is `border border-border`.
//! - The animation classes are tw-animate-css's: they are the popup's
//!   starting and ending looks, with a 150 ms transition (`super::motion`),
//!   scaled about the side facing the trigger.
//! - `align`, `side` and `side_offset`, which shadcn gives
//!   `HoverCardContent`, are the root's props: the root places the card.
//! - `text-base` is added to the content: a portal's content inherits the
//!   page body's text (the browser's 16 px, on Tailwind's base line height
//!   1.5), which NSH's text, 14 points by default, does not have.

use nsh_core::prelude::*;

use super::motion;

const CONTENT: &str = "z-50 w-64 rounded-md border border-border bg-popover p-4 \
    text-popover-foreground shadow-md outline-hidden text-base";

/// The props of [`HoverCard`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct HoverCardProps {
    /// Open (controlled): the app owns the value and changes it in
    /// `on_open_change`. Without it the card keeps its own state.
    pub open: Prop<Option<bool>>,
    /// Open at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// Milliseconds the pointer rests on the trigger before the card opens
    /// (default 700).
    #[prop(default = 700.0)]
    pub open_delay: Prop<f64>,
    /// Milliseconds the pointer is away before it closes (default 300).
    #[prop(default = 300.0)]
    pub close_delay: Prop<f64>,
    /// The side of the trigger it goes on (default `bottom`).
    #[prop(default)]
    pub side: Prop<Placement>,
    /// How it lines up with the trigger (default `center`).
    #[prop(default)]
    pub align: Prop<Align>,
    /// The gap between the trigger and the card, in points (default 4).
    #[prop(default = 4.0)]
    pub side_offset: Prop<f32>,
    /// What it previews (shadcn's `HoverCardTrigger asChild`).
    pub trigger: Prop<Option<Slot>>,
    /// The [`HoverCardContent`], made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// The events of a [`HoverCard`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum HoverCardEvent {
    /// It asks to open or close: set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's HoverCard: a preview of what its trigger points at, shown
/// while the pointer rests on it.
#[component]
pub fn HoverCard(props: HoverCardProps, cx: Scope<HoverCardEvent>) -> impl IntoElement {
    nsh_base::hover_card()
        .open(props.open)
        .default_open(props.default_open)
        .open_delay(props.open_delay)
        .close_delay(props.close_delay)
        .placement(props.side)
        .align(props.align)
        .offset(move || Points(props.side_offset.get()))
        .popup_class(move || Some(SharedString::from(motion::transition(props.side.get()))))
        .popup_starting_class(move || Some(SharedString::from(motion::starting(props.side.get()))))
        .popup_ending_class(Some(SharedString::from(motion::ENDING)))
        .on_open_change(move |e: &nsh_base::HoverCardEvent| {
            if let nsh_base::HoverCardEvent::OpenChange(open, _) = e {
                cx.emit(HoverCardEvent::OpenChange(*open));
            }
        })
        .trigger(props.trigger.place(|trigger| trigger))
        .child(props.children)
}

/// The props of [`HoverCardContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct HoverCardContentProps {
    /// What it shows.
    pub children: Prop<Children>,
}

/// shadcn's HoverCardContent: the card.
#[component]
pub fn HoverCardContent(props: HoverCardContentProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(CONTENT))).child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<HoverCard>();
    registry.register::<HoverCardContent>();
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_class_is_known_to_nsh() {
        let unknown = crate::cn::unknown_classes(super::CONTENT);
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
