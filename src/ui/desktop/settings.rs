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
                .child(section(t("Host behavior"), behavior_card()))
                .child(section(t("Providers"), providers_card()))
                .child(section(t("Text generation"), text_generation_card())),
        ))
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
