//! [`Pagination`] and its parts, after shadcn/ui's `pagination.tsx`
//! (new-york-v4). A link is `nsh_base`'s `Link` (presses, the keyboard, the
//! link role, the current page) drawn with [`crate::Button`]'s classes, as
//! shadcn draws it with `buttonVariants`.
//!
//! shadcn's pagination is markup the app writes: its links, the current
//! one, the ellipses. So is this one; `nsh_base`'s `Pagination`, which
//! decides the pages itself, is not its model.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - The links' `buttonVariants` are [`crate::Button`]'s, with their own
//!   adaptations; `has-[>svg]:px-*` holds for Previous and Next, which
//!   always show a chevron.
//! - Previous and Next take `gap-1 px-2.5 sm:pl-2.5` / `sm:pr-2.5` after
//!   the button's classes, as shadcn's `className`; in CSS the button's
//!   `has-[>svg]:px-3` outranks a plain class (`:has()` adds to its
//!   specificity), so both keep 12 points of padding, as here, where
//!   `px-3` comes last.
//! - The chevrons are 16 points (the button's `[&_svg]:size-4`).
//! - `PaginationEllipsis`'s `sr-only` "More pages" is left out: the
//!   ellipsis is `aria-hidden` in shadcn, so no screen reader hears it.
//! - The content and its items are a list for screen readers (`<ul>`,
//!   `<li>`).

use nsh_core::prelude::*;

use super::cls;
use crate::cn::cn;
use crate::icon::{IconName, icon};
use crate::ui::button::{ButtonSize, ButtonVariant, button_classes};

const PAGINATION: &str = "mx-auto flex w-full justify-center";
const CONTENT: &str = "flex flex-row items-center gap-1";
const PREVIOUS: &str = "gap-1 px-2.5 sm:pl-2.5";
const NEXT: &str = "gap-1 px-2.5 sm:pr-2.5";
/// The button's `has-[>svg]:px-3`, which outranks `px-2.5` in CSS.
const WITH_ICON: &str = "px-3";
/// `hidden sm:block` of the words "Previous" and "Next".
const WORD: &str = "hidden sm:block";
const ICON: &str = "size-4";
const ELLIPSIS: &str = "flex size-9 items-center justify-center";

fn link_classes(active: bool, size: ButtonSize) -> SharedString {
    button_classes(
        if active {
            ButtonVariant::Outline
        } else {
            ButtonVariant::Ghost
        },
        size,
        false,
    )
}

fn step_classes(step: &str) -> SharedString {
    let button = button_classes(ButtonVariant::Ghost, ButtonSize::Default, true);
    cn([button.as_ref(), step, WITH_ICON])
}

/// The props of the parts that hold only children: [`Pagination`],
/// [`PaginationContent`] and [`PaginationItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PaginationPartProps {
    /// What the part holds.
    pub children: Prop<Children>,
}

/// shadcn's Pagination: a navigation landmark named "pagination", its
/// content centered.
#[component]
pub fn Pagination(props: PaginationPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .role(AccessibilityRole::Navigation)
        .label("pagination")
        .app_class(cls(PAGINATION))
        .child(props.children)
}

/// shadcn's PaginationContent: the items in a row.
#[component]
pub fn PaginationContent(props: PaginationPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .role(AccessibilityRole::List)
        .app_class(cls(CONTENT))
        .child(props.children)
}

/// shadcn's PaginationItem: one item.
#[component]
pub fn PaginationItem(props: PaginationPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().child(props.children)
}

/// The props of [`PaginationLink`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PaginationLinkProps {
    /// The page's number.
    pub label: Prop<Option<SharedString>>,
    /// Where it goes: a URL opens outside the app; any other path is the
    /// app's to open (`onPress`).
    pub href: Prop<Option<SharedString>>,
    /// The page the user is on: an outline button, and the current page for
    /// screen readers.
    #[prop(default)]
    pub is_active: Prop<bool>,
    /// The button size (default `icon`, a 36-point square).
    #[prop(default = ButtonSize::Icon)]
    pub size: Prop<ButtonSize>,
    /// What screen readers say, over the label.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Content after the label.
    pub children: Prop<Children>,
}

