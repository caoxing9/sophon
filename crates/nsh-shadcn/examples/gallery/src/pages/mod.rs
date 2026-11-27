//! The gallery's pages: one per component, laid out as shadcn's docs show
//! the component. Each family of components has its own file; a page's body
//! is drawn under the gallery's header ("<Title>" and "Toggle theme") in a
//! content area of 880 × 640 points, as `tools/reference/<page>.html` is.

use nsh_core::prelude::*;

pub mod basics;
pub mod button;
pub mod chat;
pub mod forms;
pub mod layout;
pub mod menus;
pub mod overlays;

/// A page: its id (the shadcn component's file name, `"button"`), its
/// title and its body.
pub struct Page {
    pub id: &'static str,
    pub title: &'static str,
    pub body: fn() -> View,
}

/// Every page, in the sidebar's order.
pub fn all() -> Vec<Page> {
    let mut pages = Vec::new();
    pages.extend(button::pages());
    pages.extend(basics::pages());
    pages.extend(forms::pages());
    pages.extend(overlays::pages());
    pages.extend(menus::pages());
    pages.extend(layout::pages());
    pages.extend(chat::pages());
    pages
}

/// A titled row of examples, as the pages lay them out.
pub fn section(title: &'static str, content: View) -> View {
    view()
        .gap_3()
        .child(
            text(title)
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(move || nsh_base::theme().muted_foreground),
        )
        .child(content)
}

/// A row of examples that wraps, items centered on a line.
pub fn row() -> View {
    view().flex_row().flex_wrap().items_center().gap_2()
}
