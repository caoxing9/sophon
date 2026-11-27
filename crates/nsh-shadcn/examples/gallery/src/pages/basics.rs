//! The basics family's pages: Badge, Card, Separator, Label, Kbd, Skeleton,
//! Spinner, Alert, Avatar, Aspect Ratio, Empty, Item, Breadcrumb,
//! Pagination, Progress and Table, each with the examples shadcn's docs
//! show.

use nsh_base::theme;
use nsh_core::element::{ImageSrc, input};
use nsh_core::prelude::*;
use nsh_shadcn::{self as ui, IconName};

use super::{Page, row, section};

pub fn pages() -> Vec<Page> {
    vec![
        Page {
            id: "badge",
            title: "Badge",
            body: || body(badge()),
        },
        Page {
            id: "card",
            title: "Card",
            body: || body(card()),
        },
        Page {
            id: "separator",
            title: "Separator",
            body: || body(separator()),
        },
        Page {
            id: "label",
            title: "Label",
            body: || body(label()),
        },
        Page {
            id: "kbd",
            title: "Kbd",
            body: || body(kbd()),
        },
        Page {
            id: "skeleton",
            title: "Skeleton",
            body: || body(skeleton()),
        },
        Page {
            id: "spinner",
            title: "Spinner",
            body: || body(spinner()),
        },
        Page {
            id: "alert",
            title: "Alert",
            body: || body(alert()),
        },
        Page {
            id: "avatar",
            title: "Avatar",
            body: || body(avatar()),
        },
        Page {
            id: "aspect-ratio",
            title: "Aspect Ratio",
            body: || body(aspect_ratio()),
        },
        Page {
            id: "empty",
            title: "Empty",
            body: || body(empty()),
        },
        Page {
            id: "item",
            title: "Item",
            body: || body(item()),
        },
        Page {
            id: "breadcrumb",
            title: "Breadcrumb",
            body: || body(breadcrumb()),
        },
        Page {
            id: "pagination",
            title: "Pagination",
            body: || body(pagination()),
        },
        Page {
            id: "progress",
            title: "Progress",
            body: || body(progress()),
        },
        Page {
            id: "table",
            title: "Table",
            body: || body(table()),
        },
    ]
}

/// A page's body at the browser's base text size (16 points, 24 tall), which
/// shadcn's components inherit from the page's `<body>` in
/// `tools/reference/<id>.html`; NSH's default text is smaller.
fn body(content: View) -> View {
    view().app_class(cls("text-base")).child(content)
}

fn cls(classes: &'static str) -> Option<SharedString> {
    Some(SharedString::from(classes))
}

/// shadcn's Input classes (the forms family's), drawn here for the card's
/// form: NSH has no `transition-[color,box-shadow]`, `file:` or
/// `selection:text-*`.
const INPUT: &str = "h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-1 \
    text-base shadow-xs outline-none placeholder:text-muted-foreground \
    disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm \
    dark:bg-input/30 focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50";

/// shadcn's Checkbox classes (the forms family's), on the box of
/// `nsh_base`'s checkbox, for the label's example.
const CHECKBOX: &str = "size-4 shrink-0 rounded-[4px] border border-input shadow-xs transition-shadow \
    outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 \
    disabled:cursor-not-allowed disabled:opacity-50 checked:border-primary checked:bg-primary \
    checked:text-primary-foreground dark:bg-input/30 dark:checked:bg-primary \
    items-center justify-center";

// ---- Badge -----------------------------------------------------------------

fn badge() -> View {
    let count = "h-5 min-w-5 rounded-full px-1 font-mono tabular-nums";
    view()
        .gap_8()
        .child(section(
            "Variants",
            row()
                .child(ui::badge("Badge"))
                .child(ui::badge("Secondary").secondary())
                .child(ui::badge("Destructive").destructive())
                .child(ui::badge("Outline").outline()),
        ))
        .child(section(
            "With icon and counts",
            row()
                .child(
                    ui::badge("Verified")
                        .secondary()
                        .icon(Some(IconName::BadgeCheck))
                        .app_class(cls("bg-blue-500 text-white dark:bg-blue-600")),
                )
                .child(ui::badge("8").app_class(cls(count)))
                .child(ui::badge("99").destructive().app_class(cls(count)))
                .child(ui::badge("20+").outline().app_class(cls(count))),
        ))
        .child(section(
            "With spinner",
            row()
                .gap_4()
                .child(spinner_badge("Syncing", ui::BadgeVariant::Default))
                .child(spinner_badge("Updating", ui::BadgeVariant::Secondary))
                .child(spinner_badge("Processing", ui::BadgeVariant::Outline)),
        ))
}