/// The events of [`PaginationLink`], [`PaginationPrevious`] and
/// [`PaginationNext`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum PaginationLinkEvent {
    /// It was pressed: a click, a tap, Return or Space.
    Press,
}

/// shadcn's PaginationLink: a page's number as a ghost button, an outline
/// one for the current page.
#[component]
pub fn PaginationLink(
    props: PaginationLinkProps,
    cx: Scope<PaginationLinkEvent>,
) -> impl IntoElement {
    let label = props.label;
    nsh_base::link()
        .href(props.href)
        .current(props.is_active)
        .accessibility_label(props.accessibility_label)
        .app_class(move || cls(link_classes(props.is_active.get(), props.size.get())))
        .on_press(move || cx.emit(PaginationLinkEvent::Press))
        .child(dynamic(move || label.get().map(|l| text(l).into_element())))
        .child(props.children)
}

/// The props of [`PaginationPrevious`] and [`PaginationNext`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PaginationStepProps {
    /// Where it goes.
    pub href: Prop<Option<SharedString>>,
}

/// shadcn's PaginationPrevious: a chevron and "Previous", a ghost button;
/// screen readers hear "Go to previous page".
#[component]
pub fn PaginationPrevious(
    props: PaginationStepProps,
    cx: Scope<PaginationLinkEvent>,
) -> impl IntoElement {
    nsh_base::link()
        .href(props.href)
        .accessibility_label("Go to previous page")
        .app_class(cls(step_classes(PREVIOUS)))
        .on_press(move || cx.emit(PaginationLinkEvent::Press))
        .child(icon(IconName::ChevronLeft).app_class(cls(ICON)))
        .child(text("Previous").app_class(cls(WORD)))
}

/// shadcn's PaginationNext: "Next" and a chevron, a ghost button; screen
/// readers hear "Go to next page".
#[component]
pub fn PaginationNext(
    props: PaginationStepProps,
    cx: Scope<PaginationLinkEvent>,
) -> impl IntoElement {
    nsh_base::link()
        .href(props.href)
        .accessibility_label("Go to next page")
        .app_class(cls(step_classes(NEXT)))
        .on_press(move || cx.emit(PaginationLinkEvent::Press))
        .child(text("Next").app_class(cls(WORD)))
        .child(icon(IconName::ChevronRight).app_class(cls(ICON)))
}

/// The props of [`PaginationEllipsis`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct PaginationEllipsisProps {}

/// shadcn's PaginationEllipsis: `…` for the pages left out, hidden from
/// screen readers.
#[component]
pub fn PaginationEllipsis(
    _props: PaginationEllipsisProps,
    _cx: Scope<NoEvent>,
) -> impl IntoElement {
    view()
        .accessibility_elements_hidden(true)
        .app_class(cls(ELLIPSIS))
        .child(icon(IconName::Ellipsis).app_class(cls(ICON)))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Pagination>();
    registry.register::<PaginationContent>();
    registry.register::<PaginationItem>();
    registry.register::<PaginationLink>();
    registry.register::<PaginationPrevious>();
    registry.register::<PaginationNext>();
    registry.register::<PaginationEllipsis>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all: Vec<SharedString> = vec![step_classes(PREVIOUS), step_classes(NEXT)];
        for active in [false, true] {
            for size in [
                ButtonSize::Icon,
                ButtonSize::Default,
                ButtonSize::Sm,
                ButtonSize::Lg,
            ] {
                all.push(link_classes(active, size));
            }
        }
        super::super::assert_known(
            all.iter()
                .map(|s| s.as_ref())
                .chain([PAGINATION, CONTENT, WORD, ICON, ELLIPSIS]),
        );
    }
}
