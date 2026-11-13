//! The desktop's settings page: every section, with what only a desktop
//! does (pairing a phone, updating and tuning the service).

use crate::i18n::{t, tf};
use nsh_core::prelude::*;

use crate::store;
use crate::ui::kit::*;
use crate::ui::kit::text;
use crate::ui::shared::settings::*;

pub(crate) fn settings(compact: bool) -> impl IntoElement {
    view()
        .flex_1()
        .child(when(move || compact, || {
            view().px(sp(S4)).pt(sp(S2)).child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home))
        }))
        .child(page(
            compact,
            view()
                .child(section(t("Appearance"), appearance_card()))
                .child(section(t("Sending"), sending_card()))
                .when(cfg!(target_os = "macos"), |v| v.child(section(t("Dictation"), dictation_card())))
                .child(section(t("Storage"), storage_card()))
                .child(section(t("Power"), keep_awake_card()))
                .child(when(crate::update::can_check, || section(t("Updates"), updates_card())))
                .child(section(
                    t("Host"),
                    host_card()
                        .child(row(
                            t("Pair a device"),
                            t("Reach this host from your phone through the encrypted relay"),
                            btn(t("Pair…"), Variant::Outline, Sz::Sm, Some(Icon::Link2), store::open_pairing),
                            false,
                        ))
                        .child(relay_row())
                        .child(if store::service_bundled() {
                            // The desktop app updates its own service.
                            row(
                                t("Update service"),
                                t("The service comes with the desktop daemon app and updates with it"),
                                view(),
                                false,
                            )
                        } else {
                            row(
                                t("Update service"),
                                t("Installs the latest service release; Sophon reconnects on its own"),
                                btn(t("Update"), Variant::Outline, Sz::Sm, Some(Icon::ArrowDown), store::update_daemon),
                                false,
                            )
                        }),
                ))
                .child(section(t("This Mac"), card().child(row(
                    t("Run agents on this Mac"),
                    t("Off: this Mac only connects to other computers, and starts no service of its own"),
                    switch(store::local_service, store::set_local_service).accessibility_label(t("Run agents on this Mac")),
                    true,
                ))))
                .child(when(store::local_service, || section(t("Spaces"), spaces_card())))
                .child(section(t("Devices"), devices_card()))
                .child(section(t("Host behavior"), behavior_card()))
                .child(section(
                    t("Feedback"),
                    card().child(row(
                        t("Report an issue"),
                        t("A bug, an idea or a question, as an issue on GitHub"),
                        btn(t("Report…"), Variant::Outline, Sz::Sm, Some(Icon::GitPullRequest), || store::open_modal(store::Modal::ReportIssue)),
                        true,
                    )),
                ))
                .child(section(t("Coding agents"), crate::ui::shared::agents::agents_card(true)))
                .child(section(t("Text generation"), text_generation_card())),
        ))
}

/// The spaces on this Mac: each one's name, folder, port and whether its
/// service runs, to rename, switch to or remove; and a new one.
fn spaces_card() -> impl IntoElement {
    card().child(dynamic(|| {
        let spaces = store::spaces();
        let current = store::current_host().map(|h| h.id);
        let mine = store::my_space_label().unwrap_or_else(|| t("My space").to_owned());
        Some(untracked(move || {
            let home = std::env::var("HOME").unwrap_or_default();
            let short = |p: &std::path::Path| {
                let p = p.to_string_lossy().into_owned();
                match p.strip_prefix(&home) {
                    Some(rest) => format!("~{rest}"),
                    None => p,
                }
            };
            let detail = |folder: &std::path::Path, port: Option<u16>, running: bool| {
                let mut parts = vec![short(folder)];
                if let Some(port) = port {
                    parts.push(tf("port {}", &[&port.to_string()]));
                }
                parts.push(if running { t("Running").to_owned() } else { t("Stopped").to_owned() });
                parts.join(" · ")
            };
            let switch_btn = |id: Option<String>, here: bool| -> AnyElement {
                if here {
                    text(t("In use")).text_size(tx(TEXT_SM)).text_color(FG_MUTED).into_element()
                } else {
                    btn(t("Switch"), Variant::Outline, Sz::Sm, None, move || store::switch_host(id.clone())).into_element()
                }
            };
            let first = crate::spaces::home_of_first();
            let mut c = view().child(row(
                mine.clone(),
                detail(&first, crate::spaces::port_of(&first).or(Some(6767)), true),
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S1))
                    .child(switch_btn(None, current.is_none()))
                    .child(icon_btn(Icon::Pencil, Sz::Sm, t("Rename"), move || {
                        store::open_modal(store::Modal::RenameMySpace { name: store::settings().my_space_name.unwrap_or_default() })
                    })),
                true,
            ));
            for h in spaces {
                let Some(folder) = crate::spaces::home_of(&h.url) else { continue };
                let here = current.as_deref() == Some(h.id.as_str());
                let (rename_id, rename_name) = (h.id.clone(), h.label());
                let (remove_id, remove_name, remove_folder) = (h.id.clone(), h.label(), short(&folder));
                c = c.child(row(
                    h.label(),
                    detail(&folder, crate::spaces::port_of(&folder), crate::spaces::running(&folder)),
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S1))
                        .child(switch_btn(Some(h.id.clone()), here))
                        .child(icon_btn(Icon::Pencil, Sz::Sm, t("Rename"), move || {
                            store::open_modal(store::Modal::RenameHost { id: rename_id.clone(), name: rename_name.clone() })
                        }))
                        .child(icon_btn(Icon::Trash2, Sz::Sm, t("Remove space"), move || {
                            store::open_modal(store::Modal::RemoveSpace { id: remove_id.clone(), name: remove_name.clone(), folder: remove_folder.clone() })
                        })),
                    false,
                ));
            }
            c.child(row(
                t("New space"),
                t("Its own projects, conversations and schedules, for another person or another kind of work"),
                btn(t("New space…"), Variant::Outline, Sz::Sm, Some(Icon::FolderPlus), || store::open_modal(store::Modal::CreateSpace)),
                false,
            ))
            .into_element()
        }))
    }))
}

