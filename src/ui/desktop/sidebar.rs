//! The sidebar: the host line, "New workspace", then pinned
//! workspaces and projects with their workspaces; "Add project" at the foot.

use crate::i18n::{t, tf};
use nsh_base::button;
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store::{self, Conn, Screen};
use crate::ui::shared::projects;

pub(crate) fn sidebar() -> View {
    view()
        .child(host_line())
        .child(
            view()
                .px(sp(S2))
                // The space between the two groups, which the first
                // project's own top margin no longer gives.
                .pb(sp(S2))
                .child(nav_item(Icon::SquarePen, t("New workspace"), || store::screen() == Screen::NewWorkspace && !store::form().scoped, || {
                    store::open_new_workspace(None)
                }))
                .child(nav_item(Icon::Search, t("Search"), || false, || store::open_modal(store::Modal::Palette)).child(hint("⌘K")))
                .child(nav_item(Icon::CalendarClock, t("Automations"), || store::screen() == Screen::Schedules, store::open_schedules)),
        )
        // The projects under a heading of their own, adding one beside it
        // (monocode's sidebar): "New workspace" above is the only other +.
        // The list puts it over the projects themselves, so under the
        // pinned conversations when there are any.
        // A Mac that runs no agents of its own, connected nowhere yet: how to
        // reach one, in place of a list it has none of.
        .child(dynamic(|| {
            let elsewhere = !store::local_service() && store::current_host().is_none();
            Some(untracked(move || {
                if elsewhere {
                    view().flex_1().child(projects::projects_heading(true)).child(connect_elsewhere()).into_element()
                } else {
                    projects::project_list(true, || empty_list().into_element()).into_element()
                }
            }))
        }))
        .child(footer())
}

/// The connected host, as the host chooser shows it.
/// The host in use, and the menu that switches hosts (the host chooser).
fn host_line() -> impl IntoElement {
    // The host and its menu sit on the sidebar's rail (the rows' inset),
    // the menu as wide as the rows under it.
    // A narrow window shows the sidebar alone, across it.
    let sidebar = if window().width.0 < 600.0 { window().width.0 } else { store::sidebar_width() };
    let width = sidebar - 2.0 * S2;
    view().px(sp(S2)).child(dropdown_wide(host_body(), width)
        .child(dynamic(|| {
            let hosts = store::hosts();
            let current = store::current_host().map(|h| h.id);
            Some(untracked(move || {
                let is_space = |h: &store::HostProfile| crate::spaces::home_of(&h.url).is_some();
                let spaces: Vec<_> = hosts.iter().filter(|h| is_space(h)).cloned().collect();
                let local = store::local_service();
                let mut m = view();
                // This Mac's own spaces, while it runs agents of its own.
                if local {
                    m = m
                        .child(menu_heading(if spaces.is_empty() { t("Hosts") } else { t("Spaces on this Mac") }))
                        .child(menu_row(
                            store::my_space_label().unwrap_or_else(|| t("This Mac").to_owned()),
                            Some(Icon::Cpu),
                            None,
                            current.is_none(),
                            || store::choose_host(None),
                        ));
                    for h in spaces {
                        let id = h.id.clone();
                        m = m.child(menu_row(h.label(), Some(Icon::Folder), None, current.as_deref() == Some(h.id.as_str()), move || {
                            store::choose_host(Some(id.clone()))
                        }));
                    }
                    m = m.child(menu_row(t("New space…"), Some(Icon::FolderPlus), None, false, || store::open_modal(store::Modal::CreateSpace)));
                }
                let others: Vec<_> = hosts.iter().filter(|h| !is_space(h)).cloned().collect();
                if !others.is_empty() {
                    if local {
                        m = m.child(menu_divider());
                    }
                    m = m.child(menu_heading(if local { t("Other computers") } else { t("Computers") }));
                }
                for h in others {
                    let id = h.id.clone();
                    m = m.child(menu_row(h.label(), Some(Icon::Server), None, current.as_deref() == Some(h.id.as_str()), move || {
                        store::choose_host(Some(id.clone()))
                    }));
                }
                m = m
                    .child(menu_divider())
                    .child(menu_row(t("Add host…"), Some(Icon::Plus), None, false, || {
                        store::open_modal(store::Modal::AddHost)
                    }))
                    .child(menu_row(t("Pair a device…"), Some(Icon::Link2), None, false, store::open_pairing));
                if let Some(cur) = store::current_host() {
                    let (id, name) = (cur.id.clone(), cur.label());
                    m = m
                        .child(menu_row(t("Rename this host…"), Some(Icon::Pencil), None, false, move || {
                            store::open_modal(store::Modal::RenameHost { id: id.clone(), name: name.clone() })
                        }))
                        .child(menu_row_danger(t("Remove this host"), Icon::Trash2, move || store::remove_host(&cur.id)));
                }
                m.into_element()
            }))
        })))
}

