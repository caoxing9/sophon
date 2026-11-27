//! Button.

use nsh_base::theme;
use nsh_core::prelude::*;
use nsh_shadcn::{self as ui, IconName};

use super::{Page, row, section};

pub fn pages() -> Vec<Page> {
    vec![Page { id: "button", title: "Button", body }]
}

fn body() -> View {
    let presses = signal(0_u32);
    let press = move || presses.update(|n| *n += 1);
    view()
        .gap_8()
        .child(section(
            "Variants",
            row()
                .child(ui::button("Button").on_press(press))
                .child(ui::button("Secondary").secondary().on_press(press))
                .child(ui::button("Destructive").destructive().on_press(press))
                .child(ui::button("Outline").outline().on_press(press))
                .child(ui::button("Ghost").ghost().on_press(press))
                .child(ui::button("Link").link().on_press(press)),
        ))
        .child(section(
            "Sizes",
            row()
                .child(ui::button("Extra small").xs())
                .child(ui::button("Small").sm())
                .child(ui::button("Default"))
                .child(ui::button("Large").lg()),
        ))
        .child(section(
            "With icons",
            row()
                .child(ui::button("New branch").outline().sm().icon(Some(IconName::GitBranch)))
                .child(ui::button("Send").icon(Some(IconName::Send)))
                .child(ui::icon_button(IconName::Plus, "Add").outline())
                .child(ui::icon_button(IconName::ArrowUp, "Submit").secondary().size(ui::ButtonSize::IconSm))
                .child(ui::button("Please wait").disabled(true).icon(Some(IconName::LoaderCircle))),
        ))
        .child(section(
            "Disabled",
            row()
                .child(ui::button("Button").disabled(true))
                .child(ui::button("Secondary").secondary().disabled(true))
                .child(ui::button("Destructive").destructive().disabled(true))
                .child(ui::button("Outline").outline().disabled(true)),
        ))
        .child(
            text(move || format!("Pressed: {}", presses.get()))
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
}
