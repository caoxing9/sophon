//! The desktop's settings page: every section, with what only a desktop
//! does (pairing a phone, updating and tuning the service).

use crate::i18n::t;
use nsh_core::prelude::*;

use crate::store;
use crate::ui::kit::*;
use crate::ui::kit::text;
use crate::ui::shared::settings::*;

pub(crate) fn settings(compact: bool) -> impl IntoElement {
    view()
        .flex_1()
        .child(when(move || compact, || {
            view().px(px(S4)).pt(px(S2)).child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home))
        }))
        .child(page(
            compact,
            view()
                .child(section(t("Appearance"), appearance_card()))
                .child(section(t("Sending"), sending_card()))
                .child(section(t("Power"), keep_awake_card()))
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
                .child(section(t("Devices"), devices_card()))
                .child(section(t("Host behavior"), behavior_card()))
                .child(section(t("Coding agents"), crate::ui::shared::agents::agents_card(true)))
                .child(section(t("Text generation"), text_generation_card())),
        ))
}

/// The devices that reach this Mac through its LAN bridge: those connected
/// now (each can be cut off) and those paired on the LAN (each can be
/// forgotten), and unpairing everything, the QR's token included. Devices
/// on the relay reach the service directly and are not seen here.
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
                    match &d.space {
                        Some(space) => format!("{} · {space}", t("Connected now, over the LAN")),
                        None => t("Connected now, over the LAN").to_owned(),
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
                        Some(space) => format!("{} · {space}", t("Paired on the LAN")),
                        None => t("Paired on the LAN").to_owned(),
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
            .px(px(S4))
            .py(px(S4))
            .gap(px(S2))
            .border_t_1()
            .border_color(BORDER)
            .child(text(t("Relay server")).text_size(px(TEXT_BASE)))
            .child(
                text(t("Where phones reach this Mac away from home. Empty: the default relay. Applies after a service restart"))
                    .text_size(px(TEXT_SM))
                    .text_color(FG_MUTED),
            )
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(S2))
                    .child(
                        field()
                            .flex_1()
                            .min_w(px(0.0))
                            .value(move || value.get())
                            .on_change(move |v: &str| value.set(v.to_owned()))
                            .placeholder("relay.paseo.sh")
                            .font_family(|| MONO)
                            .text_size(px(TEXT_SM)),
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
