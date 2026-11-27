//! What shadcn's three menus share: `dropdown-menu.tsx`, `context-menu.tsx`
//! and `menubar.tsx` draw their items, labels, separators, shortcuts and
//! submenus with (almost) the same classes. Each family file has its own
//! components (`DropdownMenuItem`, `ContextMenuItem`, `MenubarItem`, …);
//! they draw with the class strings below, picked by [`Kind`] where the
//! three sources differ.
//!
//! Changes from shadcn's classes, for all three menus:
//!
//! - `data-[highlighted]:` is `focus:`: nsh-base's menus move the keyboard
//!   focus to the item under the pointer or the keys, so the highlighted
//!   item is the focused one.
//! - `data-[disabled]:` is `disabled:`; `data-[inset]:pl-8` and
//!   `data-[variant=destructive]:…` are appended while the `inset` and
//!   `variant` props say so.
//! - `[&_svg]:…` (an icon's size and muted color) is the `icon` prop's.
//! - The content's `p-1` is on a view around the items instead of the
//!   popup: nsh-base puts the items in a scroll view inside the popup, which
//!   would clip a separator's `-mx-1` at the popup's padding.
//! - `max-h-(--radix-…-available-height)`, `overflow-*` and
//!   `origin-(--radix-…)` are nsh-base's: the popup is held to the room on
//!   its side and its items scroll.
//! - `animate-in fade-in-0 zoom-in-95 slide-in-from-*-2` /
//!   `animate-out fade-out-0 zoom-out-95` are nsh-base's popup
//!   transitions, as the overlays family draws them: the popup starts as
//!   `opacity-0 scale-95` moved 8 points toward its trigger (the side it
//!   is asked to go on; nsh-base may flip it where there is no room) and
//!   ends as `opacity-0 scale-95`, with `transition duration-150` on CSS's
//!   `ease` (tw-animate's) scaling about the side of the trigger
//!   (`origin-*`, for Radix's `origin-(--radix-…-transform-origin)`). A
//!   submenu's popup takes its menu's looks (nsh-base), so it comes in
//!   from its menu's side rather than from the left.
//! - `data-[state=open]:` on a submenu's trigger follows the submenu's
//!   open state, which nsh-base reports (`onOpenChange`): nsh-base gives
//!   its `aria-expanded` and keyboard focus to a node around the trigger a
//!   Rust component fills, so `focus:` cannot reach the trigger's own view.

use nsh_base::{MenuItemElement, MenuSeparatorElement, menu_item, menu_separator};
use nsh_core::element::{ImageSrc, Svg};
use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};

/// Which of shadcn's menus a part belongs to, where their classes differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Dropdown,
    Context,
    Menubar,
}

/// How a menu item looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum MenuItemVariant {
    /// The text color.
    #[default]
    Default,
    /// A destructive action, in the destructive color.
    Destructive,
}

/// The popup of every menu's content (`DropdownMenuContent`,
/// `ContextMenuContent`), without its `p-1` (on the items' view) and with
/// nsh-base's transition in place of the animations.
pub(crate) const CONTENT: &str = "z-50 min-w-[8rem] rounded-md border border-border bg-popover \
    text-popover-foreground shadow-md";

/// `MenubarContent`'s popup: 12rem wide at least.
pub(crate) const MENUBAR_CONTENT: &str = "z-50 min-w-[12rem] rounded-md border border-border \
    bg-popover text-popover-foreground shadow-md";

/// The look a popup goes out to (tw-animate's `fade-out-0 zoom-out-95`),
/// and comes in from where it does not slide.
pub(crate) const PRESENCE: &str = "opacity-0 scale-95";

/// The transition between a popup's looks on `side` of its trigger (or
/// at it, `None`): tw-animate's 150 ms on CSS's `ease`.
pub(crate) fn motion(side: Option<Placement>) -> &'static str {
    match side {
        Some(Placement::Bottom) => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)] origin-top",
        Some(Placement::Top) => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)] origin-bottom",
        Some(Placement::Left) => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)] origin-right",
        Some(Placement::Right) => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)] origin-left",
        _ => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)]",
    }
}

