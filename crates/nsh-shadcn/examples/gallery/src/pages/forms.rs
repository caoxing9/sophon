//! The forms family's pages: the examples shadcn's docs show for each
//! component, laid out with the classes `tools/reference/<id>.html` lays
//! shadcn's own markup out with. Labels and separators, which another
//! family ports, are drawn here with `label.tsx`'s and `separator.tsx`'s
//! classes.

use nsh_base::theme;
use nsh_core::element::{ImageSrc, svg};
use nsh_core::prelude::*;
use nsh_shadcn::{self as ui, IconName};

use super::{Page, section};

pub fn pages() -> Vec<Page> {
    vec![
        Page {
            id: "input",
            title: "Input",
            body: input,
        },
        Page {
            id: "textarea",
            title: "Textarea",
            body: textarea,
        },
        Page {
            id: "checkbox",
            title: "Checkbox",
            body: checkbox,
        },
        Page {
            id: "radio-group",
            title: "Radio Group",
            body: radio_group,
        },
        Page {
            id: "switch",
            title: "Switch",
            body: switch,
        },
        Page {
            id: "slider",
            title: "Slider",
            body: slider,
        },
        Page {
            id: "toggle",
            title: "Toggle",
            body: toggle,
        },
        Page {
            id: "toggle-group",
            title: "Toggle Group",
            body: toggle_group,
        },
        Page {
            id: "button-group",
            title: "Button Group",
            body: button_group,
        },
        Page {
            id: "native-select",
            title: "Native Select",
            body: native_select,
        },
        Page {
            id: "input-otp",
            title: "Input OTP",
            body: input_otp,
        },
        Page {
            id: "input-group",
            title: "Input Group",
            body: input_group,
        },
        Page {
            id: "field",
            title: "Field",
            body: field,
        },
    ]
}

/// A view drawing with `classes` (the reference's `<div class="…">`).
fn div(classes: &'static str) -> View {
    view().app_class(Some(SharedString::from(classes)))
}

/// A text drawing with `classes`.
fn p(content: &'static str, classes: &'static str) -> Text {
    text(content).app_class(Some(SharedString::from(classes)))
}

/// shadcn's `Label` (`label.tsx`'s classes).
const LABEL: &str = "flex items-center gap-2 text-sm leading-none font-medium select-none";

/// shadcn's `Label`.
fn label(content: &'static str) -> View {
    div(LABEL).child(text(content))
}

/// Two columns of sections.
fn columns(left: View, right: View) -> View {
    div("flex flex-row gap-8").child(left).child(right)
}

/// A column of sections.
fn column() -> View {
    div("flex flex-1 flex-col gap-8")
}

/// A muted line of text the tests read.
fn status(content: impl Fn() -> String + 'static) -> Text {
    text(content)
        .text_sm()
        .text_color(move || theme().muted_foreground)
}

fn some(s: &'static str) -> Option<SharedString> {
    Some(SharedString::from(s))
}

fn input() -> View {
    columns(
        column()
            .child(section(
                "Default",
                div("flex w-full max-w-sm flex-col")
                    .child(ui::input().placeholder(some("Email")).email()),
            ))
            .child(section(
                "Disabled",
                div("flex w-full max-w-sm flex-col").child(
                    ui::input()
                        .placeholder(some("Email"))
                        .email()
                        .disabled(true),
                ),
            ))
            .child(section(
                "Invalid",
                div("flex w-full max-w-sm flex-col").child(
                    ui::input()
                        .placeholder(some("Email"))
                        .default_value(some("not an email"))
                        .email()
                        .invalid(true),
                ),
            )),
        column()
            .child(section(
                "With label",
                div("grid w-full max-w-sm items-center gap-3")
                    .child(label("Email"))
                    .child(ui::input().placeholder(some("Email")).email()),
            ))
            .child(section(
                "With button",
                div("flex w-full max-w-sm items-center gap-2")
                    .child(ui::input().placeholder(some("Email")).email())
                    .child(ui::button("Subscribe").outline()),
            ))
            .child(section(
                "With text",
                div("grid w-full max-w-sm items-center gap-3")
                    .child(label("Email"))
                    .child(ui::input().placeholder(some("Email")).email())
                    .child(p(
                        "Enter your email address.",
                        "text-sm text-muted-foreground",
                    )),
            )),
    )
}

