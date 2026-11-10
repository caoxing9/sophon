//! The composer's model picker, as T3 Code's: a rail of the starred models
//! and of each agent (an account of its own is an agent), a search over all
//! of them, the models of the one picked (starred, numbered ⌘1…⌘9, the
//! earlier ones under "Show more"), the model's reasoning levels beside
//! them, and a footer with its speed, its other switches and the access
//! mode.
//!
//! The rail is the harness: a model of the open agent's own provider
//! switches its model in place; a model of another agent continues the
//! conversation in that agent (a fork, [`store::continue_in`]), never runs
//! inside this one. Claude Code lists Anthropic's models only: one an
//! Anthropic-compatible gateway names (`provider/model`) is left out.

use crate::i18n::{t, tf};
use nsh_base::{PopoverEvent, popover};
use nsh_core::{element::FontWeight, prelude::*};

use crate::daemon::model::Provider;
use crate::store::{self, Control};
use crate::ui::kit::text;
use crate::ui::kit::*;

/// What the rail shows.
#[derive(Clone, PartialEq)]
enum Tab {
    Favorites,
    Agent(String),
}

/// A model the picker lists.
#[derive(Clone, PartialEq)]
struct Row {
    provider: String,
    agent: String,
    model: String,
    name: String,
    /// Through a gateway the agent was pointed at (Magpie): a
    /// `provider/model` id, not one of the agent's own.
    gateway: bool,
    /// A subscription signed in to another client (Copilot's, ChatGPT's
    /// through Magpie): its account answers for the use.
    subscription: bool,
}



impl Row {
    fn key(&self) -> String {
        format!("{}:{}", self.provider, self.model)
    }
}

/// The models `p` runs itself: Claude Code's are Anthropic's (no
/// `provider/model` ids of a gateway).
fn rows_of(p: &Provider) -> Vec<Row> {
    // The agent's own list on this computer first (Claude Code's catalog,
    // Codex's): it names models the service's built-in list may not have
    // yet, in the agent's order.
    if let Some(local) = store::local_models(&p.id) {
        let row = |id: &str, name: &str| {
            let prefix = id.split_once('/').map(|(p, _)| p);
            Row {
                provider: p.id.clone(),
                agent: p.label.clone(),
                model: id.to_owned(),
                name: name.to_owned(),
                gateway: prefix.is_some(),
                subscription: prefix.is_some_and(|p| store::SUBSCRIPTION_PREFIXES.contains(&p)),
            }
        };
        let mut rows: Vec<Row> = local.iter().map(|m| row(&m.id, &m.name)).collect();
        // The service's extras (1M variants), not its bare aliases (`fable`
        // from settings.json): the agent's list names their model.
        for m in &p.models {
            let alias = matches!(m.id.as_str(), "opus" | "sonnet" | "haiku" | "fable" | "default");
            if !alias && !rows.iter().any(|r| r.model == m.id) {
                rows.push(row(&m.id, &m.label));
            }
        }
        return rows
            .into_iter()
            .filter(|r| p.id != "claude" || !r.model.contains('/'))
            .filter(|r| !store::forbidden_model(&r.model, &r.name))
            .collect();
    }
    p.models
        .iter()
        .filter(|m| p.id != "claude" || !m.id.contains('/'))
        // A Claude model through a subscription is not offered at all.
        .filter(|m| !store::forbidden_model(&m.id, &m.label))
        .map(|m| {
            let prefix = m.id.split_once('/').map(|(p, _)| p);
            Row {
                provider: p.id.clone(),
                agent: p.label.clone(),
                model: m.id.clone(),
                name: m.label.clone(),
                gateway: prefix.is_some() || m.description.contains("via magpie"),
                subscription: prefix.is_some_and(|p| store::SUBSCRIPTION_PREFIXES.contains(&p)),
            }
        })
        .collect()
}

/// The agent's mark: Claude's and Codex's (OpenAI's) own, else a glyph.
pub(crate) fn mark(provider: &str, size: f32) -> impl IntoElement {
    match provider {
        "claude" => icon(Icon::Claude, size),
        "codex" => icon(Icon::Codex, size).text_color(FG),
        _ => icon(Icon::Cpu, size).text_color(FG_MUTED),
    }
}

