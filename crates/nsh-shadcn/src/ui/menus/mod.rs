//! The menus family of components. Each component is one file in this
//! directory; this module declares them, re-exports their public API (types
//! and constructors, which the crate root re-exports in turn) and registers
//! them for TSX.

use nsh_core::prelude::*;

mod menu_common;

pub mod combobox;
pub mod command;
pub mod context_menu;
pub mod dropdown_menu;
pub mod menubar;
pub mod navigation_menu;
pub mod select;

pub use combobox::{Combobox, ComboboxElement, ComboboxEvents, ComboboxProps};
pub use command::{Command, CommandElement, CommandEvents, CommandItem, CommandProps};
pub use context_menu::*;
pub use dropdown_menu::*;
pub use menubar::*;
pub use navigation_menu::*;
pub use nsh_base::{SelectItem, SelectValue};
pub use select::{Select, SelectElement, SelectEvents, SelectProps, SelectSize};
pub use menu_common::MenuItemVariant;

/// A dropdown menu: `dropdown_menu().trigger(dropdown_menu_trigger("Open").outline()).child(items)`.
pub fn dropdown_menu() -> DropdownMenuElement {
    dropdown_menu::dropdown_menu()
}

/// A dropdown menu's trigger with a button's look: `dropdown_menu_trigger("Open").outline()`.
pub fn dropdown_menu_trigger(label: impl Into<SharedString>) -> DropdownMenuTriggerElement {
    dropdown_menu::dropdown_menu_trigger().label(Some(label.into()))
}

/// A dropdown menu's item: `dropdown_menu_item("Profile").child(dropdown_menu_shortcut("⇧⌘P"))`.
pub fn dropdown_menu_item(label: impl Into<SharedString>) -> DropdownMenuItemElement {
    dropdown_menu::dropdown_menu_item().label(Some(label.into()))
}

/// A dropdown menu's checkbox item.
pub fn dropdown_menu_checkbox_item(label: impl Into<SharedString>) -> DropdownMenuCheckboxItemElement {
    dropdown_menu::dropdown_menu_checkbox_item().label(Some(label.into()))
}

/// A dropdown menu's radio item.
pub fn dropdown_menu_radio_item(label: impl Into<SharedString>) -> DropdownMenuRadioItemElement {
    dropdown_menu::dropdown_menu_radio_item().label(Some(label.into()))
}

/// A dropdown menu's label.
pub fn dropdown_menu_label(label: impl Into<SharedString>) -> DropdownMenuLabelElement {
    dropdown_menu::dropdown_menu_label().label(label.into())
}

/// A dropdown menu's separator.
pub fn dropdown_menu_separator() -> DropdownMenuSeparatorElement {
    dropdown_menu::dropdown_menu_separator()
}

/// A dropdown menu item's shortcut.
pub fn dropdown_menu_shortcut(keys: impl Into<SharedString>) -> DropdownMenuShortcutElement {
    dropdown_menu::dropdown_menu_shortcut().keys(keys.into())
}

/// A dropdown menu's submenu: `dropdown_menu_sub("Invite users").child(items)`.
pub fn dropdown_menu_sub(label: impl Into<SharedString>) -> DropdownMenuSubElement {
    dropdown_menu::dropdown_menu_sub().label(label.into())
}

/// A context menu: `context_menu().trigger(area).child(items)`.
pub fn context_menu() -> ContextMenuElement {
    context_menu::context_menu()
}

/// A menubar: `menubar().child(menubar_menu("File").child(items))`.
pub fn menubar() -> MenubarElement {
    menubar::menubar()
}

/// A menu of a menubar, its trigger reading `label`.
pub fn menubar_menu(label: impl Into<SharedString>) -> MenubarMenuElement {
    menubar::menubar_menu().label(label.into())
}

/// A context menu's item: `context_menu_item("Profile").child(context_menu_shortcut("⇧⌘P"))`.
pub fn context_menu_item(label: impl Into<SharedString>) -> ContextMenuItemElement {
    context_menu::context_menu_item().label(Some(label.into()))
}

/// A context menu's checkbox item.
pub fn context_menu_checkbox_item(label: impl Into<SharedString>) -> ContextMenuCheckboxItemElement {
    context_menu::context_menu_checkbox_item().label(Some(label.into()))
}

