//! The overlays family of components. Each component is one file in this
//! directory; this module declares them, re-exports their public API (types
//! and constructors, which the crate root re-exports in turn) and registers
//! them for TSX.
//!
//! The roots of the modal overlays (`Dialog`, `AlertDialog`, `Sheet`,
//! `Drawer`) keep the open state their parts find through context
//! (`open.rs`); the anchored ones (`Popover`, `HoverCard`, `Tooltip`) are
//! `nsh_base`'s, their content a part of this crate inside the popup, which
//! enters and leaves with NSH's popup transitions (`motion.rs`).

mod alert_dialog;
mod dialog;
mod drawer;
mod hover_card;
mod motion;
mod open;
mod popover;
mod sheet;
mod sonner;
mod tooltip;

use nsh_core::prelude::*;

pub use alert_dialog::{
    AlertDialog, AlertDialogAction, AlertDialogActionElement, AlertDialogActionEvent,
    AlertDialogCancel, AlertDialogCancelElement, AlertDialogContent, AlertDialogContentElement,
    AlertDialogDescription, AlertDialogDescriptionElement, AlertDialogElement, AlertDialogEvent,
    AlertDialogFooter, AlertDialogFooterElement, AlertDialogHeader, AlertDialogHeaderElement,
    AlertDialogMedia, AlertDialogMediaElement, AlertDialogSize, AlertDialogTitle,
    AlertDialogTitleElement,
};
pub use dialog::{
    Dialog, DialogClose, DialogCloseElement, DialogContent, DialogContentElement, DialogDescription,
    DialogDescriptionElement, DialogElement, DialogEvent, DialogFooter, DialogFooterElement,
    DialogHeader, DialogHeaderElement, DialogTitle, DialogTitleElement,
};
pub use drawer::{
    Drawer, DrawerClose, DrawerCloseElement, DrawerContent, DrawerContentElement,
    DrawerDescription, DrawerDescriptionElement, DrawerDirection, DrawerElement, DrawerEvent,
    DrawerFooter, DrawerFooterElement, DrawerHeader, DrawerHeaderElement, DrawerTitle,
    DrawerTitleElement,
};
pub use hover_card::{
    HoverCard, HoverCardContent, HoverCardContentElement, HoverCardElement, HoverCardEvent,
};
pub use popover::{
    Popover, PopoverContent, PopoverContentElement, PopoverDescription,
    PopoverDescriptionElement, PopoverElement, PopoverEvent, PopoverHeader, PopoverHeaderElement,
    PopoverTitle, PopoverTitleElement,
};
pub use sheet::{
    Sheet, SheetClose, SheetCloseElement, SheetContent, SheetContentElement, SheetDescription,
    SheetDescriptionElement, SheetElement, SheetEvent, SheetFooter, SheetFooterElement,
    SheetHeader, SheetHeaderElement, SheetSide, SheetTitle, SheetTitleElement,
};
pub use sonner::{
    SonnerToast, ToastId, ToastType, Toaster, ToasterElement, ToasterPosition, dismiss,
    dismiss_all, toast,
};
pub use tooltip::{Tooltip, TooltipContent, TooltipContentElement, TooltipElement, TooltipEvent};

// ---- Dialog ----------------------------------------------------------------

/// A dialog: `dialog().trigger(button("Open").outline()).child(dialog_content()…)`.
pub fn dialog() -> DialogElement {
    dialog::dialog()
}

/// The window of a [`dialog`]: its header, body and footer as children.
pub fn dialog_content() -> DialogContentElement {
    dialog::dialog_content()
}

/// The title and description of a dialog, stacked.
pub fn dialog_header() -> DialogHeaderElement {
    dialog::dialog_header()
}

/// The actions of a dialog.
pub fn dialog_footer() -> DialogFooterElement {
    dialog::dialog_footer()
}

/// A dialog's title: `dialog_title("Edit profile")`.
pub fn dialog_title(text: impl Into<SharedString>) -> DialogTitleElement {
    dialog::dialog_title().text(text.into())
}

/// A dialog's description, under its title.
pub fn dialog_description(text: impl Into<SharedString>) -> DialogDescriptionElement {
    dialog::dialog_description().text(text.into())
}

/// `button` closes the dialog it is in when pressed:
/// `dialog_close(button("Cancel").outline())`.
pub fn dialog_close(button: impl IntoElement) -> DialogCloseElement {
    dialog::dialog_close().button(button)
}

// ---- Alert dialog ----------------------------------------------------------

/// An alert dialog: `alert_dialog().trigger(button("Delete")).child(alert_dialog_content()…)`.
pub fn alert_dialog() -> AlertDialogElement {
    alert_dialog::alert_dialog()
}

/// The window of an [`alert_dialog`]: its header and footer as children.
pub fn alert_dialog_content() -> AlertDialogContentElement {
    alert_dialog::alert_dialog_content()
}

/// The media, title and description of an alert dialog.
pub fn alert_dialog_header() -> AlertDialogHeaderElement {
    alert_dialog::alert_dialog_header()
}

/// The actions of an alert dialog.
pub fn alert_dialog_footer() -> AlertDialogFooterElement {
    alert_dialog::alert_dialog_footer()
}

/// An alert dialog's title.
pub fn alert_dialog_title(text: impl Into<SharedString>) -> AlertDialogTitleElement {
    alert_dialog::alert_dialog_title().text(text.into())
}

