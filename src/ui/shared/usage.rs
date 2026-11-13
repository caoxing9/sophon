//! Usage: what the agents used on the computer, from their own session
//! logs (tokens, cache, an API-price estimate, by model and by day), and
//! the plan limits the daemon reports, with their trend and a forecast.
//! The desktop's page and the phone's tab: the phone has no logs, so the
//! computer's Sophon reads its own and sends the sums.

use crate::i18n::{t, tf};
use nsh_base::plot::{Curve, Scale, line, plot};
use nsh_core::{
    element::{FontWeight, Rows, View},
    prelude::*,
};

use crate::ui::kit::text;
use crate::ui::kit::*;
use crate::{
    daemon::connection::{ProviderUsage, UsageWindow},
    store,
    usage::{self, Range, Stats, Totals, limits, local_offset},
};

pub(crate) fn usage_page(compact: bool) -> impl IntoElement {
    let range = signal(Range::Week);
    let busy = computed(|| store::local_usage().loading);
    scroll().flex_1().child(
        view()
            .w_full()
            .max_w(px(CONTENT_MAX_WIDTH + 80.0))
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
                    .child(text(t("Usage")).text_size(tx(TEXT_2XL)).font_weight(FontWeight(500)).flex_1())
                    .child(dynamic(move || busy.get().then(|| untracked(|| text(t("Reading session logs…")).text_size(tx(TEXT_SM)).text_color(FG_MUTED)))))
                    .child(icon_btn(Icon::RotateCw, Sz::Md, t("Refresh"), store::refresh_usage)),
            )
            .child(history_section(range))
            .child(limits_section()),
    )
}

/// Both halves without a page around them, for the phone's tab.
pub(crate) fn usage_sections() -> View {
    let range = signal(Range::Week);
    view().gap(sp(S6)).child(history_section(range)).child(limits_section())
}

/// The session-log half: the range switch, totals, the days and the models.
fn history_section(range: Signal<Range>) -> View {
    let local = computed(store::local_usage);
    // The phone's numbers are the computer's own, so nothing to explain.
    let remote = computed(|| !MOBILE && !store::on_this_mac());
    view()
        .gap(sp(S4))
        .child(
            // On the phone the title and the switch do not fit one line.
            view()
                .when(!MOBILE, |v| v.flex_row().items_center())
                .gap(sp(S3))
                .child(
                    text(if MOBILE { t("The computer's sessions") } else { t("This computer's sessions") })
                        .text_size(tx(TEXT_LG))
                        .font_weight(FontWeight(500))
                        .when(!MOBILE, |v| v.flex_1()),
                )
                .child(segmented(
                    vec![(Range::Today, t("Today").into()), (Range::Week, t("7 days").into()), (Range::Month, t("30 days").into())],
                    move || range.get(),
                    move |r| range.set(r),
                )),
        )
        .child(dynamic(move || {
            remote.get().then(|| {
                untracked(|| {
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(icon(Icon::Info, 14.0).text_color(FG_MUTED))
                        .child(
                            text(t("Connected to another computer: these numbers cover the sessions on this one only. The limits below come from the connected host"))
                                .text_size(tx(TEXT_SM))
                                .text_color(FG_MUTED)
                                .flex_1(),
                        )
                })
            })
        }))
        .child(dynamic(move || {
            let (report, loading) = local.with(|l| (l.report.clone(), l.loading));
            let r = range.get();
            Some(untracked(move || match report {
                None if loading => muted(t("Reading session logs…")).into_element(),
                None if cfg!(any(target_os = "ios", target_os = "android", target_arch = "wasm32")) => muted(t("Session statistics come from Sophon on the computer: pair with it there (not with the service alone) to see them")).into_element(),
                None => muted(t("No session logs found")).into_element(),
                Some(report) => {
                    let stats = report.range(r).clone();
                    view()
                        .gap(sp(S4))
                        .child(totals(&stats.totals))
                        .when(r != Range::Today, |v| v.child(days_chart(&stats, report.last_day)))
                        .child(models_table(&stats))
                        .child(
                            text(t("Cost is an estimate at API list prices, not your subscription bill. Models without a known price are left out of it"))
                                .text_size(tx(TEXT_XS))
                                .text_color(FG_FAINT),
                        )
                        .into_element()
                }
            }))
        }))
}

