//! Notices over the main surface, with an undo for archiving: a toast at
//! its top right, under the title bar, clear of the composer.

use crate::i18n::t;
use nsh_core::prelude::*;

use crate::store;
use crate::ui::kit::*;
use crate::ui::kit::text;

pub(crate) fn notice_bar(top: Memo<Length>) -> impl IntoElement {
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
    // Over the surface from its top: placed in the flow, after what fills
    // the surface, the bar was laid out from the surface's foot, below the
    // window.
    view()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .pointer_events(PointerEvents::BoxNone)
        .child(dynamic(move || {
        let (m, action) = message.get()?;
        Some(untracked(move || {
            // A toast as shadcn's (Sonner): a card at the surface's top
            // right, clear of the page's title row and of the composer.
            view()
                .absolute()
                // Under a page's title row, whose right end has the
                // page's actions (Save, Create, Refresh).
                .top(move || top.get())
                .mt(px(64.0))
                .right(sp(S4))
                .pointer_events(PointerEvents::BoxNone)
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .min_w(px(240.0))
                        .max_w(px(420.0))
                        .pl(sp(S3))
                        .pr(sp(S1))
                        .py(sp(S2))
                        .rounded(px(R_LG))
                        .bg(POPOVER())
                        .border_1()
                        .border_color(BORDER())
                        .shadow_lg()
                        .child(icon(Icon::Info, 15.0).text_color(FG_MUTED()))
                        .child(
                            text(m.clone())
                                .text_size(tx(TEXT_SM))
                                .text_color(FG())
                                .number_of_lines(2)
                                .flex_1(),
                        )
                        .when(action.is_some(), |v| v.child(btn(t("Undo"), Variant::Outline, Sz::Sm, None, store::run_notice_action)))
                        .child(icon_btn(Icon::X, Sz::Sm, t("Close"), store::dismiss_notice)),
                )
                .into_element()
        }))
    }))
}
