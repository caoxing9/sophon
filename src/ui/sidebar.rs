//! The sidebar, as Paseo's: the host line, "New workspace", then pinned
//! workspaces and projects with their workspaces; "Add project" at the foot.

use crate::i18n::{t, tf};
use nsh_base::button;
use nsh_core::{
    element::{FontWeight, Position, View},
    prelude::*,
};
use super::kit::text;

use super::kit::*;
use crate::store::{self, Conn, Row, Screen};

pub(crate) fn sidebar() -> View {
    let rows = computed(store::rows);
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
        .child(dynamic(move || {
            let empty = rows.with(Vec::is_empty);
            Some(untracked(move || {
                if empty {
                    empty_list().into_element()
                } else {
                    virtual_list(move || rows.with(Vec::len), move |at: Position| row(at, rows))
                        .keys(move || rows.with(|r| r.iter().map(Row::key).collect::<Vec<_>>()))
                        .estimated_item_size(px(32.0))
                        .content_padding_bottom(px(S2))
                        .flex_1()
                        .into_element()
                }
            }))
        }))
        .child(update_pill())
        .child(footer())
}

/// The connected host, as Paseo's host chooser shows it.
/// The host in use, and the menu that switches hosts (Paseo's host chooser).
fn host_line() -> impl IntoElement {
    dropdown(host_body())
        .child(dynamic(|| {
            let hosts = store::hosts();
            let current = store::current_host().map(|h| h.id);
            Some(untracked(move || {
                let mut m = view()
                    .child(menu_heading(t("Hosts")))
                    .child(menu_row(t("This Mac"), Some(Icon::Cpu), Some(t("The Sophon service on this Mac").into()), current.is_none(), || {
                        store::switch_host(None)
                    }));
                for h in hosts.clone() {
                    let id = h.id.clone();
                    m = m.child(menu_row(h.name.clone(), Some(Icon::Server), Some(h.url.clone().into()), current.as_deref() == Some(h.id.as_str()), move || {
                        store::switch_host(Some(id.clone()))
                    }));
                }
                m = m
                    .child(menu_divider())
                    .child(menu_row(t("Add host…"), Some(Icon::Plus), Some(t("Address or pairing link").into()), false, || {
                        store::open_modal(store::Modal::AddHost)
                    }))
                    .child(menu_row(t("Pair a device…"), Some(Icon::Link2), None, false, store::open_pairing));
                if let Some(cur) = store::current_host() {
                    m = m.child(menu_row_danger(t("Remove this host"), Icon::Trash2, move || store::remove_host(&cur.id)));
                }
                m.into_element()
            }))
        }))
}

