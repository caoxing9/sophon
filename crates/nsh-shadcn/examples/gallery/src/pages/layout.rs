//! The layout family's pages: Tabs, Accordion, Collapsible, Scroll Area,
//! Resizable, Sidebar, Carousel, Calendar and Chart, as shadcn's docs show
//! them. Parts of other families the examples compose (Card, Input, Label,
//! Separator) are drawn here with their shadcn classes.

use nsh_core::prelude::*;
use nsh_shadcn::{self as ui};

use super::{Page, section};

/// A titled example: `section` around any element.
fn example(title: &'static str, content: impl IntoElement) -> View {
    section(title, view().items_start().child(content))
}

pub fn pages() -> Vec<Page> {
    vec![
        Page { id: "tabs", title: "Tabs", body: tabs_page },
        Page { id: "accordion", title: "Accordion", body: accordion_page },
        Page { id: "collapsible", title: "Collapsible", body: collapsible_page },
        Page { id: "scroll-area", title: "Scroll Area", body: scroll_area_page },
        Page { id: "resizable", title: "Resizable", body: resizable_page },
        Page { id: "sidebar", title: "Sidebar", body: sidebar_page },
        Page { id: "carousel", title: "Carousel", body: carousel_page },
        Page { id: "calendar", title: "Calendar", body: calendar_page },
        Page { id: "chart", title: "Chart", body: chart_page },
    ]
}

/// A view with shadcn classes.
fn div(class: &'static str) -> View {
    view().app_class(Some(SharedString::from(class)))
}

/// A text with shadcn classes.
fn span(content: impl Into<SharedString>, class: &'static str) -> Text {
    text(content.into()).app_class(Some(SharedString::from(class)))
}

/// shadcn's Card.
fn card() -> View {
    div("flex flex-col gap-6 rounded-xl border border-border bg-card py-6 text-card-foreground shadow-sm")
}

/// shadcn's CardHeader with a CardTitle and a CardDescription.
fn card_header(title: &'static str, description: &'static str) -> View {
    div("grid auto-rows-min items-start gap-2 px-6")
        .child(span(title, "leading-none font-semibold"))
        .child(span(description, "text-sm text-muted-foreground"))
}

/// shadcn's Label over an Input, in a `grid gap-3`.
fn field(label: &'static str, value: &'static str, masked: bool) -> View {
    div("grid gap-3")
        .child(span(label, "flex items-center gap-2 text-sm leading-none font-medium select-none"))
        .child(
            nsh_base::input()
                .default_value(value.to_owned())
                .masked(masked)
                .label(label)
                .app_class(Some(SharedString::from(
                    "h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 text-sm \
                     shadow-xs dark:bg-input/30",
                ))),
        )
}

fn tabs_page() -> View {
    let account = card()
        .child(card_header(
            "Account",
            "Make changes to your account here. Click save when you're done.",
        ))
        .child(
            div("grid gap-6 px-6")
                .child(field("Name", "Pedro Duarte", false))
                .child(field("Username", "@peduarte", false)),
        )
        .child(div("flex items-center px-6").child(ui::button("Save changes")));
    let password = card()
        .child(card_header(
            "Password",
            "Change your password here. After saving, you'll be logged out.",
        ))
        .child(
            div("grid gap-6 px-6")
                .child(field("Current password", "", true))
                .child(field("New password", "", true)),
        )
        .child(div("flex items-center px-6").child(ui::button("Save password")));
    view()
        .flex_row()
        .items_start()
        .gap_8()
        .child(
            div("flex w-full max-w-sm flex-col gap-6").child(
                ui::tabs("account")
                    .child(
                        ui::tabs_list()
                            .child(ui::tabs_trigger("account", "Account"))
                            .child(ui::tabs_trigger("password", "Password")),
                    )
                    .child(ui::tabs_content("account").child(account))
                    .child(ui::tabs_content("password").child(password)),
            ),
        )
        .child(
            view()
                .gap_8()
                .child(example(
                    "Line",
                    ui::tabs("overview").child(
                        ui::tabs_list()
                            .line()
                            .child(ui::tabs_trigger("overview", "Overview"))
                            .child(ui::tabs_trigger("analytics", "Analytics"))
                            .child(ui::tabs_trigger("reports", "Reports")),
                    ),
                ))
                .child(example(
                    "Vertical",
                    ui::tabs("general").vertical().child(
                        ui::tabs_list()
                            .child(ui::tabs_trigger("general", "General"))
                            .child(ui::tabs_trigger("security", "Security"))
                            .child(ui::tabs_trigger("billing", "Billing").disabled(true)),
                    ),
                )),
        )
}

