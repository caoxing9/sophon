//! The Agents settings section, after T3 Code's providers settings: one
//! row per provider (Paseo's word for an agent CLI or an instance of one)
//! with its status and sign-in at a glance; a row opens to its details
//! (where its CLI and files are, its sign-in, its name, updating or
//! removing it); on the desktop also the switch that turns it on, and the
//! steps that add a provider: another account, signed in here, or an API
//! endpoint. Instances are separate providers: nothing switches accounts
//! on its own.

use crate::i18n::{t, tf};
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use serde_json::Value;

use crate::daemon::Provider;
use crate::daemon::agents::{self, AgyAuth, Base, Diagnostic, KeyInstance, PRESETS};
use crate::store::{self, Modal, SignIn};
use crate::ui::kit::*;
use crate::ui::kit::text;
use crate::ui::shared::model_picker::mark;
use crate::ui::shared::modals::{actions, sheet};
use nsh_base::{ButtonElement, button};
use nsh_core::element::AdaptiveColor;

/// The card of agents. `editable`: the switches, the details' changes and
/// adding (the desktop); the phone only reads.
pub(crate) fn agents_card(editable: bool) -> impl IntoElement {
    // The provider whose details are open.
    let open: Signal<Option<String>> = signal(None);
    card().child(dynamic(move || {
        let list = store::providers();
        let cfg = store::daemon_config();
        let diags: Vec<Option<Diagnostic>> = list.iter().map(|p| store::diagnostic(&p.id)).collect();
        let updates: Vec<Option<store::AgentUpdate>> =
            diags.iter().map(|d| d.as_ref().and_then(|d| store::agent_update(d.path.as_deref()?))).collect();
        Some(untracked(move || {
            let mut v = view();
            if list.is_empty() {
                v = v.child(loading_block(120.0));
            }
            for (i, ((p, d), u)) in list.into_iter().zip(diags).zip(updates).enumerate() {
                v = v.child(agent_row(p, d, u, cfg.as_ref(), editable, i == 0, open));
            }
            if editable {
                v = v.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .pl(sp(S2))
                        .pr(sp(S3))
                        .py(sp(S2))
                        .border_t_1()
                        .border_color(BORDER())
                        .child(
                            btn(t("Add provider"), Variant::Ghost, Sz::Md, Some(Icon::Plus), || store::open_modal(Modal::AddProvider)),
                        )
                        .child(view().flex_1())
                        .child(icon_btn(Icon::RotateCw, Sz::Sm, t("Check again"), store::refresh_agents)),
                );
            }
            v.into_element()
        }))
    }))
}

/// Where a provider stands, as its row says it first.
#[derive(Clone, Copy, PartialEq)]
enum Standing {
    Off,
    Checking,
    Ready,
    Authenticated,
    Unchecked,
    UncheckedCreds,
    Unauthenticated,
    SignedOut,
    Missing,
}

impl Standing {
    fn of(p: &Provider, d: &Diagnostic, on: bool) -> Standing {
        if !on {
            Standing::Off
        } else if p.status == "loading" {
            Standing::Checking
        } else if !p.available() {
            Standing::Missing
        } else if let Some(l) = &d.login {
            match l.status.as_deref() {
                Some("authenticated") => Standing::Authenticated,
                Some("unknown") if matches!(l.method.as_deref(), Some("gemini-api-key" | "agent-platform")) => {
                    Standing::UncheckedCreds
                }
                Some("unknown") => Standing::Unchecked,
                Some("unauthenticated") => Standing::Unauthenticated,
                _ if !l.signed_in => Standing::SignedOut,
                _ => Standing::Ready,
            }
        } else {
            Standing::Ready
        }
    }

    fn label(self) -> &'static str {
        match self {
            Standing::Off => t("Disabled"),
            Standing::Checking => t("Checking…"),
            Standing::Ready => t("Ready"),
            Standing::Authenticated => t("Authenticated"),
            Standing::Unchecked => t("Installed · Sign-in not checked"),
            Standing::UncheckedCreds => t("Configured · Not verified yet"),
            Standing::Unauthenticated => t("Not authenticated"),
            Standing::SignedOut => t("Not signed in"),
            Standing::Missing => t("Unavailable"),
        }
    }

    fn color(self) -> AdaptiveColor {
        match self {
            Standing::Off | Standing::Checking => FG_FAINT(),
            Standing::Unchecked | Standing::UncheckedCreds => FG_MUTED(),
            Standing::Ready | Standing::Authenticated => SUCCESS,
            Standing::SignedOut | Standing::Unauthenticated => WARNING,
            Standing::Missing => DANGER,
        }
    }
}

fn dot(color: AdaptiveColor) -> View {
    view().w(px(7.0)).h(px(7.0)).rounded(px(R_FULL)).flex_shrink_0().bg(color)
}

#[allow(clippy::too_many_arguments)]
fn agent_row(
    p: Provider,
    d: Option<Diagnostic>,
    u: Option<store::AgentUpdate>,
    cfg: Option<&Value>,
    editable: bool,
    first: bool,
    open: Signal<Option<String>>,
) -> View {
    let family = agents::family(&p.id, cfg);
    let on = agents::enabled(cfg, &p);
    let gateway = agents::through_gateway(&p, &family);
    let native_claude = agents::native_claude(&p.id, cfg);
    let d = d.unwrap_or_default();
    let standing = Standing::of(&p, &d, on);

    let installed = d.version.as_deref().and_then(crate::agent_update::version_of);
    let newer = match (&u, &installed) {
        (Some(store::AgentUpdate { latest: Some(l), .. }), Some(i)) => crate::agent_update::newer(i, l).then(|| l.clone()),
        _ => None,
    };

    // Its name, its version, and an update when there is one.
    let mut title = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(sp(S2))
        .child(text(p.label.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)));
    // The version alone (`2.1.3`): what the CLI said when asked may be a
    // failure ("did not answer"), not a version to show beside its name.
    if let Some(v) = &installed {
        title = title.child(text(v.clone()).font_family(mono_font).text_size(tx(TEXT_SM)).text_color(FG_MUTED()));
    }
    if let (Some(latest), Some(path)) = (&newer, &d.path)
        && editable
        && !u.as_ref().is_some_and(|u| u.updating)
    {
        let path = path.clone();
        title = title.child(
            button()
                .on_press(move || store::update_agent(&path))
                .accessibility_label(tf("Update to {}", &[latest]))
                .p(px(2.0))
                .rounded(px(R_FULL))
                .hover(|s| s.bg(highlight()))
                .child(icon(Icon::ArrowUp, 14.0).text_color(ACCENT())),
        );
    }
    if gateway {
        title = title.child(badge(t("Through a gateway"), if native_claude { WARNING } else { FG_MUTED() }));
    }

    let model_count = store::offered_models(&p).len();
    // How it stands: the dot, the word, then what it is signed in with,
    // or why it cannot run.
    let mut summary: Vec<String> = Vec::new();
    match standing {
        Standing::Missing => summary.extend(p.error.clone().or(d.error.clone()).or_else(|| Some(t("Not installed, or not found on the service's PATH").into()))),
        Standing::Unchecked | Standing::UncheckedCreds | Standing::Unauthenticated => {
            if let Some(m) = d.login.as_ref().and_then(|l| l.method.as_deref()).and_then(method_label) {
                summary.push(m);
            }
            if model_count > 0 {
                summary.push(tf("{} models", &[&model_count.to_string()]));
            }
        }
        _ => {
            if let Some(l) = d.login.as_ref().filter(|l| l.signed_in) {
                summary.push(login_summary(l));
            }
            if model_count > 0 {
                summary.push(tf("{} models", &[&model_count.to_string()]));
            }
        }
    }
    let status = view()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .child(dot(standing.color()))
        .child(
            text(std::iter::once(standing.label().to_owned()).chain(summary).collect::<Vec<_>>().join(" · "))
                .flex_1()
                .min_w(px(0.0))
                .text_size(tx(TEXT_SM))
                .text_color(if standing == Standing::Missing { DANGER } else { FG_MUTED() })
                .number_of_lines(2),
        );

    let id = p.id.clone();
    let opened = computed(move || open.with(|o| o.as_deref() == Some(id.as_str())));
    let id = p.id.clone();
    let summary_row = button()
        .on_press(move || open.update(|o| *o = if o.as_deref() == Some(id.as_str()) { None } else { Some(id.clone()) }))
        .flex_1()
        .min_w(px(0.0))
        .flex_row()
        .items_start()
        .gap(sp(S3))
        .child(
            view()
                .w(px(28.0))
                .h(px(28.0))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(R_LG))
                .bg(SURFACE2())
                .child(mark(&family, 16.0)),
        )
        .child(view().flex_1().min_w(px(0.0)).gap(px(3.0)).child(title).child(status));

    let mut top = view().flex_row().items_start().gap(sp(S3)).child(summary_row);
    if editable {
        let id = p.id.clone();
        top = top.child(switch(move || on, move |next| store::patch_providers(agents::enable_patch(&id, next))));
    }
    let cfg_owned = cfg.cloned();
    view()
        .px(sp(S4))
        .py(sp(S4))
        .gap(sp(S3))
        .when(!first, |v| v.border_t_1().border_color(BORDER()))
        .bg(move || if opened.get() { SURFACE1() } else { SURFACE0() })
        .child(top)
        .child(dynamic(move || {
            let shown = opened.get();
            if !shown {
                return None;
            }
            let (p, d, u, cfg, family, newer) = (p.clone(), d.clone(), u.clone(), cfg_owned.clone(), family.clone(), newer.clone());
            Some(untracked(move || {
                store::load_local_models();
                details(p, d, u, cfg.as_ref(), editable, family, gateway, newer).into_element()
            }))
        }))
}

