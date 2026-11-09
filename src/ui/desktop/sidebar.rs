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
                .px(px(S2))
                .pb(px(S2))
                .child(nav_item(Icon::SquarePen, t("New workspace"), || store::screen() == Screen::NewWorkspace, || {
                    store::open_new_workspace(None)
                }))
                .child(nav_item(Icon::History, t("History"), || store::screen() == Screen::History, store::open_history))
                .child(nav_item(Icon::Search, t("Search"), || false, || store::open_modal(store::Modal::Palette)))
                .child(nav_item(Icon::CalendarClock, t("Schedules"), || store::screen() == Screen::Schedules, store::open_schedules)),
        )
        .child(projects::project_list(true, || empty_list().into_element()))
        .child(update_pill())
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
    view().px(px(S2)).child(dropdown_wide(host_body(), width)
        .child(dynamic(|| {
            let hosts = store::hosts();
            let current = store::current_host().map(|h| h.id);
            Some(untracked(move || {
                let mut m = view()
                    .child(menu_heading(t("Hosts")))
                    .child(menu_row(t("This Mac"), Some(Icon::Cpu), None, current.is_none(), || {
                        store::switch_host(None)
                    }));
                for h in hosts.clone() {
                    let id = h.id.clone();
                    m = m.child(menu_row(h.name.clone(), Some(Icon::Server), None, current.as_deref() == Some(h.id.as_str()), move || {
                        store::switch_host(Some(id.clone()))
                    }));
                }
                m = m
                    .child(menu_divider())
                    .child(menu_row(t("Add host…"), Some(Icon::Plus), None, false, || {
                        store::open_modal(store::Modal::AddHost)
                    }))
                    .child(menu_row(t("Pair a device…"), Some(Icon::Link2), None, false, store::open_pairing));
                if let Some(cur) = store::current_host() {
                    m = m.child(menu_row_danger(t("Remove this host"), Icon::Trash2, move || store::remove_host(&cur.id)));
                }
                m.into_element()
            }))
        })))
}

fn host_body() -> View {
    let conn = computed(store::conn);
    let remote = computed(|| store::current_host().map(|h| h.name));
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .px(px(S2))
        .py(px(S2))
        .mb(px(S1))
        .child(
            mark(26.0, Lift::Small),
        )
        .rounded(px(R_LG))
        .hover(|s| s.bg(highlight()))
        .child(
            // The host alone: the mark beside it says whose app this is.
            view()
                .flex_1()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(5.0))
                        .child(status_dot(move || match conn.get() {
                            Conn::Connected { .. } => Status::Done,
                            Conn::Down(_) => Status::Failed,
                            _ => Status::Idle,
                        }))
                        .child(
                            text(move || {
                                let name = remote.get();
                                match conn.get() {
                                    Conn::Starting | Conn::Connecting(_) => match name { Some(n) => tf("Connecting to {}…", &[&n]), None => t("Connecting…").to_owned() },
                                    Conn::Connected { hostname, .. } => name.unwrap_or(if hostname.is_empty() { t("This Mac").into() } else { hostname }),
                                    Conn::Down(_) => tf("{} offline", &[&name.unwrap_or_else(|| t("Host").into())]),
                                }
                            })
                            .text_size(px(TEXT_BASE))
                            .font_weight(FontWeight(500))
                            .number_of_lines(1),
                        ),
                ),
        )
}

fn nav_item(
    glyph: Icon,
    label: &'static str,
    active: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    let active = computed(active);
    button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap(px(S2))
        .h(px(CONTROL_MD))
        .px(px(S2))
        .rounded(px(R_LG))
        .bg(move || if active.get() { highlight() } else { tint(SIDEBAR, 0.0) })
        .hover(|s| s.bg(highlight()))
        .active(|s| s.opacity(0.7))
        .child(icon(glyph, 16.0).text_color(FG_MUTED))
        .child(text(label).text_size(px(TEXT_BASE)).text_color(FG))
}

