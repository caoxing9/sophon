//! Schedules, as Paseo's: prompts that start agents on a cadence.

use crate::i18n::t;
use nsh_core::{element::FontWeight, prelude::*};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::{daemon::connection::Schedule, store};

pub(crate) fn schedules(compact: bool) -> impl IntoElement {
    let list = computed(store::schedules);
    scroll().flex_1().child(
        view()
            .w_full()
            .max_w(px(820.0))
            .self_center()
            .px(px(S6))
            .py(px(S8))
            .gap(px(S6))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(S2))
                    .when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home)))
                    .child(text(t("Schedules")).text_size(px(TEXT_2XL)).font_weight(FontWeight(500)).flex_1())
                    .child(btn(t("New schedule"), Variant::Primary, Sz::Md, Some(Icon::Plus), || {
                        store::open_modal(store::Modal::NewSchedule)
                    })),
            )
            .child(dynamic(move || {
                let items = list.get();
                Some(untracked(move || match items.clone() {
                    None => text(t("Loading…")).text_color(FG_MUTED).into_element(),
                    Some(items) if items.is_empty() => view()
                        .items_center()
                        .gap(px(S2))
                        .py(px(S8))
                        .child(icon(Icon::CalendarClock, 24.0).text_color(FG_FAINT))
                        .child(text(t("No schedules yet")).text_size(px(TEXT_BASE)).text_color(FG_MUTED))
                        .into_element(),
                    Some(items) => {
                        let mut c = card();
                        for (i, s) in items.into_iter().enumerate() {
                            c = c.child(row(s, i == 0));
                        }
                        c.into_element()
                    }
                }))
            })),
    )
}

fn when_label(at: &Option<String>) -> String {
    at.as_deref().map(|t| t.get(..16).unwrap_or(t).replace('T', " ")).unwrap_or_else(|| "—".into())
}

fn row(s: Schedule, first: bool) -> impl IntoElement {
    let (tone, label) = match s.status.as_str() {
        "active" => (SUCCESS, t("Active")),
        "paused" => (WARNING, t("Paused")),
        _ => (FG_MUTED, t("Completed")),
    };
    let paused = s.status == "paused";
    let (a, b, c) = (s.id.clone(), s.id.clone(), s.id.clone());
    view()
        .flex_row()
        .items_center()
        .gap(px(S4))
        .p(px(S4))
        .when(!first, |v| v.border_t_1().border_color(BORDER))
        .child(icon(Icon::CalendarClock, 18.0).text_color(FG_MUTED))
        .child(
            view()
                .flex_1()
                .gap(px(3.0))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .child(text(s.name.clone()).text_size(px(TEXT_BASE)).font_weight(FontWeight(500)))
                        .child(badge(label, tone)),
                )
                .child(text(s.prompt.clone()).text_size(px(TEXT_SM)).text_color(FG).number_of_lines(2))
                .child(
                    text(format!(
                        "{} · {} · next {} · last {}",
                        s.cadence,
                        s.target,
                        when_label(&s.next_run),
                        when_label(&s.last_run)
                    ))
                    .text_size(px(TEXT_SM))
                    .text_color(FG_MUTED)
                    .number_of_lines(1),
                ),
        )
        .child(btn(t("Run now"), Variant::Outline, Sz::Sm, Some(Icon::Play), move || store::schedule_op(&a, "run-once")))
        .child(btn(
            if paused { t("Resume") } else { t("Pause") },
            Variant::Outline,
            Sz::Sm,
            None,
            move || store::schedule_op(&b, if paused { "resume" } else { "pause" }),
        ))
        .child(icon_btn(Icon::Trash2, Sz::Sm, t("Delete schedule"), move || store::schedule_op(&c, "delete")))
}