fn host_body() -> View {
    let conn = computed(store::conn);
    // Another computer's or space's name; this Mac's first space's once it
    // has one (renamed, or beside other spaces), else the host's own name.
    let remote = computed(|| store::current_host().map(|h| h.label()).or_else(store::my_space_label));
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .px(sp(S2))
        // As tall as the rows under it, its status mark where their icons are.
        .h(sp(CONTROL_MD))
        .mb(sp(S1))
        .rounded(px(R_LG))
        .hover(|s| s.bg(sidebar_hover()))
        .child(
            // The host alone: no app mark beside it, which says nothing
            // here. A long name ends in an ellipsis inside the row.
            view()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .min_w(px(0.0))
                        .gap(px(5.0))
                        .child(crate::ui::shared::conn_mark())
                        .child(
                            text(move || {
                                let name = remote.get();
                                match conn.get() {
                                    Conn::Starting | Conn::Connecting(_) => match name { Some(n) => tf("Connecting to {}…", &[&n]), None => t("Connecting…").to_owned() },
                                    Conn::Connected { hostname, .. } => name.unwrap_or(if hostname.is_empty() { t("This Mac").into() } else { hostname }),
                                    // This Mac's service is being started, not offline.
                                    Conn::Down(_) if store::current_host().is_none() && store::service_start_error().is_none() => t("Starting the service…").to_owned(),
                                    Conn::Down(_) => tf("{} offline", &[&name.unwrap_or_else(|| t("This Mac").into())]),
                                }
                            })
                            .text_size(tx(TEXT_BASE))
                            .font_weight(FontWeight(500))
                            .number_of_lines(1)
                            .flex_shrink(1.0)
                            .min_w(px(0.0)),
                        ),
                ),
        )
        // That it opens a list to switch from: a row is otherwise a place
        // to go, and this one is not.
        .child(icon(Icon::ChevronsUpDown, 14.0).text_color(FG_FAINT()))
}

fn nav_item(
    glyph: Icon,
    label: &'static str,
    active: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> nsh_base::ButtonElement {
    let active = computed(active);
    button()
        .on_press(on_press)
        .accessibility_label(label)
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .h(sp(CONTROL_MD))
        // Rows that fill under the pointer keep a hairline apart.
        .my(px(1.0))
        .px(sp(S2))
        .rounded(px(R_LG))
        .bg(move || if active.get() { SIDEBAR_SELECTED() } else { tint(SIDEBAR(), 0.0) })
        .hover(|s| s.bg(sidebar_hover()))
        .active(|s| s.opacity(0.7))
        .child(icon(glyph, 16.0).text_color(FG_MUTED()))
        .child(text(label).text_size(tx(TEXT_BASE)).text_color(FG()).flex_1())
}

/// A row's shortcut, faint at its end.
fn hint(keys: &'static str) -> impl IntoElement {
    text(keys).text_size(tx(TEXT_XS)).text_color(FG_FAINT()).mr(px(2.0))
}

fn empty_list() -> impl IntoElement {
    service_status()
}

/// A Mac that runs no agents of its own, connected to no computer yet: the
/// computers nearby, and adding one by its link or address.
fn connect_elsewhere() -> impl IntoElement {
    view()
        .flex_1()
        .px(sp(S3))
        .pt(sp(S3))
        .gap(sp(S3))
        .child(text(t("Connect to the computer your agents run on")).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED()))
        .child(crate::ui::shared::modals::nearby_list())
        .child(btn(t("Add host…"), Variant::Secondary, Sz::Sm, Some(Icon::Plus), || store::open_modal(store::Modal::AddHost)))
}

