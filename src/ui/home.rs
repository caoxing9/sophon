//! The main surface with no workspace open.

use crate::i18n::t;
use nsh_core::{element::FontWeight, prelude::*};
use super::kit::text;

use super::kit::*;
use crate::store;

pub(crate) fn home() -> impl IntoElement {
    view().flex_1().items_center().justify_center().gap(px(S4)).p(px(S8)).child(
        view()
            .items_center()
            .gap(px(S3))
            .child(
                view()
                    .w(px(44.0))
                    .h(px(44.0))
                    .rounded(px(R_XL))
                    .bg(tint(ACCENT, 0.12))
                    .items_center()
                    .justify_center()
                    .child(icon(Icon::Bot, 22.0).text_color(ACCENT)),
            )
            .child(text(t("Select a workspace")).text_size(px(TEXT_XL)).font_weight(FontWeight(500)))
            .child(
                text(t("Pick one in the sidebar, or start a new workspace with an agent"))
                    .text_size(px(TEXT_BASE))
                    .text_color(FG_MUTED)
                    .text_center(),
            )
            .child(
                view().pt(px(S2)).child(btn(t("New workspace"), Variant::Primary, Sz::Md, Some(Icon::SquarePen), || {
                    store::open_new_workspace(None)
                })),
            ),
    )
}
