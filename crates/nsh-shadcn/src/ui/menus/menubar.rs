//! [`Menubar`], [`MenubarMenu`] and their parts, after shadcn/ui's
//! `menubar.tsx` (new-york-v4): nsh-base's `Toolbar` around nsh-base's
//! `Menu`s (the arrow keys move along the bar, and while one menu is open
//! the pointer over another trigger opens that one), with `MenuItem`,
//! `MenuSeparator` and `Submenu` inside.
//!
//! The classes are shadcn's, with the changes `menu_common.rs` lists for
//! every menu, and these:
//!
//! - `MenubarMenu` draws its `MenubarTrigger` from its `label` and takes
//!   its `MenubarContent`'s items as children (`content_class` is the
//!   content's `className`, on the view around the items, as
//!   `dropdown_menu.rs` says). The trigger's `data-[state=open]:` classes
//!   follow the menu's open state; its `focus:` classes are not drawn:
//!   nsh-base gives the trigger's keyboard focus to a node around the view
//!   a Rust component fills the trigger with.
//! - The content's `alignOffset` of -4 is `-translate-x-1` on the popup
//!   (nsh-base's `align` has no offset); its `sideOffset` is 8.
//! - `MenubarPortal`, `MenubarGroup` and `MenubarRadioGroup` have no
//!   classes and no part; a radio item says whether it is `checked`.
//! - `MenubarSub`'s popup has the content's classes (`min-w-[12rem]
//!   shadow-md`, not `min-w-[8rem] shadow-lg`: nsh-base gives a submenu its
//!   menu's popup classes).

use nsh_base::{MenuEvent, menu, toolbar};
use nsh_core::prelude::*;

use super::menu_common::{self as common, Kind, MenuItemVariant};
use crate::cn::cn;
use crate::icon::IconName;

const MENUBAR: &str = "flex h-9 items-center gap-1 rounded-md border border-border bg-background p-1 shadow-xs";

const TRIGGER: &str = "flex items-center rounded-sm px-2 py-1 text-sm font-medium outline-hidden select-none";

/// `data-[state=open]:bg-accent data-[state=open]:text-accent-foreground`.
const TRIGGER_OPEN: &str = "bg-accent text-accent-foreground";

/// `alignOffset={-4}`.
const ALIGN_OFFSET: &str = "-translate-x-1";

/// The props of [`Menubar`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarProps {
    /// Its menus (`MenubarMenu`).
    pub children: Prop<Children>,
}

/// shadcn's Menubar: a bar of menus.
#[component]
pub fn Menubar(props: MenubarProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    toolbar()
        .app_class(Some(SharedString::from(MENUBAR)))
        .child(props.children)
}

/// The props of [`MenubarMenu`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarMenuProps {
    /// `MenubarTrigger`'s text.
    pub label: Prop<SharedString>,
    /// Open (controlled): the app owns it and sets it in `onOpenChange`.
    pub open: Prop<Option<bool>>,
    /// `MenubarContent`'s `className`.
    pub content_class: Prop<Option<SharedString>>,
    /// `MenubarContent`'s items, made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// The events of a [`MenubarMenu`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum MenubarMenuEvent {
    /// It opened or closed: set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's MenubarMenu with its MenubarTrigger and MenubarContent.
#[component]
pub fn MenubarMenu(props: MenubarMenuProps, cx: Scope<MenubarMenuEvent>) -> impl IntoElement {
    let content_class = props.content_class;
    let open = signal(false);
    menu()
        .open(props.open)
        .align(Align::Start)
        .offset(px(8.0))
        .popup_class(move || {
            Some(common::content_classes(
                &cn([common::MENUBAR_CONTENT, ALIGN_OFFSET]),
                common::motion(Some(Placement::Bottom)),
                None,
            ))
        })
        .popup_starting_class(common::starting(Some(Placement::Bottom)))
        .popup_ending_class(common::PRESENCE)
        .on_open_change(move |e: &MenuEvent| {
            if let MenuEvent::OpenChange(now, _) = e {
                open.set(*now);
                cx.emit(MenubarMenuEvent::OpenChange(*now));
            }
        })
        .trigger(
            view()
                .app_class(move || Some(trigger_classes(open.get())))
                .child(text(props.label)),
        )
        .child(common::items_view(content_class, props.children))
}

/// A trigger's classes, open or not.
fn trigger_classes(open: bool) -> SharedString {
    cn([TRIGGER, if open { TRIGGER_OPEN } else { "" }])
}

// ---- parts (as dropdown_menu.rs's, by name) ----


