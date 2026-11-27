//! [`NavigationMenu`] and its parts, after shadcn/ui's
//! `navigation-menu.tsx` (new-york-v4). nsh-base has no navigation menu:
//! each item's content is nsh-base's `Popover` at its trigger (a press
//! opens and closes it, a press outside or Escape closes it), with the
//! looks shadcn gives the content when the menu has no shared viewport
//! (`viewport={false}`, what shadcn's demo uses on the desktop: each
//! content under its own trigger).
//!
//! Changes from shadcn's classes and behavior:
//!
//! - Radix opens an item's content when the pointer rests on its trigger
//!   and moves the open content along the bar with the pointer; here a
//!   press opens it (nsh-base's `Popover`), and the shared `Viewport`
//!   (`viewport={true}`, its size animating from one content to the next)
//!   and `NavigationMenuIndicator` are not ported.
//! - `NavigationMenuItem` draws its `NavigationMenuTrigger` from its
//!   `label` and takes its `NavigationMenuContent` as children
//!   (`content_class` is the content's `className`); an item without a
//!   `label` places its children as they are (a `NavigationMenuLink`).
//! - `data-[state=open]:` on the trigger, and `group-data-[state=open]:` on
//!   its chevron, follow the popover's open state, which nsh-base reports
//!   (`onOpenChange`): nsh-base gives the trigger's states to a node around
//!   the view a Rust component fills the trigger with, so the trigger's own
//!   `focus:` and `focus-visible:` classes are not drawn either.
//! - The content's `group-data-[viewport=false]/navigation-menu:…` classes
//!   are the popover's own; `top-full mt-1.5` is its offset of 6 below the
//!   trigger, `md:absolute md:w-auto top-0 left-0 w-full` is nsh-base's
//!   placement; `**:data-[slot=navigation-menu-link]:focus:…` (no
//!   descendant selectors) is left out. The `data-[motion=…]` slides
//!   between contents are not ported; the `fade-in-0 zoom-in-95` entrance
//!   and exit are nsh-base's popup transitions (`menu_common.rs`).
//! - The root's `max-w-max flex-1` (grow, but no wider than the items)
//!   is left out, which leaves it as wide as its items: NSH has no
//!   `max-w-max`.
//! - `list-none` (a `<ul>`'s bullets) and `outline-1` are left out: NSH
//!   has no list markers, and its focus ring is the `ring` classes.
//!   `transition-[color,box-shadow]` is `transition`.
//! - `NavigationMenuLink`'s `data-[active=true]:` is the `active` prop's;
//!   `[&_svg]:…` is the app's icon's own class (`size-4
//!   text-muted-foreground`).

use nsh_base::{PopoverEvent, popover};
use nsh_core::prelude::*;

use super::menu_common;
use crate::cn::cn;
use crate::icon::{IconName, icon};

const ROOT: &str = "group/navigation-menu relative flex items-center justify-center";

const LIST: &str = "group flex flex-1 items-center justify-center gap-1";

const ITEM: &str = "relative";

/// shadcn's `navigationMenuTriggerStyle()`: a trigger's look, for a link
/// laid out among the triggers (`NavigationMenuLink`'s `class`).
pub const NAVIGATION_MENU_TRIGGER_STYLE: &str = "group inline-flex h-9 w-max items-center \
    justify-center rounded-md bg-background px-4 py-2 text-sm font-medium transition outline-none \
    hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground \
    focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none \
    disabled:opacity-50";

/// `data-[state=open]:…` of the trigger.
const TRIGGER_OPEN: &str = "bg-accent/50 text-accent-foreground hover:bg-accent focus:bg-accent";

const CHEVRON: &str = "relative top-[1px] ml-1 size-3 transition duration-300";
/// `group-data-[state=open]:rotate-180`.
const CHEVRON_OPEN: &str = "rotate-180";

const CONTENT: &str = "p-2 pr-2.5 overflow-hidden rounded-md border border-border bg-popover \
    text-popover-foreground shadow";

/// The content's `duration-200`, over the popup transition's 150 ms.
const DURATION: &str = "duration-200";

const LINK: &str = "flex flex-col gap-1 rounded-sm p-2 text-sm transition-all outline-none \
    hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground \
    focus-visible:ring-[3px] focus-visible:ring-ring/50";

/// `data-[active=true]:…` of a link.
const LINK_ACTIVE: &str = "bg-accent/50 text-accent-foreground hover:bg-accent focus:bg-accent";

/// The props of [`NavigationMenu`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct NavigationMenuProps {
    /// Its list (`NavigationMenuList`).
    pub children: Prop<Children>,
}

/// shadcn's NavigationMenu: links and menus of links, for a site's
/// navigation.
#[component]
pub fn NavigationMenu(props: NavigationMenuProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    view()
        .role(AccessibilityRole::Navigation)
        .app_class(Some(SharedString::from(ROOT)))
        .child(props.children)
}