thread_local! {
    /// While the panel is open: picks its `n`th numbered model (⌘1…⌘9).
    static DIGITS: std::cell::RefCell<Option<std::rc::Rc<dyn Fn(usize)>>> = const { std::cell::RefCell::new(None) };
}

/// ⌘`n` while the picker is open: its `n`th model, not the `n`th tab.
pub(crate) fn take_digit(n: usize) -> bool {
    let Some(f) = DIGITS.with_borrow(Clone::clone) else { return false };
    f(n);
    true
}

/// Hover and keyboard highlight of a row (T3's `foreground/9%`).
fn row_hover() -> AdaptiveColor {
    tint(FG, 0.09)
}

/// The panel's columns' fill (T3's `muted/40`).
fn column_fill() -> AdaptiveColor {
    tint(FG, 0.025)
}

/// Its dividers (T3's `border/70`).
fn divider() -> AdaptiveColor {
    tint(BORDER, 0.7)
}

/// The picker as T3 Code's: a chip with the agent's mark, the model and its
/// reasoning, opening a panel of three columns (agents, models, reasoning)
/// over a footer (speed and the agent's other switches, the access mode).
pub(crate) fn model_picker(live: bool, current_name: Memo<String>) -> impl IntoElement {
    let open = signal(false);
    let provider = computed(move || store::controls(live).provider);
    let suffix = computed(move || {
        let c = store::controls(live);
        let opts = crate::ui::shared::composer::thinking_options(live);
        let thinking = c.thinking.as_deref().and_then(|t| opts.iter().find(|o| o.id == t)).map(|o| o.label.clone());
        let fast = store::features(live).iter().any(|f| is_speed(&f.id) && matches!(f.value, Ok(true)));
        (thinking, fast)
    });
    // ⌘1…⌘9 pick while it is open; the tabs get them back when it closes.
    effect(move || {
        if !open.get() {
            DIGITS.with_borrow_mut(|d| *d = None);
        }
    });
    popover()
        .open(move || Some(open.get()))
        .on_open_change(move |e: &PopoverEvent| {
            if let PopoverEvent::OpenChange(next, _) = e {
                open.set(*next);
            }
        })
        .placement(nsh_core::overlay::Placement::Top)
        .align(nsh_core::overlay::Align::Start)
        .offset(px(4.0))
        .popup_style(|s: nsh_core::element::StyleRefinement| {
            s.bg(POPOVER).rounded(px(R_LG)).border_1().border_color(tint(FG, 0.10)).shadow_lg().overflow_hidden()
        })
        .trigger(
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .h(px(CONTROL_SM))
                .px(px(S2))
                .min_w(px(0.0))
                .flex_shrink(1.0)
                .rounded(px(R_MD))
                .label(t("Model"))
                .group()
                .hover(|s| s.bg(highlight()))
                .bg(move || if open.get() { highlight() } else { tint(FG, 0.0) })
                .child(dynamic(move || {
                    let p = provider.get();
                    Some(untracked(move || mark(&p, 16.0).into_element()))
                }))
                .child(
                    text(move || current_name.get())
                        .text_size(px(TEXT_SM))
                        .font_weight(FontWeight(500))
                        .text_color(tint(FG, 0.8))
                        .number_of_lines(1)
                        .max_w(px(224.0))
                        .flex_shrink(1.0),
                )
                .child(dynamic(move || {
                    let (thinking, fast) = suffix.get();
                    if thinking.is_none() && !fast {
                        return None;
                    }
                    Some(untracked(move || {
                        let tone = move || if open.get() { color(FG) } else { color(FG_MUTED) };
                        let mut v = view().flex_row().items_center().gap(px(4.0)).flex_shrink_0();
                        if let Some(t) = thinking {
                            v = v.child(text(t).text_size(px(TEXT_SM)).font_weight(FontWeight(500)).text_color(tone).group_hover(|s| s.text_color(FG)));
                        }
                        if fast {
                            v = v.child(icon(Icon::Zap, 12.0).text_color(tone).group_hover(|s| s.text_color(FG)));
                        }
                        v.into_element()
                    }))
                })),
        )
        .child(panel(live, open))
}

/// Whether a feature is the agent's speed (Claude's fast mode, Codex's
/// service tier): the footer's segmented control, Standard | Fast.
fn is_speed(id: &str) -> bool {
    matches!(id, "fast_mode" | "fastMode" | "fast" | "service_tier" | "serviceTier")
}

