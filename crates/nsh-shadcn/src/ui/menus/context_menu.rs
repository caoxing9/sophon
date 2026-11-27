//! [`ContextMenu`] and its parts, after shadcn/ui's `context-menu.tsx`
//! (new-york-v4), on nsh-base's `ContextMenu`, `MenuItem`, `MenuSeparator`
//! and `Submenu`.
//!
//! On macOS (and every desktop) nsh-base's `ContextMenu` given children is
//! the in-app menu drawn on the core's overlays, styled with these classes:
//! the secondary button opens it at the pointer. nsh-base shows a native
//! menu only for its `items` prop (the system's `UIMenu` on iOS, a popup
//! menu on Android, `MenuButton`'s `NSMenu` on the desktops), which this
//! component never sets.
//!
//! The classes are shadcn's, with the changes `menu_common.rs` lists for
//! every menu, and these:
//!
//! - `ContextMenuTrigger` is the `trigger` slot: the app's view (shadcn's
//!   trigger has no classes of its own); `ContextMenuContent` is the root's
//!   (`content_class` is its `className`, on the view around the items
//!   inside the popup's border, as `dropdown_menu.rs` says: `w-52` is 2
//!   points wider than shadcn's). nsh-base opens it 2 points below the
//!   pointer, its start at the pointer (`offset` 2); Radix opens it 2
//!   points right of the pointer, its top at the pointer (`side="right"
//!   sideOffset={2}`): 2 points apart on each axis.
//! - `ContextMenuPortal`, `ContextMenuGroup` and `ContextMenuRadioGroup`
//!   have no classes and no part; a radio item says whether it is
//!   `checked`.
//! - `ContextMenuSub` draws its `ContextMenuSubTrigger` from its `label`
//!   and takes its `ContextMenuSubContent`'s items as children; its popup
//!   has the content's classes (`shadow-md`, not `shadow-lg`: nsh-base gives
//!   a submenu its menu's popup classes), its `content_class` the view
//!   around its items.

use nsh_base::ContextMenuEvents;
use nsh_core::prelude::*;

use super::menu_common::{self as common, Kind, MenuItemVariant};
use crate::icon::IconName;

/// The props of [`ContextMenu`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ContextMenuProps {
    /// Open (controlled): the app owns it and sets it in `onOpenChange`.
    pub open: Prop<Option<bool>>,
    /// `ContextMenuTrigger`'s `disabled`: the secondary button does nothing.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// `ContextMenuContent`'s `className` (a width, `w-52`).
    pub content_class: Prop<Option<SharedString>>,
    /// The area a secondary click opens the menu on.
    pub trigger: Prop<Option<Slot>>,
    /// The items, made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// The events of a [`ContextMenu`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ContextMenuEvent {
    /// It opened or closed: set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's ContextMenu: a menu the secondary button opens on its trigger.
#[component]
pub fn ContextMenu(props: ContextMenuProps, cx: Scope<ContextMenuEvent>) -> impl IntoElement {
    let content_class = props.content_class;
    nsh_base::context_menu()
        .open(props.open)
        .disabled(props.disabled)
        .offset(px(2.0))
        .popup_class(move || Some(common::content_classes(common::CONTENT, common::motion(Some(Placement::Right)), None)))
        .popup_starting_class(common::starting(Some(Placement::Right)))
        .popup_ending_class(common::PRESENCE)
        .on_open_change(move |e: &ContextMenuEvents| {
            if let ContextMenuEvents::OpenChange(open, _) = e {
                cx.emit(ContextMenuEvent::OpenChange(*open));
            }
        })
        .trigger(props.trigger.place(|t| t))
        .child(common::items_view(content_class, props.children))
}

// ---- parts (as dropdown_menu.rs's, by name) ----


/// The props of [`ContextMenuItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ContextMenuItemProps {
    /// The text of the item.
    pub label: Prop<Option<SharedString>>,
    /// A Lucide icon before the label.
    pub icon: Prop<Option<IconName>>,
    /// Lined up with the checkable items' labels.
    #[prop(default)]
    pub inset: Prop<bool>,
    /// `default` or `destructive`.
    #[prop(default)]
    pub variant: Prop<MenuItemVariant>,
    /// Not chosen; half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// Content after the label: a shortcut.
    pub children: Prop<Children>,
}

/// The events of a [`ContextMenuItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ContextMenuItemEvent {
    /// It was chosen (a press, Return); the menu closes.
    Select,
}

/// shadcn's ContextMenuItem.
#[component]
pub fn ContextMenuItem(props: ContextMenuItemProps, cx: Scope<ContextMenuItemEvent>) -> impl IntoElement {
    common::item(props.disabled, props.inset, props.variant, props.icon, props.label, props.children)
        .on_select(move || cx.emit(ContextMenuItemEvent::Select))
}

