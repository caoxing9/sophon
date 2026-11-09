//! Dropdown menus on `nsh/base`'s `Menu`: a popover surface with a hairline
//! border and a soft shadow, rows with an optional icon, a description and
//! a check for the current choice.

#![allow(dead_code)]

use nsh_base::{MenuElement, MenuItemElement, menu, menu_item, menu_separator};
use nsh_core::{
    element::{FontWeight, StyleRefinement, View},
    prelude::*,
};
use super::controls::text;

use super::{
    icon::{Icon, icon},
    tokens::*,
};

fn popup(s: StyleRefinement) -> StyleRefinement {
    // Near opaque: what is under a menu must not read through its rows.
    s.bg(AdaptiveColor::new(Hsla::from(rgb(0xfdfcf9)).opacity(0.985), Hsla::from(rgb(0x2b2b28)).opacity(0.985)))
        .backdrop_blur(px(GLASS_BLUR))
        .backdrop_saturate(1.7)
        .rounded(px(14.0))
        .border_1()
        .border_color(glass_edge())
        .p(px(S1))
        .min_w(px(220.0))
        .max_w(px(360.0))
        .shadow_lg()
}

/// A menu that opens under `trigger`.
pub(crate) fn dropdown(trigger: impl IntoElement) -> MenuElement {
    // A menu over the keyboard is half hidden: it goes first.
    menu().offset(px(6.0)).popup_style(popup).trigger(trigger).on_open(|| {
        if MOBILE {
            dismiss_keyboard();
        }
    })
}

/// A menu that opens under `trigger` as wide as `width` (a sidebar's
/// header: the menu lines up with the rows under it, edge to edge).
pub(crate) fn dropdown_wide(trigger: impl IntoElement, width: f32) -> MenuElement {
    dropdown(trigger)
        .align(nsh_core::overlay::Align::Start)
        .offset(px(4.0))
        .popup_style(move |s: StyleRefinement| popup(s).w(px(width)).min_w(px(width)).max_w(px(width)))
}

/// A menu that opens above `trigger` (pickers in the composer, which sits
/// at the foot of the window).
pub(crate) fn dropup(trigger: impl IntoElement) -> MenuElement {
    dropdown(trigger).placement(nsh_core::overlay::Placement::Top)
}

/// A long list inside a menu, scrolling past `max` points.
pub(crate) fn menu_scroll(max: f32, content: impl IntoElement) -> impl IntoElement {
    scroll().max_h(px(max)).child(content)
}

/// A row of a menu.
pub(crate) fn menu_row(
    label: impl Into<SharedString>,
    lead: Option<Icon>,
    detail: Option<SharedString>,
    checked: bool,
    on_select: impl Fn() + 'static,
) -> MenuItemElement {
    let label: SharedString = label.into();
    let mut body = view().flex_1().gap(px(1.0)).child(
        text(label).text_size(px(TEXT_BASE)).text_color(FG).number_of_lines(1),
    );
    if let Some(detail) = detail.filter(|d| !d.is_empty()) {
        body = body.child(text(detail).text_size(px(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1));
    }
    let mut row = menu_item()
        .on_select(on_select)
        .flex_row()
        .items_center()
        .gap(px(S2))
        .min_h(px(CONTROL_MD))
        .px(px(S2))
        .py(px(5.0))
        .rounded(px(R_LG))
        .hover(|s| s.bg(highlight()))
        .active(|s| s.bg(highlight()))
        .disabled_style(|s| s.opacity(0.4));
    if let Some(lead) = lead {
        row = row.child(icon(lead, 16.0).text_color(FG_MUTED));
    }
    row = row.child(body);
    row.child(
        view().w(px(16.0)).child(if checked {
            icon(Icon::Check, 16.0).text_color(ACCENT).into_element()
        } else {
            view().into_element()
        }),
    )
}

/// A destructive row (archive, remove).
pub(crate) fn menu_row_danger(label: &'static str, lead: Icon, on_select: impl Fn() + 'static) -> MenuItemElement {
    menu_item()
        .on_select(on_select)
        .flex_row()
        .items_center()
        .gap(px(S2))
        .min_h(px(CONTROL_MD))
        .px(px(S2))
        .rounded(px(R_LG))
        .hover(|s| s.bg(tint(DESTRUCTIVE, 0.1)))
        .child(icon(lead, 16.0).text_color(DESTRUCTIVE))
        .child(text(label).text_size(px(TEXT_BASE)).text_color(DESTRUCTIVE))
}

/// A heading inside a menu.
pub(crate) fn menu_heading(label: impl Into<SharedString>) -> View {
    let label: SharedString = label.into();
    view().px(px(S2)).pt(px(S2)).pb(px(S1)).child(
        text(label).text_size(px(TEXT_XS)).font_weight(FontWeight(500)).text_color(FG_FAINT),
    )
}

pub(crate) fn menu_divider() -> impl IntoElement {
    menu_separator().h(px(1.0)).my(px(S1)).mx(px(S1)).bg(BORDER)
}

/// A pill trigger for a picker in a toolbar: icon, label, chevron.
pub(crate) fn picker_trigger(lead: Option<Icon>, label: impl IntoReactive<SharedString>) -> View {
    let mut v = view()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .h(px(CONTROL_SM))
        .px(px(S2))
        .min_w(px(0.0))
        .flex_shrink(1.0)
        .rounded(px(R_FULL))
        .hover(|s| s.bg(highlight()));
    if let Some(lead) = lead {
        v = v.child(icon(lead, 15.0).text_color(FG_MUTED));
    }
    v.child(
        text(label)
            .text_size(px(TEXT_SM))
            .font_weight(FontWeight(500))
            .text_color(FG_MUTED)
            .number_of_lines(1)
            .max_w(px(180.0))
            .flex_shrink(1.0),
    )
    .child(icon(Icon::ChevronDown, 13.0).text_color(FG_FAINT))
}