fn textarea() -> View {
    let message = signal(SharedString::default());
    columns(
        column()
            .child(section(
                "Default",
                div("flex w-full flex-col")
                    .child(ui::textarea().placeholder(some("Type your message here."))),
            ))
            .child(section(
                "Disabled",
                div("flex w-full flex-col").child(
                    ui::textarea()
                        .placeholder(some("Type your message here."))
                        .disabled(true),
                ),
            ))
            .child(status(move || format!("Message: {}", message.get()))),
        column()
            .child(section(
                "With label",
                div("grid w-full gap-3")
                    .child(label("Your message"))
                    .child(ui::textarea().placeholder(some("Type your message here."))),
            ))
            .child(section(
                "With button",
                div("grid w-full gap-2")
                    .child(
                        ui::textarea()
                            .placeholder(some("Write a message"))
                            .on_change(move |value: &SharedString| message.set(value.clone())),
                    )
                    .child(ui::button("Send message")),
            ))
            .child(section(
                "With text",
                div("grid w-full gap-3")
                    .child(label("Your Message"))
                    .child(ui::textarea().placeholder(some("Type your message here.")))
                    .child(p(
                        "Your message will be copied to the support team.",
                        "text-sm text-muted-foreground",
                    )),
            )),
    )
}

fn checkbox() -> View {
    let terms = signal(false);
    let card = signal(true);
    columns(
        column().child(section(
            "Demo",
            div("flex flex-col gap-6")
                .child(
                    div("flex items-center gap-3")
                        .child(
                            ui::checkbox()
                                .accessibility_label(some("Terms"))
                                .on_checked_change(move |checked: &bool| terms.set(*checked)),
                        )
                        .child(label("Accept terms and conditions")),
                )
                .child(
                    div("flex items-start gap-3")
                        .child(ui::checkbox().default_checked(true))
                        .child(
                            div("grid shrink gap-2")
                                .child(label("Accept terms and conditions"))
                                .child(p(
                                    "By clicking this checkbox, you agree to the terms and conditions.",
                                    "text-sm text-muted-foreground",
                                )),
                        ),
                )
                .child(
                    div("flex items-start gap-3")
                        .child(ui::checkbox().disabled(true))
                        .child(div("flex items-center gap-2 text-sm leading-none font-medium select-none opacity-50").child(text("Enable notifications"))),
                )
                .child(
                    view()
                        .app_class(move || Some(SharedString::from(if card.get() {
                            "flex items-start gap-3 rounded-lg border border-border p-3 hover:bg-accent/50 border-blue-600 bg-blue-50 dark:border-blue-900 dark:bg-blue-950"
                        } else {
                            "flex items-start gap-3 rounded-lg border border-border p-3 hover:bg-accent/50"
                        })))
                        .child(ui::checkbox().default_checked(true).on_checked_change(move |c: &bool| card.set(*c)).app_class(some(
                            "checked:border-blue-600 checked:bg-blue-600 checked:text-white dark:checked:border-blue-700 dark:checked:bg-blue-700",
                        )))
                        .child(
                            div("grid shrink gap-1.5 font-normal")
                                .child(p("Enable notifications", "text-sm leading-none font-medium"))
                                .child(p(
                                    "You can enable or disable notifications at any time.",
                                    "text-sm text-muted-foreground",
                                )),
                        ),
                ),
        )),
        column()
            .child(section(
                "With text",
                div("flex gap-2")
                    .child(ui::checkbox())
                    .child(
                        div("grid shrink gap-1.5 leading-none")
                            .child(p("Accept terms and conditions", "text-sm leading-none font-medium"))
                            .child(p(
                                "You agree to our Terms of Service and Privacy Policy.",
                                "text-sm text-muted-foreground",
                            )),
                    ),
            ))
            .child(section(
                "Invalid",
                div("flex items-center gap-2")
                    .child(ui::checkbox().invalid(true))
                    .child(label("Accept terms and conditions")),
            ))
            .child(status(move || format!("Accepted: {}", if terms.get() { "yes" } else { "no" }))),
    )
}