/// Human-readable label for a provider's auth method.
fn method_label(method: &str) -> Option<String> {
    Some(match method {
        "claude.ai" => t("Claude account").into(),
        "chatgpt" => t("ChatGPT account").into(),
        "api_key" | "apiKey" => t("API key").into(),
        "oauth-personal" => t("Google account").into(),
        "oauth-business" => "Gemini Enterprise".into(),
        "gemini-api-key" => t("Gemini API key").into(),
        "agent-platform" => "Agent Platform".into(),
        other if !other.is_empty() => other.to_owned(),
        _ => return None,
    })
}

/// `ChatGPT account · me@x.com · ChatGPT Plus`: what a provider is signed
/// in with.
fn login_summary(l: &agents::Login) -> String {
    let mut parts: Vec<String> = vec![
        l.method.as_deref().and_then(method_label).unwrap_or_else(|| t("Signed in").into()),
    ];
    parts.extend(l.email.clone());
    parts.extend(l.plan.clone());
    parts.join(" · ")
}

/// A provider's details, under its row: its warnings, how it signs in,
/// its name (an instance Sophon added), where its CLI and files are, and
/// what can be done to it.
#[allow(clippy::too_many_arguments)]
fn details(
    p: Provider,
    d: Diagnostic,
    u: Option<store::AgentUpdate>,
    cfg: Option<&Value>,
    editable: bool,
    family: String,
    gateway: bool,
    newer: Option<String>,
) -> View {
    let removable = agents::created_by_sophon(cfg, &p.id);
    let native_claude = agents::native_claude(&p.id, cfg);
    let on = agents::enabled(cfg, &p);
    let mut v = view().gap(sp(S3)).pl(px(40.0));

    if gateway && native_claude {
        v = v.child(warning_line(t("Some of its models come through a gateway. Claude Code must run natively, on your own account: use its own models").into()));
    }
    if p.id == "codex"
        && let Some((installed, catalog)) = store::codex_behind_its_models()
    {
        v = v.child(warning_line(codex_behind_text(&installed, &catalog)));
    }
    if let Some(u) = &u
        && let Some(line) = update_line(u, newer.as_deref())
    {
        v = v.child(line);
    }

    // Signed in: with what (with Sign out for Antigravity OAuth). Not: how to.
    let mut login = view().gap(sp(S2));
    match sign_in_way(&p, &d, cfg, &family) {
        Some(way) => login = login.child(sign_in_block(p.id.clone(), way, editable)),
        None => {
            let can_logout = editable
                && family == "antigravity"
                && d.login.as_ref().is_some_and(|l| {
                    l.signed_in && l.method.as_deref().is_some_and(|m| m.starts_with("oauth"))
                });
            let line = login_line(&d).unwrap_or_else(|| muted(t("Not reported").into()));
            if can_logout {
                let id = p.id.clone();
                login = login.child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap(sp(S2))
                        .child(line)
                        .child(
                            button()
                                .on_press(move || store::sign_out(&id))
                                .px(sp(S2))
                                .h(px(22.0))
                                .rounded(px(R_MD))
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(highlight()))
                                .child(text(t("Sign out")).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED())),
                        ),
                );
            } else {
                login = login.child(line);
            }
        }
    }
    v = v.child(field_row(t("Account"), login));

    if editable && removable {
        v = v.child(field_row(t("Display name"), name_field(p.id.clone(), p.label.clone())));
    }
    if let Some(path) = &d.path {
        v = v.child(field_row(t("Executable"), mono(path.clone())));
    }
    if let Some(files) = &d.files {
        v = v.child(field_row(t("Config folder"), mono(files.clone())));
    }

    let has_favorites = {
        let prefix = format!("{}:", p.id);
        store::favorite_models().iter().any(|k| k.starts_with(&prefix))
    };
    if p.available() || !store::offered_models(&p).is_empty() || has_favorites {
        v = v.child(models_section(p.clone(), editable));
    }

    // Not installed on this Mac (Sophon looked in the usual folders and
    // the desktop apps): it can install the two it knows how to.
    let install = store::agent_install(&p.id);
    if let Some(e) = install.as_ref().and_then(|i| i.error.clone()) {
        v = v.child(text(tf("Could not install it: {}", &[&e])).text_size(tx(TEXT_SM)).text_color(DANGER));
    }
    let installable = editable && store::on_this_mac() && on && p.status == "unavailable" && crate::agent_update::installable(&p.id);
    let mut acts = view().flex_row().flex_wrap().items_center().gap(sp(S2));
    let mut any = false;
    if installable {
        let id = p.id.clone();
        let running = install.as_ref().is_some_and(|i| i.running);
        acts = acts.child(btn(
            if running { t("Installing…") } else { t("Install") },
            Variant::Primary,
            Sz::Sm,
            Some(Icon::Download),
            move || store::install_agent(&id),
        ));
        any = true;
    }
    if editable
        && let (Some(latest), Some(path)) = (&newer, &d.path)
        && !u.as_ref().is_some_and(|u| u.updating)
    {
        let path = path.clone();
        acts = acts.child(btn(tf("Update to {}", &[latest]), Variant::Outline, Sz::Sm, Some(Icon::Download), move || store::update_agent(&path)));
        any = true;
    }
    if editable && removable {
        let (id, label) = (p.id.clone(), p.label.clone());
        acts = acts.child(btn(t("Remove"), Variant::Ghost, Sz::Sm, Some(Icon::Trash2), move || {
            store::open_modal(Modal::RemoveAgent { id: id.clone(), label: label.clone() })
        }));
        any = true;
    }
    if any {
        v = v.child(acts);
    }
    v
}

/// A labelled line of the details: its label on the left, its value.
fn field_row(label: &'static str, value: impl IntoElement) -> View {
    view()
        .flex_row()
        .items_start()
        .gap(sp(S3))
        .child(text(label).w(px(120.0)).flex_shrink_0().text_size(tx(TEXT_SM)).text_color(FG_MUTED()).pt(px(2.0)))
        .child(view().flex_1().min_w(px(0.0)).child(value))
}

fn mono(s: String) -> nsh_core::element::Text {
    text(s).font_family(mono_font).text_size(tx(TEXT_SM)).text_color(FG())
}

const MODEL_FILTER_THRESHOLD: usize = 8;