fn panel(live: bool, open: Signal<bool>) -> impl IntoElement {
    let c = store::controls(live);
    let favorites_now = store::favorite_models();
    // T3: the stars first when there are any, else the agent in use.
    let tab = signal(if favorites_now.is_empty() { Tab::Agent(c.provider.clone()) } else { Tab::Favorites });
    let query = signal(String::new());
    let providers = computed(|| store::providers().into_iter().filter(|p| p.enabled).collect::<Vec<_>>());
    let searching = computed(move || !query.get().trim().is_empty());
    view()
        .child(
            view()
                .flex_row()
                .h(px(346.0))
                .child(
                    view()
                        .flex_row()
                        .w(px(360.0))
                        .overflow_hidden()
                        .child(when(move || !searching.get(), move || rail(providers, tab)))
                        .child(
                            view()
                                .flex_1()
                                .min_w(px(0.0))
                                .bg(column_fill())
                                .when(true, |v| v.border_l_1().border_color(divider()))
                                .child(search(query))
                                .child(list(live, open, providers, tab, query)),
                        ),
                )
                .child(reasoning(live)),
        )
        .child(footer(live))
}

/// The search: an underline that takes the accent while it has the keys.
fn search(query: Signal<String>) -> impl IntoElement {
    view().px(px(S3)).pt(px(10.0)).child(
        view()
            .flex_row()
            .items_center()
            .gap(px(S1))
            .pb(px(6.0))
            .border_b_1()
            .border_color(divider())
            .focus_within(|s| s.border_color(ACCENT))
            .child(icon(Icon::Search, 16.0).text_color(tint(FG_MUTED, 0.55)))
            .child(
                nsh_core::element::input()
                    .value(move || query.get())
                    .on_change(move |v: &str| query.set(v.to_owned()))
                    .placeholder(t("Search models…"))
                    .auto_focus(true)
                    .flex_1()
                    .h(px(26.0))
                    .text_size(px(TEXT_BASE))
                    .text_color(FG)
                    .caret_color(|| color(ACCENT)),
            ),
    )
}

/// An agent's initials, on its mark when another instance of the same CLI
/// is on too (two Claude accounts): `Claude Work` is `CW`.
fn initials(label: &str) -> String {
    label.split_whitespace().filter_map(|w| w.chars().next()).take(2).collect::<String>().to_uppercase()
}

/// The rail: the stars, a hairline, then each agent; the one shown marked by
/// an accent bar at its edge, one unavailable faded.
fn rail(providers: Memo<Vec<Provider>>, tab: Signal<Tab>) -> impl IntoElement {
    let button = move |target: Tab, body: AnyElement, label: String, faded: bool, badge: Option<String>| {
        let t2 = target.clone();
        let tooltip = tip(label.clone());
        let b = nsh_base::button()
            .on_press(move || tab.set(target.clone()))
            .accessibility_label(label)
            .w(px(36.0))
            .h(px(36.0))
            .rounded(px(R_MD))
            .items_center()
            .justify_center()
            .opacity(if faded { 0.5 } else { 1.0 })
            .hover(|s| s.bg(tint(FG, 0.10)))
            .child(view().w(px(24.0)).h(px(24.0)).items_center().justify_center().child(body).when_some(badge, |v, b| {
                v.child(
                    view()
                        .absolute()
                        .right(px(-3.0))
                        .bottom(px(-3.0))
                        .h(px(12.0))
                        .min_w(px(12.0))
                        .px(px(2.0))
                        .rounded(px(R_FULL))
                        .items_center()
                        .justify_center()
                        .bg(POPOVER)
                        .border_1()
                        .border_color(tint(FG, 0.15))
                        .child(text(b).text_size(px(7.0)).font_weight(FontWeight(600)).text_color(FG_MUTED)),
                )
            }))
            .child(
                view()
                    .absolute()
                    .right(px(0.0))
                    .top(px(8.0))
                    .w(px(3.0))
                    .h(px(20.0))
                    .rounded(px(R_FULL))
                    .bg(ACCENT)
                    .opacity(move || if tab.get() == t2 { 1.0_f32 } else { 0.0 }),
            );
        view().tooltip(tooltip).child(b)
    };
    view().w(px(44.0)).flex_shrink_0().bg(tint(FG, 0.015)).child(dynamic(move || {
        let list = providers.get();
        let cfg = store::daemon_config();
        Some(untracked(move || {
            let family = |p: &Provider| crate::daemon::agents::family(&p.id, cfg.as_ref());
            let mut col = view()
                .p(px(S1))
                .gap(px(S1))
                .child(button(Tab::Favorites, icon(Icon::StarFilled, 20.0).text_color(FG).into_element(), t("Favorites").into(), false, None))
                .child(view().h(px(1.0)).bg(divider()));
            for p in &list {
                let shared = list.iter().filter(|q| family(q) == family(p)).count() > 1;
                let badge = shared.then(|| initials(&p.label));
                col = col.child(button(Tab::Agent(p.id.clone()), mark(&family(p), 20.0).into_element(), p.label.clone(), !p.available(), badge));
            }
            scroll().flex_1().child(col).into_element()
        }))
    }))
}