/// The props of [`MenubarItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarItemProps {
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

/// The events of a [`MenubarItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum MenubarItemEvent {
    /// It was chosen (a press, Return); the menu closes.
    Select,
}

/// shadcn's MenubarItem.
#[component]
pub fn MenubarItem(props: MenubarItemProps, cx: Scope<MenubarItemEvent>) -> impl IntoElement {
    common::item(props.disabled, props.inset, props.variant, props.icon, props.label, props.children)
        .on_select(move || cx.emit(MenubarItemEvent::Select))
}

impl MenubarItemElement {
    /// `.inset(true)`.
    pub fn inset_label(self) -> Self {
        self.inset(true)
    }
    /// `.variant(MenuItemVariant::Destructive)`.
    pub fn destructive(self) -> Self {
        self.variant(MenuItemVariant::Destructive)
    }
}

/// The props of [`MenubarCheckboxItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarCheckboxItemProps {
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

/// The events of a [`MenubarCheckboxItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum MenubarCheckboxItemEvent {
    /// It was chosen: the value it asks for. Set `checked` to follow it.
    #[args(checked)]
    CheckedChange(bool),
}

/// shadcn's MenubarCheckboxItem.
#[component]
pub fn MenubarCheckboxItem(
    props: MenubarCheckboxItemProps,
    cx: Scope<MenubarCheckboxItemEvent>,
) -> impl IntoElement {
    let checked = props.checked;
    common::checkbox_item(Kind::Menubar, checked, props.disabled, props.label, props.children)
        .on_select(move || cx.emit(MenubarCheckboxItemEvent::CheckedChange(!checked.get_untracked())))
}

/// The props of [`MenubarRadioItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarRadioItemProps {
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

/// The events of a [`MenubarRadioItem`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum MenubarRadioItemEvent {
    /// It was chosen: make it the group's `checked` one.
    Select,
}

/// shadcn's MenubarRadioItem.
#[component]
pub fn MenubarRadioItem(
    props: MenubarRadioItemProps,
    cx: Scope<MenubarRadioItemEvent>,
) -> impl IntoElement {
    common::radio_item(Kind::Menubar, props.checked, props.disabled, props.label, props.children)
        .on_select(move || cx.emit(MenubarRadioItemEvent::Select))
}

/// The props of [`MenubarLabel`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarLabelProps {
    /// The text.
    pub label: Prop<SharedString>,
    /// Lined up with the checkable items' labels.
    #[prop(default)]
    pub inset: Prop<bool>,
}

/// shadcn's MenubarLabel: a heading over items.
#[component]
pub fn MenubarLabel(props: MenubarLabelProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::label(Kind::Menubar, props.inset, props.label)
}

/// The props of [`MenubarSeparator`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarSeparatorProps {}

/// shadcn's MenubarSeparator.
#[component]
pub fn MenubarSeparator(props: MenubarSeparatorProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = (props, cx);
    common::separator()
}

/// The props of [`MenubarShortcut`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarShortcutProps {
    /// The keys (`⇧⌘P`).
    pub keys: Prop<SharedString>,
}

/// shadcn's MenubarShortcut: keys at an item's end.
#[component]
pub fn MenubarShortcut(props: MenubarShortcutProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::shortcut(props.keys)
}

/// The props of [`MenubarSub`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MenubarSubProps {
    /// `MenubarSubTrigger`'s text.
    pub label: Prop<SharedString>,
    /// The trigger lined up with the checkable items' labels.
    #[prop(default)]
    pub inset: Prop<bool>,
    /// It does not open.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// `MenubarSubContent`'s `className`, on the view around its items.
    pub content_class: Prop<Option<SharedString>>,
    /// `MenubarSubContent`'s items, made as it opens.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// shadcn's MenubarSub with its SubTrigger and SubContent.
#[component]
pub fn MenubarSub(props: MenubarSubProps, cx: Scope<NoEvent>) -> impl IntoElement {
    let _ = cx;
    common::sub(
        Kind::Menubar,
        props.label,
        props.inset,
        props.disabled,
        props.content_class,
        props.children,
    )
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Menubar>();
    registry.register::<MenubarMenu>();
    registry.register::<MenubarItem>();
    registry.register::<MenubarCheckboxItem>();
    registry.register::<MenubarRadioItem>();
    registry.register::<MenubarLabel>();
    registry.register::<MenubarSeparator>();
    registry.register::<MenubarShortcut>();
    registry.register::<MenubarSub>();
}