#[derive(Clone)]
struct ListedModel {
    id: String,
    name: String,
    reported: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModelGroup {
    Favorite,
    Visible,
    Hidden,
}

/// The provider's model list with favorite stars, custom ordering (↑/↓),
/// and per-model picker visibility switches.
fn models_section(p: Provider, editable: bool) -> View {
    let filter = signal(String::new());
    // A row toggled on its own keeps its placement until a bulk toggle or
    // re-opening the provider row regroups the list.
    let held_hidden: Signal<std::collections::HashMap<String, bool>> = signal(std::collections::HashMap::new());
    let show_filter = store::offered_models(&p).len() > MODEL_FILTER_THRESHOLD;

    let mut section = view()
        .gap(sp(S2))
        .pt(sp(S3))
        .mt(sp(S1))
        .border_t_1()
        .border_color(BORDER());

    // Header: title, live counts, Reset order, and bulk Show all / Hide all.
    let p_hdr = p.clone();
    section = section.child(dynamic(move || {
        let raw = store::offered_models(&p_hdr);
        let favs = store::favorite_models();
        let order = store::model_order(&p_hdr.id);
        let hidden = store::hidden_models(&p_hdr.id);
        let pid = p_hdr.id.clone();
        Some(untracked(move || {
            let prefix = format!("{pid}:");
            let is_fav = |id: &str| favs.iter().any(|k| k == &format!("{pid}:{id}"));
            let unreported_favs = favs
                .iter()
                .filter_map(|k| k.strip_prefix(&prefix))
                .filter(|slug| !raw.iter().any(|m| m.id == *slug))
                .count();
            let fav_count = raw.iter().filter(|m| is_fav(&m.id)).count() + unreported_favs;
            let hidden_count = raw.iter().filter(|m| hidden.iter().any(|h| h == &m.id)).count();
            let all_hidden = !raw.is_empty() && raw.iter().all(|m| hidden.iter().any(|h| h == &m.id));
            let all_ids: Vec<String> = raw.iter().map(|m| m.id.clone()).collect();

            let mut stats = vec![tf("{} models", &[&raw.len().to_string()])];
            if fav_count > 0 {
                stats.push(tf("{} favorites", &[&fav_count.to_string()]));
            }
            if hidden_count > 0 {
                stats.push(tf("{} hidden", &[&hidden_count.to_string()]));
            }

            let mut bar = view()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap(sp(S2))
                .child(text(t("Models")).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG()))
                .child(text(stats.join(" · ")).text_size(tx(TEXT_XS)).text_color(FG_MUTED()))
                .child(view().flex_1());

            if editable && !order.is_empty() {
                let pid = pid.clone();
                bar = bar.child(
                    button()
                        .on_press(move || store::set_model_order(&pid, Vec::new()))
                        .px(sp(S2))
                        .h(px(24.0))
                        .rounded(px(R_MD))
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(highlight()))
                        .child(text(t("Reset order")).text_size(tx(TEXT_XS)).text_color(FG_MUTED())),
                );
            }
            if editable && !all_ids.is_empty() {
                let pid = pid.clone();
                bar = bar.child(
                    button()
                        .on_press(move || {
                            held_hidden.set(std::collections::HashMap::new());
                            store::set_models_hidden(&pid, &all_ids, !all_hidden);
                        })
                        .px(sp(S2))
                        .h(px(24.0))
                        .rounded(px(R_MD))
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(highlight()))
                        .child(
                            text(if all_hidden { t("Show all") } else { t("Hide all") })
                                .text_size(tx(TEXT_XS))
                                .text_color(FG_MUTED()),
                        ),
                );
            }
            bar.into_element()
        }))
    }));

    // Filter bar (kept outside the list's dynamic so typing never drops focus).
    if show_filter {
        section = section.child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .child(
                    field()
                        .w(px(220.0))
                        .value(move || filter.get())
                        .on_change(move |v: &str| filter.set(v.to_owned()))
                        .placeholder(t("Filter models…")),
                )
                .child(dynamic(move || {
                    (!filter.get().is_empty()).then(|| {
                        untracked(move || {
                            button()
                                .on_press(move || filter.set(String::new()))
                                .accessibility_label(t("Clear"))
                                .w(px(24.0))
                                .h(px(24.0))
                                .rounded(px(R_MD))
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(highlight()))
                                .child(icon(Icon::X, 13.0).text_color(FG_MUTED()))
                                .into_element()
                        })
                    })
                })),
        );
    }

    // Grouped scrollable model rows.
    section = section.child(dynamic(move || {
        let raw = store::offered_models(&p);
        let favs = store::favorite_models();
        let order = store::model_order(&p.id);
        let hidden = store::hidden_models(&p.id);
        let held = held_hidden.get();
        let needle = filter.get().trim().to_lowercase();
        let pid = p.id.clone();

        Some(untracked(move || {
            let prefix = format!("{pid}:");
            let mut display: Vec<ListedModel> = raw
                .iter()
                .map(|m| ListedModel { id: m.id.clone(), name: m.name.clone(), reported: true })
                .collect();
            for key in &favs {
                if let Some(slug) = key.strip_prefix(&prefix)
                    && !display.iter().any(|m| m.id == slug)
                {
                    display.push(ListedModel { id: slug.to_owned(), name: slug.to_owned(), reported: false });
                }
            }
            let is_fav = |id: &str| favs.iter().any(|k| k == &format!("{pid}:{id}"));
            let is_hidden = |id: &str| hidden.iter().any(|h| h == id);
            let placed_hidden = |id: &str| held.get(id).copied().unwrap_or_else(|| is_hidden(id));

            store::group_provider_models(&mut display, |m| &m.id, is_fav, placed_hidden, &order);

            let group_of = |m: &ListedModel| -> ModelGroup {
                if is_fav(&m.id) {
                    ModelGroup::Favorite
                } else if m.reported && placed_hidden(&m.id) {
                    ModelGroup::Hidden
                } else {
                    ModelGroup::Visible
                }
            };
            let fav_count = display.iter().filter(|m| group_of(m) == ModelGroup::Favorite).count();
            let is_filtering = show_filter && !needle.is_empty();
            let visible: Vec<(usize, ListedModel)> = display
                .iter()
                .enumerate()
                .filter(|(_, m)| {
                    !is_filtering
                        || m.name.to_lowercase().contains(&needle)
                        || m.id.to_lowercase().contains(&needle)
                })
                .map(|(idx, m)| (idx, m.clone()))
                .collect();

            if visible.is_empty() {
                let empty_msg = if is_filtering {
                    t("No matching models")
                } else {
                    t("No models reported for this provider yet")
                };
                return view()
                    .px(sp(S3))
                    .py(sp(S2))
                    .rounded(px(R_LG))
                    .border_1()
                    .border_color(BORDER())
                    .bg(SURFACE0())
                    .child(text(empty_msg).text_size(tx(TEXT_XS)).text_color(FG_MUTED()))
                    .into_element();
            }

            let display_ids: Vec<String> = display.iter().map(|m| m.id.clone()).collect();
            let mut list = view().p(sp(S1)).gap(px(2.0));
            let mut prev_group: Option<ModelGroup> = None;

            for (pos, (idx, m)) in visible.iter().enumerate() {
                let g = group_of(m);
                let starts_group = prev_group != Some(g);
                prev_group = Some(g);

                if starts_group {
                    let heading = match g {
                        ModelGroup::Favorite if fav_count > 0 => Some(t("Favorites")),
                        ModelGroup::Visible if fav_count > 0 => Some(t("All")),
                        ModelGroup::Hidden => Some(t("Hidden from picker")),
                        _ => None,
                    };
                    if let Some(label) = heading {
                        list = list.child(
                            view()
                                .px(sp(S2))
                                .pt(if pos == 0 { px(4.0) } else { px(10.0) })
                                .pb(px(2.0))
                                .child(text(label).text_size(tx(TEXT_XS)).font_weight(FontWeight(500)).text_color(FG_FAINT())),
                        );
                    }
                }

                let m_fav = g == ModelGroup::Favorite;
                let m_hidden = m.reported && is_hidden(&m.id);
                let can_move_up = !is_filtering && *idx > 0 && group_of(&display[idx - 1]) == g;
                let can_move_down = !is_filtering && idx + 1 < display.len() && group_of(&display[idx + 1]) == g;

                let fav_key = format!("{pid}:{}", m.id);
                let star_btn = view()
                    .tooltip(tooltip_text(if m_fav { t("Remove from favorites") } else { t("Add to favorites") }))
                    .child(
                        button()
                            .disabled(!editable)
                            .on_press(move || store::toggle_favorite_model(&fav_key))
                            .accessibility_label(if m_fav { t("Remove from favorites") } else { t("Add to favorites") })
                            .w(px(24.0))
                            .h(px(24.0))
                            .rounded(px(R_MD))
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(highlight()))
                            .child(if m_fav {
                                icon(Icon::StarFilled, 13.0).text_color(WARNING).into_element()
                            } else {
                                icon(Icon::Star, 13.0).text_color(FG_FAINT()).into_element()
                            }),
                    );

                let mut label_col = view()
                    .flex_1()
                    .min_w(px(0.0))
                    .flex_row()
                    .items_baseline()
                    .gap(sp(S2))
                    .child(
                        text(m.name.clone())
                            .text_size(tx(TEXT_SM))
                            .text_color(if m_hidden { FG_MUTED() } else { FG() })
                            .number_of_lines(1)
                            .flex_shrink(1.0)
                            .min_w(px(0.0)),
                    );
                if m.id != m.name {
                    label_col = label_col.child(
                        text(m.id.clone())
                            .font_family(mono_font)
                            .text_size(tx(TEXT_XS))
                            .text_color(FG_FAINT())
                            .number_of_lines(1)
                            .flex_shrink(1.0)
                            .min_w(px(0.0)),
                    );
                }

                let mut row = view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .h(px(34.0))
                    .px(sp(S2))
                    .rounded(px(R_MD))
                    .opacity(if m_hidden { 0.55 } else { 1.0 })
                    .hover(|s| s.bg(highlight()))
                    .child(star_btn)
                    .child(label_col);

                if editable && !m_hidden && !is_filtering {
                    let up_ids = display_ids.clone();
                    let up_pid = pid.clone();
                    let up_idx = *idx;
                    let up_btn = view()
                        .tooltip(tooltip_text(t("Move up")))
                        .child(
                            button()
                                .disabled(!can_move_up)
                                .on_press(move || {
                                    if up_idx > 0 {
                                        let mut next = up_ids.clone();
                                        next.swap(up_idx, up_idx - 1);
                                        store::set_model_order(&up_pid, next);
                                    }
                                })
                                .accessibility_label(t("Move up"))
                                .w(px(22.0))
                                .h(px(22.0))
                                .rounded(px(R_MD))
                                .items_center()
                                .justify_center()
                                .opacity(if can_move_up { 1.0 } else { 0.25 })
                                .when(can_move_up, |b| b.hover(|s| s.bg(highlight())))
                                .child(icon(Icon::ArrowUp, 12.0).text_color(FG_MUTED())),
                        );

                    let dn_ids = display_ids.clone();
                    let dn_pid = pid.clone();
                    let dn_idx = *idx;
                    let dn_btn = view()
                        .tooltip(tooltip_text(t("Move down")))
                        .child(
                            button()
                                .disabled(!can_move_down)
                                .on_press(move || {
                                    if dn_idx + 1 < dn_ids.len() {
                                        let mut next = dn_ids.clone();
                                        next.swap(dn_idx, dn_idx + 1);
                                        store::set_model_order(&dn_pid, next);
                                    }
                                })
                                .accessibility_label(t("Move down"))
                                .w(px(22.0))
                                .h(px(22.0))
                                .rounded(px(R_MD))
                                .items_center()
                                .justify_center()
                                .opacity(if can_move_down { 1.0 } else { 0.25 })
                                .when(can_move_down, |b| b.hover(|s| s.bg(highlight())))
                                .child(icon(Icon::ArrowDown, 12.0).text_color(FG_MUTED())),
                        );

                    row = row.child(view().flex_row().items_center().gap(px(2.0)).flex_shrink_0().child(up_btn).child(dn_btn));
                }

                if editable && m.reported {
                    let sw_pid = pid.clone();
                    let sw_mid = m.id.clone();
                    let was_hidden = m_hidden;
                    row = row.child(
                        view()
                            .tooltip(tooltip_text(if m_hidden { t("Hidden from picker") } else { t("Shown in picker") }))
                            .child(switch(
                                move || !was_hidden,
                                move |shown| {
                                    held_hidden.update(|map| {
                                        map.entry(sw_mid.clone()).or_insert(was_hidden);
                                    });
                                    store::set_model_hidden(&sw_pid, &sw_mid, !shown);
                                },
                            )),
                    );
                }

                list = list.child(row);
            }

            view()
                .rounded(px(R_LG))
                .border_1()
                .border_color(BORDER())
                .bg(SURFACE0())
                .overflow_hidden()
                .child(scroll().max_h(px(280.0)).child(list))
                .into_element()
        }))
    }));

    section
}