/// The devices that reach this Mac through its bridge, over the LAN or the
/// relay: those connected now (each can be cut off) and those paired (each
/// can be forgotten), and unpairing everything, the QR's tokens included.
/// A device paired before the bridge had its place at the relay may still
/// reach a service's own relay, which this does not see.
fn devices_card() -> impl IntoElement {
    crate::lan::load_devices();
    card().child(dynamic(|| {
        let connected = crate::lan::connected();
        let paired = crate::lan::paired();
        Some(untracked(move || {
            let mut c = view();
            let mut first = true;
            let mut line = |c: View, label: String, hint: String, control: AnyElement| {
                let r = row(label, hint, control, first);
                first = false;
                c.child(r)
            };
            for d in &connected {
                let conn = d.conn;
                c = line(
                    c,
                    d.device.clone(),
                    {
                        let how = if d.relayed { t("Connected now, through the relay") } else { t("Connected now, over the LAN") };
                        match &d.space {
                            Some(space) => format!("{how} · {space}"),
                            None => how.to_owned(),
                        }
                    },
                    btn(t("Disconnect"), Variant::Outline, Sz::Sm, None, move || crate::lan::disconnect(conn)).into_element(),
                );
            }
            for p in paired.iter().filter(|p| !connected.iter().any(|c| c.device_id.as_deref() == Some(p.id.as_str()))) {
                let id = p.id.clone();
                c = line(
                    c,
                    p.name.clone(),
                    match &p.space {
                        Some(space) => format!("{} · {space}", t("Paired")),
                        None => t("Paired").to_owned(),
                    },
                    btn(t("Remove"), Variant::Outline, Sz::Sm, None, move || crate::lan::revoke(&id)).into_element(),
                );
            }
            if connected.is_empty() && paired.is_empty() {
                c = line(c, t("No device connected").to_owned(), t("Phones on this network find this Mac and ask to pair; you allow each one here").to_owned(), view().into_element());
            }
            c.child(row(
                t("Unpair every device"),
                t("Devices paired by QR code or on the LAN must pair again"),
                btn(t("Unpair all…"), Variant::Outline, Sz::Sm, None, || store::open_modal(store::Modal::RevokeDevices)),
                false,
            ))
            .into_element()
        }))
    }))
}

/// The relay this Mac's service registers with, for phones away from home:
/// the default one, or the user's own (a Cloudflare Worker, a server).
/// It applies when the service restarts; pair the phone again after.
fn relay_row() -> impl IntoElement {
    let value = signal(crate::daemon::connection::relay_endpoint().unwrap_or_default());
    let saved = signal(value.get_untracked());
    let local = computed(|| store::current_host().is_none());
    when(local, move || {
        view()
            .px(sp(S4))
            .py(sp(S4))
            .gap(sp(S2))
            .border_t_1()
            .border_color(BORDER)
            .child(text(t("Relay server")).text_size(tx(TEXT_BASE)))
            .child(
                text(t("Where phones reach this Mac away from home. Empty: the default relay. Applies after a service restart"))
                    .text_size(tx(TEXT_SM))
                    .text_color(FG_MUTED),
            )
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .child(
                        field()
                            .flex_1()
                            .min_w(px(0.0))
                            .value(move || value.get())
                            .on_change(move |v: &str| value.set(v.to_owned()))
                            .placeholder("relay.paseo.sh")
                            .font_family(mono_font)
                            .text_size(tx(TEXT_SM)),
                    )
                    .child(
                        btn(t("Save"), Variant::Outline, Sz::Sm, None, move || {
                            match crate::daemon::connection::set_relay_endpoint(&value.get_untracked()) {
                                Ok(()) => {
                                    saved.set(value.get_untracked());
                                    store::open_modal(store::Modal::RestartDaemon);
                                }
                                Err(e) => store::notify(e),
                            }
                        })
                        .disabled(move || value.get() == saved.get()),
                    ),
            )
    })
}

/// The running version and a check for a newer one now: Sophon looks every
/// hour by itself, which is a wait right after a release.
fn updates_card() -> View {
    use crate::update::{self, Phase};
    let st = computed(update::state);
    card().child(row(
        move || SharedString::from(tf("Sophon {}", &[&st.with(|s| s.current.clone())])),
        move || {
            let s = st.get();
            SharedString::from(match (&s.phase, &s.version) {
                _ if s.checking => t("Checking…").to_owned(),
                (Phase::Idle, _) => match &s.checked {
                    Some(Ok(())) => t("This is the newest version").to_owned(),
                    Some(Err(e)) => tf("Could not check: {}", &[e]),
                    None => t("Sophon looks for a newer version every hour").to_owned(),
                },
                (Phase::Downloading(_), Some(v)) => tf("Downloading {}…", &[v]),
                (Phase::Failed(e), _) => tf("The update failed: {}", &[e]),
                (_, Some(v)) => tf("Version {} is available: the sidebar offers it", &[v]),
                _ => String::new(),
            })
        },
        btn(t("Check for updates"), Variant::Outline, Sz::Sm, Some(Icon::RotateCw), update::check_now)
            .disabled(move || st.with(|s| s.checking || !matches!(s.phase, Phase::Idle))),
        true,
    ))
}