fn radio_option(value: &'static str, title: &'static str) -> View {
    div("flex items-center gap-3")
        .child(ui::radio_group_item(value).accessibility_label(some(title)))
        .child(label(title))
}

fn radio_group() -> View {
    let density = signal(SharedString::from("comfortable"));
    columns(
        column()
            .child(section(
                "Demo",
                div("flex flex-col").child(
                    ui::radio_group()
                        .default_value(some("comfortable"))
                        .on_value_change(move |value: &SharedString| density.set(value.clone()))
                        .child(radio_option("default", "Default"))
                        .child(radio_option("comfortable", "Comfortable"))
                        .child(radio_option("compact", "Compact")),
                ),
            ))
            .child(status(move || format!("Density: {}", density.get()))),
        column().child(section(
            "Disabled",
            div("flex flex-col").child(
                ui::radio_group()
                    .default_value(some("one"))
                    .disabled(true)
                    .child(radio_option("one", "Option One"))
                    .child(radio_option("two", "Option Two")),
            ),
        )),
    )
}

fn switch() -> View {
    let airplane = signal(false);
    columns(
        column()
            .child(section(
                "Demo",
                div("flex items-center gap-2")
                    .child(
                        ui::switch()
                            .accessibility_label(some("Airplane Mode"))
                            .on_checked_change(move |checked: &bool| airplane.set(*checked)),
                    )
                    .child(label("Airplane Mode")),
            ))
            .child(status(move || {
                format!(
                    "Airplane mode: {}",
                    if airplane.get() { "on" } else { "off" }
                )
            })),
        column()
            .child(section(
                "Sizes",
                div("flex items-center gap-4")
                    .child(ui::switch().size(ui::SwitchSize::Sm))
                    .child(ui::switch().size(ui::SwitchSize::Sm).default_checked(true))
                    .child(ui::switch())
                    .child(ui::switch().default_checked(true)),
            ))
            .child(section(
                "Disabled",
                div("flex items-center gap-4")
                    .child(ui::switch().disabled(true))
                    .child(ui::switch().disabled(true).default_checked(true)),
            )),
    )
}

fn field() -> View {
    columns(
        column()
            .child(section(
                "Input",
                div("w-full max-w-md").child(
                    ui::field_set().child(
                        ui::field_group()
                            .child(
                                ui::field()
                                    .child(ui::field_label("Username"))
                                    .child(ui::input().placeholder(some("Max Leiter")))
                                    .child(ui::field_description(
                                        "Choose a unique username for your account.",
                                    )),
                            )
                            .child(
                                ui::field()
                                    .child(ui::field_label("Password"))
                                    .child(ui::field_description("Must be at least 8 characters long."))
                                    .child(ui::input().placeholder(some("••••••••")).password()),
                            ),
                    ),
                ),
            ))
            .child(section(
                "Switch",
                div("w-full max-w-md").child(
                    ui::field()
                        .orientation(ui::FieldOrientation::Horizontal)
                        .child(
                            ui::field_content()
                                .child(ui::field_label("Multi-factor authentication"))
                                .child(ui::field_description(
                                    "Enable multi-factor authentication. If you do not have a two-factor device, you can use a one-time code sent to your email.",
                                )),
                        )
                        .child(ui::switch().accessibility_label(some("MFA"))),
                ),
            )),
        column()
            .child(section(
                "Choice card",
                div("w-full max-w-md").child(
                    ui::field_group().child(
                        ui::field_set()
                            .child(ui::field_label("Compute Environment"))
                            .child(ui::field_description(
                                "Select the compute environment for your cluster.",
                            ))
                            .child(
                                ui::radio_group()
                                    .default_value(some("kubernetes"))
                                    .child(choice_card(
                                        "kubernetes",
                                        "Kubernetes",
                                        "Run GPU workloads on a K8s configured cluster.",
                                    ))
                                    .child(choice_card(
                                        "vm",
                                        "Virtual Machine",
                                        "Access a VM configured cluster to run GPU workloads.",
                                    )),
                            ),
                    ),
                ),
            ))
            .child(section(
                "Checkbox and error",
                div("w-full max-w-md").child(
                    ui::field_group()
                        .child(
                            ui::field()
                                .orientation(ui::FieldOrientation::Horizontal)
                                .child(ui::checkbox().default_checked(true).accessibility_label(some("Same as shipping")))
                                .child(
                                    ui::field_label("Same as shipping address")
                                        .app_class(some("font-normal")),
                                ),
                        )
                        .child(
                            ui::field()
                                .invalid(true)
                                .child(ui::field_label("Email"))
                                .child(ui::input().default_value(some("max@")).invalid(true))
                                .child(ui::field_error(["Enter a valid email address."])),
                        ),
                ),
            )),
    )
}