fn accordion_page() -> View {
    view().child(accordion_demo())
}

fn accordion_demo() -> impl IntoElement {
    let item = |value: &'static str, title: &'static str, first: &'static str, second: &'static str| {
        ui::accordion_item(value)
            .child(ui::accordion_trigger(title))
            .child(
                ui::accordion_content()
                    .content_class(Some("flex flex-col gap-4 text-balance".into()))
                    .child(text(first))
                    .child(text(second)),
            )
    };
    ui::accordion()
        .collapsible(true)
        .default_value(vec!["item-1".into()])
        .app_class(Some("w-full".into()))
        .child(item(
            "item-1",
            "Product Information",
            "Our flagship product combines cutting-edge technology with sleek design. Built with \
             premium materials, it offers unparalleled performance and reliability.",
            "Key features include advanced processing capabilities, and an intuitive user \
             interface designed for both beginners and experts.",
        ))
        .child(item(
            "item-2",
            "Shipping Details",
            "We offer worldwide shipping through trusted courier partners. Standard delivery \
             takes 3-5 business days, while express shipping ensures delivery within 1-2 business \
             days.",
            "All orders are carefully packaged and fully insured. Track your shipment in \
             real-time through our dedicated tracking portal.",
        ))
        .child(item(
            "item-3",
            "Return Policy",
            "We stand behind our products with a comprehensive 30-day return policy. If you're \
             not completely satisfied, simply return the item in its original condition.",
            "Our hassle-free return process includes free return shipping and full refunds \
             processed within 48 hours of receiving the returned item.",
        ))
}

/// The demo's mono rows.
fn repo(name: &'static str) -> View {
    div("rounded-md border border-border px-4 py-2 font-mono text-sm").child(text(name))
}

fn collapsible_page() -> View {
    let open = signal(false);
    view().items_start().child(
        ui::collapsible()
            .open(move || Some(open.get()))
            .on_open_change(move |next: &bool| open.set(*next))
            .app_class(Some("flex w-[350px] flex-col gap-2".into()))
            .child(
                div("flex items-center justify-between gap-4 px-4")
                    .child(span("@peduarte starred 3 repositories", "text-sm font-semibold"))
                    .child(
                        ui::collapsible_trigger()
                            .variant(Some(ui::ButtonVariant::Ghost))
                            .size(ui::ButtonSize::Icon)
                            .icon(Some(ui::IconName::ChevronsUpDown))
                            .accessibility_label(Some("Toggle".into()))
                            .app_class(Some("size-8".into())),
                    ),
            )
            .child(repo("@radix-ui/primitives"))
            .child(
                ui::collapsible_content()
                    .content_class(Some("flex flex-col gap-2".into()))
                    .child(repo("@radix-ui/colors"))
                    .child(repo("@stitches/react")),
            ),
    )
}