/// The look a popup on `side` comes in from: tw-animate's `fade-in-0
/// zoom-in-95 slide-in-from-*-2`.
pub(crate) fn starting(side: Option<Placement>) -> &'static str {
    match side {
        Some(Placement::Bottom) => "opacity-0 scale-95 -translate-y-2",
        Some(Placement::Top) => "opacity-0 scale-95 translate-y-2",
        Some(Placement::Left) => "opacity-0 scale-95 translate-x-2",
        Some(Placement::Right) => "opacity-0 scale-95 -translate-x-2",
        _ => PRESENCE,
    }
}

/// The view around the items: the content's `p-1`.
pub(crate) const ITEMS: &str = "p-1";

const ITEM: &str = "relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm \
    outline-hidden select-none focus:bg-accent focus:text-accent-foreground \
    disabled:pointer-events-none disabled:opacity-50";

const ITEM_DESTRUCTIVE: &str = "text-destructive focus:bg-destructive/10 focus:text-destructive \
    dark:focus:bg-destructive/20";

const INSET: &str = "pl-8";

const CHECKABLE: &str = "relative flex cursor-default items-center gap-2 rounded-sm py-1.5 pr-2 \
    pl-8 text-sm outline-hidden select-none focus:bg-accent focus:text-accent-foreground \
    disabled:pointer-events-none disabled:opacity-50";

const MENUBAR_CHECKABLE: &str = "relative flex cursor-default items-center gap-2 rounded-xs py-1.5 \
    pr-2 pl-8 text-sm outline-hidden select-none focus:bg-accent focus:text-accent-foreground \
    disabled:pointer-events-none disabled:opacity-50";

/// The span around a checkable item's mark; shown while checked
/// (Radix's `ItemIndicator` renders nothing otherwise).
const INDICATOR: &str = "pointer-events-none absolute left-2 flex size-3.5 items-center \
    justify-center opacity-0 checked:opacity-100";

const LABEL: &str = "px-2 py-1.5 text-sm font-medium";
const CONTEXT_LABEL: &str = "px-2 py-1.5 text-sm font-medium text-foreground";

const SEPARATOR: &str = "-mx-1 my-1 h-px bg-border";

const SHORTCUT: &str = "ml-auto text-xs tracking-widest text-muted-foreground";

const SUB_TRIGGER: &str = "flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm \
    outline-hidden select-none focus:bg-accent focus:text-accent-foreground";
const CONTEXT_SUB_TRIGGER: &str = "flex cursor-default items-center rounded-sm px-2 py-1.5 text-sm \
    outline-hidden select-none focus:bg-accent focus:text-accent-foreground";
const MENUBAR_SUB_TRIGGER: &str = "flex cursor-default items-center rounded-sm px-2 py-1.5 text-sm \
    outline-none select-none focus:bg-accent focus:text-accent-foreground";

/// `data-[state=open]:bg-accent data-[state=open]:text-accent-foreground`.
const SUB_TRIGGER_OPEN: &str = "bg-accent text-accent-foreground";

/// `[&_svg:not([class*='size-'])]:size-4 [&_svg:not([class*='text-'])]:text-muted-foreground`.
const ICON: &str = "size-4 text-muted-foreground";
/// `data-[variant=destructive]:*:[svg]:text-destructive!`.
const ICON_DESTRUCTIVE: &str = "size-4 text-destructive";

/// An item's classes.
pub(crate) fn item_classes(inset: bool, variant: MenuItemVariant) -> SharedString {
    cn([
        ITEM,
        if inset { INSET } else { "" },
        if variant == MenuItemVariant::Destructive {
            ITEM_DESTRUCTIVE
        } else {
            ""
        },
    ])
}

/// A checkbox or radio item's classes.
pub(crate) fn checkable_classes(kind: Kind) -> &'static str {
    match kind {
        Kind::Menubar => MENUBAR_CHECKABLE,
        Kind::Dropdown | Kind::Context => CHECKABLE,
    }
}

