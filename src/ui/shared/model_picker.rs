//! The composer's model picker, as TeaBranch's: a rail of the starred
//! models and of each agent, a search over all of them, and the models of
//! the one picked, each with a star.
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
fn mark(provider: &str, size: f32) -> impl IntoElement {
    match provider {
        "claude" => icon(Icon::Claude, size),
        "codex" => icon(Icon::Codex, size).text_color(FG),
        _ => icon(Icon::Cpu, size).text_color(FG_MUTED),
    }
}

/// The picker: its pill (the agent's mark and the model) and its panel.
pub(crate) fn model_picker(live: bool, current_name: Memo<String>) -> impl IntoElement {
    let open = signal(false);
    let provider = computed(move || store::controls(live).provider);
    popover()
        .open(move || Some(open.get()))
        .on_open_change(move |e: &PopoverEvent| {
            if let PopoverEvent::OpenChange(next, _) = e {
                open.set(*next);
            }
        })
        .placement(nsh_core::overlay::Placement::Top)
        .align(nsh_core::overlay::Align::Start)
        .offset(px(6.0))
        .popup_style(|s: nsh_core::element::StyleRefinement| {
            s.bg(POPOVER).rounded(px(14.0)).border_1().border_color(BORDER).shadow_lg().overflow_hidden()
        })
        .trigger(
            view()
                .flex_row()
                .items_center()
                .gap(px(5.0))
                .h(px(CONTROL_SM))
                .px(px(S2))
                .min_w(px(0.0))
                .flex_shrink(1.0)
                .rounded(px(R_FULL))
                .hover(|s| s.bg(highlight()))
                .child(dynamic(move || {
                    let p = provider.get();
                    Some(untracked(move || mark(&p, 15.0).into_element()))
                }))
                .child(
                    text(move || current_name.get())
                        .text_size(px(TEXT_SM))
                        .font_weight(FontWeight(500))
                        .text_color(FG_MUTED)
                        .number_of_lines(1)
                        .max_w(px(200.0))
                        .flex_shrink(1.0),
                )
                .child(icon(Icon::ChevronDown, 13.0).text_color(FG_FAINT)),
        )
        .child(panel(live, open))
}

fn panel(live: bool, open: Signal<bool>) -> impl IntoElement {
    let c = store::controls(live);
    let tab = signal(Tab::Agent(c.provider.clone()));
    let query = signal(String::new());
    let providers = computed(|| store::providers().into_iter().filter(|p| p.enabled).collect::<Vec<_>>());
    view()
        .flex_row()
        .w(px(400.0))
        .h(px(380.0))
        .child(rail(providers, tab))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .px(px(S3))
                        .h(px(40.0))
                        .border_b_1()
                        .border_color(BORDER)
                        .child(icon(Icon::Search, 14.0).text_color(FG_MUTED))
                        .child(
                            nsh_core::element::input()
                                .value(move || query.get())
                                .on_change(move |v: &str| query.set(v.to_owned()))
                                .placeholder(t("Search models"))
                                .auto_focus(true)
                                .flex_1()
                                .text_size(px(TEXT_BASE))
                                .text_color(FG)
                                .caret_color(|| color(ACCENT)),
                        ),
                )
                .child(list(live, open, providers, tab, query)),
        )
}