/// An alert dialog's description.
pub fn alert_dialog_description(text: impl Into<SharedString>) -> AlertDialogDescriptionElement {
    alert_dialog::alert_dialog_description().text(text.into())
}

/// An icon in a muted square before an alert dialog's title.
pub fn alert_dialog_media(icon: crate::IconName) -> AlertDialogMediaElement {
    alert_dialog::alert_dialog_media().icon(Some(icon))
}

/// The button that confirms: `alert_dialog_action("Continue").on_press(..)`.
pub fn alert_dialog_action(label: impl Into<SharedString>) -> AlertDialogActionElement {
    alert_dialog::alert_dialog_action().label(label.into())
}

/// The button that cancels.
pub fn alert_dialog_cancel(label: impl Into<SharedString>) -> AlertDialogCancelElement {
    alert_dialog::alert_dialog_cancel().label(label.into())
}

// ---- Sheet -----------------------------------------------------------------

/// A sheet: `sheet().trigger(button("Open").outline()).child(sheet_content()…)`.
pub fn sheet() -> SheetElement {
    sheet::sheet()
}

/// The panel of a [`sheet`] (on the right; `.side(..)`).
pub fn sheet_content() -> SheetContentElement {
    sheet::sheet_content()
}

/// The title and description of a sheet.
pub fn sheet_header() -> SheetHeaderElement {
    sheet::sheet_header()
}

/// The actions of a sheet, at its end.
pub fn sheet_footer() -> SheetFooterElement {
    sheet::sheet_footer()
}

/// A sheet's title.
pub fn sheet_title(text: impl Into<SharedString>) -> SheetTitleElement {
    sheet::sheet_title().text(text.into())
}

/// A sheet's description.
pub fn sheet_description(text: impl Into<SharedString>) -> SheetDescriptionElement {
    sheet::sheet_description().text(text.into())
}

/// `button` closes the sheet it is in when pressed.
pub fn sheet_close(button: impl IntoElement) -> SheetCloseElement {
    sheet::sheet_close().button(button)
}

// ---- Drawer ----------------------------------------------------------------

/// A drawer: `drawer().trigger(button("Open Drawer").outline()).child(drawer_content()…)`.
pub fn drawer() -> DrawerElement {
    drawer::drawer()
}

/// The panel of a [`drawer`] (at the bottom, with a handle).
pub fn drawer_content() -> DrawerContentElement {
    drawer::drawer_content()
}

/// The title and description of a drawer.
pub fn drawer_header() -> DrawerHeaderElement {
    drawer::drawer_header()
}

/// The actions of a drawer, at its end.
pub fn drawer_footer() -> DrawerFooterElement {
    drawer::drawer_footer()
}

/// A drawer's title.
pub fn drawer_title(text: impl Into<SharedString>) -> DrawerTitleElement {
    drawer::drawer_title().text(text.into())
}

/// A drawer's description.
pub fn drawer_description(text: impl Into<SharedString>) -> DrawerDescriptionElement {
    drawer::drawer_description().text(text.into())
}

/// `button` closes the drawer it is in when pressed.
pub fn drawer_close(button: impl IntoElement) -> DrawerCloseElement {
    drawer::drawer_close().button(button)
}

// ---- Popover ---------------------------------------------------------------

/// A popover: `popover().trigger(button("Open").outline()).child(popover_content()…)`.
pub fn popover() -> PopoverElement {
    popover::popover()
}

/// The card a [`popover`] shows.
pub fn popover_content() -> PopoverContentElement {
    popover::popover_content()
}

/// A title over a description, in a popover.
pub fn popover_header() -> PopoverHeaderElement {
    popover::popover_header()
}

/// A popover's title.
pub fn popover_title(text: impl Into<SharedString>) -> PopoverTitleElement {
    popover::popover_title().text(text.into())
}

/// A popover's description.
pub fn popover_description(text: impl Into<SharedString>) -> PopoverDescriptionElement {
    popover::popover_description().text(text.into())
}

// ---- Hover card ------------------------------------------------------------

/// A hover card: `hover_card().trigger(button("@nextjs").link()).child(hover_card_content()…)`.
pub fn hover_card() -> HoverCardElement {
    hover_card::hover_card()
}

/// The card a [`hover_card`] shows.
pub fn hover_card_content() -> HoverCardContentElement {
    hover_card::hover_card_content()
}

// ---- Tooltip ---------------------------------------------------------------

/// A tooltip: `tooltip().trigger(button("Hover").outline()).child(tooltip_content("Add to library"))`.
pub fn tooltip() -> TooltipElement {
    tooltip::tooltip()
}

/// What a [`tooltip`] says, in its bubble.
pub fn tooltip_content(text: impl Into<SharedString>) -> TooltipContentElement {
    tooltip::tooltip_content().text(Some(text.into()))
}

// ---- Sonner ----------------------------------------------------------------

/// Where the page's toasts show (sonner's `<Toaster />`); [`toast`] shows one.
pub fn toaster() -> ToasterElement {
    sonner::toaster()
}

/// Registers this family's components for TSX (`nsh-shadcn`).
pub(crate) fn register(registry: &mut Registry) {
    dialog::register(registry);
    alert_dialog::register(registry);
    sheet::register(registry);
    drawer::register(registry);
    popover::register(registry);
    hover_card::register(registry);
    tooltip::register(registry);
    sonner::register(registry);
}
