//! Desktop keyboard shortcuts and the command palette (⌘K).

use crate::i18n::t;
use nsh_base::{CommandEntry, CommandItem, command};
use nsh_core::prelude::*;
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store::{self, ExplorerTab, Modal, Screen, Tab};

/// Binds the app's shortcuts for as long as the shell lives.
pub(crate) fn bind() {
    menu();
    for (keystroke, action) in keys(false) {
        on_key(&keystroke, move |_| action());
    }
    // The composer's editor, while it has the keyboard, takes every
    // Command and Control key for its own keymap and drops those it has no
    // command for, the menu's too: it runs these itself.
    crate::ui::shared::composer_doc::set_app_keys(keys(true));
}

/// The app's shortcuts and what they do; `with_menu` adds the menu items'
/// (the menu bar takes those itself, outside the editor).
fn keys(with_menu: bool) -> Vec<(String, std::rc::Rc<dyn Fn()>)> {
    fn key(k: &str, f: impl Fn() + 'static) -> (String, std::rc::Rc<dyn Fn()>) {
        (k.to_owned(), std::rc::Rc::new(f))
    }
    let mut out = vec![
        key("mod+k", || store::open_modal(Modal::Palette)),
        key("shift+mod+t", store::new_terminal),
        // VS Code's: ⌃` shows and hides the terminal panel (⌘J too), ⌃⇧`
        // opens another terminal.
        key("ctrl+`", store::toggle_terminal_panel),
        key("mod+j", store::toggle_terminal_panel),
        key("shift+ctrl+`", store::new_terminal),
        key("mod+s", store::save_current_file),
        key("mod+f", || {
            if store::selected_agent().is_some() {
                store::toggle_find();
            }
        }),
        key("mod+b", store::toggle_sidebar),
        key("mod+e", store::toggle_explorer),
    ];
    for n in 1..=9usize {
        out.push(key(&format!("mod+{n}"), move || {
            if let Some(tab) = store::tab_list().get(n - 1).cloned() {
                store::open_tab(tab);
            }
        }));
    }
    if with_menu {
        out.extend([
            key("mod+,", store::open_settings),
            key("mod+n", || store::open_new_workspace(None)),
            key("mod+t", new_chat),
            key("mod+w", store::close_current),
        ]);
    }
    out
}

fn new_chat() {
    if matches!(store::screen(), Screen::Workspace(_)) {
        store::open_tab(Tab::Draft);
    }
}

/// The menu bar, as a Mac app's: ⌘W closes the open tab (not the window),
/// and ⌘Q asks twice before quitting, as Chrome does; the Edit menu keeps
/// the text fields' own commands.
fn menu() {
    use nsh_core::desktop::MenuRole as R;
    use nsh_core::element::desktop::{MenuItem as M, set_application_menu};
    set_application_menu(vec![
        M::submenu(
            "Sophon",
            [
                M::role(R::About),
                M::separator(),
                M::new(t("Settings…")).shortcut("mod+,").on_select(store::open_settings),
                M::separator(),
                M::role(R::Hide),
                M::role(R::HideOthers),
                M::role(R::ShowAll),
                M::separator(),
                M::new(t("Quit Sophon")).shortcut("mod+q").on_select(|| store::request_quit("Press ⌘Q again to quit Sophon")),
            ],
        ),
        M::submenu(
            t("File"),
            [
                M::new(t("New workspace")).shortcut("mod+n").on_select(|| store::open_new_workspace(None)),
                M::new(t("New chat")).shortcut("mod+t").on_select(new_chat),
                M::separator(),
                M::new(t("Close tab")).shortcut("mod+w").on_select(store::close_current),
            ],
        ),
        M::submenu(
            t("Edit"),
            [
                M::role(R::Undo),
                M::role(R::Redo),
                M::separator(),
                M::role(R::Cut),
                M::role(R::Copy),
                M::role(R::Paste),
                M::role(R::SelectAll),
            ],
        ),
        M::submenu(t("View"), [M::role(R::ToggleFullScreen)]),
        M::submenu(t("Window"), [M::role(R::Minimize), M::role(R::Zoom)]),
    ]);
}