/// The rail: the stars, then each agent (one unavailable faded).
fn rail(providers: Memo<Vec<Provider>>, tab: Signal<Tab>) -> impl IntoElement {
    let button = move |target: Tab, body: AnyElement, label: String, faded: bool| {
        let t2 = target.clone();
        nsh_base::button()
            .on_press(move || tab.set(target.clone()))
            .accessibility_label(label)
            .w(px(32.0))
            .h(px(32.0))
            .rounded(px(R_MD))
            .items_center()
            .justify_center()
            .opacity(if faded { 0.45 } else { 1.0 })
            .bg(move || if tab.get() == t2 { highlight() } else { tint(FG, 0.0) })
            .hover(|s| s.bg(highlight()))
            .child(body)
    };
    view().w(px(44.0)).items_center().gap(px(S1)).p(px(6.0)).border_r_1().border_color(BORDER).child(dynamic(move || {
        let list = providers.get();
        Some(untracked(move || {
            let mut col = view().gap(px(S1)).child(button(
                Tab::Favorites,
                icon(Icon::Star, 16.0).text_color(FG_MUTED).into_element(),
                t("Starred").into(),
                false,
            ));
            for p in list {
                let faded = !p.available();
                col = col.child(button(Tab::Agent(p.id.clone()), mark(&p.id, 16.0).into_element(), p.label.clone(), faded));
            }
            col.into_element()
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
            let mut body = view().p(px(S1));
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
                                    .py(px(S2)),
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
                        return r.name.to_lowercase().contains(&needle) || r.model.to_lowercase().contains(&needle);
                    }
                    match &tab {
                        Tab::Favorites => favs.contains(&r.key()),
                        Tab::Agent(id) => &r.provider == id,
                    }
                })
                .collect();
            if rows.is_empty() {
                let why = if needle.is_empty() && tab == Tab::Favorites { t("No starred models yet") } else { t("No matching models") };
                return note(why.into(), String::new());
            }
            let show_agent = !needle.is_empty() || tab == Tab::Favorites;
            // The agent's own models first, then those a gateway serves.
            let mut rows = rows;
            rows.sort_by_key(|r| r.gateway);
            // Claude: each family's newest, as Claude Code lists them; the
            // earlier ones folded under a row of their own.
            let older = if tab == Tab::Agent("claude".into()) && needle.is_empty() {
                older_rows(&rows, c.model.as_deref().unwrap_or(""))
            } else {
                Default::default()
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
                body = body.child(row(live, open, r, selected, show_agent, &favs));
            }
            if !earlier.is_empty() {
                let count = earlier.len().to_string();
                body = body.child(
                    nsh_base::button()
                        .on_press(move || older_open.update(|o| *o = !*o))
                        .flex_row()
                        .items_center()
                        .gap(px(S1))
                        .h(px(30.0))
                        .px(px(S2))
                        .mt(px(S1))
                        .rounded(px(R_LG))
                        .hover(|s| s.bg(highlight()))
                        .child(text(tf("Earlier versions ({})", &[&count])).text_size(px(TEXT_SM)).text_color(FG_MUTED).flex_1())
                        .child(icon(if show_older { Icon::ChevronUp } else { Icon::ChevronDown }, 14.0).text_color(FG_FAINT)),
                );
                if show_older {
                    for r in earlier {
                        body = body.child(row(live, open, r, false, false, &favs));
                    }
                }
            }
            scroll().flex_1().child(body).into_element()
        }))
    }))
}

fn row(live: bool, open: Signal<bool>, r: Row, selected: bool, show_agent: bool, favs: &[String]) -> impl IntoElement {
    let starred = favs.contains(&r.key());
    let (pick, key) = (r.clone(), r.key());
    view()
        .flex_row()
        .items_center()
        .h(px(32.0))
        .px(px(S1))
        .rounded(px(R_LG))
        .bg(if selected { highlight() } else { tint(FG, 0.0) })
        .hover(|s| s.bg(highlight()))
        .child(
            nsh_base::button()
                .on_press(move || {
                    choose(live, &pick);
                    open.set(false);
                })
                .flex_1()
                .min_w(px(0.0))
                .h_full()
                .flex_row()
                .items_center()
                .gap(px(S2))
                .px(px(6.0))
                .child(text(r.name.clone()).text_size(px(TEXT_BASE)).text_color(FG).number_of_lines(1).flex_1().min_w(px(0.0)))
                .when(r.subscription, |v| {
                    v.child(
                        view()
                            .px(px(5.0))
                            .h(px(16.0))
                            .justify_center()
                            .rounded(px(R_SM))
                            .border_1()
                            .border_color(tint(WARNING, 0.5))
                            .child(text(t("Subscription")).text_size(px(10.0)).text_color(WARNING)),
                    )
                })
                .when(show_agent, |v| v.child(text(r.agent.clone()).text_size(px(TEXT_XS)).text_color(FG_FAINT))),
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
                .child(if starred {
                    icon(Icon::StarFilled, 14.0).text_color(WARNING).into_element()
                } else {
                    icon(Icon::Star, 14.0).text_color(FG_FAINT).into_element()
                }),
        )
        .child(view().w(px(24.0)).items_center().child(if selected {
            icon(Icon::Check, 14.0).text_color(FG_MUTED).into_element()
        } else {
            view().into_element()
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