fn spinner_badge(label: &'static str, variant: ui::BadgeVariant) -> ui::BadgeElement {
    ui::badge(label)
        .variant(variant)
        .label(None::<SharedString>)
        .child(ui::spinner().app_class(cls("size-3")))
        .child(text(label))
}

// ---- Card ------------------------------------------------------------------

fn card() -> View {
    let field =
        |name: &'static str, input: AnyElement| view().gap_2().child(ui::label(name)).child(input);
    view().child(
        ui::card()
            .app_class(cls("w-full max-w-sm"))
            .child(
                ui::card_header()
                    .child(ui::card_title("Login to your account"))
                    .child(ui::card_description(
                        "Enter your email below to login to your account",
                    ))
                    .action(ui::card_action().child(ui::button("Sign Up").link())),
            )
            .child(
                ui::card_content().child(
                    view()
                        .gap_6()
                        .child(field(
                            "Email",
                            input().placeholder("m@example.com").app_class(cls(INPUT)).into_element(),
                        ))
                        .child(
                            view()
                                .gap_2()
                                .child(
                                    view()
                                        .flex_row()
                                        .items_center()
                                        .child(ui::label("Password"))
                                        .child(
                                            nsh_base::link()
                                                .href("#")
                                                .app_class(cls(
                                                    "ml-auto inline-block text-sm underline-offset-4 hover:underline",
                                                ))
                                                .child(text("Forgot your password?")),
                                        ),
                                )
                                .child(input().masked(true).app_class(cls(INPUT))),
                        ),
                ),
            )
            .child(
                ui::card_footer()
                    .app_class(cls("flex-col gap-2"))
                    .child(ui::button("Login").app_class(cls("w-full")))
                    .child(ui::button("Login with Google").outline().app_class(cls("w-full"))),
            ),
    )
}

// ---- Separator -------------------------------------------------------------

fn separator() -> View {
    view()
        .child(
            view()
                .app_class(cls("space-y-1"))
                .child(text("Radix Primitives").app_class(cls("text-sm leading-none font-medium")))
                .child(
                    text("An open-source UI component library.")
                        .app_class(cls("text-sm text-muted-foreground")),
                ),
        )
        .child(ui::separator().app_class(cls("my-4")))
        .child(
            view()
                .app_class(cls("flex h-5 items-center space-x-4 text-sm"))
                .child(text("Blog"))
                .child(ui::separator().vertical())
                .child(text("Docs"))
                .child(ui::separator().vertical())
                .child(text("Source")),
        )
}

// ---- Label -----------------------------------------------------------------

fn label() -> View {
    let accepted = signal(false);
    let toggle = move || accepted.update(|a| *a = !*a);
    view().child(
        view()
            .app_class(cls("flex items-center space-x-2"))
            .child(
                nsh_base::checkbox()
                    .checked(accepted)
                    .accessibility_label("Accept terms and conditions")
                    .indicator_class(cls(CHECKBOX))
                    .indicator(dynamic(move || {
                        accepted.get().then(|| {
                            ui::icon(IconName::Check)
                                .app_class(cls("size-3.5"))
                                .into_element()
                        })
                    }))
                    .on_change(move |checked: &bool| accepted.set(*checked)),
            )
            .child(ui::label("Accept terms and conditions").on_press(toggle)),
    )
}

// ---- Kbd -------------------------------------------------------------------

