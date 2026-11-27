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
use crate::daemon::agents::{self, Base, Diagnostic, KeyInstance, PRESETS};
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
        } else if d.login.as_ref().is_some_and(|l| !l.signed_in) {
            Standing::SignedOut
        } else {
            Standing::Ready
        }
    }

    fn label(self) -> &'static str {
        match self {
            Standing::Off => t("Disabled"),
            Standing::Checking => t("Checking…"),
            Standing::Ready => t("Ready"),
            Standing::SignedOut => t("Not signed in"),
            Standing::Missing => t("Unavailable"),
        }
    }

    fn color(self) -> AdaptiveColor {
        match self {
            Standing::Off | Standing::Checking => FG_FAINT(),
            Standing::Ready => SUCCESS,
            Standing::SignedOut => WARNING,
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
    if let Some(v) = &d.version {
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

    // How it stands: the dot, the word, then what it is signed in with,
    // or why it cannot run.
    let mut summary: Vec<String> = Vec::new();
    match standing {
        Standing::Missing => summary.extend(p.error.clone().or(d.error.clone()).or_else(|| Some(t("Not installed, or not found on the service's PATH").into()))),
        _ => {
            if let Some(l) = d.login.as_ref().filter(|l| l.signed_in) {
                summary.push(login_summary(l));
            }
            if !p.models.is_empty() {
                summary.push(tf("{} models", &[&p.models.len().to_string()]));
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
    let details = details(p, d, u, cfg, editable, family, gateway, newer);
    view()
        .px(sp(S4))
        .py(sp(S4))
        .gap(sp(S3))
        .when(!first, |v| v.border_t_1().border_color(BORDER()))
        .bg(move || if opened.get() { SURFACE1() } else { SURFACE0() })
        .child(top)
        .child(dynamic(move || {
            let shown = opened.get();
            let details = details.clone();
            shown.then(|| untracked(move || details.into_element()))
        }))
}

/// `ChatGPT account · me@x.com · ChatGPT Plus`: what a provider is signed
/// in with.
fn login_summary(l: &agents::Login) -> String {
    let mut parts: Vec<String> = vec![match l.method.as_deref() {
        Some("claude.ai") => t("Claude account").into(),
        Some("chatgpt") => t("ChatGPT account").into(),
        Some("api_key" | "apiKey" | "gemini-api-key" | "agent-platform") => t("API key").into(),
        Some("oauth-personal" | "oauth-business") => t("Google account").into(),
        Some(m) => m.to_owned(),
        None => t("Signed in").into(),
    }];
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

    // Signed in: with what. Not: how to.
    let mut login = view().gap(sp(S2));
    match sign_in_way(&p, &d, cfg, &family) {
        Some(way) => login = login.child(sign_in_block(p.id.clone(), way, editable)),
        None => login = login.child(login_line(&d).unwrap_or_else(|| muted(t("Not reported").into()))),
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
    /// Codex's own sign-in to ChatGPT, started from Sophon.
    Codex,
    /// A command run in a terminal on the host.
    Command(String),
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
        "codex" => Some(Way::Codex),
        "claude" if mine => dir("CLAUDE_CONFIG_DIR").map(|dir| Way::Command(agents::login_command(Base::Claude, &dir))),
        "claude" => d.path.clone().map(|path| Way::Command(format!("'{}' auth login", path.replace('\'', "")))),
        "antigravity" if mine => dir("GEMINI_HOME").map(|dir| Way::Command(agents::login_command(Base::Antigravity, &dir))),
        "antigravity" => antigravity_login(d).map(Way::Command),
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
        Way::Codex if !editable => view().child(muted(t("Sign it in from Sophon on the desktop").to_owned())),
        Way::Codex => codex_sign_in(id),
    }
}

/// Codex's sign-in to ChatGPT, started here (as T3 Code starts it): its
/// page opens in this Mac's browser; from another computer than the
/// host, the page takes a code. Where it stands, as it goes.
pub(crate) fn codex_sign_in(id: String) -> View {
    let device = !store::on_this_mac();
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
                None => row
                    .child(btn(t("Continue with ChatGPT"), Variant::Primary, Sz::Sm, Some(Icon::Codex), start))
                    .into_element(),
                Some(SignIn::Failed(error)) => view()
                    .gap(sp(S2))
                    .child(text(tf("The sign-in did not finish: {}", &[&error])).text_size(tx(TEXT_SM)).text_color(DANGER))
                    .child(row.child(btn(t("Try again"), Variant::Primary, Sz::Sm, Some(Icon::Codex), start)))
                    .into_element(),
                Some(SignIn::Starting) => row.child(working(t("Opening the sign-in…"))).child(btn(t("Cancel"), Variant::Ghost, Sz::Sm, None, cancel)).into_element(),
                Some(SignIn::Waiting { url, code: None }) => view()
                    .gap(sp(S2))
                    .child(muted(t("Finish signing in in your browser. Choose the account this provider is for").to_owned()))
                    .child(
                        row.child(btn(t("Open the page again"), Variant::Outline, Sz::Sm, Some(Icon::ExternalLink), move || {
                            drop(open_url(url.clone()));
                        }))
                        .child(btn(t("Cancel"), Variant::Ghost, Sz::Sm, None, cancel)),
                    )
                    .into_element(),
                Some(SignIn::Waiting { url, code: Some(code) }) => {
                    let copy = code.clone();
                    view()
                        .gap(sp(S2))
                        .child(muted(tf("On any device, open {} and enter this code:", &[&url])))
                        .child(
                            row.child(text(code).font_family(mono_font).text_size(tx(TEXT_LG)).font_weight(FontWeight(600)))
                                .child(icon_btn(Icon::Copy, Sz::Sm, t("Copy"), move || {
                                    set_clipboard(copy.clone());
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
        return Some(text(t("Not signed in")).text_size(tx(TEXT_SM)).text_color(WARNING));
    }
    let mut parts: Vec<String> = vec![match l.method.as_deref() {
        Some("claude.ai") => t("Claude account").into(),
        Some("chatgpt") => t("ChatGPT account").into(),
        Some("api_key" | "apiKey" | "gemini-api-key" | "agent-platform") => t("API key").into(),
        Some("oauth-personal" | "oauth-business") => t("Google account").into(),
        Some(m) => m.to_owned(),
        None => t("Signed in").into(),
    }];
    parts.extend(l.email.clone());
    parts.extend(l.plan.clone());
    if let Some(day) = &l.expires {
        parts.push(tf("until {}", &[day]));
    }
    Some(muted(parts.join(" · ")))
}

/// Antigravity's sign-in to a Google account, in the folder it says it
/// keeps, when it says it is signed out of one.
fn antigravity_login(d: &Diagnostic) -> Option<String> {
    let l = d.login.as_ref().filter(|l| !l.signed_in && l.method.as_deref() == Some("oauth-personal"))?;
    Some(agents::login_command(Base::Antigravity, l.config_dir.as_deref()?))
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
            set_clipboard(copy.clone());
            store::notify(t("Command copied").into());
        }))
}

// ------------------------------------------------------------------ adding

fn base_label(b: Base) -> &'static str {
    match b {
        Base::Claude => "Claude Code",
        Base::Codex => "Codex",
        Base::Antigravity => "Antigravity",
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
    let step = signal(0usize);
    // An account: its name and folder. An endpoint: its name, base,
    // address, models and key.
    let name = signal(String::new());
    let dir = signal(String::new());
    let base = signal(Base::Claude);
    let url = signal(String::new());
    let models = signal(String::new());
    let key = signal(String::new());
    // The account added: its id, label and kind (the last step signs it in).
    let added: Signal<Option<(String, String, Base, String)>> = signal(None);

    let slug = move || agents::new_id(&name.get(), "account", &[]);
    let folder = move |b: Base| {
        let typed = dir.get();
        if typed.trim().is_empty() { b.default_dir(&slug()) } else { typed }
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
        Kind::Account(_) if name.with(|n| n.trim().is_empty()) => Some(t("Give it a name")),
        Kind::Account(_) if target.with(Option::is_none) => Some(t("Use a full path: Sophon does not know that computer's home folder")),
        Kind::Account(_) => None,
        Kind::Endpoint(_) => endpoint.with(|e| e.as_ref().and_then(|(_, k)| agents::key_instance_problem(k)).map(t)),
    });

    let add = move || {
        if problem.get_untracked().is_some() {
            return;
        }
        match kind.get_untracked() {
            Kind::Account(b) => {
                let Some(dir) = target.get_untracked() else { return };
                let n = name.get_untracked();
                let id = agents::new_id(&n, b.id(), &store::provider_ids());
                let label = format!("{} ({})", base_label(b), n.trim());
                store::patch_providers(agents::account_patch(&id, b, &label, &dir));
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
                0 => choose_kind(kind).into_element(),
                1 => details_step(kind, name, dir, base, url, models, key, target, folder_placeholder(kind, name)).into_element(),
                _ => sign_in_step(added).into_element(),
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
        .flex_1()
        .min_w(px(0.0))
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
                .child(text(t(label)).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)))
                .child(text(t(note)).text_size(tx(TEXT_XS)).text_color(FG_MUTED())),
        )
        .child(dynamic(move || {
            selected.get().then(|| icon(Icon::Check, 14.0).text_color(ACCENT()).into_element())
        }))
}

fn choose_kind(kind: Signal<Kind>) -> View {
    fn pair(a: impl IntoElement, b: impl IntoElement) -> View {
        view().flex_row().gap(sp(S3)).child(a).child(b)
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
            view().flex_1(),
        ))
        .child(section_label(t("API endpoints")));
    let mut it = endpoints.into_iter();
    while let Some(a) = it.next() {
        list = list.child(match it.next() {
            Some(b) => pair(a, b),
            None => pair(a, view().flex_1()),
        });
    }
    list
}

/// The placeholder of an account's folder: the one it gets unless another
/// is typed.
fn folder_placeholder(kind: Signal<Kind>, name: Signal<String>) -> impl Fn() -> String + 'static {
    move || match kind.get() {
        Kind::Account(b) => b.default_dir(&agents::new_id(&name.get(), "account", &[])),
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
                                    Base::Codex | Base::Antigravity => t("OpenAI Responses endpoint, e.g. https://example.com/v1"),
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
fn sign_in_step(added: Signal<Option<(String, String, Base, String)>>) -> View {
    let Some((id, label, b, dir)) = added.get_untracked() else { return view() };
    let intro = hint(tf("“{}” is added. Sign it in: it uses this account until you remove it", &[&label]));
    let way = match b {
        Base::Codex => codex_sign_in(id),
        _ => sign_in_block(id, Way::Command(agents::login_command(b, &dir)), true),
    };
    view().gap(sp(S3)).child(intro).child(way)
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
