//! Dropdown Menu, Context Menu, Menubar, Select, Combobox, Command and
//! Navigation Menu, each as shadcn's docs show it (its `-demo` example).

use nsh_base::theme;
use nsh_core::prelude::*;
use nsh_shadcn as ui;

use super::Page;

pub fn pages() -> Vec<Page> {
    vec![
        Page { id: "dropdown-menu", title: "Dropdown Menu", body: dropdown_menu },
        Page { id: "context-menu", title: "Context Menu", body: context_menu },
        Page { id: "menubar", title: "Menubar", body: menubar },
        Page { id: "select", title: "Select", body: select },
        Page { id: "combobox", title: "Combobox", body: combobox },
        Page { id: "command", title: "Command", body: command },
        Page { id: "navigation-menu", title: "Navigation Menu", body: navigation_menu },
    ]
}

/// A line under a demo saying what was chosen last.
fn chosen(last: Signal<SharedString>) -> View {
    view().child(
        text(move || format!("Chosen: {}", last.get()))
            .text_sm()
            .text_color(move || theme().muted_foreground),
    )
}

/// `dropdown-menu-demo.tsx`.
fn dropdown_menu() -> View {
    let last = signal(SharedString::from("none"));
    let item = move |label: &'static str| {
        ui::dropdown_menu_item(label).on_select(move || last.set(label.into()))
    };
    view()
        .gap_4()
        .items_start()
        .child(
            ui::dropdown_menu()
                .align_start()
                .content_class(Some("w-56".into()))
                .trigger(ui::dropdown_menu_trigger("Open").outline())
                .child(ui::dropdown_menu_label("My Account"))
                .child(item("Profile").child(ui::dropdown_menu_shortcut("⇧⌘P")))
                .child(item("Billing").child(ui::dropdown_menu_shortcut("⌘B")))
                .child(item("Settings").child(ui::dropdown_menu_shortcut("⌘S")))
                .child(item("Keyboard shortcuts").child(ui::dropdown_menu_shortcut("⌘K")))
                .child(ui::dropdown_menu_separator())
                .child(item("Team"))
                .child(
                    ui::dropdown_menu_sub("Invite users")
                        .child(item("Email"))
                        .child(item("Message"))
                        .child(ui::dropdown_menu_separator())
                        .child(item("More...")),
                )
                .child(item("New Team").child(ui::dropdown_menu_shortcut("⌘+T")))
                .child(ui::dropdown_menu_separator())
                .child(item("GitHub"))
                .child(item("Support"))
                .child(item("API").disabled(true))
                .child(ui::dropdown_menu_separator())
                .child(item("Log out").child(ui::dropdown_menu_shortcut("⇧⌘Q"))),
        )
        .child(chosen(last))
}

/// `context-menu-demo.tsx`.
fn context_menu() -> View {
    let last = signal(SharedString::from("none"));
    let bookmarks = signal(true);
    let urls = signal(false);
    let person = signal("pedro");
    let item = move |label: &'static str| {
        ui::context_menu_item(label).on_select(move || last.set(label.into()))
    };
    let radio = move |label: &'static str, value: &'static str| {
        ui::context_menu_radio_item(label)
            .checked(move || person.get() == value)
            .on_select(move || person.set(value))
    };
    view()
        .gap_4()
        .items_start()
        .child(
            ui::context_menu()
                .content_class(Some("w-52".into()))
                .trigger(
                    view()
                        .app_class(Some(SharedString::from(
                            "flex h-[150px] w-[300px] items-center justify-center rounded-md border \
                             border-border border-dashed text-sm",
                        )))
                        .child(text("Right click here")),
                )
                .child(item("Back").inset(true).child(ui::context_menu_shortcut("⌘[")))
                .child(
                    item("Forward")
                        .inset(true)
                        .disabled(true)
                        .child(ui::context_menu_shortcut("⌘]")),
                )
                .child(item("Reload").inset(true).child(ui::context_menu_shortcut("⌘R")))
                .child(
                    ui::context_menu_sub("More Tools")
                        .inset(true)
                        .content_class(Some("w-44".into()))
                        .child(item("Save Page..."))
                        .child(item("Create Shortcut..."))
                        .child(item("Name Window..."))
                        .child(ui::context_menu_separator())
                        .child(item("Developer Tools"))
                        .child(ui::context_menu_separator())
                        .child(item("Delete").destructive()),
                )
                .child(ui::context_menu_separator())
                .child(
                    ui::context_menu_checkbox_item("Show Bookmarks")
                        .checked(bookmarks)
                        .on_checked_change(move |on: &bool| bookmarks.set(*on)),
                )
                .child(
                    ui::context_menu_checkbox_item("Show Full URLs")
                        .checked(urls)
                        .on_checked_change(move |on: &bool| urls.set(*on)),
                )
                .child(ui::context_menu_separator())
                .child(ui::context_menu_label("People").inset(true))
                .child(radio("Pedro Duarte", "pedro"))
                .child(radio("Colm Tuite", "colm")),
        )
        .child(
            text(move || {
                format!(
                    "Chosen: {}, bookmarks {}, URLs {}, {}",
                    last.get(),
                    if bookmarks.get() { "on" } else { "off" },
                    if urls.get() { "on" } else { "off" },
                    person.get()
                )
            })
            .text_sm()
            .text_color(move || theme().muted_foreground),
        )
}

