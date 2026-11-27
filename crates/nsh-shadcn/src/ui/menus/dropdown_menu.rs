//! [`DropdownMenu`] and its parts, after shadcn/ui's `dropdown-menu.tsx`
//! (new-york-v4), on nsh-base's `Menu`, `MenuItem`, `MenuSeparator` and
//! `Submenu`: a popup at the trigger on the desktops (a sheet on phones,
//! nsh-base's presentation), the keys, typeahead, cascading submenus.
//!
//! The classes are shadcn's, with the changes `menu_common.rs` lists for
//! every menu, and these:
//!
//! - `DropdownMenuContent` is the root's: its `className` is the
//!   `content_class` prop, on the view around the items inside the popup's
//!   border (nsh-base gives submenus their menu's popup classes, so a
//!   width on the popup would size every submenu too): `w-56` makes the
//!   content 2 points wider than shadcn's; its `side`, `align` and `sideOffset` the root's
//!   (`align` center and `sideOffset` 4 by default, as Radix's and
//!   shadcn's). `DropdownMenuPortal`, `DropdownMenuGroup` and
//!   `DropdownMenuRadioGroup` have no classes and no part here: items are
//!   the root's children in order, and a radio item says whether it is
//!   `checked`.
//! - `DropdownMenuSub` draws its `DropdownMenuSubTrigger` from its `label`
//!   and takes its `DropdownMenuSubContent`'s items as children. nsh-base
//!   gives a submenu's popup its menu's popup classes, so a submenu's
//!   popup has the content's `shadow-md` rather than `shadow-lg`, and its
//!   `content_class` (a width) goes on the view around its items, inside
//!   the popup's border.
//! - [`DropdownMenuTrigger`] is shadcn's `<DropdownMenuTrigger asChild>`
//!   around a `<Button>`: it draws Button's classes itself, as the trigger
//!   nsh-base's menu presses (a `Button` inside the trigger would take the
//!   presses itself).

use nsh_base::{MenuEvent, menu};
use nsh_core::prelude::*;

use super::menu_common::{self as common, Kind, MenuItemVariant};
use crate::icon::{IconName, icon};
use crate::ui::button::{ButtonSize, ButtonVariant, button_classes};

/// The props of [`DropdownMenu`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuProps {
    /// Open (controlled): the app owns it and sets it in `onOpenChange`.
    pub open: Prop<Option<bool>>,
    /// Open at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// The side of the trigger the content goes on (`side`).
    #[prop(default)]
    pub side: Prop<Placement>,
    /// How the content lines up with the trigger (`align`, default center).
    #[prop(default = Align::Center)]
    pub align: Prop<Align>,
    /// The gap between the trigger and the content (`sideOffset`, 4).
    #[prop(default = Points(4.0))]
    pub side_offset: Prop<Points>,
    /// `DropdownMenuContent`'s `className`: classes after the content's own
    /// (a width, `w-56`).
    pub content_class: Prop<Option<SharedString>>,
    /// What opens it: a [`DropdownMenuTrigger`], or the app's view.
    pub trigger: Prop<Option<Slot>>,
    /// The items, made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// The events of a [`DropdownMenu`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum DropdownMenuEvent {
    /// It opened or closed: set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's DropdownMenu: a menu its trigger opens.
#[component]
pub fn DropdownMenu(props: DropdownMenuProps, cx: Scope<DropdownMenuEvent>) -> impl IntoElement {
    let content_class = props.content_class;
    let side = props.side;
    menu()
        .open(props.open)
        .default_open(props.default_open)
        .placement(props.side)
        .align(props.align)
        .offset(props.side_offset)
        .popup_class(move || Some(common::content_classes(common::CONTENT, common::motion(Some(side.get())), None)))
        .popup_starting_class(move || Some(SharedString::from(common::starting(Some(side.get())))))
        .popup_ending_class(common::PRESENCE)
        .on_open_change(move |e: &MenuEvent| {
            if let MenuEvent::OpenChange(open, _) = e {
                cx.emit(DropdownMenuEvent::OpenChange(*open));
            }
        })
        .trigger(props.trigger.place(|t| t))
        .child(common::items_view(content_class, props.children))
}

/// The props of [`DropdownMenuTrigger`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuTriggerProps {
    /// The text on it.
    pub label: Prop<Option<SharedString>>,
    /// Button's `variant`.
    #[prop(default)]
    pub variant: Prop<ButtonVariant>,
    /// Button's `size`.
    #[prop(default)]
    pub size: Prop<ButtonSize>,
    /// A Lucide icon before the label.
    pub icon: Prop<Option<IconName>>,
    /// Content after the label.
    pub children: Prop<Children>,
}

/// shadcn's `<DropdownMenuTrigger asChild><Button …/></DropdownMenuTrigger>`:
/// a button's look for a menu's `trigger` (the menu takes its presses).
#[component]
pub fn DropdownMenuTrigger(props: DropdownMenuTriggerProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    let label = props.label;
    view()
        .cursor_pointer()
        .app_class(move || {
            Some(button_classes(props.variant.get(), props.size.get(), props.icon.get().is_some()))
        })
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(Some(SharedString::from(match props.size.get() {
                        ButtonSize::Xs | ButtonSize::IconXs => "size-3",
                        _ => "size-4",
                    })))
                    .into_element()
            })
        }))
        .child(dynamic(move || label.get().map(|label| text(label).into_element())))
        .child(props.children)
}