/// A radio option drawn as a card: a FieldLabel around a horizontal Field.
fn choice_card(
    value: &'static str,
    title: &'static str,
    description: &'static str,
) -> ui::FieldLabelElement {
    ui::field_label("").label(None).child(
        ui::field()
            .orientation(ui::FieldOrientation::Horizontal)
            .child(
                ui::field_content()
                    .child(ui::field_title(title))
                    .child(ui::field_description(description)),
            )
            .child(ui::radio_group_item(value).accessibility_label(some(title))),
    )
}

fn slider() -> View {
    let volume = signal(50.0_f64);
    columns(
        column()
            .child(section(
                "Demo",
                div("flex flex-col").child(
                    ui::slider()
                        .default_value(vec![50.0])
                        .max(100.0)
                        .step(1.0)
                        .accessibility_label(some("Volume"))
                        .on_value_change(move |v: &Vec<f64>| volume.set(v[0]))
                        .app_class(some("w-[60%]")),
                ),
            ))
            .child(section(
                "Range",
                div("flex flex-col").child(
                    ui::slider()
                        .default_value(vec![25.0, 75.0])
                        .app_class(some("w-[60%]")),
                ),
            ))
            .child(section(
                "Disabled",
                div("flex flex-col").child(
                    ui::slider()
                        .default_value(vec![33.0])
                        .disabled(true)
                        .app_class(some("w-[60%]")),
                ),
            ))
            .child(status(move || format!("Volume: {}", volume.get()))),
        column().child(section(
            "Vertical",
            div("flex h-44 flex-row gap-8")
                .child(
                    ui::slider()
                        .default_value(vec![50.0])
                        .orientation(ui::SliderOrientation::Vertical),
                )
                .child(
                    ui::slider()
                        .default_value(vec![20.0, 80.0])
                        .orientation(ui::SliderOrientation::Vertical),
                ),
        )),
    )
}

/// Lucide's bookmark filled and stroked blue, as shadcn's demo draws it
/// while on (`*:[svg]:fill-blue-500 *:[svg]:stroke-blue-500`).
fn blue_bookmark() -> AnyElement {
    let markup = IconName::Bookmark.svg_markup().replace(
        r#"fill="none" stroke="currentColor""#,
        r##"fill="#3b82f6" stroke="#3b82f6""##,
    );
    svg(ImageSrc::svg_markup(&markup))
        .size(px(16.0))
        .flex_shrink_0()
        .into_element()
}

fn toggle() -> View {
    let bookmarked = signal(false);
    columns(
        column()
            .child(section(
                "Demo",
                div("flex flex-row").child(
                    ui::toggle()
                        .outline()
                        .sm()
                        .pressed(move || Some(bookmarked.get()))
                        .on_pressed_change(move |on: &bool| bookmarked.set(*on))
                        .accessibility_label(some("Toggle bookmark"))
                        .app_class(some("aria-pressed:bg-transparent"))
                        .child(dynamic(move || {
                            if bookmarked.get() {
                                blue_bookmark()
                            } else {
                                ui::icon(IconName::Bookmark).into_element()
                            }
                        }))
                        .child(text("Bookmark")),
                ),
            ))
            .child(section(
                "Outline",
                div("flex flex-row").child(
                    ui::toggle()
                        .outline()
                        .icon(Some(IconName::Italic))
                        .accessibility_label(some("Toggle italic")),
                ),
            ))
            .child(section(
                "With text",
                div("flex flex-row").child(
                    ui::toggle()
                        .icon(Some(IconName::Italic))
                        .label(some("Italic"))
                        .accessibility_label(some("Toggle italic")),
                ),
            ))
            .child(status(move || {
                format!(
                    "Bookmarked: {}",
                    if bookmarked.get() { "yes" } else { "no" }
                )
            })),
        column()
            .child(section(
                "Sizes",
                div("flex flex-row items-center gap-2")
                    .child(
                        ui::toggle()
                            .sm()
                            .icon(Some(IconName::Italic))
                            .accessibility_label(some("Small")),
                    )
                    .child(
                        ui::toggle()
                            .icon(Some(IconName::Italic))
                            .accessibility_label(some("Default")),
                    )
                    .child(
                        ui::toggle()
                            .lg()
                            .icon(Some(IconName::Italic))
                            .accessibility_label(some("Large")),
                    ),
            ))
            .child(section(
                "On",
                div("flex flex-row items-center gap-2")
                    .child(
                        ui::toggle()
                            .default_pressed(true)
                            .icon(Some(IconName::Bold))
                            .accessibility_label(some("Bold on")),
                    )
                    .child(
                        ui::toggle()
                            .outline()
                            .default_pressed(true)
                            .icon(Some(IconName::Bold))
                            .accessibility_label(some("Bold on outline")),
                    ),
            ))
            .child(section(
                "Disabled",
                div("flex flex-row").child(
                    ui::toggle()
                        .disabled(true)
                        .icon(Some(IconName::Underline))
                        .accessibility_label(some("Toggle underline")),
                ),
            )),
    )
}

