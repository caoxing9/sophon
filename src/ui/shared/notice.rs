//! The notice over the foot of the main surface, with its undo.

use crate::i18n::t;
use nsh_core::prelude::*;

use crate::store;
use crate::ui::kit::*;
use crate::ui::kit::text;

pub(crate) fn notice_bar() -> impl IntoElement {
    let message = computed(store::notice);
    // Each notice goes after a while. Timed here, where the bar lives: a
    // timer started where the notice was raised ends with that place (a
    // sheet that closes as it raises one), and the notice stayed for ever.
    let id = computed(store::notice_id);
    effect(move || {
        if let Some(id) = id.get() {
            set_timeout(std::time::Duration::from_secs(8), move || store::dismiss_notice_numbered(id));
        }
    });
    view().child(dynamic(move || {
        let (m, action) = message.get()?;
        Some(untracked(move || {
            view()
                .absolute()
                .bottom(px(96.0))
                .left(px(0.0))
                .right(px(0.0))
                .items_center()
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .max_w(px(560.0))
                        .pl(px(S4))
                        .pr(px(S1))
                        .py(px(S1))
                        .rounded(px(R_FULL))
                        .bg(FG)
                        .shadow_lg()
                        .child(icon(Icon::Info, 15.0).text_color(SURFACE0))
                        .child(
                            text(m.clone())
                                .text_size(px(TEXT_SM))
                                .text_color(SURFACE0)
                                .number_of_lines(2)
                                .flex_shrink(1.0),
                        )
                        .when(action.is_some(), |v| {
                            v.child(
                                nsh_base::button()
                                    .on_press(store::run_notice_action)
                                    .px(px(S3))
                                    .h(px(26.0))
                                    .rounded(px(R_FULL))
                                    .items_center()
                                    .justify_center()
                                    .bg(tint(SURFACE0, 0.18))
                                    .child(text(t("Undo")).text_size(px(TEXT_SM)).font_weight(nsh_core::element::FontWeight(600)).text_color(SURFACE0)),
                            )
                        })
                        .child(
                            nsh_base::button()
                                .on_press(store::dismiss_notice)
                                .w(px(26.0))
                                .h(px(26.0))
                                .rounded(px(R_FULL))
                                .items_center()
                                .justify_center()
                                .child(icon(Icon::X, 14.0).text_color(SURFACE0)),
                        ),
                )
                .into_element()
        }))
    }))
}