/// A Claude model's family and version, from its label: `Opus 4.8 1M` is
/// (`opus`, [4, 8]).
fn family_version(label: &str) -> Option<(String, Vec<u32>)> {
    let mut words = label.split_whitespace();
    let family = words.next()?.to_lowercase();
    if !matches!(family.as_str(), "opus" | "sonnet" | "haiku" | "fable") {
        return None;
    }
    let version = words.next()?.split('.').map(|n| n.parse().ok()).collect::<Option<Vec<u32>>>()?;
    Some((family, version))
}

/// Claude Code's own list (`/model`) has each family's newest model: the
/// rows older than their family's newest, the selected one kept.
fn older_rows(rows: &[Row], selected: &str) -> std::collections::HashSet<String> {
    let mut newest: std::collections::HashMap<String, Vec<u32>> = std::collections::HashMap::new();
    for r in rows {
        if let Some((family, version)) = family_version(&r.name) {
            let best = newest.entry(family).or_default();
            if version > *best {
                *best = version;
            }
        }
    }
    rows.iter()
        .filter(|r| r.model != selected)
        .filter(|r| family_version(&r.name).is_some_and(|(family, version)| newest.get(&family).is_some_and(|best| version < *best)))
        .map(|r| r.key())
        .collect()
}

fn list(live: bool, open: Signal<bool>, providers: Memo<Vec<Provider>>, tab: Signal<Tab>, query: Signal<String>) -> impl IntoElement {
    let favorites = computed(store::favorite_models);
    let older_open = signal(false);
    view().flex_1().min_h(px(0.0)).child(dynamic(move || {
        let tab = tab.get();
        let needle = query.get().trim().to_lowercase();
        let all: Vec<Provider> = providers.get();
        let favs = favorites.get();
        let c = store::controls(live);
        let show_older = older_open.get();
        Some(untracked(move || {
            let mut body = view().pl(px(S2)).pr(px(1.0)).py(px(6.0)).gap(px(2.0));
            // Another agent, in an open conversation: picking continues there.
            if let Tab::Agent(id) = &tab {
                if needle.is_empty() {
                    if let Some(p) = all.iter().find(|p| &p.id == id) {
                        if !p.available() {
                            return note(
                                tf("{} is not available", &[&p.label]),
                                p.error.clone().unwrap_or_else(|| t("Not installed on this computer, or it can't start").into()),
                            );
                        }
                        if live && p.id != c.provider {
                            body = body.child(
                                text(tf("Continues in {}: a new agent, with this conversation", &[&p.label]))
                                    .text_size(px(TEXT_XS))
                                    .text_color(FG_MUTED)
                                    .px(px(S2))
                                    .py(px(S1)),
                            );
                        }
                    }
                }
            }
            let rows: Vec<Row> = all
                .iter()
                .filter(|p| p.available())
                .flat_map(rows_of)
                .filter(|r| {
                    if !needle.is_empty() {
                        return r.name.to_lowercase().contains(&needle) || r.model.to_lowercase().contains(&needle) || r.agent.to_lowercase().contains(&needle);
                    }
                    match &tab {
                        Tab::Favorites => favs.contains(&r.key()),
                        Tab::Agent(id) => &r.provider == id,
                    }
                })
                .collect();
            if rows.is_empty() {
                let why = if needle.is_empty() && tab == Tab::Favorites { t("No favorites yet") } else { t("No matching models") };
                DIGITS.with_borrow_mut(|d| *d = None);
                return note(why.into(), String::new());
            }
            let show_agent = !needle.is_empty() || tab == Tab::Favorites;
            // The agent's own models first, then those a gateway serves.
            let mut rows = rows;
            rows.sort_by_key(|r| r.gateway);
            // A search puts the starred first, as T3's ranking does.
            if !needle.is_empty() {
                rows.sort_by_key(|r| !favs.contains(&r.key()));
            }
            // Claude: each family's newest, as Claude Code lists them; the
            // earlier ones folded under "Show more" (the starred stay).
            let older = if tab == Tab::Agent("claude".into()) && needle.is_empty() {
                let selected = c.model.as_deref().unwrap_or("");
                match store::local_models("claude") {
                    Some(local) => rows
                        .iter()
                        .filter(|r| r.model != selected && !local.iter().any(|m| m.main && m.id == r.model))
                        .map(Row::key)
                        .filter(|k| !favs.contains(k))
                        .collect(),
                    None => older_rows(&rows, selected).into_iter().filter(|k| !favs.contains(k)).collect(),
                }
            } else {
                std::collections::HashSet::new()
            };
            let (rows, earlier): (Vec<Row>, Vec<Row>) = rows.into_iter().partition(|r| !older.contains(&r.key()));
            // A bare alias Claude Code resolves to a family's newest (`opus`
            // from settings.json) repeats that row: shown only while in use.
            let rows: Vec<Row> = rows
                .into_iter()
                .filter(|r| {
                    r.provider != "claude"
                        || !matches!(r.model.as_str(), "opus" | "sonnet" | "haiku" | "fable" | "default")
                        || c.model.as_deref() == Some(r.model.as_str())
                        || !needle.is_empty()
                })
                .collect();
            let mut shown: Vec<Row> = rows.clone();
            if show_older {
                shown.extend(earlier.iter().cloned());
            }
            // ⌘1…⌘9: the first nine shown.
            let numbered: Vec<Row> = shown.iter().take(9).cloned().collect();
            let pick_nth = numbered.clone();
            DIGITS.with_borrow_mut(|d| {
                *d = Some(std::rc::Rc::new(move |n: usize| {
                    if let Some(r) = pick_nth.get(n.wrapping_sub(1)) {
                        pick(live, open, r);
                    }
                }))
            });
            let number = |r: &Row| numbered.iter().position(|x| x == r).map(|i| i + 1);
            let mut gateway_heading = false;
            for r in rows {
                if r.gateway && !gateway_heading && matches!(tab, Tab::Agent(_)) && needle.is_empty() {
                    gateway_heading = true;
                    body = body.child(
                        view()
                            .px(px(S2))
                            .pt(px(S3))
                            .pb(px(S1))
                            .gap(px(2.0))
                            .child(text(t("Through Magpie")).text_size(px(TEXT_XS)).font_weight(FontWeight(500)).text_color(FG_FAINT))
                            .child(
                                text(t("Still this agent's harness; the requests go to another provider"))
                                    .text_size(px(TEXT_XS))
                                    .text_color(FG_FAINT),
                            ),
                    );
                }
                let selected = r.provider == c.provider && c.model.as_deref() == Some(r.model.as_str());
                let n = number(&r);
                body = body.child(row(live, open, r, selected, show_agent, &favs, n));
            }
            if show_older {
                for r in earlier.iter().cloned() {
                    let n = number(&r);
                    body = body.child(row(live, open, r, false, false, &favs, n));
                }
            }
            if !earlier.is_empty() {
                body = body.child(
                    nsh_base::button()
                        .on_press(move || older_open.update(|o| *o = !*o))
                        .flex_row()
                        .items_center()
                        .h(px(32.0))
                        .px(px(S2))
                        .rounded(px(R_SM))
                        .group()
                        .hover(|s| s.bg(row_hover()))
                        .child(
                            text(if show_older { t("Show less") } else { t("Show more") })
                                .text_size(px(TEXT_BASE))
                                .text_color(FG_MUTED)
                                .group_hover(|s| s.text_color(FG)),
                        ),
                );
            }
            scroll().flex_1().child(body).into_element()
        }))
    }))
}

