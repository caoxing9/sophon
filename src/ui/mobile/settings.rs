//! The phone's settings tab: how it looks and sends, and the computer it
//! follows. The computer's own settings are the desktop's.

use crate::i18n::t;
use nsh_core::prelude::*;

use crate::store;
use crate::ui::kit::*;
use crate::ui::shared::settings::*;

pub(crate) fn settings() -> impl IntoElement {
    store::load_agents();
    view()
        .flex_1()
        .child(page(
            true,
            view()
                .child(section(t("Appearance"), appearance_card()))
                .child(section(t("Sending"), sending_card()))
                .child(section(t("Notifications"), notifications_card()))
                .child(section(t("Storage"), storage_card()))
                .child(section(
                    t("Computer"),
                    host_card()
                        .child(row(
                            t("Name"),
                            || store::current_host().map(|h| h.label()).unwrap_or_default(),
                            btn(t("Rename"), Variant::Outline, Sz::Sm, Some(Icon::Pencil), || {
                                if let Some(h) = store::current_host() {
                                    let name = h.label();
                                    store::open_modal(store::Modal::RenameHost { id: h.id, name });
                                }
                            }),
                            false,
                        ))
                        .child(row(
                        t("Forget this computer"),
                        t("Pair it again from its QR code"),
                        btn(t("Forget"), Variant::Outline, Sz::Sm, Some(Icon::Trash2), || {
                            if let Some(h) = store::current_host() {
                                store::remove_host(&h.id);
                            }
                        }),
                        false,
                    )),
                ))
                .child(section(
                    t("Feedback"),
                    card().child(row(
                        t("Report an issue"),
                        t("A bug, an idea or a question, as an issue on GitHub"),
                        btn(t("Report…"), Variant::Outline, Sz::Sm, Some(Icon::GitPullRequest), || store::open_modal(store::Modal::ReportIssue)),
                        true,
                    )),
                ))
                .child(section(t("Coding agents"), crate::ui::shared::agents::agents_card(false)))
                // The last card rests above the floating bar, as the other
                // tabs' lists do: the page's own foot and the gap before
                // this make up the rest of the bar's room.
                .child(view().h(px(super::home::BAR_ROOM - S4 - S6))),
        ))
}

/// Push notifications from the computer in use: whether this phone wants
/// them, and whether they say what they are about.
fn notifications_card() -> View {
    card()
        .child(row(
            t("Push notifications"),
            // Said plainly when the computer cannot send any.
            move || {
                if store::push_served() || !store::settings().push.0 {
                    t("When an agent waits for you or finishes; an Apple Watch shows them too")
                } else {
                    t("This computer sends none yet: it needs a relay server of its own with the push gateway")
                }
                .to_owned()
            },
            switch(|| store::settings().push.0, |on| store::set_push(on, store::settings().push_hide)),
            true,
        ))
        .child(row(
            t("Hide content"),
            t("Notifications say only that something needs you, not what"),
            switch(|| store::settings().push_hide, |hide| store::set_push(store::settings().push.0, hide)),
            false,
        ))
}