/// Bold, italic and underline in a toggle group.
fn formatting(group: ui::ToggleGroupElement) -> View {
    div("flex flex-row").child(
        group
            .child(
                ui::toggle_group_item("bold")
                    .icon(Some(IconName::Bold))
                    .accessibility_label(some("Toggle bold")),
            )
            .child(
                ui::toggle_group_item("italic")
                    .icon(Some(IconName::Italic))
                    .accessibility_label(some("Toggle italic")),
            )
            .child(
                ui::toggle_group_item("strikethrough")
                    .icon(Some(IconName::Underline))
                    .accessibility_label(some("Toggle strikethrough")),
            ),
    )
}

fn toggle_group() -> View {
    let on = signal(Vec::<SharedString>::new());
    columns(
        column()
            .child(section(
                "Outline",
                formatting(
                    ui::toggle_group()
                        .r#type(ui::ToggleGroupType::Multiple)
                        .variant(ui::ToggleVariant::Outline)
                        .on_value_change(move |v: &Vec<SharedString>| on.set(v.clone())),
                ),
            ))
            .child(section(
                "Single",
                formatting(ui::toggle_group().default_value(vec!["italic".into()])),
            ))
            .child(section(
                "Small",
                formatting(ui::toggle_group().size(ui::ToggleSize::Sm)),
            ))
            .child(status(move || {
                let on = on.get();
                format!(
                    "On: {}",
                    if on.is_empty() {
                        "none".to_string()
                    } else {
                        on.join(", ")
                    }
                )
            })),
        column()
            .child(section(
                "Large",
                formatting(
                    ui::toggle_group()
                        .r#type(ui::ToggleGroupType::Multiple)
                        .size(ui::ToggleSize::Lg),
                ),
            ))
            .child(section(
                "Disabled",
                formatting(
                    ui::toggle_group()
                        .r#type(ui::ToggleGroupType::Multiple)
                        .disabled(true),
                ),
            ))
            .child(section(
                "Spacing",
                div("flex flex-row").child(
                    ui::toggle_group()
                        .r#type(ui::ToggleGroupType::Multiple)
                        .variant(ui::ToggleVariant::Outline)
                        .spacing(2)
                        .size(ui::ToggleSize::Sm)
                        .child(
                            ui::toggle_group_item("star")
                                .icon(Some(IconName::Star))
                                .label(some("Star")),
                        )
                        .child(
                            ui::toggle_group_item("heart")
                                .icon(Some(IconName::Heart))
                                .label(some("Heart")),
                        )
                        .child(
                            ui::toggle_group_item("bookmark")
                                .icon(Some(IconName::Bookmark))
                                .label(some("Bookmark")),
                        ),
                ),
            )),
    )
}