fn scroll_area_page() -> View {
    let mut tags = div("p-4").child(span("Tags", "mb-4 text-sm leading-none font-medium"));
    for i in (1..=50).rev() {
        tags = tags
            .child(span(format!("v1.2.0-beta.{i}"), "text-sm"))
            .child(div("shrink-0 bg-border h-px w-full my-2"));
    }
    let mut works = div("flex w-max flex-row gap-4 p-4");
    for artist in ["Ornella Binni", "Tom Byrom", "Vladimir Malyavko"] {
        works = works.child(
            div("shrink-0")
                .child(
                    div("overflow-hidden rounded-md")
                        .child(
                            div("flex h-[400px] w-[300px] items-center justify-center bg-muted")
                                .child(ui::icon(ui::IconName::Image).app_class(Some(
                                    "size-8 text-muted-foreground".into(),
                                ))),
                        ),
                )
                .child(
                    span("Photo by ", "pt-2 text-xs text-muted-foreground")
                        .child(span(artist, "font-semibold text-foreground")),
                ),
        );
    }
    view()
        .flex_row()
        .items_start()
        .gap_8()
        .child(
            ui::scroll_area()
                .app_class(Some("h-72 w-48 rounded-md border border-border".into()))
                .child(tags),
        )
        .child(
            ui::scroll_area()
                .horizontal()
                .app_class(Some("w-96 rounded-md border border-border whitespace-nowrap".into()))
                .child(works),
        )
}

/// A panel's centered label.
fn pane(label: &'static str, class: &'static str) -> View {
    div(class).child(span(label, "font-semibold"))
}

fn resizable_page() -> View {
    let demo = ui::resizable_panel_group()
        .app_class(Some("max-w-md rounded-lg border border-border md:min-w-[450px]".into()))
        .child(ui::resizable_panel(50.0).child(pane("One", "flex h-[200px] items-center justify-center p-6")))
        .child(
            ui::resizable_panel(50.0).child(
                ui::resizable_panel_group()
                    .vertical()
                    .child(
                        ui::resizable_panel(25.0)
                            .child(pane("Two", "flex h-full items-center justify-center p-6")),
                    )
                    .child(
                        ui::resizable_panel(75.0)
                            .child(pane("Three", "flex h-full items-center justify-center p-6")),
                    ),
            ),
        );
    let handle = ui::resizable_panel_group()
        .with_handle(true)
        .app_class(Some("min-h-[200px] max-w-md rounded-lg border border-border md:min-w-[450px]".into()))
        .child(ui::resizable_panel(25.0).child(pane("Sidebar", "flex h-full items-center justify-center p-6")))
        .child(ui::resizable_panel(75.0).child(pane("Content", "flex h-full items-center justify-center p-6")));
    view()
        .gap_8()
        .child(example("Default", demo))
        .child(example("With handle", handle))
}

/// A slide of the carousel demos: a card with its number.
fn slide(index: usize, content: &'static str, number: &'static str) -> impl IntoElement {
    div("p-1").child(card().child(div(content).child(span(format!("{}", index + 1), number))))
}

fn carousel_page() -> View {
    let mut items = ui::carousel_content();
    for i in 0..5 {
        items = items.child(ui::carousel_item().child(slide(
            i,
            "flex aspect-square items-center justify-center p-6",
            "text-4xl font-semibold",
        )));
    }
    let demo = ui::carousel()
        .accessibility_label(Some("Demo".into()))
        .app_class(Some("w-full max-w-xs".into()))
        .child(items)
        .child(ui::carousel_previous())
        .child(ui::carousel_next());
    let mut items = ui::carousel_content().content_class(Some("-mt-1 h-[200px]".into()));
    for i in 0..5 {
        items = items.child(
            ui::carousel_item()
                .app_class(Some("pt-1 md:basis-1/2".into()))
                .child(slide(i, "flex items-center justify-center p-6", "text-3xl font-semibold")),
        );
    }
    let vertical = ui::carousel()
        .vertical()
        .accessibility_label(Some("Orientation".into()))
        .app_class(Some("w-full max-w-xs".into()))
        .child(items)
        .child(ui::carousel_previous())
        .child(ui::carousel_next());
    view()
        .flex_row()
        .items_start()
        .child(div("flex w-[416px] flex-col items-center px-12").child(demo))
        .child(div("flex w-[400px] flex-col items-center py-12").child(vertical))
}