fn kbd() -> View {
    view()
        .gap_8()
        .child(section(
            "Demo",
            view()
                .app_class(cls("flex flex-col items-start gap-4"))
                .child(
                    ui::kbd_group()
                        .child(ui::kbd("⌘"))
                        .child(ui::kbd("⇧"))
                        .child(ui::kbd("⌥"))
                        .child(ui::kbd("⌃")),
                )
                .child(
                    ui::kbd_group()
                        .child(ui::kbd("Ctrl"))
                        .child(text("+"))
                        .child(ui::kbd("B")),
                ),
        ))
        .child(section(
            "Group",
            view().child(
                text("Use ")
                    .app_class(cls("text-sm text-muted-foreground"))
                    .child(
                        ui::kbd_group()
                            .child(ui::kbd("Ctrl + B"))
                            .child(ui::kbd("Ctrl + K")),
                    )
                    .child(" to open the command palette"),
            ),
        ))
        .child(section(
            "Button",
            row()
                .gap_4()
                .child(
                    ui::button("Accept")
                        .outline()
                        .sm()
                        .app_class(cls("pr-2"))
                        .child(ui::kbd("⏎")),
                )
                .child(
                    ui::button("Cancel")
                        .outline()
                        .sm()
                        .app_class(cls("pr-2"))
                        .child(ui::kbd("Esc")),
                ),
        ))
}

// ---- Skeleton --------------------------------------------------------------

fn skeleton() -> View {
    view()
        .gap_8()
        .child(section(
            "Demo",
            view()
                .app_class(cls("flex flex-row items-center space-x-4"))
                .child(ui::skeleton().app_class(cls("h-12 w-12 rounded-full")))
                .child(
                    view()
                        .app_class(cls("space-y-2"))
                        .child(ui::skeleton().app_class(cls("h-4 w-[250px]")))
                        .child(ui::skeleton().app_class(cls("h-4 w-[200px]"))),
                ),
        ))
        .child(section(
            "Card",
            view()
                .app_class(cls("flex flex-col space-y-3"))
                .child(ui::skeleton().app_class(cls("h-[125px] w-[250px] rounded-xl")))
                .child(
                    view()
                        .app_class(cls("space-y-2"))
                        .child(ui::skeleton().app_class(cls("h-4 w-[250px]")))
                        .child(ui::skeleton().app_class(cls("h-4 w-[200px]"))),
                ),
        ))
}

// ---- Spinner ---------------------------------------------------------------

fn spinner() -> View {
    view()
        .gap_8()
        .child(section(
            "Demo",
            view()
                .app_class(cls("flex w-full max-w-xs flex-col gap-4"))
                .child(
                    ui::item()
                        .muted()
                        .child(ui::item_media().child(ui::spinner()))
                        .child(ui::item_content().child(
                            ui::item_title("Processing payment...").app_class(cls("line-clamp-1")),
                        ))
                        .child(
                            ui::item_content()
                                .app_class(cls("flex-none justify-end"))
                                .child(text("$100.00").app_class(cls("text-sm tabular-nums"))),
                        ),
                ),
        ))
        .child(section(
            "Sizes",
            row()
                .gap_6()
                .child(ui::spinner().app_class(cls("size-3")))
                .child(ui::spinner().app_class(cls("size-4")))
                .child(ui::spinner().app_class(cls("size-6")))
                .child(ui::spinner().app_class(cls("size-8"))),
        ))
        .child(section(
            "Button",
            row()
                .gap_4()
                .child(spinner_button("Loading...", ui::ButtonVariant::Default))
                .child(spinner_button("Please wait", ui::ButtonVariant::Outline))
                .child(spinner_button("Processing", ui::ButtonVariant::Secondary)),
        ))
}

/// shadcn's `<Button disabled size="sm"><Spinner />{label}</Button>`: the
/// spinner goes before the label, as the button's children (its
/// `has-[>svg]:px-2.5` written out).
fn spinner_button(label: &'static str, variant: ui::ButtonVariant) -> ui::ButtonElement {
    ui::button(label)
        .variant(variant)
        .sm()
        .disabled(true)
        .label(None::<SharedString>)
        .accessibility_label(Some(SharedString::from(label)))
        .app_class(cls("px-2.5"))
        .child(ui::spinner())
        .child(text(label))
}

// ---- Alert -----------------------------------------------------------------

