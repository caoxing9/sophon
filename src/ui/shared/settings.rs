//! Settings sections: each a label over a card of rows,
//! controls in the rows' trailing slot. The desktop and the phone each
//! assemble their page from them.

use crate::i18n::{Lang, t, tf};
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store::{self, Conn, SendBehavior};

/// Theme and language.
pub(crate) fn appearance_card() -> View {
    card()
        .child(row(
            t("Theme"),
            t("Light, dark, or as the system is set"),
            segmented(
                vec![
                    (nsh_core::platform::Appearance::System, t("System").into()),
                    (nsh_core::platform::Appearance::Light, t("Light").into()),
                    (nsh_core::platform::Appearance::Dark, t("Dark").into()),
                ],
                store::appearance,
                store::set_appearance,
            ),
            true,
        ))
        .child(row(
            t("Language"),
            t("The language of Sophon's interface"),
            segmented(
                vec![(Lang::Auto, t("System").into()), (Lang::En, "English".into()), (Lang::Zh, "中文".into())],
                crate::i18n::lang,
                store::set_language,
            ),
            false,
        ))
        .child(row(
            t("Interface font"),
            t("Any font installed on this device"),
            font_field(false, "SF Pro"),
            false,
        ))
        .child(row(
            t("Code font"),
            t("For code, terminals and numbers"),
            font_field(true, "JetBrains Mono"),
            false,
        ))
        .child(row(
            t("Text size"),
            t("The size of the interface's text, in points"),
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon_btn(Icon::Minus, Sz::Sm, t("Smaller"), || store::set_font_size(store::look().0 - 1.0)))
                .child(
                    nsh_base::button()
                        .on_press(|| store::set_font_size(store::DEFAULT_FONT_SIZE))
                        .accessibility_label(t("Back to the default size"))
                        .w(px(40.0))
                        .items_center()
                        .child(text(|| format!("{}", store::look().0)).text_size(tx(TEXT_BASE)).font_family(mono_font)),
                )
                .child(icon_btn(Icon::Plus, Sz::Sm, t("Larger"), || store::set_font_size(store::look().0 + 1.0))),
            false,
        ))
        .child(row(
            t("Compact"),
            t("Tighter rows and gaps: more on screen"),
            switch(|| store::look().1, store::set_compact),
            false,
        ))
        // The same, by hand: `fontFamily`, `monoFontFamily`, `fontSize` and
        // `compact` in the settings file, read when Sophon starts.
        .when(!MOBILE && cfg!(target_os = "macos"), |card| {
            card.child(row(
                t("Settings file"),
                t("fontFamily, monoFontFamily, fontSize and compact can be set there too; Sophon reads it when it starts"),
                btn(t("Open settings.json"), Variant::Outline, Sz::Sm, None, store::open_settings_file),
                false,
            ))
        })
}

/// The font in use, or the default's name: pressed, the installed fonts
/// to pick from.
fn font_field(mono: bool, _example: &'static str) -> impl IntoElement {
    btn(
        move || -> SharedString {
            let (ui, code) = store::fonts();
            match if mono { code } else { ui } {
                Some(name) => name.into(),
                None if mono => "Menlo".into(),
                None => t("System font").into(),
            }
        },
        Variant::Outline,
        Sz::Sm,
        Some(Icon::ChevronsUpDown),
        move || store::open_modal(store::Modal::Fonts { mono }),
    )
}

/// What Return does while an agent runs.
pub(crate) fn sending_card() -> View {
    card().child(row(
        t("While an agent is running"),
        t("What Return does with your message"),
        segmented(
            vec![
                (SendBehavior::Steer, t("Steer").into()),
                (SendBehavior::Interrupt, t("Interrupt").into()),
                (SendBehavior::Queue, t("Queue").into()),
            ],
            || store::settings().send_behavior,
            store::set_send_behavior,
        ),
        true,
    ))
}

/// Whether this Mac stays awake while its agents run (the desktop's).
/// The key that starts and stops dictation (the desktop's).
pub(crate) fn dictation_card() -> View {
    use store::DictationKey;
    card().child(row(
        t("Dictation key"),
        t("Fn: hold it to talk, let go to stop (set macOS's \"Press 🌐 key to\" to Do Nothing, System Settings → Keyboard). ⇧⌘D: press to start and to stop"),
        segmented(
            vec![(DictationKey::Fn, "Fn".into()), (DictationKey::ShiftCmdD, "⇧⌘D".into()), (DictationKey::Off, t("Off").into())],
            store::dictation_key,
            store::set_dictation_key,
        ),
        true,
    ))
}

pub(crate) fn keep_awake_card() -> View {
    use store::KeepAwake;
    card().child(row(
        t("Prevent sleep"),
        t("Keeps this Mac awake while its agents run"),
        segmented(
            vec![(KeepAwake::Off, t("Off").into()), (KeepAwake::On, t("On").into()), (KeepAwake::Display, t("Screen on too").into())],
            || store::settings().keep_awake,
            |k| store::edit_settings(|s| s.keep_awake = k),
        ),
        true,
    ))
}