impl DropdownMenuTriggerElement {
    /// `.variant(ButtonVariant::Outline)`.
    pub fn outline(self) -> Self {
        self.variant(ButtonVariant::Outline)
    }
    /// `.variant(ButtonVariant::Ghost)`.
    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }
}

impl DropdownMenuElement {
    /// `.align(Align::Start)`.
    pub fn align_start(self) -> Self {
        self.align(Align::Start)
    }
    /// `.align(Align::End)`.
    pub fn align_end(self) -> Self {
        self.align(Align::End)
    }
}

// ---- parts (context_menu.rs and menubar.rs have the same, by name) ----

/// The props of [`DropdownMenuItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuItemProps {
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

/// The events of a [`DropdownMenuItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum DropdownMenuItemEvent {
    /// It was chosen (a press, Return); the menu closes.
    Select,
}

/// shadcn's DropdownMenuItem.
#[component]
pub fn DropdownMenuItem(props: DropdownMenuItemProps, cx: Scope<DropdownMenuItemEvent>) -> impl IntoElement {
    common::item(props.disabled, props.inset, props.variant, props.icon, props.label, props.children)
        .on_select(move || cx.emit(DropdownMenuItemEvent::Select))
}

impl DropdownMenuItemElement {
    /// `.inset(true)`.
    pub fn inset_label(self) -> Self {
        self.inset(true)
    }
    /// `.variant(MenuItemVariant::Destructive)`.
    pub fn destructive(self) -> Self {
        self.variant(MenuItemVariant::Destructive)
    }
}

/// The props of [`DropdownMenuCheckboxItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuCheckboxItemProps {
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

/// The events of a [`DropdownMenuCheckboxItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum DropdownMenuCheckboxItemEvent {
    /// It was chosen: the value it asks for. Set `checked` to follow it.
    #[args(checked)]
    CheckedChange(bool),
}

/// shadcn's DropdownMenuCheckboxItem.
#[component]
pub fn DropdownMenuCheckboxItem(
    props: DropdownMenuCheckboxItemProps,
    cx: Scope<DropdownMenuCheckboxItemEvent>,
) -> impl IntoElement {
    let checked = props.checked;
    common::checkbox_item(Kind::Dropdown, checked, props.disabled, props.label, props.children)
        .on_select(move || cx.emit(DropdownMenuCheckboxItemEvent::CheckedChange(!checked.get_untracked())))
}

/// The props of [`DropdownMenuRadioItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuRadioItemProps {
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

/// The events of a [`DropdownMenuRadioItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum DropdownMenuRadioItemEvent {
    /// It was chosen: make it the group's `checked` one.
    Select,
}

/// shadcn's DropdownMenuRadioItem.
#[component]
pub fn DropdownMenuRadioItem(
    props: DropdownMenuRadioItemProps,
    cx: Scope<DropdownMenuRadioItemEvent>,
) -> impl IntoElement {
    common::radio_item(Kind::Dropdown, props.checked, props.disabled, props.label, props.children)
        .on_select(move || cx.emit(DropdownMenuRadioItemEvent::Select))
}

/// The props of [`DropdownMenuLabel`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuLabelProps {
    /// The text.
    pub label: Prop<SharedString>,
    /// Lined up with the checkable items' labels.
    #[prop(default)]
    pub inset: Prop<bool>,
}

/// shadcn's DropdownMenuLabel: a heading over items.
#[component]
pub fn DropdownMenuLabel(props: DropdownMenuLabelProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::label(Kind::Dropdown, props.inset, props.label)
}

/// The props of [`DropdownMenuSeparator`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuSeparatorProps {}

/// shadcn's DropdownMenuSeparator.
#[component]
pub fn DropdownMenuSeparator(props: DropdownMenuSeparatorProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = (props, cx);
    common::separator()
}

/// The props of [`DropdownMenuShortcut`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuShortcutProps {
    /// The keys (`⇧⌘P`).
    pub keys: Prop<SharedString>,
}

/// shadcn's DropdownMenuShortcut: keys at an item's end.
#[component]
pub fn DropdownMenuShortcut(props: DropdownMenuShortcutProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::shortcut(props.keys)
}

/// The props of [`DropdownMenuSub`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DropdownMenuSubProps {
    /// `DropdownMenuSubTrigger`'s text.
    pub label: Prop<SharedString>,
    /// The trigger lined up with the checkable items' labels.
    #[prop(default)]
    pub inset: Prop<bool>,
    /// It does not open.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// `DropdownMenuSubContent`'s `className`, on the view around its items.
    pub content_class: Prop<Option<SharedString>>,
    /// `DropdownMenuSubContent`'s items, made as it opens.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// shadcn's DropdownMenuSub with its SubTrigger and SubContent.
#[component]
pub fn DropdownMenuSub(props: DropdownMenuSubProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::sub(
        Kind::Dropdown,
        props.label,
        props.inset,
        props.disabled,
        props.content_class,
        props.children,
    )
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<DropdownMenu>();
    registry.register::<DropdownMenuTrigger>();
    registry.register::<DropdownMenuItem>();
    registry.register::<DropdownMenuCheckboxItem>();
    registry.register::<DropdownMenuRadioItem>();
    registry.register::<DropdownMenuLabel>();
    registry.register::<DropdownMenuSeparator>();
    registry.register::<DropdownMenuShortcut>();
    registry.register::<DropdownMenuSub>();
}
