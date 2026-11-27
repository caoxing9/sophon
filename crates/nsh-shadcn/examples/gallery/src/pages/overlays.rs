//! This family's pages: Dialog, Alert Dialog, Sheet, Drawer, Popover,
//! Hover Card, Tooltip and Sonner, each as shadcn's docs demo it (the
//! `*-demo.tsx` examples, and `sheet-side.tsx`, `sonner-types.tsx`), the
//! triggers at the top of the page, centered.
//!
//! The demos compose components of other families (Label, Input, Avatar);
//! they are drawn here with those components' shadcn classes, so this
//! family depends on no other.

use std::time::Duration;

use nsh_core::prelude::*;
use nsh_shadcn::{self as ui, IconName};

use super::Page;

pub fn pages() -> Vec<Page> {
    vec![
        Page { id: "dialog", title: "Dialog", body: dialog },
        Page { id: "alert-dialog", title: "Alert Dialog", body: alert_dialog },
        Page { id: "sheet", title: "Sheet", body: sheet },
        Page { id: "drawer", title: "Drawer", body: drawer },
        Page { id: "popover", title: "Popover", body: popover },
        Page { id: "hover-card", title: "Hover Card", body: hover_card },
        Page { id: "tooltip", title: "Tooltip", body: tooltip },
        Page { id: "sonner", title: "Sonner", body: sonner },
    ]
}

/// Classes as an `app_class` value.
fn cls(classes: &'static str) -> Option<SharedString> {
    Some(SharedString::from(classes))
}

/// A page's body: its examples centered across the page.
fn centered() -> View {
    view().items_center().gap_4()
}

/// shadcn's `Label`, drawn with its classes (and the demo's `extra`).
fn label(name: &'static str, extra: &'static str) -> impl IntoElement {
    text(name).app_class(Some(SharedString::from(format!(
        "flex items-center gap-2 text-sm leading-none font-medium select-none {extra}"
    ))))
}

/// shadcn's `Input` holding `value`, drawn with its classes (and the demo's
/// `extra`). Its `transition-[color,box-shadow]` is `transition-colors`
/// (NSH has no arbitrary transition properties).
fn input(value: &'static str, extra: &'static str) -> impl IntoElement {
    nsh_core::element::input()
        .default_value(value.to_owned())
        .app_class(Some(SharedString::from(format!(
            "h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 text-base \
             shadow-xs transition-colors outline-none placeholder:text-muted-foreground \
             md:text-sm dark:bg-input/30 focus-visible:border-ring focus-visible:ring-[3px] \
             focus-visible:ring-ring/50 {extra}"
        ))))
}

/// A labeled field of the demos: `grid gap-3`.
fn field(name: &'static str, value: &'static str) -> impl IntoElement {
    view().app_class(cls("grid gap-3")).child(label(name, "")).child(input(value, ""))
}

fn dialog() -> View {
    centered().child(
        ui::dialog().trigger(ui::button("Open Dialog").outline()).child(
            ui::dialog_content()
                .app_class(cls("sm:max-w-[425px]"))
                .child(
                    ui::dialog_header()
                        .child(ui::dialog_title("Edit profile"))
                        .child(ui::dialog_description(
                            "Make changes to your profile here. Click save when you're done.",
                        )),
                )
                .child(
                    view()
                        .app_class(cls("grid gap-4"))
                        .child(field("Name", "Pedro Duarte"))
                        .child(field("Username", "@peduarte")),
                )
                .child(
                    ui::dialog_footer()
                        .child(ui::dialog_close(ui::button("Cancel").outline()))
                        .child(ui::button("Save changes")),
                ),
        ),
    )
}

fn alert_dialog() -> View {
    centered().child(
        ui::alert_dialog().trigger(ui::button("Show Dialog").outline()).child(
            ui::alert_dialog_content()
                .child(
                    ui::alert_dialog_header()
                        .child(ui::alert_dialog_title("Are you absolutely sure?"))
                        .child(ui::alert_dialog_description(
                            "This action cannot be undone. This will permanently delete your \
                             account and remove your data from our servers.",
                        )),
                )
                .child(
                    ui::alert_dialog_footer()
                        .child(ui::alert_dialog_cancel("Cancel"))
                        .child(ui::alert_dialog_action("Continue")),
                ),
        ),
    )
}