fn host_body() -> View {
    let conn = computed(store::conn);
    let remote = computed(|| store::current_host().map(|h| h.name));
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .px(px(S4))
        .pt(px(S2))
        .pb(px(S3))
        .child(
            image(crate::LOGO).w(px(26.0)).h(px(26.0)).rounded(px(7.0)).shadow_sm(),
        )
        .rounded(px(R_LG))
        .hover(|s| s.bg(highlight()))
        .child(
            view()
                .flex_1()
                .gap(px(1.0))
                .child(text("Sophon").text_size(px(TEXT_BASE)).font_weight(FontWeight(600)))
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
                            .text_size(px(TEXT_SM))
                            .text_color(FG_MUTED)
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

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Section,
    Project,
    Workspace,
}

fn row(at: Position, rows: Memo<Vec<Row>>) -> impl IntoElement {
    let item = computed(move || rows.with(|r| r.get(at.index()).cloned()));
    let kind = computed(move || {
        item.with(|r| {
            r.as_ref().map(|r| match r {
                Row::Section(_) => Kind::Section,
                Row::Project(_) => Kind::Project,
                Row::Workspace(_) => Kind::Workspace,
            })
        })
    });
    view().child(dynamic(move || {
        let kind = kind.get()?;
        Some(untracked(move || match kind {
            Kind::Section => section_row(item).into_element(),
            Kind::Project => project_row(item).into_element(),
            Kind::Workspace => workspace_row(item).into_element(),
        }))
    }))
}

fn section_row(item: Memo<Option<Row>>) -> impl IntoElement {
    view().px(px(S4)).pt(px(S3)).pb(px(S1)).child(section_label(move || match item.get() {
        Some(Row::Section(s)) => s,
        _ => String::new(),
    }))
}

fn project_row(item: Memo<Option<Row>>) -> impl IntoElement {
    let project = computed(move || match item.get() {
        Some(Row::Project(p)) => Some(p),
        _ => None,
    });
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .mx(px(S2))
        .pl(px(S2))
        .pr(px(2.0))
        .mt(px(S3))
        .h(px(28.0))
        .child(icon(Icon::Folder, 15.0).text_color(FG_FAINT))
        .child(
            text(move || project.with(|p| p.as_ref().map(|p| p.name.clone()).unwrap_or_default()))
                .text_size(px(TEXT_SM))
                .font_weight(FontWeight(500))
                .text_color(FG_MUTED)
                .number_of_lines(1)
                .flex_1(),
        )
        .child(icon_btn(Icon::Plus, Sz::Sm, t("New workspace in project"), move || {
            if let Some(p) = project.get_untracked().filter(|p| !p.root_path.is_empty()) {
                store::open_new_workspace(Some(p.root_path));
            }
        }))
        .child(
            dropdown(
                view()
                    .w(px(24.0))
                    .h(px(24.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(R_FULL))
                    .hover(|s| s.bg(highlight()))
                    .label(t("Project actions"))
                    .child(icon(Icon::Ellipsis, 15.0).text_color(FG_FAINT)),
            )
            .align(nsh_core::overlay::Align::End)
            .child(dynamic(move || {
                let p = project.get()?;
                Some(untracked(move || {
                    let (r, d, name) = (p.id.clone(), p.id.clone(), p.name.clone());
                    let (root, pname) = (p.root_path.clone(), p.name.clone());
                    view()
                        .child(menu_row(t("Project settings"), Some(Icon::Settings2), None, false, move || {
                            store::open_project_settings(&root, &pname)
                        }))
                        .child(menu_row(t("Rename"), Some(Icon::Pencil), None, false, move || {
                            store::open_modal(store::Modal::RenameProject { id: r.clone(), name: name.clone() })
                        }))
                        .child(menu_divider())
                        .child(menu_row_danger(t("Remove from Sophon"), Icon::Trash2, move || store::remove_project(&d)))
                        .into_element()
                }))
            })),
        )
}

fn workspace_row(item: Memo<Option<Row>>) -> impl IntoElement {
    let ws = computed(move || match item.get() {
        Some(Row::Workspace(w)) => Some(w),
        _ => None,
    });
    let selected = computed(move || {
        let id = ws.with(|w| w.as_ref().map(|w| w.id.clone()));
        id.is_some_and(|id| store::screen() == Screen::Workspace(id))
    });
    let status = move || ws.with(|w| w.as_ref().map(|w| Status::of_workspace(&w.status)).unwrap_or(Status::Idle));

    let main = button()
        .on_press(move || {
            if let Some(w) = ws.get_untracked() {
                store::open_workspace(&w.id);
            }
        })
        .flex_1()
        .min_w(px(0.0))
        .flex_row()
        .items_center()
        .gap(px(S2))
        .h(px(32.0))
        .pl(px(S2))
        .child(view().w(px(15.0)).items_center().child(status_dot(status)))
        .child(
            text(move || ws.with(|w| w.as_ref().map(|w| w.name.clone()).unwrap_or_default()))
                .text_size(px(TEXT_BASE))
                .text_color(move || if selected.get() { FG } else { FG })
                .number_of_lines(1)
                .flex_1(),
        )
        .child(dynamic(move || {
            let labels = ws.with(|w| w.as_ref().map(|w| w.labels.clone()).unwrap_or_default());
            if labels.is_empty() {
                return None;
            }
            Some(untracked(move || {
                let mut row = view().flex_row().gap(px(3.0));
                for l in labels.clone() {
                    row = row.child(view().w(px(6.0)).h(px(6.0)).rounded(px(R_FULL)).bg(label_tone(&store::label_color(&l))));
                }
                row
            }))
        }))
        .child(
            text(move || {
                ws.with(|w| match w {
                    Some(w) if w.additions + w.deletions > 0 => format!("+{} −{}", w.additions, w.deletions),
                    _ => String::new(),
                })
            })
            .text_size(px(TEXT_XS))
            .font_family(|| MONO)
            .text_color(FG_FAINT),
        );

    let more = dropdown(
        view()
            .w(px(24.0))
            .h(px(24.0))
            .items_center()
            .justify_center()
            .rounded(px(R_FULL))
            .hover(|s| s.bg(highlight()))
            .child(icon(Icon::Ellipsis, 15.0).text_color(FG_FAINT)),
    )
    .align(nsh_core::overlay::Align::End)
    .child(dynamic(move || {
        let w = ws.get()?;
        Some(untracked(move || {
            let (pin_id, archive_id, rename_id) = (w.id.clone(), w.id.clone(), w.id.clone());
            let (name, name2) = (w.name.clone(), w.name.clone());
            let pinned = w.pinned;
            view()
                .child(menu_row(
                    if pinned { t("Unpin") } else { t("Pin") },
                    Some(if pinned { Icon::PinOff } else { Icon::Pin }),
                    None,
                    false,
                    move || store::pin_workspace(&pin_id, !pinned),
                ))
                .child(menu_row(t("Mark as unread"), Some(Icon::CircleDot), None, false, {
                    let id = w.id.clone();
                    move || store::mark_unread(&id)
                }))
                .child(menu_row(t("Labels…"), Some(Icon::Layers), None, false, {
                    let id = w.id.clone();
                    move || store::open_modal(store::Modal::Labels { workspace: id.clone() })
                }))
                .child(menu_row(t("Recover files"), Some(Icon::RotateCw), Some(t("If its folder went missing").into()), false, {
                    let id = w.id.clone();
                    move || store::recover_workspace(&id)
                }))
                .child(menu_row(t("Rename"), Some(Icon::Pencil), None, false, move || {
                    store::open_modal(store::Modal::RenameWorkspace { id: rename_id.clone(), name: name.clone() })
                }))
                .child(menu_divider())
                .child(menu_row_danger(t("Archive workspace"), Icon::Archive, move || {
                    store::open_modal(store::Modal::ArchiveWorkspace { id: archive_id.clone(), name: name2.clone() })
                }))
                .into_element()
        }))
    }));

    view()
        .flex_row()
        .items_center()
        .mx(px(S2))
        .pr(px(S1))
        .rounded(px(R_LG))
        .bg(move || if selected.get() { highlight() } else { tint(SIDEBAR, 0.0) })
        .hover(|s| s.bg(highlight()))
        .child(main)
        .child(more)
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
    let shown = computed(move || st.with(|s| s.version.is_some() && s.phase != Phase::Idle));
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
                        update::install()
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