/// An instance's name, changed when it is submitted or saved.
fn name_field(id: String, label: String) -> View {
    let typed = signal(label.clone());
    let changed = computed(move || typed.with(|n| !n.trim().is_empty() && n.trim() != label));
    let save = move || {
        let name = typed.get_untracked();
        if !name.trim().is_empty() {
            store::patch_providers(serde_json::json!({ "providers": { id.clone(): { "label": name.trim() } } }));
        }
    };
    let submit = save.clone();
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .child(
            field()
                .flex_1()
                .value(move || typed.get())
                .on_change(move |v: &str| typed.set(v.to_owned()))
                .on_submit(move |_: &str| submit()),
        )
        .child(dynamic(move || {
            let shown = changed.get();
            let save = save.clone();
            Some(untracked(move || {
                if shown { btn(t("Save"), Variant::Secondary, Sz::Sm, None, save).into_element() } else { view().into_element() }
            }))
        }))
}

/// How a provider that is not signed in signs in from here.
#[derive(Clone)]
enum Way {
    /// The CLI's own sign-in, started from Sophon: Codex's to ChatGPT, or
    /// Antigravity's to Google.
    App(Brand),
    /// A command run in a terminal on the host.
    Command(String),
}

/// Whose account an in-app sign-in is.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Brand {
    ChatGpt,
    Google,
    AntigravityConnect,
}