/// What the palette lists: actions, then workspaces and the open
/// workspace's agents.
fn items() -> Vec<CommandItem> {
    let mut items = vec![
        CommandItem::new("new-workspace", t("New workspace")).group(t("Actions")).shortcut("mod+n"),
        CommandItem::new("settings", t("Settings")).group(t("Actions")).shortcut("mod+,"),
        CommandItem::new("history", t("History")).group(t("Actions")).keywords(["sessions", "archive"]),
        CommandItem::new("schedules", t("Schedules")).group(t("Actions")).keywords(["cron", "automation"]),
        CommandItem::new("usage", t("Usage")).group(t("Actions")).keywords(["limits", "quota", "cost"]),
        CommandItem::new("import", t("Import a session")).group(t("Actions")).keywords(["claude", "codex", "resume"]),
        CommandItem::new("toggle-sidebar", t("Toggle sidebar")).group(t("Actions")).shortcut("mod+b"),
    ];
    if store::current_workspace().is_some() {
        items.push(CommandItem::new("new-agent", t("New agent")).group(t("Actions")).shortcut("mod+t"));
        items.push(CommandItem::new("new-terminal", t("New terminal")).group(t("Actions")).shortcut("shift+ctrl+`"));
        items.push(CommandItem::new("toggle-terminal", t("Toggle terminal")).group(t("Actions")).shortcut("ctrl+`"));
        items.push(CommandItem::new("toggle-explorer", t("Toggle explorer")).group(t("Actions")).shortcut("mod+e"));
        items.push(CommandItem::new("changes", t("Show changes")).group(t("Actions")).keywords(["diff", "git"]));
        items.push(CommandItem::new("files", t("Show files")).group(t("Actions")).keywords(["tree", "explorer"]));
        items.push(CommandItem::new("commit", t("Commit all changes")).group(t("Git")).keywords(["git"]));
        for a in store::current_workspace().map(|w| store::workspace_agents(&w.id)).unwrap_or_default() {
            items.push(CommandItem::new(format!("agent:{}", a.id), a.title.clone()).group(t("Agents")).keywords([a.provider]));
        }
    }
    for r in store::rows() {
        if let store::Row::Workspace(w) = r {
            items.push(
                CommandItem::new(format!("ws:{}", w.id), w.name.clone()).group(t("Workspaces")).keywords([w.project_name]),
            );
        }
    }
    items
}

fn run(id: &str) {
    store::close_modal();
    match id {
        "new-workspace" => store::open_new_workspace(None),
        "settings" => store::open_settings(),
        "history" => store::open_history(),
        "schedules" => store::open_schedules(),
        "usage" => store::open_usage(),
        "import" => {
            store::search_sessions("");
            store::open_modal(Modal::ImportSession);
        }
        "toggle-sidebar" => store::toggle_sidebar(),
        "new-agent" => new_chat(),
        "new-terminal" => store::new_terminal(),
        "toggle-terminal" => store::toggle_terminal_panel(),
        "toggle-explorer" => store::toggle_explorer(),
        "changes" => store::set_explorer_tab(ExplorerTab::Changes),
        "files" => store::set_explorer_tab(ExplorerTab::Files),
        "commit" => store::open_modal(Modal::Commit),
        _ => {
            if let Some(ws) = id.strip_prefix("ws:") {
                store::open_workspace(ws);
            } else if let Some(agent) = id.strip_prefix("agent:") {
                store::open_tab(Tab::Agent(agent.to_owned()));
            }
        }
    }
}

pub(crate) fn palette() -> impl IntoElement {
    glass_strong(view())
        .w(px(560.0))
        .max_w(relative(0.92))
        .rounded(px(22.0))
        .overflow_hidden()
        .child(
            command()
                .items(items())
                .placeholder(t("Search workspaces, agents and actions…"))
                .accessibility_label(t("Command palette"))
                .auto_focus(true)
                .on_select(|id: &SharedString| run(id))
                .empty(view().p(px(S4)).child(text(t("No results")).text_size(px(TEXT_BASE)).text_color(FG_MUTED)))
                .h(px(420.0))
                .search_prefix(icon(Icon::Search, 17.0).text_color(FG_MUTED))
                .search_bar_class("flex-row items-center gap-3 px-4 h-14 border-b border-border")
                .search_class("flex-1 text-base text-foreground")
                .group_class("px-4 pt-3 pb-1 text-xs font-medium text-muted-foreground")
                .item_class("mx-2 rounded-lg focus:bg-accent hover:bg-accent")
                .list_class("pb-2")
                .separator(false)
                .on_cancel(store::close_modal)
                .render_item(|entry: &CommandEntry| {
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .h(px(CONTROL_MD + 4.0))
                        .px(px(S3))
                        .child(text(entry.label.clone()).text_size(px(TEXT_BASE)).text_color(FG).number_of_lines(1).flex_1())
                        .child(
                            text(entry.shortcut_label.clone().unwrap_or_default())
                                .text_size(px(TEXT_SM))
                                .text_color(FG_FAINT),
                        )
                }),
        )
}