/// A context menu's radio item.
pub fn context_menu_radio_item(label: impl Into<SharedString>) -> ContextMenuRadioItemElement {
    context_menu::context_menu_radio_item().label(Some(label.into()))
}

/// A context menu's label.
pub fn context_menu_label(label: impl Into<SharedString>) -> ContextMenuLabelElement {
    context_menu::context_menu_label().label(label.into())
}

/// A context menu's separator.
pub fn context_menu_separator() -> ContextMenuSeparatorElement {
    context_menu::context_menu_separator()
}

/// A context menu item's shortcut.
pub fn context_menu_shortcut(keys: impl Into<SharedString>) -> ContextMenuShortcutElement {
    context_menu::context_menu_shortcut().keys(keys.into())
}

/// A context menu's submenu: `context_menu_sub("Invite users").child(items)`.
pub fn context_menu_sub(label: impl Into<SharedString>) -> ContextMenuSubElement {
    context_menu::context_menu_sub().label(label.into())
}

/// A menubar's item: `menubar_item("Profile").child(menubar_shortcut("⇧⌘P"))`.
pub fn menubar_item(label: impl Into<SharedString>) -> MenubarItemElement {
    menubar::menubar_item().label(Some(label.into()))
}

/// A menubar's checkbox item.
pub fn menubar_checkbox_item(label: impl Into<SharedString>) -> MenubarCheckboxItemElement {
    menubar::menubar_checkbox_item().label(Some(label.into()))
}

/// A menubar's radio item.
pub fn menubar_radio_item(label: impl Into<SharedString>) -> MenubarRadioItemElement {
    menubar::menubar_radio_item().label(Some(label.into()))
}

/// A menubar's label.
pub fn menubar_label(label: impl Into<SharedString>) -> MenubarLabelElement {
    menubar::menubar_label().label(label.into())
}

/// A menubar's separator.
pub fn menubar_separator() -> MenubarSeparatorElement {
    menubar::menubar_separator()
}

/// A menubar item's shortcut.
pub fn menubar_shortcut(keys: impl Into<SharedString>) -> MenubarShortcutElement {
    menubar::menubar_shortcut().keys(keys.into())
}

/// A menubar's submenu: `menubar_sub("Invite users").child(items)`.
pub fn menubar_sub(label: impl Into<SharedString>) -> MenubarSubElement {
    menubar::menubar_sub().label(label.into())
}

/// A select: `select(items).placeholder("Select a fruit")`.
pub fn select(items: Vec<SelectItem>) -> SelectElement {
    select::select().items(items)
}

/// A combobox: `combobox(items).placeholder("Select a framework")`.
pub fn combobox(items: Vec<SelectItem>) -> ComboboxElement {
    combobox::combobox().items(items)
}

/// A command palette: `command(items).placeholder("Type a command or search...")`.
pub fn command(items: Vec<CommandItem>) -> CommandElement {
    command::command().items(items)
}

/// A navigation menu: `navigation_menu().child(navigation_menu_list().child(items))`.
pub fn navigation_menu() -> NavigationMenuElement {
    navigation_menu::navigation_menu()
}

/// A navigation menu's list of items.
pub fn navigation_menu_list() -> NavigationMenuListElement {
    navigation_menu::navigation_menu_list()
}

/// A navigation menu's item whose trigger reads `label`; its children are
/// the content.
pub fn navigation_menu_item(label: impl Into<SharedString>) -> NavigationMenuItemElement {
    navigation_menu::navigation_menu_item().label(Some(label.into()))
}

/// A navigation menu's item without a trigger: its children (a link).
pub fn navigation_menu_link_item() -> NavigationMenuItemElement {
    navigation_menu::navigation_menu_item()
}

/// A navigation menu's link.
pub fn navigation_menu_link() -> NavigationMenuLinkElement {
    navigation_menu::navigation_menu_link()
}

/// Registers this family's components for TSX (`nsh-shadcn`).
pub(crate) fn register(registry: &mut Registry) {
    dropdown_menu::register(registry);
    context_menu::register(registry);
    menubar::register(registry);
    select::register(registry);
    combobox::register(registry);
    command::register(registry);
    navigation_menu::register(registry);
}