/// A label's classes.
pub(crate) fn label_classes(kind: Kind, inset: bool) -> SharedString {
    let base = match kind {
        Kind::Context => CONTEXT_LABEL,
        Kind::Dropdown | Kind::Menubar => LABEL,
    };
    cn([base, if inset { INSET } else { "" }])
}

/// A submenu trigger's classes, open or not.
pub(crate) fn sub_trigger_classes(kind: Kind, inset: bool, open: bool) -> SharedString {
    let base = match kind {
        Kind::Dropdown => SUB_TRIGGER,
        Kind::Context => CONTEXT_SUB_TRIGGER,
        Kind::Menubar => MENUBAR_SUB_TRIGGER,
    };
    cn([
        base,
        if inset { INSET } else { "" },
        if open { SUB_TRIGGER_OPEN } else { "" },
    ])
}

/// The chevron at a submenu trigger's end: `ml-auto size-4`, muted in the
/// dropdown and context menus (their `[&_svg]` rule), the text's color in
/// the menubar's (`ml-auto h-4 w-4`, no such rule).
pub(crate) fn sub_chevron_classes(kind: Kind) -> &'static str {
    match kind {
        Kind::Menubar => "ml-auto h-4 w-4",
        Kind::Dropdown | Kind::Context => "ml-auto size-4 text-muted-foreground",
    }
}

/// An item: icon, label, then the app's content (a shortcut).
pub(crate) fn item(
    disabled: Prop<bool>,
    inset: Prop<bool>,
    variant: Prop<MenuItemVariant>,
    icon_name: Prop<Option<IconName>>,
    label: Prop<Option<SharedString>>,
    children: Prop<Children>,
) -> MenuItemElement {
    menu_item()
        .disabled(disabled)
        .app_class(move || Some(item_classes(inset.get(), variant.get())))
        .child(dynamic(move || {
            icon_name.get().map(|name| {
                let class = match variant.get() {
                    MenuItemVariant::Destructive => ICON_DESTRUCTIVE,
                    _ => ICON,
                };
                icon(name)
                    .app_class(Some(SharedString::from(class)))
                    .into_element()
            })
        }))
        .child(dynamic(move || label.get().map(|label| text(label).into_element())))
        .child(children)
}

/// A checkbox item: a check before the label while `checked`.
pub(crate) fn checkbox_item(
    kind: Kind,
    checked: Prop<bool>,
    disabled: Prop<bool>,
    label: Prop<Option<SharedString>>,
    children: Prop<Children>,
) -> MenuItemElement {
    menu_item()
        .checked(move || Some(checked.get()))
        .disabled(disabled)
        .indicator_class(INDICATOR)
        .indicator(icon(IconName::Check).app_class(Some(SharedString::from("size-4"))))
        .app_class(Some(SharedString::from(checkable_classes(kind))))
        .child(dynamic(move || label.get().map(|label| text(label).into_element())))
        .child(children)
}

/// A radio item: a dot before the label while `checked`.
pub(crate) fn radio_item(
    kind: Kind,
    checked: Prop<bool>,
    disabled: Prop<bool>,
    label: Prop<Option<SharedString>>,
    children: Prop<Children>,
) -> MenuItemElement {
    menu_item()
        .role(Some(AccessibilityRole::MenuItemRadio))
        .checked(move || Some(checked.get()))
        .disabled(disabled)
        .indicator_class(INDICATOR)
        .indicator(dot().app_class(Some(SharedString::from("size-2"))))
        .app_class(Some(SharedString::from(checkable_classes(kind))))
        .child(dynamic(move || label.get().map(|label| text(label).into_element())))
        .child(children)
}