impl ContextMenuItemElement {
    /// `.inset(true)`.
    pub fn inset_label(self) -> Self {
        self.inset(true)
    }
    /// `.variant(MenuItemVariant::Destructive)`.
    pub fn destructive(self) -> Self {
        self.variant(MenuItemVariant::Destructive)
    }
}

/// The props of [`ContextMenuCheckboxItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ContextMenuCheckboxItemProps {
    /// The text of the item.
    pub label: Prop<Option<SharedString>>,
    /// Checked: a check before the label. The app owns it.
    #[prop(default)]
    pub checked: Prop<bool>,
    /// Not chosen; half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// Content after the label.
    pub children: Prop<Children>,
}

/// The events of a [`ContextMenuCheckboxItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ContextMenuCheckboxItemEvent {
    /// It was chosen: the value it asks for. Set `checked` to follow it.
    #[args(checked)]
    CheckedChange(bool),
}

/// shadcn's ContextMenuCheckboxItem.
#[component]
pub fn ContextMenuCheckboxItem(
    props: ContextMenuCheckboxItemProps,
    cx: Scope<ContextMenuCheckboxItemEvent>,
) -> impl IntoElement {
    let checked = props.checked;
    common::checkbox_item(Kind::Context, checked, props.disabled, props.label, props.children)
        .on_select(move || cx.emit(ContextMenuCheckboxItemEvent::CheckedChange(!checked.get_untracked())))
}

/// The props of [`ContextMenuRadioItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ContextMenuRadioItemProps {
    /// The text of the item.
    pub label: Prop<Option<SharedString>>,
    /// The group's chosen one: a dot before the label. The app owns it
    /// (shadcn's `RadioGroup` `value`).
    #[prop(default)]
    pub checked: Prop<bool>,
    /// Not chosen; half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// Content after the label.
    pub children: Prop<Children>,
}

/// The events of a [`ContextMenuRadioItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ContextMenuRadioItemEvent {
    /// It was chosen: make it the group's `checked` one.
    Select,
}

/// shadcn's ContextMenuRadioItem.
#[component]
pub fn ContextMenuRadioItem(
    props: ContextMenuRadioItemProps,
    cx: Scope<ContextMenuRadioItemEvent>,
) -> impl IntoElement {
    common::radio_item(Kind::Context, props.checked, props.disabled, props.label, props.children)
        .on_select(move || cx.emit(ContextMenuRadioItemEvent::Select))
}

/// The props of [`ContextMenuLabel`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ContextMenuLabelProps {
    /// The text.
    pub label: Prop<SharedString>,
    /// Lined up with the checkable items' labels.
    #[prop(default)]
    pub inset: Prop<bool>,
}

/// shadcn's ContextMenuLabel: a heading over items.
#[component]
pub fn ContextMenuLabel(props: ContextMenuLabelProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::label(Kind::Context, props.inset, props.label)
}

/// The props of [`ContextMenuSeparator`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ContextMenuSeparatorProps {}

/// shadcn's ContextMenuSeparator.
#[component]
pub fn ContextMenuSeparator(props: ContextMenuSeparatorProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = (props, cx);
    common::separator()
}

/// The props of [`ContextMenuShortcut`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ContextMenuShortcutProps {
    /// The keys (`⇧⌘P`).
    pub keys: Prop<SharedString>,
}

/// shadcn's ContextMenuShortcut: keys at an item's end.
#[component]
pub fn ContextMenuShortcut(props: ContextMenuShortcutProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::shortcut(props.keys)
}

/// The props of [`ContextMenuSub`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ContextMenuSubProps {
    /// `ContextMenuSubTrigger`'s text.
    pub label: Prop<SharedString>,
    /// The trigger lined up with the checkable items' labels.
    #[prop(default)]
    pub inset: Prop<bool>,
    /// It does not open.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// `ContextMenuSubContent`'s `className`, on the view around its items.
    pub content_class: Prop<Option<SharedString>>,
    /// `ContextMenuSubContent`'s items, made as it opens.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// shadcn's ContextMenuSub with its SubTrigger and SubContent.
#[component]
pub fn ContextMenuSub(props: ContextMenuSubProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::sub(
        Kind::Context,
        props.label,
        props.inset,
        props.disabled,
        props.content_class,
        props.children,
    )
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<ContextMenu>();
    registry.register::<ContextMenuItem>();
    registry.register::<ContextMenuCheckboxItem>();
    registry.register::<ContextMenuRadioItem>();
    registry.register::<ContextMenuLabel>();
    registry.register::<ContextMenuSeparator>();
    registry.register::<ContextMenuShortcut>();
    registry.register::<ContextMenuSub>();
}