fn service_status() -> impl IntoElement {
    let conn = computed(store::conn);
    let failed = computed(store::service_start_error);
    // This Mac's service starts on its own: a button only when that failed.
    let retry = computed(move || matches!(conn.get(), Conn::Down(_)) && store::on_this_mac() && failed.get().is_some());
    view().flex_1().px(sp(S4)).pt(sp(S4)).gap(sp(S3)).items_start().child(
        text(move || match conn.get() {
            Conn::Down(_) if !store::on_this_mac() => String::new(),
            Conn::Down(_) => match failed.get() {
                Some(e) => tf("The service on this Mac did not start: {}", &[&e]),
                None => t("Starting the service…").to_owned(),
            },
            Conn::Connected { .. } => t("No projects yet").to_owned(),
            _ => String::new(),
        })
        .text_size(tx(TEXT_SM))
        .line_height(px(18.0))
        .text_color(FG_MUTED()),
    )
    .child(when(retry, || {
        btn(
            move || -> SharedString { if store::service_starting() { t("Starting the service…").into() } else { t("Try again").into() } },
            Variant::Secondary,
            Sz::Sm,
            Some(Icon::RotateCw),
            store::start_service,
        )
    }))
}

/// The foot, a row of icons (as T3 Code's): Settings, Usage, History and
/// the rest behind "…" (pull requests, integrations, plugins, feedback);
/// at its right end the way to a newer version, while there is one. Each
/// shows which page is open, and pull requests a dot while one is unseen.
fn footer() -> impl IntoElement {
    let tool = |glyph: Icon, label: &'static str, tip: &'static str, on: fn() -> bool, open: fn()| {
        view().tooltip(tooltip_text(tip).into_element()).child(
            button()
                .on_press(open)
                .accessibility_label(label)
                .w(px(30.0))
                .h(px(30.0))
                .rounded(px(R_LG))
                .items_center()
                .justify_center()
                .bg(move || if on() { SIDEBAR_SELECTED() } else { tint(SIDEBAR(), 0.0) })
                .hover(|s| s.bg(sidebar_hover()))
                .active(|s| s.opacity(0.7))
                .child(icon(glyph, 16.0).text_color(move || if on() { FG() } else { FG_MUTED() })),
        )
    };
    let elsewhere = computed(|| matches!(store::screen(), Screen::Inbox | Screen::PullRequest { .. } | Screen::Plugins | Screen::Extensions));
    let more = dropdown(
        view()
            .w(px(30.0))
            .h(px(30.0))
            .rounded(px(R_LG))
            .items_center()
            .justify_center()
            .bg(move || if elsewhere.get() { SIDEBAR_SELECTED() } else { tint(SIDEBAR(), 0.0) })
            .hover(|s| s.bg(sidebar_hover()))
            .label(t("More"))
            .child(icon(Icon::Ellipsis, 16.0).text_color(FG_MUTED()))
            .child(when(
                || store::desk_inbox_items().iter().any(store::inbox_unseen),
                || view().absolute().top(px(5.0)).right(px(5.0)).w(px(6.0)).h(px(6.0)).rounded(px(R_FULL)).bg(DOT_RUNNING),
            )),
    )
    .side(nsh_core::overlay::Placement::Top)
    .align(nsh_core::overlay::Align::Start)
    .child(menu_row(t("Pull requests"), Some(Icon::GitPullRequest), None, false, store::open_inbox))
    .child(menu_divider())
    .child(menu_row(t("Integrations"), Some(Icon::Workflow), None, false, store::open_plugins))
    .child(menu_row(t("Plugins"), Some(Icon::Layers), None, false, store::open_extensions))
    .child(menu_divider())
    .child(menu_row(t("Feedback"), Some(Icon::MessageSquare), None, false, || store::open_modal(store::Modal::ReportIssue)));
    view()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .mx(sp(S2))
        .px(sp(S1))
        .py(sp(S2))
        .border_t_1()
        .border_color(tint(BORDER(), 0.6))
        .child(tool(Icon::Settings, t("Settings"), t("Settings  ⌘,"), || store::screen() == Screen::Settings, store::open_settings))
        .child(tool(Icon::Activity, t("Usage"), t("Usage"), || store::screen() == Screen::Usage, store::open_usage))
        .child(tool(Icon::History, t("History"), t("History"), || store::screen() == Screen::History, store::open_history))
        .child(more)
        .child(view().flex_1())
        .child(update_button())
}