fn button_group() -> View {
    let archived = signal(0_u32);
    columns(
        column()
            .child(section(
                "Demo",
                div("flex flex-row").child(
                    ui::button_group()
                        .child(
                            ui::button_group()
                                .item(ui::icon_button(IconName::ArrowLeft, "Go Back").outline()),
                        )
                        .child(
                            ui::button_group()
                                .item(
                                    ui::button("Archive")
                                        .outline()
                                        .on_press(move || archived.update(|n| *n += 1)),
                                )
                                .item(ui::button("Report").outline()),
                        )
                        .child(
                            ui::button_group()
                                .item(ui::button("Snooze").outline())
                                .item(
                                    ui::icon_button(IconName::Ellipsis, "More Options").outline(),
                                ),
                        ),
                ),
            ))
            .child(section(
                "Size",
                div("flex flex-col items-start gap-8")
                    .child(
                        ui::button_group()
                            .item(ui::button("Small").outline().sm())
                            .item(ui::button("Button").outline().sm())
                            .item(ui::button("Group").outline().sm())
                            .item(
                                ui::icon_button(IconName::Plus, "Add")
                                    .outline()
                                    .size(ui::ButtonSize::IconSm),
                            ),
                    )
                    .child(
                        ui::button_group()
                            .item(ui::button("Default").outline())
                            .item(ui::button("Button").outline())
                            .item(ui::button("Group").outline())
                            .item(ui::icon_button(IconName::Plus, "Add").outline()),
                    )
                    .child(
                        ui::button_group()
                            .item(ui::button("Large").outline().lg())
                            .item(ui::button("Button").outline().lg())
                            .item(ui::button("Group").outline().lg())
                            .item(
                                ui::icon_button(IconName::Plus, "Add")
                                    .outline()
                                    .size(ui::ButtonSize::IconLg),
                            ),
                    ),
            ))
            .child(status(move || format!("Archived: {}", archived.get()))),
        column()
            .child(section(
                "Orientation",
                div("flex flex-row").child(
                    ui::button_group()
                        .vertical()
                        .accessibility_label(some("Media controls"))
                        .item(ui::icon_button(IconName::Plus, "Volume up").outline())
                        .item(ui::icon_button(IconName::Minus, "Volume down").outline()),
                ),
            ))
            .child(section(
                "Separator and split",
                div("flex flex-row items-center gap-8")
                    .child(
                        ui::button_group()
                            .item(ui::button("Copy").secondary().sm())
                            .item(ui::button_group_separator())
                            .item(ui::button("Paste").secondary().sm()),
                    )
                    .child(
                        ui::button_group()
                            .item(ui::button("Button").secondary())
                            .item(ui::button_group_separator())
                            .item(ui::icon_button(IconName::Plus, "Add").secondary()),
                    ),
            ))
            .child(section(
                "Input",
                div("flex flex-row").child(
                    ui::button_group()
                        .item(ui::input().placeholder(some("Search...")))
                        .item(
                            ui::icon_button(IconName::Search, "Search")
                                .outline()
                                .size(ui::ButtonSize::Default),
                        ),
                ),
            ))
            .child(section(
                "Nested",
                div("flex flex-row").child(
                    ui::button_group()
                        .child(
                            ui::button_group()
                                .item(ui::button("1").outline().sm())
                                .item(ui::button("2").outline().sm())
                                .item(ui::button("3").outline().sm())
                                .item(ui::button("4").outline().sm())
                                .item(ui::button("5").outline().sm()),
                        )
                        .child(
                            ui::button_group()
                                .item(
                                    ui::icon_button(IconName::ArrowLeft, "Previous")
                                        .outline()
                                        .size(ui::ButtonSize::IconSm),
                                )
                                .item(
                                    ui::icon_button(IconName::ArrowRight, "Next")
                                        .outline()
                                        .size(ui::ButtonSize::IconSm),
                                ),
                        ),
                ),
            )),
    )
}

fn status_options() -> Vec<ui::NativeSelectOption> {
    vec![
        ui::native_select_option("", "Select status"),
        ui::native_select_option("todo", "Todo"),
        ui::native_select_option("in-progress", "In Progress"),
        ui::native_select_option("done", "Done"),
        ui::native_select_option("cancelled", "Cancelled"),
    ]
}