/// What this device keeps on disk, each with its size and a button that
/// forgets it: the pictures of the messages sent from it (`sent_pictures`),
/// and its copies of the hosts' lists and conversations (`daemon::cache`).
pub(crate) fn storage_card() -> View {
    fn size(bytes: u64) -> String {
        if bytes >= 1_048_576 { format!("{:.1} MB", bytes as f64 / 1_048_576.0) } else { format!("{} KB", bytes.div_ceil(1024)) }
    }
    let pictures = signal(crate::sent_pictures::usage());
    let copies = signal(crate::daemon::cache::usage());
    card()
        .child(row(
            t("Sent pictures"),
            move || match pictures.get() {
                (0, _) => t("Nothing kept yet").to_string(),
                (count, bytes) => tf("{} messages · {} on this device", &[&count.to_string(), &size(bytes)]),
            },
            btn(t("Clear"), Variant::Outline, Sz::Sm, None, move || {
                crate::sent_pictures::clear();
                pictures.set(crate::sent_pictures::usage());
            }),
            true,
        ))
        .child(row(
            t("Offline copies"),
            move || match copies.get() {
                (0, _) => t("Nothing kept yet").to_string(),
                (_, bytes) => tf("Lists and recent conversations, shown before the computer answers · {}", &[&size(bytes)]),
            },
            btn(t("Clear"), Variant::Outline, Sz::Sm, None, move || {
                crate::daemon::cache::clear();
                copies.set(crate::daemon::cache::usage());
            }),
            false,
        ))
}

/// The settings page frame: a centered column under the title.
pub(crate) fn page(compact: bool, sections: View) -> impl IntoElement {
    scroll().flex_1().child(
        view()
            .w_full()
            .max_w(px(720.0))
            .self_center()
            .px(px(if compact { S4 } else { S6 }))
            .py(px(if compact { S4 } else { S8 }))
            .gap(px(if compact { S6 } else { S8 }))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .child(text(t("Settings")).text_size(tx(TEXT_2XL)).font_weight(FontWeight(500))),
            )
            .child(sections.gap(px(if compact { S6 } else { S8 }))),
    )
}

pub(crate) fn section(title: &'static str, body: impl IntoElement) -> View {
    view().gap(sp(S3)).child(section_label(title)).child(body)
}

/// A settings row: title and hint on the leading rail, the control trailing.
pub(crate) fn row(title: impl IntoReactive<SharedString>, hint: impl IntoReactive<SharedString>, control: impl IntoElement, first: bool) -> View {
    view()
        .flex_row()
        .items_center()
        .gap(sp(S4))
        .px(sp(S4))
        .py(sp(S4))
        .when(!first, |v| v.border_t_1().border_color(BORDER))
        .child(
            view()
                .flex_1()
                .gap(px(2.0))
                .child(text(title).text_size(tx(TEXT_BASE)).text_color(FG))
                .child(text(hint).text_size(tx(TEXT_SM)).text_color(FG_MUTED)),
        )
        .child(control)
}

pub(crate) fn host_card() -> View {
    let conn = computed(store::conn);
    let field = move |f: fn(&Conn) -> String| move || conn.with(f);
    card()
        .child(row(
            field(|c| match c {
                // The name given to it here, when it has one.
                Conn::Connected { hostname, .. } => store::current_host().filter(|h| h.named()).map(|h| h.label()).unwrap_or_else(|| hostname.clone()),
                Conn::Down(_) => t("Offline").into(),
                _ => t("Connecting…").into(),
            }),
            field(|c| match c {
                Conn::Connected { url, .. } => url.clone(),
                Conn::Down(reason) => reason.clone(),
                _ => String::new(),
            }),
            status_dot(move || match conn.get() {
                Conn::Connected { .. } => Status::Done,
                Conn::Down(_) => Status::Failed,
                _ => Status::Idle,
            }),
            true,
        ))
        .child(row(
            t("Service version"),
            t("The Sophon service this app talks to"),
            text(field(|c| match c {
                Conn::Connected { version, .. } if !version.is_empty() => version.clone(),
                _ => "—".into(),
            }))
            .text_size(tx(TEXT_BASE))
            .font_family(mono_font)
            .text_color(FG_MUTED),
            false,
        ))
}

