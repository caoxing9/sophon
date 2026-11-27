//! [`Carousel`], [`CarouselContent`], [`CarouselItem`], [`CarouselPrevious`]
//! and [`CarouselNext`], after shadcn/ui's `carousel.tsx` (new-york-v4), on
//! `nsh_base`'s `Carousel` and its parts (the native scrolling and its
//! snapping, the keys, the previous and next buttons' disabled state).
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class or the parts differ:
//!
//! - Embla's options: `opts.loop` is `looping`; Embla's `align` is always
//!   `start` (each slide snaps to the start of the viewport); `setApi`'s
//!   `select` event is `onSelect`, with the slide's index.
//! - shadcn passes `CarouselContent`'s `className` to its inner track; here
//!   that is the `content_class` prop (the element's own classes style
//!   the `overflow-hidden` viewport around it).
//! - A vertical slide is `min-h-auto`: a browser's flex items are at
//!   least as tall as their content (`min-height: auto`), NSH's views are
//!   not; shadcn's vertical example's cards are taller than their
//!   `basis-1/2`.
//! - The previous and next buttons are `Button`'s `outline` / `icon`
//!   classes (`button.tsx`) and shadcn's; the arrow is the `icon`'s
//!   `size-4`; `sr-only` text is the buttons' accessibility label.

use nsh_core::geometry::Axis;
use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};
use crate::ui::button::{ButtonSize, ButtonVariant, button_classes};

/// The direction slides follow each other: shadcn's `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum CarouselOrientation {
    /// Side by side.
    #[default]
    Horizontal,
    /// One above the other.
    Vertical,
}

/// What a carousel's parts find of it.
#[derive(Clone, Copy)]
struct CarouselContext(CarouselOrientation);

/// shadcn's root classes.
pub(crate) const ROOT: &str = "relative";
/// shadcn's viewport around the track.
pub(crate) const VIEWPORT: &str = "overflow-hidden";

/// shadcn's track classes.
pub(crate) fn track_classes(orientation: CarouselOrientation) -> &'static str {
    match orientation {
        CarouselOrientation::Horizontal => "flex -ml-4",
        CarouselOrientation::Vertical => "flex -mt-4 flex-col",
    }
}

/// shadcn's `CarouselItem` classes.
pub(crate) fn item_classes(orientation: CarouselOrientation) -> &'static str {
    match orientation {
        CarouselOrientation::Horizontal => "min-w-0 shrink-0 grow-0 basis-full pl-4",
        CarouselOrientation::Vertical => "min-w-0 shrink-0 grow-0 basis-full pt-4 min-h-auto",
    }
}

/// shadcn's `CarouselPrevious` / `CarouselNext` classes, after the outline
/// icon button's.
pub(crate) fn control_classes(orientation: CarouselOrientation, next: bool) -> SharedString {
    let button = button_classes(ButtonVariant::Outline, ButtonSize::Icon, true);
    cn([
        &*button,
        "absolute size-8 rounded-full",
        match (orientation, next) {
            (CarouselOrientation::Horizontal, false) => "top-1/2 -left-12 -translate-y-1/2",
            (CarouselOrientation::Horizontal, true) => "top-1/2 -right-12 -translate-y-1/2",
            (CarouselOrientation::Vertical, false) => "-top-12 left-1/2 -translate-x-1/2 rotate-90",
            (CarouselOrientation::Vertical, true) => "-bottom-12 left-1/2 -translate-x-1/2 rotate-90",
        },
    ])
}

/// The props of [`Carousel`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CarouselProps {
    /// `horizontal` (the default) or `vertical`.
    #[prop(default)]
    pub orientation: Prop<CarouselOrientation>,
    /// The slides go around (Embla's `loop`).
    #[prop(default)]
    pub looping: Prop<bool>,
    /// The slide shown at first.
    #[prop(default)]
    pub default_index: Prop<u32>,
    /// What screen readers call it.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// The [`CarouselContent`] and the controls.
    pub children: Prop<Children>,
}