/// How `p` signs in, when it says it is not signed in (or, for an
/// instance that says nothing of its sign-in, cannot run). `None` when it
/// is, or runs on an endpoint's key.
fn sign_in_way(p: &Provider, d: &Diagnostic, cfg: Option<&Value>, family: &str) -> Option<Way> {
    let env = cfg.and_then(|c| c.get("providers")?.get(&p.id)?.get("env")).cloned().unwrap_or(Value::Null);
    if env.get("ANTHROPIC_BASE_URL").is_some() || env.get("OPENAI_BASE_URL").is_some() {
        return None;
    }
    let mine = agents::created_by_sophon(cfg, &p.id);
    let signed_out = match &d.login {
        Some(l) => !l.signed_in,
        None => mine && !p.available(),
    };
    if !signed_out {
        return None;
    }
    let dir = |var: &str| env.get(var).and_then(Value::as_str).map(str::to_owned).or_else(|| d.files.clone());
    match family {
        "codex" => Some(Way::App(Brand::ChatGpt)),
        "claude" if mine => dir("CLAUDE_CONFIG_DIR").map(|dir| Way::Command(agents::login_command(Base::Claude, &dir))),
        "claude" => d.path.clone().map(|path| Way::Command(format!("'{}' auth login", path.replace('\'', "")))),
        "gemini" if mine => dir("GEMINI_CLI_HOME").map(|dir| Way::Command(agents::login_command(Base::Gemini, &dir))),
        // In the browser for a Google account; by its key or project
        // otherwise (offering a Connect button when configured but unchecked).
        "antigravity" => {
            let configured = cfg.and_then(|c| c["providers"][&p.id]["authMethod"].as_str().map(str::to_owned));
            let method = configured.or_else(|| d.login.as_ref().and_then(|l| l.method.clone()));
            if method.as_deref().is_none_or(|m| m.starts_with("oauth")) {
                Some(Way::App(Brand::Google))
            } else if d.login.as_ref().and_then(|l| l.status.as_deref()) == Some("unknown") {
                Some(Way::App(Brand::AntigravityConnect))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// The sign-in of provider `id` the way it takes: Codex's from here,
/// with where it stands; a command, run in Terminal on this Mac.
fn sign_in_block(id: String, way: Way, editable: bool) -> View {
    match way {
        Way::Command(cmd) => {
            let mut v = view().gap(sp(S2));
            if editable && store::on_this_mac() {
                let run = cmd.clone();
                v = v.child(
                    view().flex_row().child(btn(t("Sign in in Terminal"), Variant::Primary, Sz::Sm, Some(Icon::Terminal), move || {
                        store::run_in_terminal(&run)
                    })),
                );
            }
            v.child(muted(t("Or run it on that computer yourself:").to_owned())).child(command_box(cmd))
        }
        Way::App(_) if !editable => view().child(muted(t("Sign it in from Sophon on the desktop").to_owned())),
        Way::App(brand) => app_sign_in(id, brand),
    }
}

/// The CLI's own sign-in, started here (as T3 Code starts it): Codex's
/// to ChatGPT, whose page opens in this Mac's browser or, from another
/// computer than the host, takes a code; Antigravity's to Google, whose
/// page opens in this Mac's browser and, from another computer, ends at
/// an address pasted back here. Where it stands, as it goes.
pub(crate) fn app_sign_in(id: String, brand: Brand) -> View {
    let device = brand == Brand::ChatGpt && !store::on_this_mac();
    let (label, mark) = match brand {
        Brand::ChatGpt => (t("Continue with ChatGPT"), Icon::Codex),
        Brand::Google => (t("Continue with Google"), Icon::Antigravity),
        Brand::AntigravityConnect => (t("Connect and load models"), Icon::Antigravity),
    };
    view().gap(sp(S2)).child(dynamic(move || {
        let state = store::sign_in_state(&id);
        let id = id.clone();
        Some(untracked(move || {
            let cancel = {
                let id = id.clone();
                move || store::cancel_sign_in(&id)
            };
            let start = {
                let id = id.clone();
                move || store::sign_in(&id, device)
            };
            let row = view().flex_row().flex_wrap().items_center().gap(sp(S2));
            match state {
                None => row.child(btn(label, Variant::Primary, Sz::Sm, Some(mark), start)).into_element(),
                Some(SignIn::Failed(error)) => view()
                    .gap(sp(S2))
                    .child(text(tf("The sign-in did not finish: {}", &[&error])).text_size(tx(TEXT_SM)).text_color(DANGER))
                    .child(row.child(btn(t("Try again"), Variant::Primary, Sz::Sm, Some(mark), start)))
                    .into_element(),
                Some(SignIn::Starting) => row.child(working(t("Opening the sign-in…"))).child(btn(t("Cancel"), Variant::Ghost, Sz::Sm, None, cancel)).into_element(),
                Some(SignIn::Verifying) => row.child(working(t("Verifying sign-in and loading models…"))).child(btn(t("Cancel"), Variant::Ghost, Sz::Sm, None, cancel)).into_element(),
                Some(SignIn::Waiting { url, code: None, paste, code_login }) => {
                    let copy_url = url.clone();
                    let open_row = row
                        .child(btn(t("Open the page again"), Variant::Outline, Sz::Sm, Some(Icon::ExternalLink), move || {
                            drop(open_url(url.clone()));
                        }))
                        .when(code_login, |r| {
                            r.child(btn(t("Copy link"), Variant::Ghost, Sz::Sm, Some(Icon::Copy), move || {
                                let _ = set_clipboard(copy_url.clone());
                                store::notify(t("Link copied").into());
                            }))
                        })
                        .child(btn(t("Cancel"), Variant::Ghost, Sz::Sm, None, cancel));
                    let mut v = view()
                        .gap(sp(S2))
                        .child(muted(if code_login {
                            t("Finish signing in in your browser, then paste the code from the Antigravity page (or that page's full URL) here:").to_owned()
                        } else {
                            t("Finish signing in in your browser. Choose the account this provider is for").to_owned()
                        }))
                        .child(open_row);
                    if paste {
                        v = v.child(paste_reply(id.clone(), code_login));
                    }
                    v.into_element()
                }
                Some(SignIn::Waiting { url, code: Some(code), .. }) => {
                    let copy = code.clone();
                    view()
                        .gap(sp(S2))
                        .child(muted(tf("On any device, open {} and enter this code:", &[&url])))
                        .child(
                            row.child(text(code).font_family(mono_font).text_size(tx(TEXT_LG)).font_weight(FontWeight(600)))
                                .child(icon_btn(Icon::Copy, Sz::Sm, t("Copy"), move || {
                                    let _ = set_clipboard(copy.clone());
                                    store::notify(t("Code copied").into());
                                }))
                                .child(btn(t("Cancel"), Variant::Ghost, Sz::Sm, None, cancel)),
                        )
                        .into_element()
                }
                Some(SignIn::Done) => row
                    .child(icon(Icon::Check, 14.0).text_color(SUCCESS))
                    .child(text(t("Signed in")).text_size(tx(TEXT_SM)).text_color(SUCCESS))
                    .into_element(),
            }
        }))
    }))
}

/// Where either the Antigravity page's code/URL (`code_login`, for
/// `agy acp --login`) or a remote loopback redirect URL is pasted.
fn paste_reply(id: String, code_login: bool) -> View {
    let typed = signal(String::new());
    let send = move || {
        let url = typed.get_untracked();
        if !url.trim().is_empty() {
            store::sign_in_reply(&id, &url);
        }
    };
    let submit = send.clone();
    let mut v = view().gap(sp(S1));
    if !code_login {
        v = v.child(muted(t("Signing in on another device? Paste the address its page went to after you signed in:").to_owned()));
    }
    v.child(
        view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .child(
                field()
                    .flex_1()
                    .value(move || typed.get())
                    .on_change(move |v: &str| typed.set(v.to_owned()))
                    .on_submit(move |_: &str| submit())
                    .placeholder(if code_login {
                        "Code or https://antigravity.google/oauth-callback?…"
                    } else {
                        "http://127.0.0.1:…/?state=…&code=…"
                    })
                    .font_family(mono_font),
            )
            .child(btn(t("Send"), Variant::Secondary, Sz::Sm, None, send).disabled(move || typed.with(|t| t.trim().is_empty()))),
    )
}

/// A newer version, the update under way, or why it failed.
fn update_line(u: &store::AgentUpdate, newer: Option<&str>) -> Option<View> {
    let row = view().flex_row().flex_wrap().items_center().gap(sp(S2));
    if u.updating {
        return Some(row.child(muted(t("Updating…").into())));
    }
    if let Some(e) = &u.error {
        let row = row.child(text(tf("Update failed: {}", &[e])).text_size(tx(TEXT_SM)).text_color(DANGER));
        return Some(match &u.install {
            Some(i) => row.child(muted(tf("Run it yourself: {}", &[&crate::agent_update::command_line(i)])).font_family(mono_font)),
            None => row,
        });
    }
    let latest = newer?;
    Some(row.child(text(tf("Version {} is out", &[latest])).text_size(tx(TEXT_SM)).text_color(ACCENT())))
}

/// A warning under an agent's name, its triangle at the first line.
fn warning_line(s: String) -> View {
    view()
        .flex_row()
        .items_start()
        .gap(px(6.0))
        .child(icon(Icon::TriangleAlert, 14.0).text_color(WARNING).mt(px(2.0)))
        .child(text(s).flex_1().text_size(tx(TEXT_SM)).text_color(WARNING))
}

/// The Codex CLI the service runs is older than the Codex that fetched its
/// model list ([`store::codex_behind_its_models`]).
pub(crate) fn codex_behind_text(installed: &str, catalog: &str) -> String {
    tf("This Codex CLI ({}) is older than the Codex its model list came from ({}): the newest models may be refused until it is updated", &[installed, catalog])
}

fn muted(s: String) -> nsh_core::element::Text {
    text(s).text_size(tx(TEXT_SM)).text_color(FG_MUTED())
}

/// `Claude account · me@x.com · Max 20x · until 2026-11-29`, or that it is
/// signed out; nothing when the provider does not say.
fn login_line(d: &Diagnostic) -> Option<nsh_core::element::Text> {
    let l = d.login.as_ref()?;
    if !l.signed_in {
        let label = match (l.status.as_deref(), l.method.as_deref().and_then(method_label)) {
            (Some("unauthenticated"), Some(m)) => format!("{} · {m}", t("Not authenticated")),
            (Some("unauthenticated"), None) => t("Not authenticated").into(),
            _ => t("Not signed in").into(),
        };
        return Some(text(label).text_size(tx(TEXT_SM)).text_color(WARNING));
    }
    let mut parts: Vec<String> = vec![
        l.method.as_deref().and_then(method_label).unwrap_or_else(|| t("Signed in").into()),
    ];
    parts.extend(l.email.clone());
    parts.extend(l.plan.clone());
    if let Some(day) = &l.expires {
        parts.push(tf("until {}", &[day]));
    }
    Some(muted(parts.join(" · ")))
}

/// A command in a mono box with a copy button.
fn command_box(cmd: String) -> View {
    let copy = cmd.clone();
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .pl(sp(S3))
        .pr(sp(S1))
        .py(sp(S1))
        .rounded(px(R_LG))
        .bg(SURFACE2())
        .border_1()
        .border_color(BORDER())
        .child(text(cmd).flex_1().min_w(px(0.0)).font_family(mono_font).text_size(tx(TEXT_SM)))
        .child(icon_btn(Icon::Copy, Sz::Sm, t("Copy"), move || {
            let _ = set_clipboard(copy.clone());
            store::notify(t("Command copied").into());
        }))
}

// ------------------------------------------------------------------ adding

fn base_label(b: Base) -> &'static str {
    match b {
        Base::Claude => "Claude Code",
        Base::Codex => "Codex",
        Base::Antigravity => "Antigravity",
        Base::Gemini => "Gemini CLI",
    }
}

fn hint(s: impl IntoReactive<SharedString>) -> nsh_core::element::Text {
    text(s).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED())
}

/// What a new provider is (the first step of adding one).
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// Another account of a CLI, signed in on its own.
    Account(Base),
    /// An API endpoint: a documented one ([`PRESETS`]), or one of the
    /// person's own (`None`).
    Endpoint(Option<usize>),
}

impl Kind {
    fn steps(self) -> &'static [&'static str] {
        match self {
            Kind::Account(_) => &["Provider", "Details", "Sign in"],
            Kind::Endpoint(_) => &["Provider", "Details"],
        }
    }
}