fn empty_list() -> impl IntoElement {
    let conn = computed(store::conn);
    let down = computed(move || matches!(conn.get(), Conn::Down(_)));
    view().flex_1().px(px(S4)).pt(px(S4)).gap(px(S3)).items_start().child(
        text(move || match conn.get() {
            Conn::Down(_) => t("The Sophon service isn't running on this Mac").to_owned(),
            Conn::Connected { .. } => t("No projects yet").to_owned(),
            _ => String::new(),
        })
        .text_size(px(TEXT_SM))
        .line_height(px(18.0))
        .text_color(FG_MUTED),
    )
    .child(when(down, || {
        btn(
            move || -> SharedString { if store::service_starting() { t("Starting the service…").into() } else { t("Start service").into() } },
            Variant::Primary,
            Sz::Sm,
            Some(Icon::Play),
            store::start_service,
        )
    }))
}

fn footer() -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap(px(S1))
        .px(px(S2))
        .py(px(S2))
        .border_t_1()
        .border_color(tint(BORDER, 0.6))
        .child(
            view().flex_1().child(nav_item(Icon::FolderPlus, t("Add project"), || false, || {
                store::open_modal(store::Modal::AddProject)
            })),
        )
        .child(icon_btn(Icon::Activity, Sz::Md, t("Usage"), store::open_usage))
        .child(icon_btn(Icon::Settings, Sz::Md, t("Settings"), store::open_settings))
}

/// A newer version on offer, above the footer: press to download, install
/// and restart.
fn update_pill() -> impl IntoElement {
    use crate::update::{self, Phase};
    let st = computed(update::state);
    // Offered once it is one restart away (fetched in the background), or
    // when there is only a page to open, or to retry.
    let shown = computed(move || {
        st.with(|s| {
            s.version.is_some()
                && match s.phase {
                    Phase::Ready | Phase::Installing | Phase::Failed(_) => true,
                    Phase::Available => !s.can_install,
                    _ => false,
                }
        })
    });
    when(shown, move || {
        let label = move || {
            st.with(|s| {
                let v = s.version.clone().unwrap_or_default();
                match &s.phase {
                    Phase::Downloading(p) => crate::i18n::tf("Downloading {}%", &[&((p * 100.0).round() as u32).to_string()]),
                    Phase::Installing => crate::i18n::t("Installing…").to_owned(),
                    Phase::Failed(_) => crate::i18n::t("Update failed · Retry").to_owned(),
                    _ if !s.can_install => crate::i18n::tf("Get {}", &[&v]),
                    _ => crate::i18n::tf("Update to {}", &[&v]),
                }
            })
        };
        let busy = computed(move || st.with(|s| matches!(s.phase, Phase::Downloading(_) | Phase::Installing)));
        let share = computed(move || st.with(|s| match s.phase {
            Phase::Downloading(p) => p,
            Phase::Installing => 1.0,
            _ => 0.0,
        }));
        view().px(px(S2)).pb(px(S2)).child(
            button()
                .on_press(move || {
                    if !busy.get_untracked() {
                        store::open_modal(store::Modal::Update)
                    }
                })
                .accessibility_label(crate::i18n::t("Update the app"))
                .flex_row()
                .items_center()
                .gap(px(S2))
                .h(px(36.0))
                .px(px(S3))
                .rounded(px(R_LG))
                .overflow_hidden()
                .bg(tint(ACCENT, 0.12))
                .border_1()
                .border_color(tint(ACCENT, 0.22))
                .hover(|s| s.bg(tint(ACCENT, 0.18)))
                .transition_colors()
                .child(
                    // The download's progress, filling the pill.
                    view()
                        .absolute()
                        .top(px(0.0))
                        .bottom(px(0.0))
                        .left(px(0.0))
                        .w(move || Length::from(relative(share.get())))
                        .bg(tint(ACCENT, 0.16)),
                )
                .child(icon(Icon::CircleArrowUp, 16.0).text_color(ACCENT))
                .child(text(label).text_size(px(TEXT_SM)).font_weight(FontWeight(600)).text_color(FG).flex_1().number_of_lines(1))
                .child(text(move || format!("v{}", st.with(|s| s.current.clone()))).text_size(px(TEXT_XS)).text_color(FG_MUTED)),
        )
    })
}
