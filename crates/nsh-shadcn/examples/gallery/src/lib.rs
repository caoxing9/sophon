//! The nsh-shadcn gallery: a page per component, laid out as shadcn's docs
//! show it, with a light / dark switch.
//!
//! Storage keys pick what it opens (the screenshot tests set them with
//! `open(".", { storage })`, the docs site in `localStorage` before it loads
//! the web build): `page`, a page's id (`"button"`); `bare` (`"1"`): the
//! page's content area alone, without the sidebar, as
//! `tools/reference/<page>.html` draws shadcn's; `scheme` (`"light"` or
//! `"dark"`): the appearance to start in.

pub mod pages;

use nsh_base::theme;
use nsh_core::element::desktop::window as app_window;
use nsh_core::platform::Appearance;
use nsh_core::prelude::*;
use nsh_core::storage::Storage;
use nsh_shadcn as ui;

#[native_module("nsh-shadcn-gallery/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
}

/// The content area: the page's title and the theme switch, then its body.
fn content(page: &pages::Page) -> View {
    view()
        .flex_1()
        .p_8()
        .gap_8()
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_between()
                .child(text(page.title).text_2xl().font_weight(FontWeight::SEMIBOLD))
                .child(ui::button("Toggle theme").outline().sm().on_press(move || {
                    set_color_scheme(if dark() { Appearance::Light } else { Appearance::Dark });
                })),
        )
        .child((page.body)())
}

/// The sidebar: every page by title.
fn sidebar(selected: Signal<usize>) -> View {
    let mut list = view().gap_1().p_3();
    for (index, page) in pages::all().into_iter().enumerate() {
        list = list.child(
            ui::button(page.title)
                .ghost()
                .sm()
                .app_class(move || {
                    Some(SharedString::from(if selected.get() == index {
                        "w-full justify-start bg-accent text-accent-foreground"
                    } else {
                        "w-full justify-start"
                    }))
                })
                .on_press(move || selected.set(index)),
        );
    }
    view()
        .w(px(200.0))
        .border_r(px(1.0))
        .border_color(move || theme().border)
        .child(scroll().flex_1().child(list))
}

/// The gallery's root: a sidebar of every page beside the selected page, or
/// the page alone (`bare`).
#[component]
pub fn App() -> impl IntoElement {
    let all = pages::all();
    let start = Storage::get_item("page")
        .and_then(|id| all.iter().position(|p| p.id == id))
        .unwrap_or(0);
    let bare = Storage::get_item("bare").as_deref() == Some("1");
    match Storage::get_item("scheme").as_deref() {
        Some("dark") => set_color_scheme(Appearance::Dark),
        Some("light") => set_color_scheme(Appearance::Light),
        _ => {}
    }
    let selected = signal(start);
    let page = move || {
        let all = pages::all();
        let index = selected.get().min(all.len().saturating_sub(1));
        content(&all[index])
    };
    let body = move || {
        // `text-base`: a browser page's body text (16px, line height 1.5),
        // which shadcn's text without a size class inherits.
        let root = view()
            .flex_1()
            .flex_row()
            .app_class(Some(SharedString::from("text-base")))
            .bg(move || theme().background)
            .text_color(move || theme().foreground);
        if bare {
            root.child(dynamic(move || page().into_element()))
        } else {
            root.child(sidebar(selected))
                .child(scroll().flex_1().child(dynamic(move || page().into_element())))
        }
    };
    app_window()
        .id("main")
        .title("nsh-shadcn")
        .default_size(if bare {
            size(px(880.0), px(640.0))
        } else {
            size(px(1080.0), px(720.0))
        })
        .restore_bounds(false)
        .content(body)
}