/// Adding a provider, in steps as T3 Code's: what it is, its details,
/// then (an account) signing it in.
pub(crate) fn add_provider() -> View {
    let kind = signal(Kind::Account(Base::Claude));
    let show_more = signal(false);
    let step = signal(0usize);
    // An account: its name and folder. An endpoint: its name, base,
    // address, models and key.
    let name = signal(String::new());
    let dir = signal(String::new());
    let base = signal(Base::Claude);
    let url = signal(String::new());
    let models = signal(String::new());
    let key = signal(String::new());
    // An Antigravity account: how it signs in, and its Google Cloud
    // project and location (its key is `key`).
    let agy_auth = signal(AgyAuth::Google);
    let gcp_project = signal(String::new());
    let gcp_location = signal(String::new());
    // The account added: its id, label and kind (the last step signs it in).
    let added: Signal<Option<(String, String, Base, String)>> = signal(None);

    let account_plan = move |b: Base| {
        agents::account_id_and_dir(
            b,
            &name.get(),
            &store::provider_ids(),
            store::daemon_config().as_ref(),
            store::agent_home().as_deref(),
        )
    };
    let folder = move |b: Base| {
        let typed = dir.get();
        if typed.trim().is_empty() { account_plan(b).1 } else { typed }
    };
    let target = computed(move || match kind.get() {
        Kind::Account(b) => agents::absolute_dir(&folder(b), store::agent_home().as_deref()),
        Kind::Endpoint(_) => None,
    });
    let endpoint = computed(move || {
        let Kind::Endpoint(preset) = kind.get() else { return None };
        let p = preset.and_then(|i| PRESETS.get(i));
        let label = name.with(|n| n.trim().to_owned());
        let label = if label.is_empty() { p.map(|p| p.label.to_owned()).unwrap_or_default() } else { label };
        let b = p.map(|p| p.base).unwrap_or_else(|| base.get());
        let id = agents::new_id(&label, p.map(|p| p.id).unwrap_or("custom"), &store::provider_ids());
        let k = KeyInstance {
            label,
            base: b,
            base_url: p.map(|p| p.base_url.to_owned()).unwrap_or_else(|| url.get().trim().to_owned()),
            key: key.get(),
            models: match p {
                Some(p) => p.models.iter().map(|(i, l)| ((*i).to_owned(), (*l).to_owned())).collect(),
                None => agents::parse_models(&models.get()),
            },
            extra_env: p.map(|p| p.extra_env.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())).collect()).unwrap_or_default(),
            claude_dir: (b == Base::Claude)
                .then(|| agents::absolute_dir(&format!("~/.claude-{id}"), store::agent_home().as_deref()))
                .flatten(),
        };
        Some((id, k))
    });
    let problem = computed(move || match kind.get() {
        Kind::Account(_) if name.with(|n| n.trim().is_empty()) => Some(t("Give it a name").to_owned()),
        Kind::Account(_) if target.with(Option::is_none) => Some(t("Use a full path: Sophon does not know that computer's home folder").to_owned()),
        Kind::Account(b) if let Some(other) = target.with(|d| d.as_deref().and_then(|d| agents::same_folder(store::daemon_config().as_ref(), b, d))) => {
            Some(tf("“{}” already uses this folder: it would be the same account twice", &[&other]))
        }
        Kind::Account(Base::Antigravity) => agents::agy_problem(agy_auth.get(), &key.get(), &gcp_project.get()).map(|p| t(p).to_owned()),
        Kind::Account(_) => None,
        Kind::Endpoint(_) => endpoint.with(|e| e.as_ref().and_then(|(_, k)| agents::key_instance_problem(k)).map(|p| t(p).to_owned())),
    });

    let add = move || {
        if problem.get_untracked().is_some() {
            return;
        }
        match kind.get_untracked() {
            Kind::Account(b) => {
                let Some(dir) = target.get_untracked() else { return };
                let n = name.get_untracked();
                let (id, _) = agents::account_id_and_dir(
                    b,
                    &n,
                    &store::provider_ids(),
                    store::daemon_config().as_ref(),
                    store::agent_home().as_deref(),
                );
                let label = format!("{} ({})", base_label(b), n.trim());
                store::patch_providers(match b {
                    Base::Antigravity => agents::agy_patch(
                        &id,
                        &label,
                        &dir,
                        agy_auth.get_untracked(),
                        &key.get_untracked(),
                        &gcp_project.get_untracked(),
                        &gcp_location.get_untracked(),
                    ),
                    _ => agents::account_patch(&id, b, &label, &dir),
                });
                store::clear_sign_in(&id);
                added.set(Some((id, label, b, dir)));
                step.set(2);
            }
            Kind::Endpoint(_) => {
                let Some((id, k)) = endpoint.get_untracked() else { return };
                store::patch_providers(agents::key_instance_patch(&id, &k));
                store::notify(tf("Added “{}”", &[&k.label]));
                store::close_modal();
            }
        }
    };

    sheet(t("Add provider"))
        .w(px(600.0))
        .child(hint(t("Another account, signed in on its own, or an API endpoint. Sophon never switches accounts on its own")))
        .child(dynamic(move || {
            let (k, at) = (kind.get(), step.get());
            Some(untracked(move || steps_bar(k.steps(), at).into_element()))
        }))
        .child(dynamic(move || {
            let at = step.get();
            Some(untracked(move || match at {
                0 => choose_kind(kind, show_more).into_element(),
                1 => details_step(kind, name, dir, base, url, models, key, target, folder_placeholder(kind, name))
                    .child(agy_details(kind, agy_auth, key, gcp_project, gcp_location))
                    .into_element(),
                _ => sign_in_step(added, agy_auth).into_element(),
            }))
        }))
        .child(dynamic(move || problem.get().filter(|_| step.get() == 1).map(|p| hint(p).text_color(WARNING).into_element())))
        .child(dynamic(move || {
            let (at, k) = (step.get(), kind.get());
            Some(untracked(move || {
                let mut row = view().flex_row().justify_end().gap(sp(S2));
                match at {
                    0 => {
                        row = row
                            .child(btn(t("Cancel"), Variant::Secondary, Sz::Md, None, store::close_modal))
                            .child(btn(t("Next"), Variant::Primary, Sz::Md, None, move || step.set(1)));
                    }
                    1 => {
                        let label = match k {
                            Kind::Account(Base::Antigravity) if !agy_auth.get_untracked().browser() => t("Add"),
                            Kind::Account(_) => t("Add and sign in"),
                            Kind::Endpoint(_) => t("Add"),
                        };
                        row = row
                            .child(btn(t("Back"), Variant::Secondary, Sz::Md, None, move || step.set(0)))
                            .child(btn(label, Variant::Primary, Sz::Md, Some(Icon::Plus), add).disabled(move || problem.with(Option::is_some)));
                    }
                    _ => {
                        row = row.child(btn(t("Done"), Variant::Primary, Sz::Md, None, store::close_modal));
                    }
                }
                row.into_element()
            }))
        }))
}