/// The profile form of the sheet demos.
fn sheet_content(side: ui::SheetSide) -> ui::SheetContentElement {
    ui::sheet_content()
        .side(side)
        .child(
            ui::sheet_header()
                .child(ui::sheet_title("Edit profile"))
                .child(ui::sheet_description(
                    "Make changes to your profile here. Click save when you're done.",
                )),
        )
        .child(
            view()
                .app_class(cls("grid flex-1 auto-rows-min gap-6 px-4"))
                .child(field("Name", "Pedro Duarte"))
                .child(field("Username", "@peduarte")),
        )
        .child(
            ui::sheet_footer()
                .child(ui::button("Save changes"))
                .child(ui::sheet_close(ui::button("Close").outline())),
        )
}

/// A row of `sheet-side.tsx`: the label right-aligned, the input over three
/// of four columns.
fn side_field(name: &'static str, value: &'static str) -> impl IntoElement {
    view()
        .app_class(cls("grid grid-cols-4 items-center gap-4"))
        .child(label(name, "text-right"))
        .child(input(value, "col-span-3"))
}

fn sheet_side(side: ui::SheetSide, name: &'static str) -> impl IntoElement {
    ui::sheet().trigger(ui::button(name).outline()).child(
        ui::sheet_content()
            .side(side)
            .child(
                ui::sheet_header()
                    .child(ui::sheet_title("Edit profile"))
                    .child(ui::sheet_description(
                        "Make changes to your profile here. Click save when you're done.",
                    )),
            )
            .child(
                view()
                    .app_class(cls("grid gap-4 py-4"))
                    .child(side_field("Name", "Pedro Duarte"))
                    .child(side_field("Username", "@peduarte")),
            )
            .child(ui::sheet_footer().child(ui::sheet_close(ui::button("Save changes")))),
    )
}

fn sheet() -> View {
    centered()
        .child(
            ui::sheet()
                .trigger(ui::button("Open").outline())
                .child(sheet_content(ui::SheetSide::Right)),
        )
        .child(
            view()
                .app_class(cls("grid grid-cols-2 gap-2"))
                .child(sheet_side(ui::SheetSide::Top, "top"))
                .child(sheet_side(ui::SheetSide::Right, "right"))
                .child(sheet_side(ui::SheetSide::Bottom, "bottom"))
                .child(sheet_side(ui::SheetSide::Left, "left")),
        )
}

/// The goals of `drawer-demo.tsx`'s bar chart.
const GOALS: [u32; 13] = [400, 300, 200, 300, 200, 278, 189, 239, 300, 200, 278, 189, 349];

/// `drawer-demo.tsx`'s recharts `BarChart` in its 120-point box: 5 points
/// of margin, 13 bands, each bar 80% of its band (`barCategoryGap` 10%),
/// on a 0–400 axis, in the foreground at 90% opacity.
fn bars() -> impl IntoElement {
    let mut chart = view().app_class(cls("mt-3 h-[120px] flex-row items-end p-[5px]"));
    for goal in GOALS {
        let height = goal as f32 / 400.0 * 110.0;
        chart = chart.child(
            view().flex_1().items_center().child(
                view()
                    .app_class(cls("w-[80%] bg-foreground opacity-90"))
                    .h(px(height)),
            ),
        );
    }
    chart
}

fn drawer() -> View {
    let goal = signal(350_i32);
    let adjust = move |by: i32| goal.update(|g| *g = (*g + by).clamp(200, 400));
    let round = |icon: IconName, name: &'static str| {
        ui::icon_button(icon, name)
            .outline()
            .app_class(cls("h-8 w-8 shrink-0 rounded-full"))
    };
    centered().child(
        ui::drawer().trigger(ui::button("Open Drawer").outline()).child(
            ui::drawer_content().child(
                view()
                    .app_class(cls("mx-auto w-full max-w-sm"))
                    .child(
                        ui::drawer_header()
                            .child(ui::drawer_title("Move Goal"))
                            .child(ui::drawer_description("Set your daily activity goal.")),
                    )
                    .child(
                        view()
                            .app_class(cls("p-4 pb-0"))
                            .child(
                                view()
                                    .app_class(cls("flex items-center justify-center space-x-2"))
                                    .child(
                                        round(IconName::Minus, "Decrease")
                                            .disabled(move || goal.get() <= 200)
                                            .on_press(move || adjust(-10)),
                                    )
                                    .child(
                                        view()
                                            .app_class(cls("flex-1 text-center"))
                                            .child(
                                                text(move || goal.get().to_string()).app_class(cls(
                                                    "text-7xl font-bold tracking-tighter",
                                                )),
                                            )
                                            .child(text("Calories/day").app_class(cls(
                                                "text-[0.70rem] text-muted-foreground uppercase",
                                            ))),
                                    )
                                    .child(
                                        round(IconName::Plus, "Increase")
                                            .disabled(move || goal.get() >= 400)
                                            .on_press(move || adjust(10)),
                                    ),
                            )
                            .child(bars()),
                    )
                    .child(
                        ui::drawer_footer()
                            .child(ui::button("Submit"))
                            .child(ui::drawer_close(ui::button("Cancel").outline())),
                    ),
            ),
        ),
    )
}