/// The model the service names chats and branches with. Default leaves the
/// choice to the service (a small model it finds, else the chat's own).
pub(crate) fn text_generation_card() -> View {
    let loaded = computed(|| store::daemon_config().is_some());
    let label = computed(|| match store::text_generation_model() {
        None => t("Default").to_owned(),
        Some((provider, model)) => store::provider(&provider)
            .and_then(|p| p.model(&model).map(|m| format!("{} · {}", m.label, p.label)))
            .unwrap_or_else(|| if model.is_empty() { provider } else { model }),
    });
    card().child(row(
        t("Text generation model"),
        t("Names chats and branches. A small, fast model is plenty and costs less"),
        dynamic(move || {
            if !loaded.get() {
                return Some(untracked(|| working(t("Loading…")).into_element()));
            }
            Some(untracked(move || {
                dropdown(picker_trigger(Some(Icon::Cpu), move || label.get()))
                    .align(nsh_core::overlay::Align::End)
                    .child(dynamic(move || {
                        let current = store::text_generation_model();
                        let providers: Vec<_> = store::providers().into_iter().filter(|p| p.available()).collect();
                        Some(untracked(move || {
                            let mut list = view().child(menu_row(
                                t("Default"),
                                None,
                                Some(t("The service picks").into()),
                                current.is_none(),
                                || store::set_text_generation_model(None),
                            ));
                            for p in &providers {
                                // As the model picker offers them: Claude Code's own
                                // models only, and never a Claude model through a
                                // subscription.
                                let models: Vec<_> = p
                                    .models
                                    .iter()
                                    .filter(|m| p.id != "claude" || !m.id.contains('/'))
                                    .filter(|m| !store::forbidden_model(&m.id, &m.label))
                                    .collect();
                                if models.is_empty() {
                                    continue;
                                }
                                list = list.child(menu_divider()).child(menu_heading(p.label.clone()));
                                for m in models {
                                    let (pid, mid) = (p.id.clone(), m.id.clone());
                                    let checked = current.as_ref().is_some_and(|(cp, cm)| *cp == pid && *cm == mid);
                                    list = list.child(menu_row(m.label.clone(), None, None, checked, move || {
                                        store::set_text_generation_model(Some((&pid, &mid)))
                                    }));
                                }
                            }
                            menu_scroll(420.0, list).into_element()
                        }))
                    }))
                    .into_element()
            }))
        }),
        true,
    ))
}

fn flag(pointer: &'static str, default: bool) -> impl IntoElement {
    switch(
        move || store::daemon_config().and_then(|c| c.pointer(pointer).and_then(serde_json::Value::as_bool)).unwrap_or(default),
        move |on| store::set_daemon_config(pointer, on.into()),
    )
}

/// A host setting as a row with its switch. The service answers its config
/// with every setting it has, defaults included: one missing is one an older
/// service lacks, shown off with a note rather than a switch that flips back.
fn host_flag(title: &'static str, hint: &'static str, pointer: &'static str, default: bool, first: bool) -> View {
    let supported = store::daemon_config().is_some_and(|c| c.pointer(pointer).is_some());
    if supported {
        return row(title, hint, flag(pointer, default), first);
    }
    row(
        title,
        tf("{} · Needs a newer service on this host", &[hint]),
        switch(|| false, |_| {}).disabled(true).opacity(0.4),
        first,
    )
}

pub(crate) fn behavior_card() -> impl IntoElement {
    let prompt = signal(String::new());
    let loaded = computed(|| store::daemon_config().is_some());
    effect(move || {
        if let Some(p) = store::daemon_config().and_then(|c| c.get("appendSystemPrompt").and_then(|p| p.as_str().map(str::to_owned))) {
            untracked(|| prompt.set(p));
        }
    });
    view().child(dynamic(move || {
        if !loaded.get() {
            return Some(untracked(|| working(t("Loading…")).into_element()));
        }
        Some(untracked(move || {
            card()
                .child(host_flag(t("Inject Sophon tools"), t("Agents can open terminals, start agents and more through MCP"), "/mcp/injectIntoAgents", true, true))
                .child(host_flag(t("Archive after merge"), t("Archive a workspace once its branch is merged"), "/autoArchiveAfterMerge", false, false))
                .child(host_flag(t("Free idle agents"), t("Stop idle agent runtimes to save memory; they resume on demand"), "/idleRuntimeCleanup", false, false))
                .child(host_flag(t("Resume on startup"), t("Pick up running agents when the service restarts"), "/resumeOnStartup", false, false))
                .child(host_flag(t("Usage reports"), t("Collect token and cost usage per provider"), "/usage/enabled", false, false))
                .child(host_flag(t("Remote relay"), t("Reach this host from your phone through the encrypted relay"), "/relay/enabled", true, false))
                .child(
                    view()
                        .px(sp(S4))
                        .py(sp(S4))
                        .gap(sp(S2))
                        .border_t_1()
                        .border_color(BORDER)
                        .child(text(t("Extra system prompt")).text_size(tx(TEXT_BASE)))
                        .child(text(t("Appended to every agent's instructions")).text_size(tx(TEXT_SM)).text_color(FG_MUTED))
                        .child(
                            field()
                                .multiline(true)
                                .h(px(88.0))
                                .py(sp(S2))
                                .value(move || prompt.get())
                                .on_change(move |v: &str| prompt.set(v.to_owned()))
                                .placeholder(t("e.g. Answer in Chinese. Run the tests before you finish")),
                        )
                        .child(
                            view().flex_row().justify_end().child(btn(t("Save prompt"), Variant::Secondary, Sz::Sm, None, move || {
                                store::set_daemon_config("/appendSystemPrompt", prompt.get_untracked().into())
                            })),
                        ),
                )
                .child(row(
                    t("Restart service"),
                    t("Restarts the service. Sophon reconnects on its own"),
                    btn(t("Restart"), Variant::Outline, Sz::Sm, Some(Icon::RotateCw), || store::open_modal(store::Modal::RestartDaemon)),
                    false,
                ))
                .into_element()
        }))
    }))
}