/// The steps of adding, `at` the current one.
fn steps_bar(steps: &'static [&'static str], at: usize) -> View {
    let mut row = view().flex_row().gap(sp(S2));
    for (i, label) in steps.iter().enumerate() {
        let current = i == at;
        let done = i < at;
        row = row.child(
            view()
                .flex_1()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .px(sp(S3))
                .py(sp(S2))
                .rounded(px(R_LG))
                .border_1()
                .border_color(if current { ACCENT() } else { BORDER() })
                .bg(if current { tint(ACCENT(), 0.06) } else { SURFACE0() })
                .child(
                    view()
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(px(R_FULL))
                        .items_center()
                        .justify_center()
                        .border_1()
                        .border_color(if current || done { ACCENT() } else { BORDER() })
                        .child(if done {
                            icon(Icon::Check, 12.0).text_color(ACCENT()).into_element()
                        } else {
                            text((i + 1).to_string()).text_size(tx(TEXT_XS)).text_color(if current { ACCENT() } else { FG_MUTED() }).into_element()
                        }),
                )
                .child(text(t(label)).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(if current { FG() } else { FG_MUTED() })),
        );
    }
    row
}

/// One choice of the first step: a card with its mark and name.
fn kind_card(k: Kind, family: &'static str, label: &'static str, note: &'static str, kind: Signal<Kind>) -> ButtonElement {
    let selected = computed(move || kind.get() == k);
    button()
        .on_press(move || kind.set(k))
        .min_w(px(0.0))
        .min_h(px(62.0))
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .px(sp(S4))
        .py(sp(S3))
        .rounded(px(R_LG))
        .border_1()
        .border_color(move || if selected.get() { ACCENT() } else { BORDER() })
        .bg(move || if selected.get() { tint(ACCENT(), 0.06) } else { SURFACE0() })
        .hover(|s| s.bg(highlight()))
        .child(mark(family, 18.0))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(2.0))
                .child(text(t(label)).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)).number_of_lines(1))
                .child(text(t(note)).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).number_of_lines(1)),
        )
        .child(dynamic(move || {
            selected.get().then(|| icon(Icon::Check, 14.0).text_color(ACCENT()).into_element())
        }))
}

fn choose_kind(kind: Signal<Kind>, show_more: Signal<bool>) -> View {
    // Wrap both slots in identical zero-padding flex-1 columns so every card
    // has the exact same width even on single-card rows.
    fn pair(a: impl IntoElement, b: impl IntoElement) -> View {
        view()
            .flex_row()
            .gap(sp(S3))
            .child(view().flex_1().min_w(px(0.0)).child(a))
            .child(view().flex_1().min_w(px(0.0)).child(b))
    }
    let mut endpoints = vec![];
    for (i, p) in PRESETS.iter().enumerate() {
        let family = match p.base {
            Base::Codex => "codex",
            _ => "claude",
        };
        endpoints.push(kind_card(Kind::Endpoint(Some(i)), family, p.label, "API key", kind));
    }
    endpoints.push(kind_card(Kind::Endpoint(None), "custom", "Custom endpoint", "API key", kind));
    let mut list = view()
        .gap(sp(S3))
        .child(section_label(t("Accounts")))
        .child(pair(
            kind_card(Kind::Account(Base::Claude), "claude", "Claude Code", "Claude account", kind),
            kind_card(Kind::Account(Base::Codex), "codex", "Codex", "ChatGPT account", kind),
        ))
        .child(pair(
            kind_card(Kind::Account(Base::Antigravity), "antigravity", "Antigravity", "Google account", kind),
            view(),
        ))
        .child(dynamic(move || {
            let expanded = show_more.get();
            Some(untracked(move || {
                let toggle = btn(
                    if expanded { t("Show less") } else { t("Show more") },
                    Variant::Ghost,
                    Sz::Sm,
                    Some(if expanded { Icon::ChevronUp } else { Icon::ChevronDown }),
                    move || show_more.update(|open| *open = !*open),
                );
                let mut block = view().gap(sp(S2)).child(view().flex_row().child(toggle));
                if expanded {
                    block = block.child(pair(
                        kind_card(Kind::Account(Base::Gemini), "gemini", "Gemini CLI", "Gemini CLI account", kind),
                        view(),
                    ));
                }
                block.into_element()
            }))
        }))
        .child(section_label(t("API endpoints")));
    let mut it = endpoints.into_iter();
    while let Some(a) = it.next() {
        list = list.child(match it.next() {
            Some(b) => pair(a, b),
            None => pair(a, view()),
        });
    }
    list
}

/// The placeholder of an account's folder: the one it gets unless another
/// is typed.
fn folder_placeholder(kind: Signal<Kind>, name: Signal<String>) -> impl Fn() -> String + 'static {
    move || match kind.get() {
        Kind::Account(b) => {
            agents::account_id_and_dir(
                b,
                &name.get(),
                &store::provider_ids(),
                store::daemon_config().as_ref(),
                store::agent_home().as_deref(),
            )
            .1
        }
        Kind::Endpoint(_) => String::new(),
    }
}