fn muted(s: &str) -> nsh_core::element::Text {
    text(s.to_owned()).text_size(tx(TEXT_BASE)).text_color(FG_MUTED)
}

fn tile(label: &str, value: String, note: Option<String>) -> View {
    view()
        .flex_1()
        .min_w(px(120.0))
        .p(sp(S3))
        .gap(px(2.0))
        .rounded(px(R_XL))
        .border_1()
        .border_color(BORDER)
        .bg(SURFACE0)
        .child(text(label.to_owned()).text_size(tx(TEXT_XS)).text_color(FG_MUTED).number_of_lines(1))
        .child(text(value).text_size(tx(TEXT_XL)).font_weight(FontWeight(500)).font_family(mono_font).number_of_lines(1))
        .when_some(note, |v, n| v.child(text(n).text_size(tx(TEXT_XS)).text_color(FG_FAINT).number_of_lines(2)))
}

fn hit_rate(t: &usage::Tokens) -> String {
    t.cache_hit_rate().map_or_else(|| "—".into(), |r| format!("{:.0}%", r * 100.0))
}

fn totals(tot: &Totals) -> View {
    let k = tot.tokens;
    let unpriced = (tot.unpriced > 0).then(|| tf("{} tokens unpriced", &[&usage::compact(tot.unpriced)]));
    view()
        .flex_row()
        .flex_wrap()
        .gap(sp(S2))
        .child(tile(t("Est. cost"), usage::dollars(tot.cost), unpriced))
        .child(tile(t("Input"), usage::compact(k.input), None))
        .child(tile(t("Output"), usage::compact(k.output), None))
        .child(tile(t("Cache read"), usage::compact(k.cache_read), None))
        .child(tile(t("Cache write"), usage::compact(k.cache_write), None))
        .child(tile(t("Cache hit rate"), hit_rate(&k), Some(tf("{} responses", &[&tot.responses.to_string()]))))
}

/// A bar per day: the estimated cost, or the tokens when nothing is priced.
fn days_chart(stats: &Stats, last_day: i64) -> View {
    let by_cost = stats.days.iter().any(|d| d.cost > 0.0);
    let value = |d: &Totals| if by_cost { d.cost } else { d.tokens.total() as f64 };
    let max = stats.days.iter().map(value).fold(0.0, f64::max).max(f64::MIN_POSITIVE);
    let n = stats.days.len() as i64;
    let mut bars = view().flex_row().items_end().gap(px(if n > 10 { 3.0 } else { 8.0 })).h(px(120.0));
    for (i, d) in stats.days.iter().enumerate() {
        let v = value(d);
        let label = if by_cost { usage::dollars(v) } else { usage::compact(v as u64) };
        bars = bars.child(
            view()
                .flex_1()
                .h_full()
                .justify_end()
                .tooltip(glass_strong(view()).px(sp(S2)).py(sp(S1)).rounded(px(R_LG)).child(text(format!("{} · {label}", usage::day_label(last_day - n + 1 + i as i64))).text_size(tx(TEXT_XS))))
                .child(
                    view()
                        .w_full()
                        .h(relative((v / max) as f32))
                        .min_h(px(if v > 0.0 { 2.0 } else { 0.0 }))
                        .rounded_t(px(R_SM))
                        .bg(if i as i64 == n - 1 { ACCENT } else { tint(ACCENT, 0.55) }),
                ),
        );
    }
    let axis = view()
        .flex_row()
        .child(text(usage::day_label(last_day - n + 1)).text_size(tx(TEXT_XS)).text_color(FG_FAINT).flex_1())
        .child(text(t("Today")).text_size(tx(TEXT_XS)).text_color(FG_FAINT));
    card()
        .p(sp(S4))
        .gap(sp(S2))
        .child(section_label(if by_cost { t("Estimated cost per day") } else { t("Tokens per day") }))
        .child(bars)
        .child(axis)
}

const NUM_W: f32 = 76.0;

fn num_cell(s: String, strong: bool) -> nsh_core::element::Text {
    text(s)
        .w(px(NUM_W))
        .text_right()
        .text_size(tx(TEXT_SM))
        .font_family(mono_font)
        .text_color(if strong { FG } else { FG_MUTED })
        .number_of_lines(1)
}