/// The events of [`Carousel`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum CarouselEvent {
    /// Another slide was selected (Embla's `select`): its index.
    #[args(index)]
    Select(u32),
}

/// shadcn's Carousel: slides shown one place at a time.
#[component]
pub fn Carousel(props: CarouselProps, cx: Scope<CarouselEvent>) -> impl IntoElement {
    let orientation = props.orientation.get_untracked();
    provide_context(CarouselContext(orientation));
    nsh_base::carousel()
    .axis(match orientation {
        CarouselOrientation::Horizontal => Axis::Horizontal,
        CarouselOrientation::Vertical => Axis::Vertical,
    })
    .looping(props.looping)
    .default_index(props.default_index)
    .accessibility_label(props.accessibility_label)
    .on_change(move |index: &u32| cx.emit(CarouselEvent::Select(*index)))
    .app_class(Some(SharedString::from(ROOT)))
    .child(props.children)
}

fn orientation() -> CarouselOrientation {
    use_context::<CarouselContext>().map_or(CarouselOrientation::Horizontal, |c| c.0)
}

/// The props of [`CarouselContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CarouselContentProps {
    /// The classes of the track the slides are laid out in (shadcn's
    /// `className` of `CarouselContent`), after its own.
    pub content_class: Prop<Option<SharedString>>,
    /// The [`CarouselItem`]s.
    pub children: Prop<Children>,
}

/// shadcn's CarouselContent: the viewport the slides scroll in.
#[component]
pub fn CarouselContent(props: CarouselContentProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let orientation = orientation();
    let class = props.content_class;
    view().app_class(Some(SharedString::from(VIEWPORT))).child(
        nsh_base::carousel_content()
            .app_class(move || {
                Some(cn([track_classes(orientation), class.get().as_deref().unwrap_or("")]))
            })
            .child(props.children),
    )
}

/// The props of [`CarouselItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CarouselItemProps {
    /// What screen readers call the slide.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// The slide's content.
    pub children: Prop<Children>,
}

/// shadcn's CarouselItem: one slide.
#[component]
pub fn CarouselItem(props: CarouselItemProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::carousel_item()
        .accessibility_label(props.accessibility_label)
        .app_class(Some(SharedString::from(item_classes(orientation()))))
        .child(props.children)
}

/// The props of [`CarouselPrevious`] and [`CarouselNext`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CarouselControlProps {
    /// What screen readers say (shadcn's `sr-only` text).
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// shadcn's CarouselPrevious: a round outline button before the slides;
/// disabled on the first slide.
#[component]
pub fn CarouselPrevious(props: CarouselControlProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::carousel_previous()
        .accessibility_label(move || props.accessibility_label.get().or_else(|| Some("Previous slide".into())))
        .cursor_pointer()
        .app_class(Some(control_classes(orientation(), false)))
        .child(icon(IconName::ArrowLeft).app_class(Some(SharedString::from("size-4"))))
}

/// shadcn's CarouselNext: a round outline button after the slides;
/// disabled on the last slide.
#[component]
pub fn CarouselNext(props: CarouselControlProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::carousel_next()
        .accessibility_label(move || props.accessibility_label.get().or_else(|| Some("Next slide".into())))
        .cursor_pointer()
        .app_class(Some(control_classes(orientation(), true)))
        .child(icon(IconName::ArrowRight).app_class(Some(SharedString::from("size-4"))))
}

pub(crate) fn register(registry: &mut Registry) {
    registry
        .register::<Carousel>()
        .register::<CarouselContent>()
        .register::<CarouselItem>()
        .register::<CarouselPrevious>()
        .register::<CarouselNext>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = crate::cn::unknown_classes(&cn([ROOT, VIEWPORT, "size-4"]));
        for o in [CarouselOrientation::Horizontal, CarouselOrientation::Vertical] {
            let (previous, next) = (control_classes(o, false), control_classes(o, true));
            let all = cn([track_classes(o), item_classes(o), &*previous, &*next]);
            unknown.extend(crate::cn::unknown_classes(&all));
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