fn alert() -> View {
    view()
        .app_class(cls("grid w-full max-w-xl items-start gap-4"))
        .children([
            ui::alert()
                .icon(Some(IconName::CircleCheck))
                .child(ui::alert_title("Success! Your changes have been saved"))
                .child(ui::alert_description(
                    "This is an alert with icon, title and description.",
                ))
                .into_element(),
            ui::alert()
                .icon(Some(IconName::Popcorn))
                .child(ui::alert_title(
                    "This Alert has a title and an icon. No description.",
                ))
                .into_element(),
            ui::alert()
                .destructive()
                .icon(Some(IconName::CircleAlert))
                .child(ui::alert_title("Unable to process your payment."))
                .child(
                    ui::alert_description("")
                        .child(
                            text("Please verify your billing information and try again.")
                                .app_class(cls("leading-relaxed")),
                        )
                        .child(
                            view()
                                .app_class(cls("text-sm"))
                                .child(bullet("Check your card details"))
                                .child(bullet("Ensure sufficient funds"))
                                .child(bullet("Verify billing address")),
                        ),
                )
                .into_element(),
        ])
}

/// A line of a `list-inside list-disc` list (NSH has no list markers): the
/// disc, then the text where the browser puts it.
fn bullet(line: &'static str) -> View {
    view()
        .flex_row()
        .child(text("•").app_class(cls("w-[19px] pl-[2px]")))
        .child(text(line))
}

// ---- Avatar ----------------------------------------------------------------

fn avatar() -> View {
    view()
        .gap_8()
        .child(section(
            "Demo",
            view()
                .app_class(cls("flex flex-row flex-wrap items-center gap-12"))
                .child(ui::avatar("CN").alt("@shadcn"))
                .child(
                    ui::avatar("ER")
                        .alt("@evilrabbit")
                        .app_class(cls("rounded-lg")),
                )
                .child(
                    ui::avatar_group()
                        .app_class(cls("grayscale"))
                        .child(ui::avatar("CN").alt("@shadcn"))
                        .child(ui::avatar("LR").alt("@maxleiter"))
                        .child(ui::avatar("ER").alt("@evilrabbit")),
                ),
        ))
        .child(section(
            "Sizes",
            row()
                .gap_4()
                .child(ui::avatar("CN").alt("@shadcn").sm())
                .child(ui::avatar("CN").alt("@shadcn"))
                .child(ui::avatar("CN").alt("@shadcn").lg()),
        ))
        .child(section(
            "Badge",
            row()
                .gap_6()
                .child(
                    ui::avatar("CN")
                        .alt("@shadcn")
                        .sm()
                        .child(ui::avatar_badge()),
                )
                .child(
                    ui::avatar("CN")
                        .alt("@shadcn")
                        .child(ui::avatar_badge().app_class(cls("bg-green-600 dark:bg-green-800"))),
                )
                .child(
                    ui::avatar("CN")
                        .alt("@shadcn")
                        .lg()
                        .child(ui::avatar_badge().icon(Some(IconName::Plus))),
                ),
        ))
        .child(section(
            "Group with count",
            row().child(
                ui::avatar_group()
                    .child(ui::avatar("CN").alt("@shadcn"))
                    .child(ui::avatar("LR").alt("@maxleiter"))
                    .child(ui::avatar("ER").alt("@evilrabbit"))
                    .child(ui::avatar_group_count("+3")),
            ),
        ))
}

// ---- Aspect Ratio ----------------------------------------------------------

/// A landscape, drawn for the page (a network photo would change the
/// screenshots); `tools/reference/aspect-ratio.html` shows the same image.
const PHOTO: &str = "data:image/png;base64,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";

fn aspect_ratio() -> View {
    view().child(
        ui::aspect_ratio(16.0 / 9.0)
            .app_class(cls("rounded-lg bg-muted"))
            .child(
                image(ImageSrc::from(PHOTO))
                    .accessible(true)
                    .role(AccessibilityRole::Image)
                    .label("Photo by Drew Beamer")
                    .app_class(cls(
                        "absolute inset-0 h-full w-full rounded-lg object-cover dark:brightness-[0.2] dark:grayscale",
                    )),
            ),
    )
}

// ---- Empty -----------------------------------------------------------------