fn head_cell(s: &str) -> nsh_core::element::Text {
    text(s.to_owned()).w(px(NUM_W)).text_right().text_size(tx(TEXT_XS)).text_color(FG_FAINT).number_of_lines(1)
}

fn models_table(stats: &Stats) -> View {
    if stats.models.is_empty() {
        return view().child(muted(t("No sessions in this range")));
    }
    // The phone: a model a row, its cost at the end and its tokens under
    // its name (seven columns do not fit).
    if MOBILE {
        let mut c = card();
        for (i, m) in stats.models.iter().enumerate() {
            let k = m.totals.tokens;
            let cost = if m.priced { usage::dollars(m.totals.cost) } else { t("unpriced").to_owned() };
            c = c.child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S3))
                    .px(sp(S4))
                    .py(sp(S3))
                    .when(i > 0, |v| v.border_t_1().border_color(BORDER))
                    .child(icon(if m.source == usage::Source::Claude { Icon::Claude } else { Icon::Codex }, 16.0).text_color(FG_MUTED))
                    .child(
                        view()
                            .flex_1()
                            .min_w(px(0.0))
                            .gap(px(2.0))
                            .child(text(m.model.clone()).text_size(tx(TEXT_SM)).font_family(mono_font).number_of_lines(1))
                            .child(
                                text(tf("{} in · {} out · {} cached", &[&usage::compact(k.input), &usage::compact(k.output), &hit_rate(&k)]))
                                    .text_size(tx(TEXT_XS))
                                    .text_color(FG_FAINT)
                                    .number_of_lines(1),
                            ),
                    )
                    .child(text(cost).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(if m.priced { FG } else { FG_MUTED })),
            );
        }
        return c;
    }
    let mut c = card().child(
        view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .px(sp(S4))
            .py(sp(S2))
            .border_b_1()
            .border_color(BORDER)
            .child(text(t("Model")).text_size(tx(TEXT_XS)).text_color(FG_FAINT).flex_1())
            .child(head_cell(t("Input")))
            .child(head_cell(t("Output")))
            .child(head_cell(t("Cache read")))
            .child(head_cell(t("Cache write")))
            .child(head_cell(t("Hit rate")))
            .child(head_cell(t("Est. cost"))),
    );
    for m in &stats.models {
        let k = m.totals.tokens;
        let cost = if m.priced { usage::dollars(m.totals.cost) } else { t("unpriced").to_owned() };
        c = c.child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .px(sp(S4))
                .py(sp(S2))
                .border_t_1()
                .border_color(BORDER)
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(icon(if m.source == usage::Source::Claude { Icon::Claude } else { Icon::Codex }, 14.0).text_color(FG_MUTED))
                        .child(text(m.model.clone()).text_size(tx(TEXT_SM)).font_family(mono_font).number_of_lines(1).flex_shrink(1.0)),
                )
                .child(num_cell(usage::compact(k.input), false))
                .child(num_cell(usage::compact(k.output), false))
                .child(num_cell(usage::compact(k.cache_read), false))
                .child(num_cell(usage::compact(k.cache_write), false))
                .child(num_cell(hit_rate(&k), false))
                .child(num_cell(cost, m.priced)),
        );
    }
    c
}

/// The plan limits, as the daemon reports them, each with its trend.
fn limits_section() -> View {
    // Only the agents this host has and has on (still loading counts): the
    // service also reports providers nobody uses here (a stale Cursor login).
    let data = computed(|| {
        let usage = store::usage()?;
        let agents = store::providers();
        if agents.is_empty() {
            return Some(usage);
        }
        Some(usage.into_iter().filter(|u| agents.iter().any(|a| a.id == u.id && a.enabled)).collect::<Vec<_>>())
    });
    let history = computed(store::limit_history);
    view()
        .gap(sp(S4))
        .child(text(t("Plan limits")).text_size(tx(TEXT_LG)).font_weight(FontWeight(500)))
        .child(dynamic(move || {
            let d = data.get();
            let h = history.get();
            Some(untracked(move || match d {
                None => loading_block(140.0).into_element(),
                Some(list) if list.is_empty() => muted(t("No usage reported by your providers")).into_element(),
                Some(list) => {
                    let mut col = view().flex_row().flex_wrap().gap(sp(S4));
                    for p in list {
                        col = col.child(provider_card(&p, &h));
                    }
                    col.into_element()
                }
            }))
        }))
}