fn native_select() -> View {
    let chosen = signal(SharedString::default());
    columns(
        column()
            .child(section(
                "Demo",
                div("flex flex-row").child(
                    ui::native_select(status_options())
                        .accessibility_label(some("Status"))
                        .on_change(move |v: &SharedString| chosen.set(v.clone())),
                ),
            ))
            .child(section(
                "Groups",
                div("flex flex-row").child(ui::native_select([
                    ui::native_select_option("", "Select department"),
                    ui::native_select_option("frontend", "Frontend").group("Engineering"),
                    ui::native_select_option("backend", "Backend").group("Engineering"),
                    ui::native_select_option("devops", "DevOps").group("Engineering"),
                    ui::native_select_option("sales-rep", "Sales Rep").group("Sales"),
                    ui::native_select_option("account-manager", "Account Manager").group("Sales"),
                    ui::native_select_option("sales-director", "Sales Director").group("Sales"),
                    ui::native_select_option("support", "Customer Support").group("Operations"),
                    ui::native_select_option("product-manager", "Product Manager")
                        .group("Operations"),
                    ui::native_select_option("ops-manager", "Operations Manager")
                        .group("Operations"),
                ])),
            ))
            .child(status(move || format!("Status: {}", chosen.get()))),
        column()
            .child(section(
                "Disabled",
                div("flex flex-row").child(
                    ui::native_select([
                        ui::native_select_option("", "Select priority"),
                        ui::native_select_option("low", "Low"),
                        ui::native_select_option("medium", "Medium"),
                        ui::native_select_option("high", "High"),
                        ui::native_select_option("critical", "Critical"),
                    ])
                    .disabled(true),
                ),
            ))
            .child(section(
                "Invalid",
                div("flex flex-row").child(
                    ui::native_select([
                        ui::native_select_option("", "Select role"),
                        ui::native_select_option("admin", "Admin"),
                        ui::native_select_option("editor", "Editor"),
                        ui::native_select_option("viewer", "Viewer"),
                        ui::native_select_option("guest", "Guest"),
                    ])
                    .invalid(true),
                ),
            ))
            .child(section(
                "Small",
                div("flex flex-row")
                    .child(ui::native_select(status_options()).size(ui::NativeSelectSize::Sm)),
            )),
    )
}

fn input_otp() -> View {
    let code = signal(SharedString::default());
    columns(
        column()
            .child(section(
                "Demo",
                div("flex flex-row").child(ui::input_otp(6).groups(2)),
            ))
            .child(section(
                "Single group",
                div("flex flex-row").child(ui::input_otp(6)),
            ))
            .child(section(
                "Separators",
                div("flex flex-row").child(ui::input_otp(6).groups(3)),
            )),
        column()
            .child(section(
                "Controlled",
                div("flex flex-col gap-2")
                    .child(
                        div("flex flex-row").child(
                            ui::input_otp(6)
                                .accessibility_label(some("One-time password"))
                                .value(move || Some(code.get()))
                                .on_change(move |v: &SharedString| code.set(v.clone())),
                        ),
                    )
                    .child(
                        text(move || {
                            let code = code.get();
                            if code.is_empty() {
                                "Enter your one-time password.".to_string()
                            } else {
                                format!("You entered: {code}")
                            }
                        })
                        .app_class(some("text-sm")),
                    ),
            ))
            .child(section(
                "Disabled",
                div("flex flex-row").child(
                    ui::input_otp(6)
                        .groups(2)
                        .default_value(some("123"))
                        .disabled(true),
                ),
            ))
            .child(section(
                "Invalid",
                div("flex flex-row").child(
                    ui::input_otp(6)
                        .groups(2)
                        .default_value(some("12"))
                        .invalid(true),
                ),
            )),
    )
}