/// Lucide's `circle` filled with the text color: shadcn's
/// `<CircleIcon className="size-2 fill-current" />`.
pub(crate) fn dot() -> Svg {
    let markup = IconName::Circle
        .svg_markup()
        .replacen(r#"fill="none""#, r#"fill="currentColor""#, 1);
    svg(ImageSrc::svg_markup(&markup))
        .size(px(8.0))
        .flex_shrink_0()
        .accessibility_elements_hidden(true)
}

/// A label: a heading over a group of items, not an item.
pub(crate) fn label(kind: Kind, inset: Prop<bool>, label: Prop<SharedString>) -> Text {
    text(label).app_class(move || Some(label_classes(kind, inset.get())))
}

/// A separator between groups of items.
pub(crate) fn separator() -> MenuSeparatorElement {
    menu_separator().app_class(Some(SharedString::from(SEPARATOR)))
}

/// A shortcut's text at an item's end.
pub(crate) fn shortcut(keys: Prop<SharedString>) -> Text {
    text(keys).app_class(Some(SharedString::from(SHORTCUT)))
}

/// A submenu: its trigger (the label and a chevron), then its items in
/// a view with the content's `p-1` and the app's `content_class`.
pub(crate) fn sub(
    kind: Kind,
    label: Prop<SharedString>,
    inset: Prop<bool>,
    disabled: Prop<bool>,
    content_class: Prop<Option<SharedString>>,
    children: Prop<Children>,
) -> AnyElement {
    let open = signal(false);
    nsh_base::submenu()
        .disabled(disabled)
        .on_open_change(move |e: &nsh_base::SubmenuEvent| {
            let nsh_base::SubmenuEvent::OpenChange(now, _) = e;
            open.set(*now);
        })
        .trigger(
            view()
                .app_class(move || Some(sub_trigger_classes(kind, inset.get(), open.get())))
                .child(text(label))
                .child(
                    icon(IconName::ChevronRight)
                        .app_class(Some(SharedString::from(sub_chevron_classes(kind)))),
                ),
        )
        .child(items_view(content_class, children))
        .into_element()
}

/// The view around a content's items: its `p-1` and the app's classes
/// (a submenu's `w-44`).
pub(crate) fn items_view(class: Prop<Option<SharedString>>, children: Prop<Children>) -> View {
    view()
        .app_class(move || Some(cn([ITEMS, class.get().as_deref().unwrap_or("")])))
        .child(children)
}

/// The popup's classes, its transition's, then the app's (`w-56`).
pub(crate) fn content_classes(base: &str, motion: &str, class: Option<SharedString>) -> SharedString {
    cn([base, motion, class.as_deref().unwrap_or("")])
}

/// Every class string the parts above can draw with, for the tests.
#[cfg(test)]
pub(crate) fn all_classes() -> Vec<String> {
    let mut all = vec![
        CONTENT.to_owned(),
        MENUBAR_CONTENT.to_owned(),
        PRESENCE.to_owned(),
        ITEMS.to_owned(),
        INDICATOR.to_owned(),
        SEPARATOR.to_owned(),
        SHORTCUT.to_owned(),
        ICON.to_owned(),
        ICON_DESTRUCTIVE.to_owned(),
        "size-2".to_owned(),
    ];
    for side in [None, Some(Placement::Top), Some(Placement::Bottom), Some(Placement::Left), Some(Placement::Right)] {
        all.push(motion(side).to_owned());
        all.push(starting(side).to_owned());
    }
    for kind in [Kind::Dropdown, Kind::Context, Kind::Menubar] {
        all.push(checkable_classes(kind).to_owned());
        all.push(sub_chevron_classes(kind).to_owned());
        for inset in [false, true] {
            all.push(label_classes(kind, inset).to_string());
            for open in [false, true] {
                all.push(sub_trigger_classes(kind, inset, open).to_string());
            }
        }
    }
    for inset in [false, true] {
        for variant in [MenuItemVariant::Default, MenuItemVariant::Destructive] {
            all.push(item_classes(inset, variant).to_string());
        }
    }
    all
}

/// Asserts every class of `classes` is one NSH knows.
#[cfg(test)]
pub(crate) fn assert_known(classes: impl IntoIterator<Item = String>) {
    let mut unknown = Vec::new();
    for class in classes {
        for class in crate::cn::unknown_classes(&class) {
            if !unknown.contains(&class) {
                unknown.push(class);
            }
        }
    }
    assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_class_is_known_to_nsh() {
        super::assert_known(super::all_classes());
    }
}