/// A row of `popover-demo.tsx`'s form.
fn dimension(name: &'static str, value: &'static str) -> impl IntoElement {
    view()
        .app_class(cls("grid grid-cols-3 items-center gap-4"))
        .child(label(name, ""))
        .child(input(value, "col-span-2 h-8"))
}

fn popover() -> View {
    centered().child(
        ui::popover().trigger(ui::button("Open popover").outline()).child(
            ui::popover_content().app_class(cls("w-80")).child(
                view()
                    .app_class(cls("grid gap-4"))
                    .child(
                        view()
                            .app_class(cls("space-y-2"))
                            .child(text("Dimensions").app_class(cls("leading-none font-medium")))
                            .child(
                                text("Set the dimensions for the layer.")
                                    .app_class(cls("text-sm text-muted-foreground")),
                            ),
                    )
                    .child(
                        view()
                            .app_class(cls("grid gap-2"))
                            .child(dimension("Width", "100%"))
                            .child(dimension("Max. width", "300px"))
                            .child(dimension("Height", "25px"))
                            .child(dimension("Max. height", "none")),
                    ),
            ),
        ),
    )
}

fn hover_card() -> View {
    centered().child(
        ui::hover_card().trigger(ui::button("@nextjs").link()).child(
            ui::hover_card_content().app_class(cls("w-80")).child(
                view()
                    .app_class(cls("flex justify-between gap-4"))
                    // shadcn's Avatar with its fallback (the image is not
                    // loaded: it would come from the network).
                    .child(
                        view()
                            .app_class(cls(
                                "relative flex size-8 shrink-0 overflow-hidden rounded-full select-none",
                            ))
                            .child(
                                view()
                                    .app_class(cls(
                                        "flex size-full items-center justify-center rounded-full \
                                         bg-muted text-sm text-muted-foreground",
                                    ))
                                    .child(text("VC")),
                            ),
                    )
                    .child(
                        // `shrink`: NSH's views do not shrink by default (CSS's
                        // do), so the column would keep its text on one line
                        // past the card.
                        view()
                            .app_class(cls("space-y-1 shrink"))
                            .child(text("@nextjs").app_class(cls("text-sm font-semibold")))
                            .child(
                                text("The React Framework – created and maintained by @vercel.")
                                    .app_class(cls("text-sm")),
                            )
                            .child(
                                text("Joined December 2021")
                                    .app_class(cls("text-xs text-muted-foreground")),
                            ),
                    ),
            ),
        ),
    )
}

fn tooltip() -> View {
    centered().child(
        ui::tooltip()
            .trigger(ui::button("Hover").outline())
            .child(ui::tooltip_content("Add to library")),
    )
}

fn sonner() -> View {
    let outline = |name: &'static str| ui::button(name).outline();
    centered()
        .child(ui::toaster())
        .child(outline("Show Toast").on_press(|| {
            ui::toast("Event has been created")
                .description("Sunday, December 03, 2023 at 9:00 AM")
                .action("Undo", || {})
                .show();
        }))
        .child(
            view()
                .app_class(cls("flex flex-row flex-wrap justify-center gap-2"))
                .child(outline("Default").on_press(|| {
                    ui::toast("Event has been created").show();
                }))
                .child(outline("Success").on_press(|| {
                    ui::toast("Event has been created").success().show();
                }))
                .child(outline("Info").on_press(|| {
                    ui::toast("Be at the area 10 minutes before the event time").info().show();
                }))
                .child(outline("Warning").on_press(|| {
                    ui::toast("Event start time cannot be earlier than 8am").warning().show();
                }))
                .child(outline("Error").on_press(|| {
                    ui::toast("Event has not been created").error().show();
                }))
                .child(outline("Promise").on_press(|| {
                    let id = ui::toast("Loading...").loading().show();
                    // The timer runs on its own: dropping its handle does
                    // not cancel it.
                    drop(set_timeout(Duration::from_secs(2), move || {
                        ui::toast("Event has been created").success().id(id).show();
                    }));
                })),
        )
}
