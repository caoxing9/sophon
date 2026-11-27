//! [`Popover`] and its parts, after shadcn/ui's `popover.tsx`
//! (new-york-v4).
//!
//! Behavior is `nsh_base::Popover`'s: the `trigger` slot (shadcn's
//! `PopoverTrigger asChild`) toggles it, it is placed against the trigger
//! on `side` (the other side when only that one has room), a press outside
//! and Escape close it, the focus goes back to the trigger.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border` is `border border-border`.
//! - The animation classes (`data-[state=open]:animate-in fade-in-0
//!   zoom-in-95 data-[side=…]:slide-in-from-…-2`, and out) are
//!   tw-animate-css's: they are the popup's starting and ending looks, with
//!   a 150 ms transition between them (`super::motion`); the
//!   `origin-(--radix-popover-content-transform-origin)` is the side facing
//!   the trigger (`origin-top` below it, …).
//! - `align`, `side` and `side_offset`, which shadcn gives
//!   `PopoverContent`, are the root's props: the root places the popup.
//! - `text-base` is added to the content: a portal's content inherits the
//!   page body's text (the browser's 16 px, on Tailwind's base line height
//!   1.5), which NSH's text, 14 points by default, does not have.

use nsh_core::prelude::*;

use super::motion;

const CONTENT: &str = "z-50 w-72 rounded-md border border-border bg-popover p-4 \
    text-popover-foreground shadow-md outline-hidden text-base";
const HEADER: &str = "flex flex-col gap-1 text-sm";
const TITLE: &str = "font-medium";
const DESCRIPTION: &str = "text-muted-foreground";

/// The props of [`Popover`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PopoverProps {
    /// Open (controlled): the app owns the value and changes it in
    /// `on_open_change`. Without it the popover keeps its own state.
    pub open: Prop<Option<bool>>,
    /// Open at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// The side of the trigger it goes on (default `bottom`).
    #[prop(default)]
    pub side: Prop<Placement>,
    /// How it lines up with the trigger (default `center`).
    #[prop(default)]
    pub align: Prop<Align>,
    /// The gap between the trigger and the popover, in points (default 4).
    #[prop(default = 4.0)]
    pub side_offset: Prop<f32>,
    /// What opens it (shadcn's `PopoverTrigger asChild`): pressing it
    /// toggles the popover, which is placed against it.
    pub trigger: Prop<Option<Slot>>,
    /// The [`PopoverContent`], made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// The events of a [`Popover`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum PopoverEvent {
    /// It asks to open or close (the trigger, a press outside, Escape): set
    /// `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's Popover: rich content in a portal, next to its trigger.
#[component]
pub fn Popover(props: PopoverProps, cx: Scope<PopoverEvent>) -> impl IntoElement {
    nsh_base::popover()
        .open(props.open)
        .default_open(props.default_open)
        .placement(props.side)
        .align(props.align)
        .offset(move || Points(props.side_offset.get()))
        .popup_class(move || Some(SharedString::from(motion::transition(props.side.get()))))
        .popup_starting_class(move || Some(SharedString::from(motion::starting(props.side.get()))))
        .popup_ending_class(Some(SharedString::from(motion::ENDING)))
        .on_open_change(move |e: &nsh_base::PopoverEvent| {
            if let nsh_base::PopoverEvent::OpenChange(open, _) = e {
                cx.emit(PopoverEvent::OpenChange(*open));
            }
        })
        .trigger(props.trigger.place(|trigger| trigger))
        .child(props.children)
}

/// The props of the parts that hold others ([`PopoverContent`],
/// [`PopoverHeader`]).
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PopoverContentProps {
    /// What it holds.
    pub children: Prop<Children>,
}

/// shadcn's PopoverContent: the card the popover shows.
#[component]
pub fn PopoverContent(props: PopoverContentProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(CONTENT))).child(props.children)
}

/// shadcn's PopoverHeader: a title over a description.
#[component]
pub fn PopoverHeader(props: PopoverContentProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(HEADER))).child(props.children)
}

/// The props of [`PopoverTitle`] and [`PopoverDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PopoverTitleProps {
    /// What it says.
    pub text: Prop<SharedString>,
}

/// shadcn's PopoverTitle.
#[component]
pub fn PopoverTitle(props: PopoverTitleProps) -> impl IntoElement {
    text(props.text).app_class(Some(SharedString::from(TITLE)))
}

/// shadcn's PopoverDescription.
#[component]
pub fn PopoverDescription(props: PopoverTitleProps) -> impl IntoElement {
    text(props.text).app_class(Some(SharedString::from(DESCRIPTION)))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Popover>();
    registry.register::<PopoverContent>();
    registry.register::<PopoverHeader>();
    registry.register::<PopoverTitle>();
    registry.register::<PopoverDescription>();
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_class_is_known_to_nsh() {
        let classes = crate::cn::cn([super::CONTENT, super::HEADER, super::TITLE, super::DESCRIPTION]);
        let unknown = crate::cn::unknown_classes(&classes);
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