/// A model's row: its mark and name, a check when in use, then on the right
/// a star (seen under the pointer, or when starred) and its ⌘ number.
fn row(live: bool, open: Signal<bool>, r: Row, selected: bool, show_agent: bool, favs: &[String], number: Option<usize>) -> impl IntoElement {
    let starred = favs.contains(&r.key());
    let (picked, key) = (r.clone(), r.key());
    view()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .min_h(px(32.0))
        .pr(px(S2))
        .rounded(px(R_SM))
        .group()
        .hover(|s| s.bg(row_hover()))
        .child(
            nsh_base::button()
                .on_press(move || pick(live, open, &picked))
                .flex_1()
                .min_w(px(0.0))
                .h(px(32.0))
                .flex_row()
                .items_center()
                .gap(px(S2))
                .pl(px(S2))
                .child(mark(&r.provider, 16.0))
                .child(text(r.name.clone()).text_size(px(TEXT_BASE)).text_color(FG).number_of_lines(1).flex_shrink(1.0).min_w(px(0.0)))
                .when(selected, |v| v.child(icon(Icon::Check, 14.0).text_color(FG_MUTED)))
                .when(r.subscription, |v| {
                    v.child(
                        view()
                            .px(px(4.0))
                            .h(px(16.0))
                            .justify_center()
                            .rounded(px(R_SM))
                            .border_1()
                            .border_color(tint(WARNING, 0.5))
                            .child(text(t("Subscription")).text_size(px(10.0)).text_color(WARNING)),
                    )
                })
                .when(show_agent, |v| v.child(text(r.agent.clone()).text_size(px(TEXT_XS)).text_color(tint(FG_MUTED, 0.7)).number_of_lines(1)))
                .child(view().flex_1()),
        )
        .child(
            nsh_base::button()
                .on_press(move || store::toggle_favorite_model(&key))
                .accessibility_label(if starred { t("Unstar") } else { t("Star") })
                .w(px(24.0))
                .h(px(24.0))
                .rounded(px(R_MD))
                .items_center()
                .justify_center()
                .opacity(if starred { 1.0 } else { 0.0 })
                .group_hover(|s| s.opacity(1.0))
                .hover(|s| s.bg(tint(FG, 0.08)))
                .child(if starred {
                    icon(Icon::StarFilled, 12.0).text_color(WARNING).into_element()
                } else {
                    icon(Icon::Star, 12.0).text_color(FG_MUTED).into_element()
                }),
        )
        .when_some(number.filter(|_| !MOBILE), |v, n| v.child(keycap(&format!("⌘{n}"))))
}

