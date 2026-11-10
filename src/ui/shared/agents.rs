//! The Agents settings section: one row per provider (Paseo's word for an
//! agent CLI or an instance of one) with its status, binary, version,
//! models and login; on the desktop also the switch that turns it on,
//! and the forms that add another account or an API-key instance.
//! Instances are separate providers: nothing switches accounts on its own.

use crate::i18n::{t, tf};
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use serde_json::Value;

use crate::daemon::Provider;
use crate::daemon::agents::{self, Base, Diagnostic, KeyInstance, PRESETS};
use crate::store::{self, Modal};
use crate::ui::kit::*;
use crate::ui::kit::text;
use crate::ui::shared::model_picker::mark;
use crate::ui::shared::modals::{actions, sheet};

/// The card of agents. `editable`: the switches, removal and the add
/// buttons (the desktop); the phone only reads.
pub(crate) fn agents_card(editable: bool) -> impl IntoElement {
    card().child(dynamic(move || {
        let list = store::providers();
        let cfg = store::daemon_config();
        let diags: Vec<Option<Diagnostic>> = list.iter().map(|p| store::diagnostic(&p.id)).collect();
        Some(untracked(move || {
            let mut v = view();
            if list.is_empty() {
                v = v.child(view().px(px(S4)).py(px(S4)).child(text(t("Loading…")).text_size(px(TEXT_SM)).text_color(FG_MUTED)));
            }
            for (i, (p, d)) in list.into_iter().zip(diags).enumerate() {
                v = v.child(agent_row(p, d, cfg.as_ref(), editable, i == 0));
            }
            if editable {
                v = v.child(
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap(px(S2))
                        .px(px(S4))
                        .py(px(S3))
                        .border_t_1()
                        .border_color(BORDER)
                        .child(btn(t("Add account…"), Variant::Outline, Sz::Sm, Some(Icon::Plus), || {
                            store::open_modal(Modal::AddAgentAccount)
                        }))
                        .child(btn(t("Add API key…"), Variant::Outline, Sz::Sm, Some(Icon::Plus), || {
                            store::open_modal(Modal::AddAgentKey)
                        }))
                        .child(view().flex_1())
                        .child(icon_btn(Icon::RotateCw, Sz::Sm, t("Check again"), store::refresh_agents)),
                );
            }
            v.into_element()
        }))
    }))
}

fn agent_row(p: Provider, d: Option<Diagnostic>, cfg: Option<&Value>, editable: bool, first: bool) -> View {
    let family = agents::family(&p.id, cfg);
    let on = agents::enabled(cfg, &p);
    let removable = agents::created_by_sophon(cfg, &p.id);
    let gateway = agents::through_gateway(&p, &family);
    let native_claude = agents::native_claude(&p.id, cfg);
    let d = d.unwrap_or_default();

    let (status, tone) = if !on {
        (t("Disabled"), FG_MUTED)
    } else if p.status == "loading" {
        (t("Checking…"), FG_MUTED)
    } else if p.available() {
        (t("Ready"), SUCCESS)
    } else {
        (t("Unavailable"), DANGER)
    };
    let mut title = view()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(S2))
        .child(text(p.label.clone()).text_size(px(TEXT_BASE)).font_weight(FontWeight(500)))
        .child(badge(status, tone));
    if gateway {
        title = title.child(badge(t("Through a gateway"), if native_claude { WARNING } else { FG_MUTED }));
    }

    // Version · models · binary: what is known, nothing in place of the rest.
    let mut facts: Vec<String> = Vec::new();
    if let Some(v) = &d.version {
        facts.push(v.clone());
    }
    if !p.models.is_empty() {
        facts.push(tf("{} models", &[&p.models.len().to_string()]));
    }
    let mut body = view().flex_1().min_w(px(0.0)).gap(px(3.0)).child(title);
    if !facts.is_empty() {
        body = body.child(muted(facts.join(" · ")));
    }
    if let Some(path) = &d.path {
        body = body.child(muted(path.clone()).font_family(|| MONO).number_of_lines(1));
    }
    if let Some(line) = login_line(&d) {
        body = body.child(line);
    }
    if on && !p.available() && p.status != "loading" {
        let why = p.error.clone().or(d.error.clone()).unwrap_or_else(|| t("Not installed, or not found on the service's PATH").into());
        body = body.child(text(why).text_size(px(TEXT_SM)).text_color(DANGER));
    }
    if gateway && native_claude {
        body = body.child(
            view()
                .flex_row()
                .items_start()
                .gap(px(6.0))
                .child(icon(Icon::TriangleAlert, 14.0).text_color(WARNING).mt(px(2.0)))
                .child(
                    text(t("Some of its models come through a gateway. Claude Code must run natively, on your own account: use its own models"))
                        .flex_1()
                        .text_size(px(TEXT_SM))
                        .text_color(WARNING),
                ),
        );
    }
    if let Some(cmd) = pending_login(&p, &d, cfg) {
        body = body.child(muted(t("Sign in on that computer:").to_owned())).child(command_box(cmd));
    }

    let mut trailing = view().flex_row().items_center().gap(px(S2));
    if editable {
        if removable {
            let (id, label) = (p.id.clone(), p.label.clone());
            trailing = trailing.child(icon_btn(Icon::Trash2, Sz::Sm, t("Remove"), move || {
                store::open_modal(Modal::RemoveAgent { id: id.clone(), label: label.clone() })
            }));
        }
        let id = p.id.clone();
        trailing = trailing.child(switch(move || on, move |next| store::patch_providers(agents::enable_patch(&id, next))));
    }

    view()
        .flex_row()
        .items_start()
        .gap(px(S3))
        .px(px(S4))
        .py(px(S4))
        .when(!first, |v| v.border_t_1().border_color(BORDER))
        .child(
            view()
                .w(px(28.0))
                .h(px(28.0))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(R_LG))
                .bg(SURFACE2)
                .child(mark(&family, 16.0)),
        )
        .child(body)
        .child(trailing)
}

