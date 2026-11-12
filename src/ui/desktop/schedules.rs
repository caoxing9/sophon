//! Schedules: prompts that start agents on a cadence.

use crate::i18n::t;
use nsh_core::{element::FontWeight, prelude::*};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::{daemon::connection::{Schedule, ScheduleRun}, store};

pub(crate) fn schedules(compact: bool) -> impl IntoElement {
    let list = computed(store::schedules);
    scroll().flex_1().child(
        view()
            .w_full()
            .max_w(px(820.0))
            .self_center()
            .px(sp(S6))
            .py(sp(S8))
            .gap(sp(S6))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home)))
                    .child(text(t("Schedules")).text_size(tx(TEXT_2XL)).font_weight(FontWeight(500)).flex_1())
                    .child(btn(t("New schedule"), Variant::Primary, Sz::Md, Some(Icon::Plus), || {
                        store::open_modal(store::Modal::NewSchedule)
                    })),
            )
            .child(dynamic(move || {
                let items = list.get();
                Some(untracked(move || match items.clone() {
                    None => loading().into_element(),
                    Some(items) if items.is_empty() => view()
                        .items_center()
                        .gap(sp(S2))
                        .py(sp(S8))
                        .child(icon(Icon::CalendarClock, 24.0).text_color(FG_FAINT))
                        .child(text(t("No schedules yet")).text_size(tx(TEXT_BASE)).text_color(FG_MUTED))
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

/// A time the service gave (UTC) on this computer's clock: `11-11 09:00`,
/// with the seconds for a run.
fn when_label(at: &Option<String>) -> String {
    at.as_deref().map(|t| local_time(t, false)).unwrap_or_else(|| "—".into())
}

fn local_time(at: &str, seconds: bool) -> String {
    let Some(utc) = crate::usage::parse::rfc3339(at) else { return at.get(..16).unwrap_or(at).replace('T', " ") };
    let local = utc + crate::usage::local_offset(utc);
    let (_, month, day) = crate::usage::civil(local.div_euclid(86_400));
    let of_day = local.rem_euclid(86_400);
    let (h, m, s) = (of_day / 3600, of_day % 3600 / 60, of_day % 60);
    if seconds { format!("{month:02}-{day:02} {h:02}:{m:02}:{s:02}") } else { format!("{month:02}-{day:02} {h:02}:{m:02}") }
}

/// How long a run took: `42s`, `3m 05s`.
fn took(run: &ScheduleRun) -> Option<String> {
    let parse = crate::usage::parse::rfc3339;
    let secs = (parse(run.ended.as_deref()?)? - parse(&run.started)?).max(0);
    Some(if secs < 60 { format!("{secs}s") } else { format!("{}m {:02}s", secs / 60, secs % 60) })
}

fn row(s: Schedule, first: bool) -> impl IntoElement {
    let (tone, label) = match s.status.as_str() {
        "active" => (SUCCESS, t("Active")),
        "paused" => (WARNING, t("Paused")),
        _ => (FG_MUTED, t("Completed")),
    };
    let paused = s.status == "paused";
    let (a, b, c, d, e, f) = (s.id.clone(), s.id.clone(), s.id.clone(), s.id.clone(), s.id.clone(), s.id.clone());
    let edit = s.clone();
    let shown = computed(move || store::schedule_runs(&e).is_some());
    let head = view()
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .p(sp(S4))
        .child(icon(Icon::CalendarClock, 18.0).text_color(FG_MUTED))
        .child(
            view()
                .flex_1()
                .gap(px(3.0))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(text(s.name.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)))
                        .child(badge(label, tone)),
                )
                .child(text(s.prompt.clone()).text_size(tx(TEXT_SM)).text_color(FG).number_of_lines(2))
                .child(
                    text(format!(
                        "{} · {} · {} {} · {} {}",
                        s.cadence,
                        s.target,
                        t("next"),
                        when_label(&s.next_run),
                        t("last"),
                        when_label(&s.last_run)
                    ))
                    .text_size(tx(TEXT_SM))
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
        .child(btn(t("Runs"), Variant::Outline, Sz::Sm, Some(Icon::History), move || store::toggle_schedule_runs(&d)))
        .child(icon_btn(Icon::Pencil, Sz::Sm, t("Edit schedule"), move || store::open_modal(store::Modal::EditSchedule(edit.clone()))))
        .child(icon_btn(Icon::Trash2, Sz::Sm, t("Delete schedule"), move || store::schedule_op(&c, "delete")));
    view().when(!first, |v| v.border_t_1().border_color(BORDER)).child(head).child(dynamic(move || {
        if !shown.get() {
            return None;
        }
        let runs = store::schedule_runs(&f)?;
        Some(untracked(move || runs_list(runs)))
    }))
}

/// A schedule's runs under its row, newest first.
fn runs_list(runs: Vec<ScheduleRun>) -> View {
    let mut list = view().px(sp(S4)).pb(sp(S3)).pl(px(S4 + 18.0 + S3)).gap(px(2.0));
    if runs.is_empty() {
        return list.child(text(t("No runs yet")).text_size(tx(TEXT_SM)).text_color(FG_MUTED));
    }
    // The last twenty: the page is a list of schedules, not a log.
    for run in runs.into_iter().take(20) {
        let (tone, label) = match run.status.as_str() {
            "succeeded" => (SUCCESS, t("Succeeded")),
            "failed" => (DANGER, t("Failed")),
            _ => (WARNING, t("Running")),
        };
        let mut line = view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .h(sp(CONTROL_SM))
            .child(text(local_time(&run.started, true)).text_size(tx(TEXT_SM)).text_color(FG).font_family(mono_font))
            .child(badge(label, tone));
        if let Some(took) = took(&run) {
            line = line.child(text(took).text_size(tx(TEXT_SM)).text_color(FG_MUTED));
        }
        line = line.child(
            text(run.error.clone().unwrap_or_default()).text_size(tx(TEXT_SM)).text_color(DANGER).number_of_lines(1).flex_1(),
        );
        if let Some(agent) = run.agent_id.clone() {
            let workspace = run.workspace_id.clone();
            line = line.child(btn(t("Open conversation"), Variant::Ghost, Sz::Sm, None, move || {
                store::open_run_agent(&agent, workspace.as_deref())
            }));
        }
        list = list.child(line);
        // What the agent answered: usually all there is to read of a run.
        if let Some(output) = run.output.clone() {
            list = list.child(
                text(output).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED).number_of_lines(3).pb(sp(S2)),
            );
        }
    }
    list
}
