//! [`Breadcrumb`] and its parts, after shadcn/ui's `breadcrumb.tsx`
//! (new-york-v4). A link is `nsh_base`'s `Link` (presses, the keyboard, the
//! link role).
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `BreadcrumbList`'s `break-words` is left out: NSH breaks a word that
//!   does not fit its line on its own.
//! - `BreadcrumbSeparator`'s `[&>svg]:size-3.5` sizes the separator's icon
//!   (the `icon` prop's: `chevron-right` by default, as shadcn's children
//!   `?? <ChevronRight />`).
//! - `BreadcrumbEllipsis`'s `sr-only` "More" is left out: the ellipsis is
//!   `aria-hidden` in shadcn, so no screen reader hears it.
//! - The list and its items are a list for screen readers (`<ol>`, `<li>`).

use nsh_core::prelude::*;
use nsh_core::{AccessibilityCurrent, AccessibilityState};

use super::cls;
use crate::icon::{IconName, icon};

const LIST: &str = "flex flex-wrap items-center gap-1.5 text-sm text-muted-foreground sm:gap-2.5";
const ITEM: &str = "inline-flex items-center gap-1.5";
const LINK: &str = "transition-colors hover:text-foreground";
const PAGE: &str = "font-normal text-foreground";
/// `[&>svg]:size-3.5`.
const SEPARATOR_ICON: &str = "size-3.5";
const ELLIPSIS: &str = "flex size-9 items-center justify-center";
const ELLIPSIS_ICON: &str = "size-4";

/// The props of the parts that hold only children: [`Breadcrumb`],
/// [`BreadcrumbList`] and [`BreadcrumbItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BreadcrumbPartProps {
    /// What the part holds.
    pub children: Prop<Children>,
}

/// shadcn's Breadcrumb: a navigation landmark named "breadcrumb".
#[component]
pub fn Breadcrumb(props: BreadcrumbPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .role(AccessibilityRole::Navigation)
        .label("breadcrumb")
        .child(props.children)
}

/// shadcn's BreadcrumbList: the steps and separators, in a row that wraps.
#[component]
pub fn BreadcrumbList(props: BreadcrumbPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .role(AccessibilityRole::List)
        .app_class(cls(LIST))
        .child(props.children)
}

/// shadcn's BreadcrumbItem: a step, a link or the page.
#[component]
pub fn BreadcrumbItem(props: BreadcrumbPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(ITEM)).child(props.children)
}

/// The props of [`BreadcrumbLink`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BreadcrumbLinkProps {
    /// The link's text.
    pub label: Prop<Option<SharedString>>,
    /// Where it goes: a URL opens outside the app; any other path is the
    /// app's to open (`onPress`).
    pub href: Prop<Option<SharedString>>,
    /// Content after the text.
    pub children: Prop<Children>,
}

/// The events of a [`BreadcrumbLink`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum BreadcrumbLinkEvent {
    /// It was pressed: a click, a tap, Return or Space.
    Press,
}

/// shadcn's BreadcrumbLink: a page up the trail, muted until hovered.
#[component]
pub fn BreadcrumbLink(
    props: BreadcrumbLinkProps,
    cx: Scope<BreadcrumbLinkEvent>,
) -> impl IntoElement {
    let label = props.label;
    nsh_base::link()
        .href(props.href)
        .app_class(cls(LINK))
        .on_press(move || cx.emit(BreadcrumbLinkEvent::Press))
        .child(dynamic(move || label.get().map(|l| text(l).into_element())))
        .child(props.children)
}

/// The props of [`BreadcrumbPage`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BreadcrumbPageProps {
    /// The page's name.
    pub label: Prop<Option<SharedString>>,
    /// Content after the name.
    pub children: Prop<Children>,
}

/// shadcn's BreadcrumbPage: the page the user is on, a link that goes
/// nowhere (`role="link" aria-disabled aria-current="page"`).
#[component]
pub fn BreadcrumbPage(props: BreadcrumbPageProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    text(move || props.label.get().unwrap_or_default())
        .role(AccessibilityRole::Link)
        .accessibility_state(AccessibilityState {
            disabled: true,
            current: Some(AccessibilityCurrent::Page),
            ..AccessibilityState::default()
        })
        .app_class(cls(PAGE))
        .child(props.children)
}

/// The props of [`BreadcrumbSeparator`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BreadcrumbSeparatorProps {
    /// The Lucide icon between the steps (default `chevron-right`).
    pub icon: Prop<Option<IconName>>,
    /// Other content in place of the icon.
    pub children: Prop<Children>,
}

/// shadcn's BreadcrumbSeparator: a 14-point chevron between two steps,
/// hidden from screen readers.
#[component]
pub fn BreadcrumbSeparator(
    props: BreadcrumbSeparatorProps,
    _cx: Scope<NoEvent>,
) -> impl IntoElement {
    let custom = !props.children.get_untracked().is_empty();
    view()
        .accessibility_elements_hidden(true)
        .child(dynamic(move || {
            let name = props
                .icon
                .get()
                .or((!custom).then_some(IconName::ChevronRight))?;
            Some(icon(name).app_class(cls(SEPARATOR_ICON)).into_element())
        }))
        .child(props.children)
}

impl BreadcrumbSeparatorElement {
    /// `.icon(Some(name))`: another icon than the chevron.
    pub fn with_icon(self, name: IconName) -> Self {
        self.icon(Some(name))
    }
}

/// The props of [`BreadcrumbEllipsis`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BreadcrumbEllipsisProps {}

/// shadcn's BreadcrumbEllipsis: `…` for the steps left out, a 36-point
/// square (the trigger of a menu of them, in shadcn's example).
#[component]
pub fn BreadcrumbEllipsis(
    _props: BreadcrumbEllipsisProps,
    _cx: Scope<NoEvent>,
) -> impl IntoElement {
    view()
        .accessibility_elements_hidden(true)
        .app_class(cls(ELLIPSIS))
        .child(icon(IconName::Ellipsis).app_class(cls(ELLIPSIS_ICON)))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Breadcrumb>();
    registry.register::<BreadcrumbList>();
    registry.register::<BreadcrumbItem>();
    registry.register::<BreadcrumbLink>();
    registry.register::<BreadcrumbPage>();
    registry.register::<BreadcrumbSeparator>();
    registry.register::<BreadcrumbEllipsis>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::assert_known([
            LIST,
            ITEM,
            LINK,
            PAGE,
            SEPARATOR_ICON,
            ELLIPSIS,
            ELLIPSIS_ICON,
        ]);
    }
}