fn muted(s: String) -> nsh_core::element::Text {
    text(s).text_size(px(TEXT_SM)).text_color(FG_MUTED)
}

/// `Claude account · me@x.com · Max 20x · until 2026-11-29`, or that it is
/// signed out; nothing when the provider does not say.
fn login_line(d: &Diagnostic) -> Option<nsh_core::element::Text> {
    let l = d.login.as_ref()?;
    if !l.signed_in {
        return Some(text(t("Not signed in")).text_size(px(TEXT_SM)).text_color(WARNING));
    }
    let mut parts: Vec<String> = vec![match l.method.as_deref() {
        Some("claude.ai") => t("Claude account").into(),
        Some("api_key" | "apiKey") => t("API key").into(),
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

/// The login command of an account instance Sophon added that is not
/// signed in yet.
fn pending_login(p: &Provider, d: &Diagnostic, cfg: Option<&Value>) -> Option<String> {
    if !agents::created_by_sophon(cfg, &p.id) {
        return None;
    }
    let env = cfg?.get("providers")?.get(&p.id)?.get("env")?;
    let (base, dir) = if let Some(dir) = env.get("CLAUDE_CONFIG_DIR").filter(|_| env.get("ANTHROPIC_BASE_URL").is_none()) {
        (Base::Claude, dir)
    } else if let Some(dir) = env.get("CODEX_HOME").filter(|_| env.get("OPENAI_BASE_URL").is_none()) {
        (Base::Codex, dir)
    } else {
        return None;
    };
    // Codex does not report its login: an unavailable instance is the hint.
    let signed_out = d.login.as_ref().map(|l| !l.signed_in).unwrap_or(!p.available());
    let dir = dir.as_str()?;
    signed_out.then(|| agents::login_command(base, dir))
}

/// A command in a mono box with a copy button.
fn command_box(cmd: String) -> View {
    let copy = cmd.clone();
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .pl(px(S3))
        .pr(px(S1))
        .py(px(S1))
        .rounded(px(R_LG))
        .bg(SURFACE2)
        .border_1()
        .border_color(BORDER)
        .child(text(cmd).flex_1().min_w(px(0.0)).font_family(|| MONO).text_size(px(TEXT_SM)))
        .child(icon_btn(Icon::Copy, Sz::Sm, t("Copy"), move || {
            set_clipboard(copy.clone());
            store::notify(t("Command copied").into());
        }))
}

// ------------------------------------------------------------------ forms

fn base_label(b: Base) -> &'static str {
    match b {
        Base::Claude => "Claude Code",
        Base::Codex => "Codex",
    }
}

fn base_choice(base: Signal<Base>) -> View {
    segmented(
        vec![(Base::Claude, base_label(Base::Claude).into()), (Base::Codex, base_label(Base::Codex).into())],
        move || base.get(),
        move |b| base.set(b),
    )
}

fn hint(s: impl IntoReactive<SharedString>) -> nsh_core::element::Text {
    text(s).text_size(px(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED)
}

/// Another account of Claude Code or Codex: a separate instance whose
/// config folder holds its own login, then the command that signs it in.
pub(crate) fn add_account() -> View {
    let base = signal(Base::Claude);
    let name = signal(String::new());
    let dir = signal(String::new());
    // After adding: the instance's label and its login command.
    let added: Signal<Option<(String, String)>> = signal(None);
    let slug = move || agents::new_id(&name.get(), "account", &[]);
    let folder = move || {
        let typed = dir.get();
        if typed.trim().is_empty() { base.get().default_dir(&slug()) } else { typed }
    };
    let target = computed(move || agents::absolute_dir(&folder(), store::agent_home().as_deref()));
    let ready = computed(move || !name.with(|n| n.trim().is_empty()) && target.with(Option::is_some));
    let add = move || {
        let (Some(dir), b, n) = (target.get_untracked(), base.get_untracked(), name.get_untracked()) else { return };
        if n.trim().is_empty() {
            return;
        }
        let id = agents::new_id(&n, b.id(), &store::provider_ids());
        let label = format!("{} ({})", base_label(b), n.trim());
        store::patch_providers(agents::account_patch(&id, b, &label, &dir));
        added.set(Some((label, agents::login_command(b, &dir))));
    };
    sheet(t("Add account")).w(px(520.0)).child(dynamic(move || {
        let done = added.get();
        Some(untracked(move || match done {
            Some((label, cmd)) => view()
                .gap(px(S3))
                .child(hint(tf("“{}” is added. Sign it in on that computer, then press Check again", &[&label])))
                .child(command_box(cmd))
                .child(actions(btn(t("Done"), Variant::Primary, Sz::Md, None, store::close_modal)))
                .into_element(),
            None => view()
                .gap(px(S3))
                .child(hint(t("A separate agent with its own login, next to the one you have. Sophon never switches accounts on its own")))
                .child(base_choice(base))
                .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).placeholder(t("Name, e.g. Work")))
                .child(
                    field()
                        .value(move || dir.get())
                        .on_change(move |v: &str| dir.set(v.to_owned()))
                        .placeholder(move || base.get().default_dir(&slug()))
                        .font_family(|| MONO),
                )
                .child(hint(move || match target.get() {
                    Some(d) => tf("Its config folder on that computer: {}", &[&d]),
                    None => t("Use a full path: Sophon does not know that computer's home folder").into(),
                }))
                .child(actions(
                    btn(t("Add"), Variant::Primary, Sz::Md, Some(Icon::Plus), add).disabled(move || !ready.get()),
                ))
                .into_element(),
        }))
    }))
}

/// An instance on an API key: a documented preset or a custom endpoint.
/// It gets its own Claude config folder, so it never falls back on the
/// subscription login (or the gateway a settings file may name).
pub(crate) fn add_api_key() -> View {
    // The preset's index; `None` is Custom.
    let preset: Signal<Option<usize>> = signal(Some(0));
    let base = signal(Base::Claude);
    let name = signal(String::new());
    let url = signal(String::new());
    let models = signal(String::new());
    let key = signal(String::new());
    let instance = computed(move || {
        let p = preset.get().and_then(|i| PRESETS.get(i));
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
        (id, k)
    });
    let problem = computed(move || instance.with(|(_, k)| agents::key_instance_problem(k)));
    let add = move || {
        let (id, k) = instance.get_untracked();
        if agents::key_instance_problem(&k).is_some() {
            return;
        }
        store::patch_providers(agents::key_instance_patch(&id, &k));
        store::notify(tf("Added “{}”", &[&k.label]));
        store::close_modal();
    };

    let mut presets = view().flex_row().flex_wrap().gap(px(S2));
    for (i, p) in PRESETS.iter().enumerate() {
        presets = presets.child(chip(p.label, None, move || preset.get() == Some(i), move || preset.set(Some(i))));
    }
    presets = presets.child(chip(t("Custom"), None, move || preset.get().is_none(), move || preset.set(None)));

    sheet(t("Add API key"))
        .w(px(520.0))
        .child(hint(t("A separate agent that runs Claude Code or Codex against another endpoint, billed to that key. It never uses your subscription login")))
        .child(presets)
        .child(dynamic(move || {
            let custom = preset.get().is_none();
            Some(untracked(move || {
                if custom {
                    view()
                        .gap(px(S3))
                        .child(base_choice(base))
                        .child(
                            field()
                                .value(move || url.get())
                                .on_change(move |v: &str| url.set(v.to_owned()))
                                .placeholder(move || match base.get() {
                                    Base::Claude => t("Anthropic-compatible endpoint, e.g. https://example.com/anthropic"),
                                    Base::Codex => t("OpenAI Responses endpoint, e.g. https://example.com/v1"),
                                })
                                .font_family(|| MONO),
                        )
                        .child(
                            field()
                                .value(move || models.get())
                                .on_change(move |v: &str| models.set(v.to_owned()))
                                .placeholder(t("Model ids, the default first, e.g. model-a, model-b"))
                                .font_family(|| MONO),
                        )
                        .into_element()
                } else {
                    let p = preset.get_untracked().and_then(|i| PRESETS.get(i)).copied();
                    let line = p
                        .map(|p| format!("{} · {}", p.base_url, p.models.iter().map(|(_, l)| *l).collect::<Vec<_>>().join(", ")))
                        .unwrap_or_default();
                    hint(line).font_family(|| MONO).into_element()
                }
            }))
        }))
        .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).placeholder(move || {
            match preset.get().and_then(|i| PRESETS.get(i)) {
                Some(p) => tf("Name (default: {})", &[p.label]),
                None => t("Name").into(),
            }
        }))
        .child(field().value(move || key.get()).on_change(move |v: &str| key.set(v.to_owned())).masked(true).placeholder(t("API key")))
        .child(hint(t("The key is kept in the service's config file on that computer, like any provider's")).text_size(px(TEXT_XS)))
        .child(hint(move || problem.get().map(t).unwrap_or_default()).text_color(WARNING))
        .child(actions(
            btn(t("Add"), Variant::Primary, Sz::Md, Some(Icon::Plus), add).disabled(move || problem.with(Option::is_some)),
        ))
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