/// The props of [`NavigationMenuList`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct NavigationMenuListProps {
    /// Its items (`NavigationMenuItem`).
    pub children: Prop<Children>,
}

/// shadcn's NavigationMenuList: the row of items.
#[component]
pub fn NavigationMenuList(props: NavigationMenuListProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    view()
        .role(AccessibilityRole::Menubar)
        .app_class(Some(SharedString::from(LIST)))
        .child(props.children)
}

/// The props of [`NavigationMenuItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct NavigationMenuItemProps {
    /// `NavigationMenuTrigger`'s text; without it the children are the
    /// item (a link).
    pub label: Prop<Option<SharedString>>,
    /// Open (controlled): the app owns it and sets it in `onOpenChange`.
    pub open: Prop<Option<bool>>,
    /// `NavigationMenuContent`'s `className`.
    pub content_class: Prop<Option<SharedString>>,
    /// `NavigationMenuContent`'s content (with a `label`), else the item.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// The events of a [`NavigationMenuItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum NavigationMenuItemEvent {
    /// Its content opened or closed.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's NavigationMenuItem with its NavigationMenuTrigger and
/// NavigationMenuContent.
#[component]
pub fn NavigationMenuItem(
    props: NavigationMenuItemProps,
    cx: Scope<NavigationMenuItemEvent>,
) -> impl IntoElement {
    let label = props.label;
    let content_class = props.content_class;
    let children = props.children;
    let open = signal(false);
    view()
        .app_class(Some(SharedString::from(ITEM)))
        .child(dynamic(move || {
            let Some(label) = label.get() else {
                return children.into_element();
            };
            popover()
                .open(props.open)
                .placement(Placement::Bottom)
                .align(Align::Start)
                .offset(px(6.0))
                .popup_class(move || {
                    Some(menu_common::content_classes(
                        CONTENT,
                        &cn([menu_common::motion(None), DURATION]),
                        content_class.get(),
                    ))
                })
                .popup_starting_class(menu_common::PRESENCE)
                .popup_ending_class(menu_common::PRESENCE)
                .on_open_change(move |e: &PopoverEvent| {
                    if let PopoverEvent::OpenChange(now, _) = e {
                        if props.open.get_untracked().is_none() {
                            open.set(*now);
                        }
                        cx.emit(NavigationMenuItemEvent::OpenChange(*now));
                    }
                })
                .trigger(
                    view()
                        .app_class(move || Some(trigger_classes(open.get() || props.open.get() == Some(true))))
                        .child(text(label))
                        .child(icon(IconName::ChevronDown).app_class(move || {
                            Some(chevron_classes(open.get() || props.open.get() == Some(true)))
                        })),
                )
                .child(children)
                .into_element()
        }))
}

/// A trigger's classes, open or not.
fn trigger_classes(open: bool) -> SharedString {
    cn([
        NAVIGATION_MENU_TRIGGER_STYLE,
        "group",
        if open { TRIGGER_OPEN } else { "" },
    ])
}

/// The trigger's chevron's classes, open or not.
fn chevron_classes(open: bool) -> SharedString {
    cn([CHEVRON, if open { CHEVRON_OPEN } else { "" }])
}

/// The props of [`NavigationMenuLink`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct NavigationMenuLinkProps {
    /// The page it is to: its look as the current one.
    #[prop(default)]
    pub active: Prop<bool>,
    /// What screen readers call it, when its content is not enough.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Its content: a title, a description.
    pub children: Prop<Children>,
}

/// The events of a [`NavigationMenuLink`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum NavigationMenuLinkEvent {
    /// It was pressed: go where it leads.
    Select,
}

/// shadcn's NavigationMenuLink.
#[component]
pub fn NavigationMenuLink(
    props: NavigationMenuLinkProps,
    cx: Scope<NavigationMenuLinkEvent>,
) -> impl IntoElement {
    view()
        .role(AccessibilityRole::Link)
        .optional_label(props.accessibility_label)
        .app_class(move || Some(cn([LINK, if props.active.get() { LINK_ACTIVE } else { "" }])))
        .on_press(move || cx.emit(NavigationMenuLinkEvent::Select))
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<NavigationMenu>();
    registry.register::<NavigationMenuList>();
    registry.register::<NavigationMenuItem>();
    registry.register::<NavigationMenuLink>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = [ROOT, LIST, ITEM, CONTENT, DURATION]
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>();
        for on in [false, true] {
            all.push(trigger_classes(on).to_string());
            all.push(chevron_classes(on).to_string());
            all.push(cn([LINK, if on { LINK_ACTIVE } else { "" }]).to_string());
        }
        menu_common::assert_known(all);
    }
}