fn input_group() -> View {
    let copied = signal(0_u32);
    columns(
        column().child(section(
            "Demo",
            div("grid w-full max-w-sm gap-6")
                .child(
                    ui::input_group()
                        .child(ui::input_group_addon().icon(Some(IconName::Search)))
                        .child(ui::input_group_input().placeholder(some("Search...")))
                        .child(ui::input_group_addon().align(ui::InputGroupAddonAlign::InlineEnd).label(some("12 results"))),
                )
                .child(
                    ui::input_group()
                        .child(ui::input_group_addon().child(ui::input_group_text("https://")))
                        .child(ui::input_group_input().placeholder(some("example.com")).app_class(some("pl-1!")))
                        .child(
                            ui::input_group_addon().align(ui::InputGroupAddonAlign::InlineEnd).child(
                                ui::input_group_button()
                                    .size(ui::InputGroupButtonSize::IconXs)
                                    .icon(Some(IconName::Info))
                                    .accessibility_label(some("Info"))
                                    .app_class(some("rounded-full")),
                            ),
                        ),
                )
                .child(
                    ui::input_group()
                        .child(ui::input_group_textarea().placeholder(some("Ask, Search or Chat...")))
                        .child(
                            ui::input_group_addon()
                                .align(ui::InputGroupAddonAlign::BlockEnd)
                                .child(
                                    ui::input_group_button()
                                        .variant(ui::ButtonVariant::Outline)
                                        .size(ui::InputGroupButtonSize::IconXs)
                                        .icon(Some(IconName::Plus))
                                        .accessibility_label(some("Attach"))
                                        .app_class(some("rounded-full")),
                                )
                                .child(ui::input_group_button().label(some("Auto")))
                                .child(ui::input_group_text("52% used").app_class(some("ml-auto")))
                                .child(div("shrink-0 bg-border h-full w-px h-4!"))
                                .child(
                                    ui::input_group_button()
                                        .variant(ui::ButtonVariant::Default)
                                        .size(ui::InputGroupButtonSize::IconXs)
                                        .icon(Some(IconName::ArrowUp))
                                        .accessibility_label(some("Send"))
                                        .disabled(true)
                                        .app_class(some("rounded-full")),
                                ),
                        ),
                )
                .child(
                    ui::input_group()
                        .child(ui::input_group_input().placeholder(some("@shadcn")))
                        .child(
                            ui::input_group_addon().align(ui::InputGroupAddonAlign::InlineEnd).child(
                                div("flex size-4 items-center justify-center rounded-full bg-primary text-primary-foreground")
                                    .child(ui::icon(IconName::Check).app_class(some("size-3"))),
                            ),
                        ),
                ),
        ))
        .child(status(move || format!("Copied: {}", copied.get()))),
        column()
            .child(section(
                "Text",
                div("grid w-full max-w-sm gap-6")
                    .child(
                        ui::input_group()
                            .child(ui::input_group_addon().child(ui::input_group_text("$")))
                            .child(ui::input_group_input().placeholder(some("0.00")))
                            .child(
                                ui::input_group_addon()
                                    .align(ui::InputGroupAddonAlign::InlineEnd)
                                    .child(ui::input_group_text("USD")),
                            ),
                    )
                    .child(
                        ui::input_group()
                            .child(ui::input_group_input().placeholder(some("Enter your username")))
                            .child(
                                ui::input_group_addon()
                                    .align(ui::InputGroupAddonAlign::InlineEnd)
                                    .child(ui::input_group_text("@company.com")),
                            ),
                    )
                    .child(
                        ui::input_group()
                            .child(ui::input_group_textarea().placeholder(some("Enter your message")))
                            .child(
                                ui::input_group_addon()
                                    .align(ui::InputGroupAddonAlign::BlockEnd)
                                    .child(ui::input_group_text("120 characters left").app_class(some(
                                        "text-xs text-muted-foreground",
                                    ))),
                            ),
                    ),
            ))
            .child(section(
                "Button",
                div("grid w-full max-w-sm gap-6")
                    .child(
                        ui::input_group()
                            .child(ui::input_group_input().default_value(some("https://x.com/shadcn")).readonly(true))
                            .child(
                                ui::input_group_addon().align(ui::InputGroupAddonAlign::InlineEnd).child(
                                    ui::input_group_button()
                                        .size(ui::InputGroupButtonSize::IconXs)
                                        .icon(Some(IconName::Copy))
                                        .accessibility_label(some("Copy"))
                                        .on_press(move || copied.update(|n| *n += 1)),
                                ),
                            ),
                    )
                    .child(
                        ui::input_group()
                            .child(ui::input_group_input().placeholder(some("Type to search...")))
                            .child(
                                ui::input_group_addon().align(ui::InputGroupAddonAlign::InlineEnd).child(
                                    ui::input_group_button()
                                        .variant(ui::ButtonVariant::Secondary)
                                        .label(some("Search")),
                                ),
                            ),
                    ),
            )),
    )
}
