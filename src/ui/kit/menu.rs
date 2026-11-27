//! Dropdown menus: nsh-shadcn's `DropdownMenu`, its items holding
//! Sophon's own icons, a description and a check for the current choice.

#![allow(dead_code)]

use nsh_shadcn::{DropdownMenuElement, DropdownMenuItemElement, DropdownMenuLabelElement};
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use super::controls::text;

use super::{
    icon::{Icon, icon},
    tokens::*,
};

/// A menu that opens under `trigger`: nsh-shadcn's `DropdownMenu`.
pub(crate) fn dropdown(trigger: impl IntoElement) -> DropdownMenuElement {
    nsh_shadcn::dropdown_menu().trigger(trigger).on_open_change(|open: &bool| {
        // A menu over the keyboard is half hidden: it goes first.
        if *open && MOBILE {
            dismiss_keyboard();
        }
    })
}

/// A menu that opens under `trigger` as wide as `width` (a sidebar's
/// header: the menu lines up with the rows under it, edge to edge).
pub(crate) fn dropdown_wide(trigger: impl IntoElement, width: f32) -> DropdownMenuElement {
    dropdown(trigger)
        .align(nsh_core::overlay::Align::Start)
        .content_class(Some(SharedString::from(format!("w-[{width}px] min-w-[{width}px] max-w-[{width}px]"))))
}

/// A menu that opens above `trigger` (pickers in the composer, which sits
/// at the foot of the window).
pub(crate) fn dropup(trigger: impl IntoElement) -> DropdownMenuElement {
    dropdown(trigger).side(nsh_core::overlay::Placement::Top)
}

/// A long list inside a menu, scrolling past `max` points.
pub(crate) fn menu_scroll(max: f32, content: impl IntoElement) -> impl IntoElement {
    scroll().max_h(px(max)).child(content)
}

/// A row of a menu: nsh-shadcn's `DropdownMenuItem`, holding Sophon's own
/// icon, a description under the label and a check for the current
/// choice.
pub(crate) fn menu_row(
    label: impl Into<SharedString>,
    lead: Option<Icon>,
    detail: Option<SharedString>,
    checked: bool,
    on_select: impl Fn() + 'static,
) -> DropdownMenuItemElement {
    let label: SharedString = label.into();
    let mut body = view().flex_1().min_w(px(0.0)).gap(px(1.0)).child(
        text(label).text_size(tx(TEXT_SM)).text_color(FG()).number_of_lines(1),
    );
    if let Some(detail) = detail.filter(|d| !d.is_empty()) {
        body = body.child(text(detail).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).number_of_lines(1));
    }
    let mut item = nsh_shadcn::dropdown_menu_item("").label(None::<SharedString>).on_select(on_select);
    if let Some(lead) = lead {
        item = item.child(icon(lead, 16.0).text_color(FG_MUTED()));
    }
    item = item.child(body);
    if checked {
        item = item.child(icon(Icon::Check, 16.0).text_color(FG()));
    }
    item
}

/// A destructive row (archive, remove).
pub(crate) fn menu_row_danger(label: &'static str, lead: Icon, on_select: impl Fn() + 'static) -> DropdownMenuItemElement {
    nsh_shadcn::dropdown_menu_item(label)
        .destructive()
        .child(icon(lead, 16.0).text_color(DESTRUCTIVE))
        .on_select(on_select)
}

/// A heading inside a menu.
pub(crate) fn menu_heading(label: impl Into<SharedString>) -> DropdownMenuLabelElement {
    nsh_shadcn::dropdown_menu_label(label)
}

pub(crate) fn menu_divider() -> impl IntoElement {
    nsh_shadcn::dropdown_menu_separator()
}

/// A pill trigger for a picker in a toolbar: icon, label, chevron.
pub(crate) fn picker_trigger(lead: Option<Icon>, label: impl IntoReactive<SharedString>) -> View {
    let mut v = view()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .h(sp(CONTROL_SM))
        .px(sp(S2))
        .min_w(px(0.0))
        .flex_shrink(1.0)
        .rounded(px(R_FULL))
        .hover(|s| s.bg(highlight()));
    if let Some(lead) = lead {
        v = v.child(icon(lead, 15.0).text_color(FG_MUTED()));
    }
    v.child(
        text(label)
            .text_size(tx(TEXT_SM))
            .font_weight(FontWeight(500))
            .text_color(FG_MUTED())
            .number_of_lines(1)
            .max_w(px(180.0))
            .flex_shrink(1.0),
    )
    .child(icon(Icon::ChevronDown, 13.0).text_color(FG_FAINT()))
}