/// A tooltip's bubble.
fn tip(label: String) -> View {
    glass_strong(view()).px(px(S2)).py(px(S1)).rounded(px(R_LG)).child(text(label).text_size(px(TEXT_XS)))
}

/// A shortcut's keycap (T3's `Kbd`).
fn keycap(label: &str) -> impl IntoElement {
    view()
        .h(px(20.0))
        .min_w(px(20.0))
        .px(px(4.0))
        .rounded(px(R_SM))
        .items_center()
        .justify_center()
        .bg(tint(FG, 0.07))
        .child(text(label.to_owned()).text_size(px(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED))
}

/// A model picked: the panel stays open for its reasoning (T3's docked
/// picker), unless it continues in another agent.
fn pick(live: bool, open: Signal<bool>, r: &Row) {
    let elsewhere = live && r.provider != store::controls(live).provider;
    choose(live, r);
    if elsewhere {
        open.set(false);
    }
}

/// The reasoning column: the model's levels, the one in use checked, the
/// agent's default marked so.
fn reasoning(live: bool) -> impl IntoElement {
    view()
        .w(px(192.0))
        .flex_shrink_0()
        .border_l_1()
        .border_color(divider())
        .bg(column_fill())
        .child(dynamic(move || {
            let c = store::controls(live);
            let opts = crate::ui::shared::composer::thinking_options(live);
            let default = default_thinking(live);
            let model = model_label(live);
            Some(untracked(move || {
                let current = c.thinking.clone().or(default.clone()).or_else(|| opts.first().map(|o| o.id.clone()));
                let mut body = view().px(px(S2)).pt(px(10.0)).pb(px(6.0)).gap(px(2.0));
                if opts.is_empty() {
                    body = body.child(view().h(px(32.0)).px(px(S2)).justify_center().child(text(t("No effort levels")).text_size(px(TEXT_BASE)).text_color(FG_MUTED)));
                }
                for o in opts {
                    let on = current.as_deref() == Some(o.id.as_str());
                    let is_default = default.as_deref() == Some(o.id.as_str());
                    let id = o.id.clone();
                    let mut b = nsh_base::button()
                        .on_press(move || store::set_control(live, Control::Thinking(id.clone())))
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .h(px(32.0))
                        .px(px(S2))
                        .rounded(px(R_SM))
                        .hover(|s| s.bg(row_hover()))
                        .child(text(o.label.clone()).text_size(px(TEXT_BASE)).text_color(FG).flex_1().number_of_lines(1));
                    b = if on {
                        b.child(icon(Icon::Check, 14.0).text_color(FG))
                    } else if is_default {
                        b.child(text(t("Default")).text_size(px(TEXT_SM)).text_color(tint(FG_MUTED, 0.7)))
                    } else {
                        b
                    };
                    body = if o.description.is_empty() { body.child(b) } else { body.child(view().tooltip(tip(o.description.clone())).child(b)) };
                }
                view()
                    .flex_1()
                    .child(
                        view().px(px(S3)).pt(px(10.0)).child(
                            view()
                                .flex_row()
                                .items_center()
                                .gap(px(S2))
                                .h(px(33.0))
                                .pb(px(6.0))
                                .border_b_1()
                                .border_color(divider())
                                .child(text(t("Reasoning")).text_size(px(TEXT_BASE)).text_color(FG_MUTED))
                                .child(view().flex_1())
                                .child(text(model).text_size(px(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1).flex_shrink(1.0)),
                        ),
                    )
                    .child(scroll().flex_1().child(body))
                    .into_element()
            }))
        }))
}

/// The level the agent uses when none is picked: its own list's, else the
/// service's.
fn default_thinking(live: bool) -> Option<String> {
    let c = store::controls(live);
    let model = c.model.as_deref()?;
    if let Some(local) = store::local_models(&c.provider).and_then(|l| l.into_iter().find(|m| m.id == model)) {
        if local.default_thinking.is_some() {
            return local.default_thinking;
        }
    }
    store::provider(&c.provider)?.model(model)?.default_thinking.clone()
}

/// The model in use, by name: the reasoning column's heading.
fn model_label(live: bool) -> String {
    let c = store::controls(live);
    let Some(model) = c.model else { return String::new() };
    store::local_models(&c.provider)
        .and_then(|l| l.into_iter().find(|m| m.id == model).map(|m| m.name))
        .or_else(|| store::provider(&c.provider).and_then(|p| p.model(&model).map(|m| m.label.clone())))
        .unwrap_or(model)
}

/// The footer: the agent's speed as Standard | Fast, its other switches each
/// a toggle, its choices as segments, and the access mode on the right.
fn footer(live: bool) -> impl IntoElement {
    view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(S2))
        .px(px(S3))
        .py(px(S2))
        .min_h(px(45.0))
        .border_t_1()
        .border_color(divider())
        .bg(column_fill())
        .child(dynamic(move || {
            let features = store::features(live);
            Some(untracked(move || {
                let mut row = view().flex_row().flex_wrap().items_center().gap(px(S2));
                for f in features {
                    let id = f.id.clone();
                    row = row.child(match f.value.clone() {
                        Ok(on) if is_speed(&f.id) => segments(vec![
                            (t("Standard").to_owned(), None, !on, { let id = id.clone(); std::rc::Rc::new(move || store::set_feature(live, &id, false.into())) as std::rc::Rc<dyn Fn()> }),
                            (t("Fast").to_owned(), Some(Icon::Zap), on, { let id = id.clone(); std::rc::Rc::new(move || store::set_feature(live, &id, true.into())) as std::rc::Rc<dyn Fn()> }),
                        ]),
                        Ok(on) => segments(vec![(f.label.clone(), Some(Icon::Workflow), on, std::rc::Rc::new(move || store::set_feature(live, &id, (!on).into())) as std::rc::Rc<dyn Fn()>)]),
                        Err((value, options)) => segments(
                            options
                                .into_iter()
                                .map(|o| {
                                    let (id, oid) = (id.clone(), o.id.clone());
                                    let on = value.as_deref() == Some(o.id.as_str());
                                    let glyph = (is_speed(&f.id) && o.id != "default").then_some(Icon::Zap);
                                    (o.label.clone(), glyph, on, std::rc::Rc::new(move || store::set_feature(live, &id, oid.clone().into())) as std::rc::Rc<dyn Fn()>)
                                })
                                .collect(),
                        ),
                    });
                }
                row.into_element()
            }))
        }))
        .child(view().flex_1())
        .child(access_mode(live))
}

/// A segmented group (T3's toggle group): its segments' label, glyph,
/// whether on, and what pressing does.
fn segments(items: Vec<(String, Option<Icon>, bool, std::rc::Rc<dyn Fn()>)>) -> AnyElement {
    let mut group = view().flex_row().rounded(px(R_LG)).bg(tint(FG, 0.05)).p(px(2.0));
    for (label, glyph, on, press) in items {
        let ink = if on { FG } else { FG_MUTED };
        let mut seg = nsh_base::button()
            .on_press(move || press())
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(4.0))
            .h(px(24.0))
            .px(px(S2))
            .rounded(px(R_MD))
            .bg(if on { tint(FG, 0.10) } else { tint(FG, 0.0) })
            .group()
            .aria_pressed(|s| s)
            .accessibility_label(label.clone());
        if let Some(g) = glyph {
            seg = seg.child(icon(g, 12.0).text_color(ink).group_hover(|s| s.text_color(FG)));
        }
        seg = seg.child(text(label).text_size(px(TEXT_SM)).text_color(ink).group_hover(|s| s.text_color(FG)));
        group = group.child(seg);
    }
    group.into_element()
}