/// `menubar-demo.tsx`.
fn menubar() -> View {
    let last = signal(SharedString::from("none"));
    let bookmarks = signal(false);
    let urls = signal(true);
    let profile = signal("benoit");
    let item = move |label: &'static str| {
        ui::menubar_item(label).on_select(move || last.set(label.into()))
    };
    let radio = move |label: &'static str, value: &'static str| {
        ui::menubar_radio_item(label)
            .checked(move || profile.get() == value)
            .on_select(move || profile.set(value))
    };
    let check = move |label: &'static str, on: Signal<bool>| {
        ui::menubar_checkbox_item(label)
            .checked(on)
            .on_checked_change(move |now: &bool| on.set(*now))
    };
    view()
        .gap_4()
        .items_start()
        .child(
            ui::menubar()
                .child(
                    ui::menubar_menu("File")
                        .child(item("New Tab").child(ui::menubar_shortcut("⌘T")))
                        .child(item("New Window").child(ui::menubar_shortcut("⌘N")))
                        .child(item("New Incognito Window").disabled(true))
                        .child(ui::menubar_separator())
                        .child(
                            ui::menubar_sub("Share")
                                .child(item("Email link"))
                                .child(item("Messages"))
                                .child(item("Notes")),
                        )
                        .child(ui::menubar_separator())
                        .child(item("Print...").child(ui::menubar_shortcut("⌘P"))),
                )
                .child(
                    ui::menubar_menu("Edit")
                        .child(item("Undo").child(ui::menubar_shortcut("⌘Z")))
                        .child(item("Redo").child(ui::menubar_shortcut("⇧⌘Z")))
                        .child(ui::menubar_separator())
                        .child(
                            ui::menubar_sub("Find")
                                .child(item("Search the web"))
                                .child(ui::menubar_separator())
                                .child(item("Find..."))
                                .child(item("Find Next"))
                                .child(item("Find Previous")),
                        )
                        .child(ui::menubar_separator())
                        .child(item("Cut"))
                        .child(item("Copy"))
                        .child(item("Paste")),
                )
                .child(
                    ui::menubar_menu("View")
                        .child(check("Always Show Bookmarks Bar", bookmarks))
                        .child(check("Always Show Full URLs", urls))
                        .child(ui::menubar_separator())
                        .child(item("Reload").inset(true).child(ui::menubar_shortcut("⌘R")))
                        .child(
                            item("Force Reload")
                                .inset(true)
                                .disabled(true)
                                .child(ui::menubar_shortcut("⇧⌘R")),
                        )
                        .child(ui::menubar_separator())
                        .child(item("Toggle Fullscreen").inset(true))
                        .child(ui::menubar_separator())
                        .child(item("Hide Sidebar").inset(true)),
                )
                .child(
                    ui::menubar_menu("Profiles")
                        .child(radio("Andy", "andy"))
                        .child(radio("Benoit", "benoit"))
                        .child(radio("Luis", "luis"))
                        .child(ui::menubar_separator())
                        .child(item("Edit...").inset(true))
                        .child(ui::menubar_separator())
                        .child(item("Add Profile...").inset(true)),
                ),
        )
        .child(chosen(last))
}