/// June 2025, `day`: far from today, so the pictures do not change with the
/// day they are taken.
fn june(day: u32) -> Option<ui::NaiveDate> {
    ui::NaiveDate::from_ymd_opt(2025, 6, day)
}

fn calendar_page() -> View {
    // June 12, 2025, and June 9 to 26: a month far from today, so the
    // pictures do not change with the day they are taken.
    let single = ui::calendar()
        .default_selected(ui::Date::Single(june(12)))
        .caption_layout(ui::CalendarCaptionLayout::Dropdown)
        .app_class(Some("rounded-md border border-border shadow-sm".into()));
    let range = ui::calendar()
        .mode(ui::CalendarMode::Range)
        .default_selected(ui::Date::Range(june(9), june(26)))
        .app_class(Some("rounded-lg border border-border".into()));
    view()
        .flex_row()
        .items_start()
        .gap_8()
        .child(example("Single", single))
        .child(example("Range", range))
}

const MONTH_NAMES: [&str; 6] = ["January", "February", "March", "April", "May", "June"];

/// shadcn's chart examples' rows: visitors per month on desktop and mobile.
fn chart_rows() -> nsh_core::element::Rows {
    nsh_core::element::Rows::from_columns(
        6,
        [
            ("desktop".into(), vec![186.0, 305.0, 237.0, 73.0, 209.0, 214.0]),
            ("mobile".into(), vec![80.0, 200.0, 120.0, 190.0, 130.0, 140.0]),
        ],
    )
}

fn chart_page() -> View {
    let labels: Vec<SharedString> = MONTH_NAMES.iter().map(|m| SharedString::from(*m)).collect();
    let short: Vec<SharedString> = MONTH_NAMES.iter().map(|m| SharedString::from(&m[..3])).collect();
    let ticks = vec![0.0, 80.0, 160.0, 240.0, 320.0];
    let bars = ui::chart_container(vec![
        ui::ChartConfigItem::new("desktop", "Desktop", "chart-1"),
        ui::ChartConfigItem::new("mobile", "Mobile", "chart-2"),
    ])
    .data(chart_rows())
    .labels(labels.clone())
    .x_axis_labels(Some(short.clone()))
    .x_scale(Some(ui::chart_band_scale()))
    .y_scale(Some(ui::chart_linear_scale(0.0, 320.0)))
    .grid(ticks.clone())
    .legend(true)
    .accessibility_label(Some("Bar chart".into()))
    .app_class(Some("w-full".into()))
    .child(ui::chart_bar("desktop", "chart-1"))
    .child(ui::chart_bar("mobile", "chart-2"));
    let area = ui::chart_container(vec![ui::ChartConfigItem::new("desktop", "Desktop", "chart-1")])
        .data(chart_rows())
        .labels(labels)
        .x_axis_labels(Some(short))
        .y_scale(Some(ui::chart_linear_scale(0.0, 320.0)))
        .grid(ticks)
        .cursor(false)
        .tooltip_indicator(ui::ChartTooltipIndicator::Line)
        .margin_x(px(12.0))
        .accessibility_label(Some("Area chart".into()))
        .app_class(Some("w-full".into()))
        .child(ui::chart_area("desktop", "chart-1"));
    let item = ui::ChartTooltipItem::new;
    let tooltips = view()
        .flex_row()
        .items_start()
        .gap_8()
        .child(
            ui::chart_tooltip_content()
                .label(Some("Page Views".into()))
                .items(vec![item("Desktop", 186.0, "chart-1"), item("Mobile", 80.0, "chart-2")])
                .app_class(Some("w-[8rem]".into())),
        )
        .child(
            ui::chart_tooltip_content()
                .label(Some("Browser".into()))
                .hide_label(true)
                .indicator(ui::ChartTooltipIndicator::Dashed)
                .items(vec![item("Chrome", 1286.0, "chart-3"), item("Firefox", 1000.0, "chart-4")])
                .app_class(Some("w-[8rem]".into())),
        )
        .child(
            ui::chart_tooltip_content()
                .label(Some("Page Views".into()))
                .indicator(ui::ChartTooltipIndicator::Line)
                .items(vec![item("Desktop", 12486.0, "chart-3")])
                .app_class(Some("w-[9rem]".into())),
        )
        .child(
            ui::chart_tooltip_content()
                .label(Some("Browser".into()))
                .hide_label(true)
                .items(vec![item("Chrome", 1286.0, "chart-1")])
                .app_class(Some("w-[8rem]".into())),
        );
    view()
        .gap_8()
        .child(
            view()
                .flex_row()
                .gap_8()
                .child(section("Bar chart", div("w-[392px]").child(bars)))
                .child(section("Area chart", div("w-[392px]").child(area))),
        )
        .child(section("Tooltip", tooltips))
}