/// The access mode's glyph: what the agent may do without asking.
fn mode_icon(id: &str) -> Icon {
    let id = id.to_lowercase();
    if id.contains("bypass") || id.contains("full") || id.contains("yolo") || id.contains("dangerous") {
        Icon::ShieldOff
    } else if id.contains("accept") || id.contains("edit") || id.contains("auto") {
        Icon::PenLine
    } else if id.contains("plan") {
        Icon::ListTodo
    } else {
        Icon::Lock
    }
}

/// The access mode: its glyph and name, opening its choices upward.
fn access_mode(live: bool) -> impl IntoElement {
    view().child(dynamic(move || {
        let c = store::controls(live);
        if c.modes.is_empty() {
            return None;
        }
        Some(untracked(move || {
            let current = c.mode.as_deref().and_then(|m| c.modes.iter().find(|o| o.id == m)).or(c.modes.first()).cloned();
            let (id, label) = current.map(|o| (o.id, o.label)).unwrap_or_default();
            let mut menu = dropup(
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .h(px(28.0))
                    .px(px(S2))
                    .rounded(px(R_MD))
                    .group()
                    .hover(|s| s.bg(row_hover()))
                    .child(icon(mode_icon(&id), 14.0).text_color(FG_MUTED).group_hover(|s| s.text_color(FG)))
                    .child(text(label).text_size(px(TEXT_SM)).text_color(FG_MUTED).group_hover(|s| s.text_color(FG)))
                    .child(icon(Icon::ChevronUp, 12.0).text_color(FG_MUTED)),
            )
            .child(menu_heading(t("Mode")));
            for o in &c.modes {
                let oid = o.id.clone();
                menu = menu.child(menu_row(
                    o.label.clone(),
                    Some(mode_icon(&o.id)),
                    Some(o.description.clone().into()),
                    c.mode.as_deref() == Some(o.id.as_str()),
                    move || store::set_control(live, Control::Mode(oid.clone())),
                ));
            }
            menu.into_element()
        }))
    }))
}

/// A model picked: in a draft, the agent and its model; in an open
/// conversation, its own provider's switches in place and another's
/// continues in that agent.
fn choose(live: bool, r: &Row) {
    let c = store::controls(live);
    if !live {
        store::set_control(false, Control::Provider(r.provider.clone()));
        store::set_control(false, Control::Model(r.model.clone()));
    } else if r.provider == c.provider {
        store::set_control(true, Control::Model(r.model.clone()));
    } else {
        store::continue_in(&r.provider, &r.model);
    }
}

fn note(title: String, detail: String) -> AnyElement {
    view()
        .flex_1()
        .p(px(S4))
        .gap(px(S1))
        .child(text(title).text_size(px(TEXT_SM)).text_color(FG_MUTED))
        .when(!detail.is_empty(), |v| v.child(text(detail).text_size(px(TEXT_XS)).text_color(FG_FAINT)))
        .into_element()
}