/// The second step: an account's name and folder, or an endpoint's name,
/// address, models and key.
#[allow(clippy::too_many_arguments)]
fn details_step(
    kind: Signal<Kind>,
    name: Signal<String>,
    dir: Signal<String>,
    base: Signal<Base>,
    url: Signal<String>,
    models: Signal<String>,
    key: Signal<String>,
    target: Memo<Option<String>>,
    placeholder: impl Fn() -> String + 'static,
) -> View {
    let k = kind.get_untracked();
    let name_field = field().test_id("provider-name").value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned()));
    match k {
        Kind::Account(b) => {
            let mut v = view().gap(sp(S3));
            if let Some(note) = second_account_note(b) {
                v = v.child(note);
            }
            v.child(labelled(t("Name"), t("Shown in the provider list and the model picker"), name_field.placeholder(t("e.g. Work"))))
                .child(labelled(
                    t("Config folder"),
                    t("Where this account keeps its sign-in and settings, on that computer"),
                    field()
                        .value(move || dir.get())
                        .on_change(move |v: &str| dir.set(v.to_owned()))
                        .placeholder(placeholder)
                        .font_family(mono_font),
                ))
                .child(hint(move || match target.get() {
                    Some(d) => tf("Its config folder on that computer: {}", &[&d]),
                    None => t("Use a full path: Sophon does not know that computer's home folder").into(),
                }))
        }
        Kind::Endpoint(preset) => {
            let p = preset.and_then(|i| PRESETS.get(i)).copied();
            let mut v = view().gap(sp(S3));
            match p {
                Some(p) => {
                    let line = format!("{} · {}", p.base_url, p.models.iter().map(|(_, l)| *l).collect::<Vec<_>>().join(", "));
                    v = v.child(hint(line).font_family(mono_font));
                }
                None => {
                    v = v
                        .child(segmented(
                            vec![(Base::Claude, base_label(Base::Claude).into()), (Base::Codex, base_label(Base::Codex).into())],
                            move || base.get(),
                            move |b| base.set(b),
                        ))
                        .child(labelled(
                            t("Endpoint"),
                            t("Its address, as the CLI asks it"),
                            field()
                                .value(move || url.get())
                                .on_change(move |v: &str| url.set(v.to_owned()))
                                .placeholder(move || match base.get() {
                                    Base::Claude => t("Anthropic-compatible endpoint, e.g. https://example.com/anthropic"),
                                    Base::Codex | Base::Antigravity | Base::Gemini => t("OpenAI Responses endpoint, e.g. https://example.com/v1"),
                                })
                                .font_family(mono_font),
                        ))
                        .child(labelled(
                            t("Models"),
                            t("Their ids, the default first"),
                            field()
                                .value(move || models.get())
                                .on_change(move |v: &str| models.set(v.to_owned()))
                                .placeholder(t("Model ids, the default first, e.g. model-a, model-b"))
                                .font_family(mono_font),
                        ));
                }
            }
            let named = p.map(|p| tf("Name (default: {})", &[p.label])).unwrap_or_else(|| t("Name").into());
            v.child(labelled(t("Name"), t("Shown in the provider list and the model picker"), name_field.placeholder(named)))
                .child(labelled(
                    t("API key"),
                    t("Kept in the service's config file on that computer, like any provider's"),
                    field().value(move || key.get()).on_change(move |v: &str| key.set(v.to_owned())).masked(true).placeholder(t("API key")),
                ))
        }
    }
}

/// A form field with its label and what it is for.
fn labelled(label: &'static str, about: &'static str, input: impl IntoElement) -> View {
    view()
        .gap(px(6.0))
        .child(text(label).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)))
        .child(input)
        .child(text(about).text_size(tx(TEXT_XS)).text_color(FG_MUTED()))
}

/// When an account of `b` is signed in already: a word on adding more,
/// as T3 Code says it. Switching between accounts to get around a plan's
/// limits is what gets accounts banned; Sophon never does it.
fn second_account_note(b: Base) -> Option<View> {
    let cfg = store::daemon_config();
    let signed = store::providers().iter().any(|p| {
        agents::family(&p.id, cfg.as_ref()) == b.id()
            && store::diagnostic(&p.id).is_some_and(|d| d.login.is_some_and(|l| l.signed_in))
    });
    signed.then(|| {
        view()
            .flex_row()
            .items_start()
            .gap(sp(S2))
            .p(sp(S3))
            .rounded(px(R_LG))
            .bg(tint(ACCENT(), 0.06))
            .child(icon(Icon::Info, 14.0).text_color(ACCENT()).mt(px(2.0)))
            .child(
                text(tf("A separate work account is fine. Sophon does not manage a pile of {} accounts and never switches between them on its own: switching to get around a plan's limits can get them banned", &[base_label(b)]))
                    .flex_1()
                    .text_size(tx(TEXT_SM))
                    .text_color(FG()),
            )
    })
}

/// The last step of adding an account: signing it in.
fn sign_in_step(added: Signal<Option<(String, String, Base, String)>>, agy_auth: Signal<AgyAuth>) -> View {
    let Some((id, label, b, dir)) = added.get_untracked() else { return view() };
    if b == Base::Antigravity && !agy_auth.get_untracked().browser() {
        return view().child(hint(tf("“{}” is added. It signs in with its key or Google Cloud project when it starts", &[&label])));
    }
    let intro = hint(tf("“{}” is added. Sign it in: it uses this account until you remove it", &[&label]));
    let way = match b {
        Base::Codex => app_sign_in(id, Brand::ChatGpt),
        Base::Antigravity => app_sign_in(id, Brand::Google),
        _ => sign_in_block(id, Way::Command(agents::login_command(b, &dir)), true),
    };
    view().gap(sp(S3)).child(intro).child(way)
}

/// An Antigravity account's sign-in, as T3 Code asks it: a Google
/// account (the subscription) or Gemini Enterprise, in the browser; a
/// Gemini API key; or Google Cloud's Agent Platform, by a key or the
/// project of the host's own Google Cloud sign-in. Nothing for others.
fn agy_details(
    kind: Signal<Kind>,
    auth: Signal<AgyAuth>,
    key: Signal<String>,
    project: Signal<String>,
    location: Signal<String>,
) -> View {
    if kind.get_untracked() != Kind::Account(Base::Antigravity) {
        return view();
    }
    let label = |a: AgyAuth| -> SharedString {
        match a {
            AgyAuth::Google => t("Google account").into(),
            AgyAuth::Enterprise => "Gemini Enterprise".into(),
            AgyAuth::ApiKey => t("Gemini API key").into(),
            AgyAuth::AgentPlatform => "Agent Platform (Vertex AI)".into(),
        }
    };
    view()
        .gap(sp(S3))
        .child(labelled(
            t("How it signs in"),
            t("A Google account uses its subscription; an API key and Agent Platform are billed by use"),
            segmented(AgyAuth::ALL.iter().map(|a| (*a, label(*a))).collect(), move || auth.get(), move |a| auth.set(a)),
        ))
        .child(dynamic(move || {
            let a = auth.get();
            Some(untracked(move || {
                let mut v = view().gap(sp(S3));
                if matches!(a, AgyAuth::ApiKey | AgyAuth::AgentPlatform) {
                    v = v.child(labelled(
                        t("API key"),
                        if a == AgyAuth::ApiKey {
                            t("A Gemini or Vertex AI express key, kept on that computer")
                        } else {
                            t("Optional: without one, the project below and that computer's Google Cloud sign-in")
                        },
                        field().value(move || key.get()).on_change(move |v: &str| key.set(v.to_owned())).masked(true).placeholder(t("API key")),
                    ));
                }
                if matches!(a, AgyAuth::Enterprise | AgyAuth::AgentPlatform) {
                    v = v
                        .child(labelled(
                            t("Google Cloud project"),
                            if a == AgyAuth::Enterprise { t("Gemini Enterprise needs it") } else { t("Used when there is no API key") },
                            field().value(move || project.get()).on_change(move |v: &str| project.set(v.to_owned())).placeholder("my-project-id").font_family(mono_font),
                        ))
                        .child(labelled(
                            t("Google Cloud location"),
                            t("The region it runs in"),
                            field().value(move || location.get()).on_change(move |v: &str| location.set(v.to_owned())).placeholder("us-central1").font_family(mono_font),
                        ));
                }
                v.into_element()
            }))
        }))
}

/// Removing an instance Sophon added: its entry leaves the config; its
/// folder and login stay on the computer.
pub(crate) fn remove_agent(id: String, label: String) -> View {
    sheet(t("Remove agent"))
        .child(hint(tf("“{}” leaves the agent list. Its config folder and login stay on that computer", &[&label])))
        .child(actions(
            btn(t("Remove"), Variant::Destructive, Sz::Md, Some(Icon::Trash2), move || {
                store::patch_providers(agents::remove_patch(&id));
                store::close_modal();
            })
            ,
        ))
}
