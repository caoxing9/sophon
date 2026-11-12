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
                .child(section(t("Storage"), storage_card()))
                .child(section(
                    t("Computer"),
                    host_card().child(row(
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
                .child(section(t("Coding agents"), crate::ui::shared::agents::agents_card(false)))
                // The last card rests above the floating bar, as the other
                // tabs' lists do: the page's own foot and the gap before
                // this make up the rest of the bar's room.
                .child(view().h(px(super::home::BAR_ROOM - S4 - S6))),
        ))
}