/// `select-demo.tsx`.
fn select() -> View {
    let value = signal(None::<SharedString>);
    let fruits = [
        ("apple", "Apple"),
        ("banana", "Banana"),
        ("blueberry", "Blueberry"),
        ("grapes", "Grapes"),
        ("pineapple", "Pineapple"),
    ];
    view()
        .gap_4()
        .items_start()
        .child(
            ui::select(
                fruits
                    .iter()
                    .map(|(value, title)| ui::SelectItem::new(*value, *title).group("Fruits"))
                    .collect(),
            )
            .placeholder("Select a fruit")
            .accessibility_label("Fruit")
            .app_class(Some(SharedString::from("w-[180px]")))
            .on_value_change(move |next: &Option<SharedString>| value.set(next.clone())),
        )
        .child(
            text(move || format!("Value: {}", value.get().as_deref().unwrap_or("none")))
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
}

/// The docs' combobox demo (`combobox-demo.tsx` of the Base UI styles,
/// shadcn's `combobox.tsx`).
fn combobox() -> View {
    let value = signal(None::<SharedString>);
    let frameworks = ["Next.js", "SvelteKit", "Nuxt.js", "Remix", "Astro"];
    view()
        .gap_4()
        .items_start()
        .child(
            ui::combobox(frameworks.iter().map(|f| ui::SelectItem::new(*f, *f)).collect())
                .placeholder("Select a framework")
                .empty("No items found.")
                .accessibility_label("Framework")
                .test_id("framework")
                .app_class(Some(SharedString::from("w-64")))
                .on_value_change(move |next: &Option<SharedString>| value.set(next.clone())),
        )
        .child(
            text(move || format!("Value: {}", value.get().as_deref().unwrap_or("none")))
                .text_sm()
                .text_color(move || theme().muted_foreground),
        )
}

/// `command-demo.tsx`.
fn command() -> View {
    use ui::{CommandItem, IconName};
    let last = signal(SharedString::from("none"));
    let items = vec![
        CommandItem::new("calendar", "Calendar").icon(IconName::Calendar).group("Suggestions"),
        CommandItem::new("emoji", "Search Emoji").icon(IconName::Smile).group("Suggestions"),
        CommandItem::new("calculator", "Calculator")
            .icon(IconName::Calculator)
            .group("Suggestions")
            .disabled(true),
        CommandItem::new("profile", "Profile").icon(IconName::User).shortcut("⌘P").group("Settings"),
        CommandItem::new("billing", "Billing")
            .icon(IconName::CreditCard)
            .shortcut("⌘B")
            .group("Settings"),
        CommandItem::new("settings", "Settings")
            .icon(IconName::Settings)
            .shortcut("⌘S")
            .group("Settings"),
    ];
    view()
        .gap_4()
        .items_start()
        .child(
            ui::command(items)
                .placeholder("Type a command or search...")
                .empty("No results found.")
                .accessibility_label("Commands")
                .app_class(Some(SharedString::from(
                    "rounded-lg border border-border shadow-md md:min-w-[450px]",
                )))
                .on_select(move |id: &SharedString| last.set(id.clone())),
        )
        .child(chosen(last))
}

/// A view with Tailwind classes.
fn class(classes: &'static str) -> View {
    view().app_class(Some(SharedString::from(classes)))
}

/// A text with Tailwind classes.
fn line(words: &'static str, classes: &'static str) -> Text {
    text(words).app_class(Some(SharedString::from(classes)))
}

/// `navigation-menu-demo.tsx` (its desktop look: `viewport={false}`).
fn navigation_menu() -> View {
    use ui::IconName;
    let last = signal(SharedString::from("none"));
    let link = move |title: &'static str| {
        ui::navigation_menu_link().on_select(move || last.set(title.into()))
    };
    // `ListItem`: a title and two lines of description.
    let list_item = move |title: &'static str, description: &'static str| {
        view().child(
            link(title)
                .child(line(title, "text-sm leading-none font-medium"))
                .child(line(description, "line-clamp-2 text-sm leading-snug text-muted-foreground")),
        )
    };
    let components = [
        ("Alert Dialog", "A modal dialog that interrupts the user with important content and expects a response."),
        ("Hover Card", "For sighted users to preview content available behind a link."),
        ("Progress", "Displays an indicator showing the completion progress of a task, typically displayed as a progress bar."),
        ("Scroll-area", "Visually or semantically separates content."),
        ("Tabs", "A set of layered sections of content—known as tab panels—that are displayed one at a time."),
        ("Tooltip", "A popup that displays information related to an element when the element receives keyboard focus or the mouse hovers over it."),
    ];
    let mut components_grid = class("grid gap-2 sm:w-[400px] md:w-[500px] md:grid-cols-2 lg:w-[600px]");
    for (title, description) in components {
        components_grid = components_grid.child(list_item(title, description));
    }
    let titled = move |title: &'static str, description: &'static str| {
        link(title)
            .child(line(title, "font-medium"))
            .child(line(description, "text-muted-foreground"))
    };
    let with_icon = move |icon: IconName, title: &'static str| {
        link(title)
            .app_class(Some(SharedString::from("flex-row items-center gap-2")))
            .child(ui::icon(icon).app_class(Some(SharedString::from("size-4 text-muted-foreground"))))
            .child(text(title))
    };
    view()
        .gap_4()
        .items_start()
        .child(
            ui::navigation_menu().child(
                ui::navigation_menu_list()
                    .app_class(Some(SharedString::from("flex-wrap")))
                    .child(
                        ui::navigation_menu_item("Home").child(
                            class("grid gap-2 md:w-[400px] lg:w-[500px] lg:grid-cols-[.75fr_1fr]")
                                .child(
                                    class("row-span-3").child(
                                        link("shadcn/ui")
                                            .app_class(Some(SharedString::from(
                                                "flex h-full w-full flex-col justify-end rounded-md \
                                                 bg-linear-to-b from-muted/50 to-muted p-4 outline-hidden \
                                                 transition-all duration-200 select-none focus:shadow-md md:p-6",
                                            )))
                                            .child(line("shadcn/ui", "mb-2 text-lg font-medium sm:mt-4"))
                                            .child(line(
                                                "Beautifully designed components built with Tailwind CSS.",
                                                "text-sm leading-tight text-muted-foreground",
                                            )),
                                    ),
                                )
                                .child(list_item(
                                    "Introduction",
                                    "Re-usable components built using Radix UI and Tailwind CSS.",
                                ))
                                .child(list_item(
                                    "Installation",
                                    "How to install dependencies and structure your app.",
                                ))
                                .child(list_item(
                                    "Typography",
                                    "Styles for headings, paragraphs, lists...etc",
                                )),
                        ),
                    )
                    .child(ui::navigation_menu_item("Components").child(components_grid))
                    .child(
                        ui::navigation_menu_link_item().child(
                            link("Docs")
                                .app_class(Some(SharedString::from(ui::NAVIGATION_MENU_TRIGGER_STYLE)))
                                .child(text("Docs")),
                        ),
                    )
                    .child(
                        ui::navigation_menu_item("List").child(
                            class("grid w-[300px] gap-4").child(
                                view()
                                    .child(titled("Components", "Browse all components in the library."))
                                    .child(titled("Documentation", "Learn how to use the library."))
                                    .child(titled("Blog", "Read our latest blog posts.")),
                            ),
                        ),
                    )
                    .child(
                        ui::navigation_menu_item("Simple").child(
                            class("grid w-[200px] gap-4").child(
                                view()
                                    .child(link("Components").child(text("Components")))
                                    .child(link("Documentation").child(text("Documentation")))
                                    .child(link("Blocks").child(text("Blocks"))),
                            ),
                        ),
                    )
                    .child(
                        ui::navigation_menu_item("With Icon").child(
                            class("grid w-[200px] gap-4").child(
                                view()
                                    .child(with_icon(IconName::CircleHelp, "Backlog"))
                                    .child(with_icon(IconName::Circle, "To Do"))
                                    .child(with_icon(IconName::CircleCheck, "Done")),
                            ),
                        ),
                    ),
            ),
        )
        .child(chosen(last))
}