/// The way to a newer version, at the foot's right end: a round arrow in
/// the accent with a dot (its words under the pointer: "Update to 0.2.45",
/// "Downloading 40%"), there only while there is one. Always there, quiet,
/// it read as an update waiting; checking is in Settings (Updates).
fn update_button() -> impl IntoElement {
    use crate::update::{self, Phase};
    let st = computed(update::state);
    let offered = computed(move || {
        st.with(|s| {
            s.version.is_some()
                && match s.phase {
                    Phase::Ready | Phase::Installing | Phase::Downloading(_) | Phase::Failed(_) => true,
                    Phase::Available => !s.can_install,
                    _ => false,
                }
        })
    });
    let label = move || -> String {
        st.with(|s| {
            let v = s.version.clone().unwrap_or_default();
            match &s.phase {
                _ if s.version.is_none() => crate::i18n::tf("Sophon {}", &[&s.current]),
                Phase::Downloading(p) => crate::i18n::tf("Downloading {}%", &[&((p * 100.0).round() as u32).to_string()]),
                Phase::Installing => crate::i18n::t("Installing…").to_owned(),
                Phase::Failed(_) => crate::i18n::t("Update failed · Retry").to_owned(),
                _ if !s.can_install => crate::i18n::tf("Get {}", &[&v]),
                _ => crate::i18n::tf("Update to {}", &[&v]),
            }
        })
    };
    let busy = computed(move || st.with(|s| matches!(s.phase, Phase::Downloading(_) | Phase::Installing)));
    let shown = computed(move || offered.get() || busy.get());
    when(shown, move || view().tooltip(dynamic(move || {
        let words = label();
        Some(untracked(move || tooltip_text(words)))
    }).into_element()).child(
        button()
            .on_press(move || {
                if !busy.get_untracked() {
                    store::open_modal(store::Modal::Update)
                }
            })
            .accessibility_label(crate::i18n::t("Update the app"))
            .w(px(30.0))
            .h(px(30.0))
            .rounded(px(R_LG))
            .items_center()
            .justify_center()
            .bg(tint(SIDEBAR(), 0.0))
            .hover(|s| s.bg(sidebar_hover()))
            .active(|s| s.opacity(0.7))
            .child(icon(Icon::CircleArrowUp, 16.0).text_color(move || if offered.get() { ACCENT() } else { FG_MUTED() }))
            .child(when(move || offered.get() && !busy.get(), || view().absolute().top(px(5.0)).right(px(5.0)).w(px(6.0)).h(px(6.0)).rounded(px(R_FULL)).bg(DANGER))),
    ))
}