fn provider_card(p: &ProviderUsage, history: &limits::History) -> View {
    let mut c = card().flex_1().min_w(px(300.0)).p(sp(S4)).gap(sp(S4)).child(
        view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .child(text(p.name.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)))
            .child(text(p.plan.clone().unwrap_or_default()).text_size(tx(TEXT_SM)).text_color(FG_MUTED)),
    );
    if let Some(e) = p.error.clone() {
        c = c.child(text(e).text_size(tx(TEXT_SM)).text_color(DANGER));
    }
    for w in &p.windows {
        let points = history.windows.get(&store::limit_key(p, w)).map(Vec::as_slice).unwrap_or_default();
        c = c.child(window_row(w, points));
    }
    c
}

fn window_row(w: &UsageWindow, points: &[limits::Point]) -> View {
    let now = usage::now();
    let used = w.used.unwrap_or(0.0).clamp(0.0, 100.0);
    let tone = if used >= 90.0 { DANGER } else if used >= 70.0 { WARNING } else { ACCENT };
    let resets = w.resets_at.as_deref().and_then(usage::parse::rfc3339);
    let length = limits::window_length(&w.id, &w.label);
    let current = limits::current_window(points, resets);
    let runs_out = w.used.and_then(|pct| limits::forecast(current, now, pct, resets, length));
    let mut v = view()
        .gap(px(5.0))
        .child(
            view()
                .flex_row()
                .child(text(w.label.clone()).text_size(tx(TEXT_SM)).flex_1())
                .child(text(format!("{used:.0}%")).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(FG_MUTED)),
        )
        .child(
            view()
                .h(px(6.0))
                .rounded(px(R_FULL))
                .bg(SURFACE3)
                .child(view().h_full().w(relative(used as f32 / 100.0)).rounded(px(R_FULL)).bg(tone)),
        );
    if current.len() >= 2 {
        v = v.child(trend(current, resets, length, tone));
    }
    let mut foot = view().flex_row().gap(sp(S2));
    if let Some(r) = resets {
        foot = foot.child(text(tf("Resets {}", &[&usage::time_label(r, &local_offset)])).text_size(tx(TEXT_XS)).text_color(FG_FAINT).flex_1());
    }
    if let Some(out) = runs_out {
        foot = foot.child(
            text(tf("At this pace, used up around {}", &[&usage::time_label(out, &local_offset)]))
                .text_size(tx(TEXT_XS))
                .font_weight(FontWeight(500))
                .text_color(WARNING),
        );
    }
    v.child(foot)
}

/// The window's readings over its span: from its start (when its length is
/// known) or the first reading, to its reset.
fn trend(points: &[limits::Point], resets: Option<i64>, length: Option<i64>, tone: AdaptiveColor) -> View {
    let first = points.first().map_or(0, |p| p.at);
    let last = points.last().map_or(0, |p| p.at);
    let start = match (resets, length) {
        (Some(r), Some(len)) => (r - len).min(first),
        _ => first,
    };
    let end = resets.unwrap_or(last).max(last + 1);
    let rows = Rows::from_columns(
        points.len(),
        [
            ("t".into(), points.iter().map(|p| p.at as f64).collect()),
            ("pct".into(), points.iter().map(|p| p.pct).collect()),
        ],
    );
    view().h(px(44.0)).border_b_1().border_color(BORDER).child(
        plot()
            .data(rows)
            .x(nsh_core::element::RowAccessor::from("t"))
            .y(nsh_core::element::RowAccessor::from("pct"))
            .x_scale(Scale::Linear { domain: Some([start as f64, end as f64]), reverse: false })
            .y_scale(Scale::Linear { domain: Some([0.0, 100.0]), reverse: false })
            .interactive(false)
            .accessibility_label(t("Use over this window"))
            .text_color(tone)
            .flex_1()
            .child(line().curve(Curve::Linear)),
    )
}
