//! [`Tooltip`] and [`TooltipContent`], after shadcn/ui's `tooltip.tsx`
//! (new-york-v4).
//!
//! Behavior is `nsh_base::Tooltip`'s, the tooltip of a view around the
//! `trigger` slot (shadcn's `TooltipTrigger asChild`; a slot passes its
//! listeners and props to the app's element, not an element's tooltip,
//! which is the core's hint overlay): on the desktops it
//! shows while the pointer rests on the trigger (after `delay_duration`, 0
//! as shadcn's `TooltipProvider` sets it); leaving, a press and Escape hide
//! it. The keyboard's focus on the app's element does not show it: the core
//! follows the focus of the element the tooltip is given to, the view
//! around it.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `animate-in fade-in-0 zoom-in-95 data-[side=…]:slide-in-from-…-2`
//!   and the `animate-out` classes are tw-animate-css's: they are the
//!   tooltip's starting and ending looks, with a 150 ms transition between
//!   them (`super::motion`), scaled about the side facing the trigger.
//! - The arrow (Radix's `TooltipPrimitive.Arrow`, an SVG Radix places in a
//!   span at the content's edge) is a view of the content: its classes
//!   `size-2.5 rotate-45 rounded-[2px] bg-foreground`, and where Radix's
//!   span and `translate-y-[calc(-50%_-_2px)]` put it written out as
//!   `absolute` offsets (NSH has no `calc()` in translations); `fill-*`
//!   paints the SVG, which it is not.
//! - `side`, `align` and `side_offset`, which shadcn gives
//!   `TooltipContent`, are the root's props; Radix adds the arrow's height
//!   (5) to the offset, and so does this.

use nsh_core::prelude::*;

use super::motion;
use crate::cn::cn;

const CONTENT: &str = "relative z-50 w-fit rounded-md bg-foreground px-3 py-1.5 text-xs \
    text-balance text-background";
const ARROW: &str = "z-50 size-2.5 rotate-45 rounded-[2px] bg-foreground";

/// Where the arrow sits on the content's edge facing the trigger: the
/// middle of that edge, 7 points of its 10 inside the content (Radix's
/// span outside the edge, moved back by `translate-y-[calc(-50%_-_2px)]`).
fn arrow_place(side: Placement) -> &'static str {
    match side {
        Placement::Top => "absolute left-1/2 -ml-[5px] -bottom-[3px]",
        Placement::Bottom => "absolute left-1/2 -ml-[5px] -top-[3px]",
        Placement::Left => "absolute top-1/2 -mt-[5px] -right-[3px]",
        Placement::Right => "absolute top-1/2 -mt-[5px] -left-[3px]",
    }
}

/// The side a tooltip is on, which its content's arrow reads.
#[derive(Clone, Copy)]
struct Side(Prop<Placement>);

/// The props of [`Tooltip`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TooltipProps {
    /// Shown (controlled): the app owns the value and changes it in
    /// `on_open_change`. Without it the tooltip keeps its own state.
    pub open: Prop<Option<bool>>,
    /// Shown at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// Milliseconds the pointer rests on the trigger before it shows
    /// (default 0, shadcn's `TooltipProvider`'s).
    #[prop(default)]
    pub delay_duration: Prop<f64>,
    /// The side of the trigger it goes on (default `top`).
    #[prop(default = Placement::Top)]
    pub side: Prop<Placement>,
    /// How it lines up with the trigger (default `center`).
    #[prop(default)]
    pub align: Prop<Align>,
    /// The gap between the trigger and the tooltip's arrow, in points
    /// (default 0).
    #[prop(default)]
    pub side_offset: Prop<f32>,
    /// What it describes (shadcn's `TooltipTrigger asChild`).
    pub trigger: Prop<Option<Slot>>,
    /// The [`TooltipContent`].
    pub children: Prop<Children>,
}

/// The events of a [`Tooltip`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum TooltipEvent {
    /// It asks to show or hide: set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's Tooltip: a label shown by its trigger while the pointer rests
/// on it.
#[component]
pub fn Tooltip(props: TooltipProps, cx: Scope<TooltipEvent>) -> impl IntoElement {
    provide_context(Side(props.side));
    // The tooltip is the wrapper's, around the app's element: a slot's
    // holder hands its listeners and props to the app's element, but not
    // an element's tooltip (the core's hint overlay).
    view()
        .tooltip(
            nsh_base::tooltip()
                .open(props.open)
                .default_open(props.default_open)
                .delay(props.delay_duration)
                .placement(props.side)
                .align(props.align)
                // Radix's offset is the side offset and the arrow's height.
                .offset(move || Points(props.side_offset.get() + 5.0))
                .app_class(move || Some(SharedString::from(motion::transition(props.side.get()))))
                .starting_class(move || Some(SharedString::from(motion::starting(props.side.get()))))
                .ending_class(Some(SharedString::from(motion::ENDING)))
                .on_open_change(move |e: &nsh_base::TooltipEvent| {
                    if let nsh_base::TooltipEvent::OpenChange(open, _) = e {
                        cx.emit(TooltipEvent::OpenChange(*open));
                    }
                })
                .child(props.children),
        )
        .child(props.trigger.place(|trigger| trigger))
}

/// The props of [`TooltipContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TooltipContentProps {
    /// What it says.
    pub text: Prop<Option<SharedString>>,
    /// Other content, after the text.
    pub children: Prop<Children>,
}

/// shadcn's TooltipContent: the bubble and its arrow.
#[component]
pub fn TooltipContent(props: TooltipContentProps) -> impl IntoElement {
    let side = use_context::<Side>();
    view()
        .app_class(Some(SharedString::from(CONTENT)))
        .child(dynamic(move || props.text.get().map(|t| text(t).into_element())))
        .child(props.children)
        .child(view().app_class(move || {
            let side = side.map(|s| s.0.get()).unwrap_or(Placement::Top);
            Some(cn([ARROW, arrow_place(side)]))
        }))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Tooltip>();
    registry.register::<TooltipContent>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        for side in [Placement::Top, Placement::Bottom, Placement::Left, Placement::Right] {
            let classes = cn([CONTENT, ARROW, arrow_place(side)]);
            let unknown = crate::cn::unknown_classes(&classes);
            assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
        }
    }
}