fn sidebar_page() -> View {
    use ui::IconName as I;
    let items = [
        ("Home", I::House),
        ("Inbox", I::Inbox),
        ("Calendar", I::Calendar),
        ("Search", I::Search),
        ("Settings", I::Settings),
    ];
    let mut menu = ui::sidebar_menu();
    for (i, (label, icon)) in items.into_iter().enumerate() {
        menu = menu.child(
            ui::sidebar_menu_item()
                .child(ui::sidebar_menu_button(label).icon(Some(icon)).is_active(i == 0)),
        );
    }
    let sidebar = ui::sidebar()
        .variant(ui::SidebarVariant::Inset)
        .collapsible(ui::SidebarCollapsible::Icon)
        .child(
            ui::sidebar_header().child(
                ui::sidebar_menu().child(
                    ui::sidebar_menu_item().child(
                        ui::sidebar_menu_button("Acme Inc")
                            .label(None)
                            .size(ui::SidebarMenuButtonSize::Lg)
                            .child(
                                div(
                                    "flex aspect-square size-8 items-center justify-center rounded-lg \
                                     bg-sidebar-primary text-sidebar-primary-foreground",
                                )
                                .child(ui::icon(I::GalleryVerticalEnd)),
                            )
                            .child(
                                div("flex flex-1 flex-col text-left text-sm leading-tight")
                                    .child(span("Acme Inc", "truncate font-medium"))
                                    .child(span("Enterprise", "truncate text-xs")),
                            )
                            .child(ui::icon(I::ChevronsUpDown).app_class(Some("ml-auto".into()))),
                    ),
                ),
            ),
        )
        .child(
            ui::sidebar_content().child(
                ui::sidebar_group()
                    .child(ui::sidebar_group_label("Application"))
                    .child(ui::sidebar_group_content().child(menu)),
            ),
        )
        .child(ui::sidebar_separator())
        .child(
            ui::sidebar_footer().child(
                ui::sidebar_menu().child(
                    ui::sidebar_menu_item()
                        .child(ui::sidebar_menu_button("shadcn").icon(Some(I::User))),
                ),
            ),
        );
    let inset = ui::sidebar_inset()
        .child(
            div("flex h-16 shrink-0 items-center gap-2 px-4")
                .child(ui::sidebar_trigger().app_class(Some("-ml-1 size-7".into())))
                .child(div("mr-2 h-4 w-px shrink-0 bg-border"))
                .child(span("Dashboard", "text-sm text-foreground")),
        )
        .child(
            div("flex flex-1 flex-col gap-4 p-4 pt-0")
                .child(
                    div("grid auto-rows-min gap-4 grid-cols-3")
                        .child(div("aspect-video rounded-xl bg-muted/50"))
                        .child(div("aspect-video rounded-xl bg-muted/50"))
                        .child(div("aspect-video rounded-xl bg-muted/50")),
                )
                .child(div("flex-1 rounded-xl bg-muted/50")),
        );
    view().child(
        ui::sidebar_provider()
            .app_class(Some("!min-h-0 h-[508px] rounded-lg border border-border".into()))
            .child(sidebar)
            .child(inset),
    )
}