fn empty() -> View {
    view()
        .app_class(cls("flex flex-row gap-6"))
        .child(
            ui::empty()
                .child(
                    ui::empty_header()
                        .child(
                            ui::empty_media()
                                .variant(ui::EmptyMediaVariant::Icon)
                                .icon(Some(IconName::FolderCode)),
                        )
                        .child(ui::empty_title("No Projects Yet"))
                        .child(ui::empty_description(
                            "You haven't created any projects yet. Get started by creating your first project.",
                        )),
                )
                .child(
                    ui::empty_content().child(
                        view()
                            .app_class(cls("flex flex-row gap-2"))
                            .child(ui::button("Create Project"))
                            .child(ui::button("Import Project").outline()),
                    ),
                )
                .child(
                    ui::button("Learn More")
                        .link()
                        .sm()
                        .app_class(cls("text-muted-foreground px-2.5"))
                        .child(ui::icon(IconName::ArrowUpRight)),
                ),
        )
        .child(
            ui::empty()
                .app_class(cls("border border-dashed border-border"))
                .child(
                    ui::empty_header()
                        .child(
                            ui::empty_media()
                                .variant(ui::EmptyMediaVariant::Icon)
                                .icon(Some(IconName::Cloud)),
                        )
                        .child(ui::empty_title("Cloud Storage Empty"))
                        .child(ui::empty_description(
                            "Upload files to your cloud storage to access them anywhere.",
                        )),
                )
                .child(ui::empty_content().child(ui::button("Upload Files").outline().sm())),
        )
}

// ---- Item ------------------------------------------------------------------

fn item() -> View {
    let open = |title: &'static str, description: &'static str| {
        ui::item_content()
            .child(ui::item_title(title))
            .child(ui::item_description(description))
    };
    view()
        .gap_8()
        .child(section(
            "Demo",
            view()
                .app_class(cls("flex w-full max-w-md flex-col gap-6"))
                .child(
                    ui::item()
                        .outline()
                        .child(open(
                            "Basic Item",
                            "A simple item with title and description.",
                        ))
                        .child(ui::item_actions().child(ui::button("Action").outline().sm())),
                )
                .child(
                    ui::item()
                        .outline()
                        .sm()
                        .href("#")
                        .child(
                            ui::item_media()
                                .icon(Some(IconName::BadgeCheck))
                                .icon_class("size-5"),
                        )
                        .child(
                            ui::item_content()
                                .child(ui::item_title("Your profile has been verified.")),
                        )
                        .child(ui::item_actions().child(ui::icon(IconName::ChevronRight))),
                ),
        ))
        .child(section(
            "Variants",
            view()
                .app_class(cls("flex flex-col gap-6"))
                .child(
                    ui::item()
                        .child(open(
                            "Default Variant",
                            "Standard styling with subtle background and borders.",
                        ))
                        .child(ui::item_actions().child(ui::button("Open").outline().sm())),
                )
                .child(
                    ui::item()
                        .outline()
                        .child(open(
                            "Outline Variant",
                            "Outlined style with clear borders and transparent background.",
                        ))
                        .child(ui::item_actions().child(ui::button("Open").outline().sm())),
                )
                .child(
                    ui::item()
                        .muted()
                        .child(open(
                            "Muted Variant",
                            "Subdued appearance with muted colors for secondary content.",
                        ))
                        .child(ui::item_actions().child(ui::button("Open").outline().sm())),
                ),
        ))
}

// ---- Breadcrumb ------------------------------------------------------------

fn breadcrumb() -> View {
    let opened = signal(SharedString::from("none"));
    let link = move |label: &'static str, href: &'static str| {
        ui::breadcrumb_item().child(
            ui::breadcrumb_link(label)
                .href(href)
                .on_press(move || opened.set(SharedString::from(label))),
        )
    };
    view()
        .gap_8()
        .child(section(
            "Demo",
            view().child(
                ui::breadcrumb().child(
                    ui::breadcrumb_list()
                        .child(link("Home", "/"))
                        .child(ui::breadcrumb_separator())
                        .child(
                            ui::breadcrumb_item()
                                .child(ui::breadcrumb_ellipsis().app_class(cls("size-4"))),
                        )
                        .child(ui::breadcrumb_separator())
                        .child(link("Components", "/docs/components"))
                        .child(ui::breadcrumb_separator())
                        .child(ui::breadcrumb_item().child(ui::breadcrumb_page("Breadcrumb"))),
                ),
            ),
        ))
        .child(section(
            "Custom separator",
            view().child(
                ui::breadcrumb().child(
                    ui::breadcrumb_list()
                        .child(link("Home", "/"))
                        .child(ui::breadcrumb_separator().with_icon(IconName::Slash))
                        .child(link("Components", "/components"))
                        .child(ui::breadcrumb_separator().with_icon(IconName::Slash))
                        .child(ui::breadcrumb_item().child(ui::breadcrumb_page("Breadcrumb"))),
                ),
            ),
        ))
        .child(
            text(move || format!("Opened: {}", opened.get()))
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
}

// ---- Pagination ------------------------------------------------------------

fn pagination() -> View {
    let page = signal(2_u32);
    let link = move |n: u32| {
        ui::pagination_item().child(
            ui::pagination_link(n.to_string())
                .href("#")
                .is_active(move || page.get() == n)
                .on_press(move || page.set(n)),
        )
    };
    view()
        .gap_8()
        .child(
            ui::pagination().child(
                ui::pagination_content()
                    .child(
                        ui::pagination_item().child(
                            ui::pagination_previous()
                                .href("#")
                                .on_press(move || page.update(|p| *p = p.saturating_sub(1).max(1))),
                        ),
                    )
                    .child(link(1))
                    .child(link(2))
                    .child(link(3))
                    .child(ui::pagination_item().child(ui::pagination_ellipsis()))
                    .child(
                        ui::pagination_item().child(
                            ui::pagination_next()
                                .href("#")
                                .on_press(move || page.update(|p| *p = (*p + 1).min(3))),
                        ),
                    ),
            ),
        )
        .child(
            text(move || format!("Page: {}", page.get()))
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
}

// ---- Progress --------------------------------------------------------------

fn progress() -> View {
    let value = signal(13.0_f32);
    set_timeout(std::time::Duration::from_millis(500), move || {
        value.set(66.0)
    });
    view().child(ui::progress(13.0).value(value).app_class(cls("w-[60%]")))
}

// ---- Table -----------------------------------------------------------------

const INVOICES: [(&str, &str, &str, &str); 7] = [
    ("INV001", "Paid", "$250.00", "Credit Card"),
    ("INV002", "Pending", "$150.00", "PayPal"),
    ("INV003", "Unpaid", "$350.00", "Bank Transfer"),
    ("INV004", "Paid", "$450.00", "Credit Card"),
    ("INV005", "Paid", "$550.00", "PayPal"),
    ("INV006", "Pending", "$200.00", "Bank Transfer"),
    ("INV007", "Unpaid", "$300.00", "Credit Card"),
];

/// The columns' widths: NSH has no table layout, so each cell of a column
/// takes its width (HTML's, for these rows at this width: Invoice
/// `w-[100px]`, the others shared by their content: 190, 292 and the
/// rest).
const COLUMNS: [&str; 4] = ["w-[100px]", "w-[190px]", "w-[292px]", "flex-1"];

fn table() -> View {
    let col = |i: usize, extra: &'static str| -> Option<SharedString> {
        Some(SharedString::from(format!("{} {extra}", COLUMNS[i])))
    };
    view().child(
        ui::table()
            .child(
                ui::table_header().child(
                    ui::table_row()
                        .child(ui::table_head("Invoice").app_class(col(0, "")))
                        .child(ui::table_head("Status").app_class(col(1, "")))
                        .child(ui::table_head("Method").app_class(col(2, "")))
                        .child(ui::table_head("Amount").app_class(col(3, "text-right"))),
                ),
            )
            .child(ui::table_body().children(INVOICES.iter().map(
                |&(invoice, status, amount, method)| {
                    ui::table_row()
                        .child(ui::table_cell(invoice).app_class(col(0, "font-medium")))
                        .child(ui::table_cell(status).app_class(col(1, "")))
                        .child(ui::table_cell(method).app_class(col(2, "")))
                        .child(ui::table_cell(amount).app_class(col(3, "text-right")))
                },
            )))
            .child(
                ui::table_footer().child(
                    ui::table_row()
                        .child(ui::table_cell("Total").app_class(cls("w-[582px]")))
                        .child(ui::table_cell("$2,500.00").app_class(col(3, "text-right"))),
                ),
            )
            .child(ui::table_caption("A list of your recent invoices.")),
    )
}
