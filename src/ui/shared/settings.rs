//! Settings sections: each a label over a card of rows,
//! controls in the rows' trailing slot. The desktop and the phone each
//! assemble their page from them.

use crate::i18n::{Lang, t, tf};
use nsh_core::{
    element::{AdaptiveColor, FontWeight, View},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store::{self, Conn, DefaultModel, SendBehavior};

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
        .child(row(t("Color theme"), t("The interface's colors"), theme_swatches(), false))
        .child(row(
            t("Background"),
            t("A solid color of your own; light or dark follows it"),
            hex_control(
                BACKGROUNDS,
                || store::color_theme().background,
                |c| store::edit_color_theme(|t| t.background = c),
            ),
            false,
        ))
        .child(row(
            t("Accent color"),
            t("Buttons, links and the caret"),
            hex_control(ACCENTS, || store::color_theme().accent, |c| store::edit_color_theme(|t| t.accent = c)),
            false,
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
        .when(!MOBILE, |card| {
            card.child(row(
                t("Content width"),
                t("How wide the conversation and the composer go"),
                segmented(
                    vec![
                        (store::ContentWidth::Standard, t("Standard").into()),
                        (store::ContentWidth::Wide, t("Wide").into()),
                        (store::ContentWidth::Full, t("Full width").into()),
                    ],
                    store::content_width_tracked,
                    store::set_content_width,
                ),
                false,
            ))
        })
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

/// Solid backgrounds to pick from at a press: light, warm, greys, dark.
const BACKGROUNDS: &[u32] = &[0xffffff, 0xf7f5ef, 0xeef1f5, 0xe9efe6, 0x2b2b2b, 0x1e1e1e, 0x101418, 0x000000];
const ACCENTS: &[u32] = &[0xc96442, 0x339cff, 0x7c5cff, 0x10a37f, 0xe5484d, 0xf59e0b, 0x000000];

/// A round colour well; ringed when it is the one in use.
fn swatch(color: u32, selected: bool, label: SharedString, on_press: impl Fn() + 'static) -> impl IntoElement {
    nsh_base::button()
        .on_press(on_press)
        .accessibility_label(label)
        .w(px(26.0))
        .h(px(26.0))
        .p(px(2.0))
        .rounded(px(R_FULL))
        .border_1()
        .border_color(if selected { ACCENT() } else { tint(BORDER(), 0.0) })
        .child(view().flex_1().rounded(px(R_FULL)).bg(AdaptiveColor::rgb(color, color)).border_1().border_color(BORDER_STRONG()))
}

/// The theme presets, each a swatch of its accent.
fn theme_swatches() -> impl IntoElement {
    let current = store::color_theme().preset;
    let mut row = view().flex_row().items_center().gap(px(4.0));
    for &(key, name, color, _, _) in THEME_PRESETS {
        row = row.child(swatch(color, current == key, name.into(), move || store::edit_color_theme(|t| t.preset = key.to_owned())));
    }
    row
}

/// A colour of the user's own: swatches to press, a hex field (applied on
/// Return: each change mounts the UI again) and a reset.
fn hex_control(choices: &'static [u32], current: fn() -> Option<u32>, set: fn(Option<u32>)) -> impl IntoElement {
    let now = current();
    let typed = signal(now.map(store::hex_of).unwrap_or_default());
    let mut wells = view().flex_row().items_center().gap(px(2.0));
    for &c in choices {
        wells = wells.child(swatch(c, now == Some(c), store::hex_of(c).into(), move || set(Some(c))));
    }
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .child(wells)
        .child(
            field()
                .w(px(92.0))
                .h(sp(CONTROL_SM))
                .placeholder("#rrggbb")
                .value(move || typed.get())
                .on_change(move |v: &str| typed.set(v.to_owned()))
                .on_submit(move |v: &str| {
                    let v = v.trim();
                    if v.is_empty() {
                        set(None);
                    } else if let Some(c) = store::parse_hex(v) {
                        set(Some(c));
                    }
                }),
        )
        .child(when(move || now.is_some(), move || icon_btn(Icon::Undo2, Sz::Sm, t("Reset"), move || set(None))))
}

/// ⌘1–⌘9 (the desktop's).
pub(crate) fn shortcuts_card() -> View {
    card().child(row(
        t("Number shortcuts"),
        t("Use ⌘1–9 to switch conversations"),
        switch(store::number_shortcuts, store::set_number_shortcuts),
        true,
    ))
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

/// What Return does while an agent runs, and how the agent's tool calls show.
pub(crate) fn sending_card() -> View {
    use crate::fold::ToolDisplay;
    card()
    .child(row(
        t("Tool calls"),
        t("The commands and reads between the agent's replies"),
        segmented(
            vec![
                (ToolDisplay::Auto, t("Expanded").into()),
                (ToolDisplay::Folded, t("Folded").into()),
                (ToolDisplay::Quiet, t("Quiet").into()),
                (ToolDisplay::Stream, t("Live").into()),
                (ToolDisplay::Hidden, t("Hidden").into()),
            ],
            store::tool_display,
            store::set_tool_display,
        ),
        true,
    ))
    .child(row(
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
        false,
    ))
}

/// When Sophon tells you of its agents with the system's notifications
/// (the desktop's): a finished turn, an approval, a question; their sound;
/// and whether the system lets it show them at all.
pub(crate) fn notifications_card() -> View {
    use nsh_core::notifications::NotificationPermission as P;
    use store::{NotifyWhen, On};
    let permission = signal(P::Default);
    let refresh = move || {
        spawn(async move {
            permission.set(notification_permission().await);
        })
    };
    refresh();
    let allowed = dynamic(move || {
        let p = permission.get();
        Some(untracked(move || match p {
            P::Granted | P::Unavailable => view().into_element(),
            P::Denied => btn(t("Open System Settings"), Variant::Outline, Sz::Sm, None, || {
                let _ = open_url("x-apple.systempreferences:com.apple.Notifications-Settings.extension".to_owned());
            })
            .into_element(),
            P::Default => btn(t("Allow"), Variant::Outline, Sz::Sm, None, move || {
                spawn(async move {
                    permission.set(request_notification_permission().await);
                });
            })
            .into_element(),
        }))
    });
    card()
        .child(row(
            t("Allowed by the system"),
            move || match permission.get() {
                P::Granted => t("Notifications show"),
                P::Denied => t("Turned off for Sophon in System Settings: nothing shows"),
                P::Default => t("Not asked yet"),
                P::Unavailable => t("This build cannot show notifications"),
            }
            .to_string(),
            allowed,
            true,
        ))
        .child(row(
            t("Turn finished"),
            t("When an agent finishing its turn is told"),
            {
                let name = |w: NotifyWhen| match w {
                    NotifyWhen::Never => t("Never"),
                    NotifyWhen::Away => t("Only when unfocused"),
                    NotifyWhen::Always => t("Always"),
                };
                dropdown(picker_trigger(None, move || SharedString::from(name(store::settings().notify_finished))))
                    .align(nsh_core::overlay::Align::End)
                    .child(dynamic(move || {
                        let current = store::settings().notify_finished;
                        Some(untracked(move || {
                            let mut list = view();
                            for w in [NotifyWhen::Never, NotifyWhen::Away, NotifyWhen::Always] {
                                list = list.child(menu_row(name(w), None, None, current == w, move || {
                                    store::edit_settings(|s| s.notify_finished = w)
                                }));
                            }
                            list
                        }))
                    }))
            },
            false,
        ))
        .child(row(
            t("Approvals"),
            t("When an agent waits for your approval"),
            switch(|| store::settings().notify_approval.0, |on| store::edit_settings(|s| s.notify_approval = On(on)))
                .accessibility_label(t("Approvals")),
            false,
        ))
        .child(row(
            t("Questions and errors"),
            t("When an agent asks you something or stops on an error"),
            switch(|| store::settings().notify_attention.0, |on| store::edit_settings(|s| s.notify_attention = On(on)))
                .accessibility_label(t("Questions and errors")),
            false,
        ))
        .child(row(
            t("Sound"),
            t("The system's notification sound"),
            switch(|| store::settings().notify_sound.0, |on| store::edit_settings(|s| s.notify_sound = On(on)))
                .accessibility_label(t("Sound")),
            false,
        ))
}

/// Whether this Mac stays awake while its agents run (the desktop's).
/// Dictation's shortcut (the desktop's), recorded as Raycast's are: a
/// press on the field, then the combination; Fn on its own by default.
pub(crate) fn dictation_card() -> View {
    let recording = computed(store::recording_dictation_key);
    let key = computed(store::dictation_key);
    let field = nsh_base::button()
        .on_press(move || store::record_dictation_key(!recording.get_untracked()))
        .accessibility_label(t("Dictation key"))
        .min_w(px(120.0))
        .h(sp(CONTROL_SM))
        .px(sp(S3))
        .rounded(px(R_LG))
        .items_center()
        .justify_center()
        .bg(SURFACE2())
        .border_1()
        .border_color(move || if recording.get() { ACCENT().into() } else { tint(BORDER(), 1.0) })
        .child(text(move || {
            if recording.get() {
                t("Press keys…").to_owned()
            } else {
                key.get().map(|k| k.label).unwrap_or_else(|| t("None").to_owned())
            }
        })
        .text_size(tx(TEXT_BASE))
        .font_weight(FontWeight(500))
        .text_color(move || if recording.get() { FG_MUTED() } else { FG() }));
    let reset = when(move || !recording.get() && !key.get().is_some_and(|k| k.is_fn()), || {
        icon_btn(Icon::RotateCw, Sz::Sm, t("Back to Fn"), || store::set_dictation_key(Some(store::Hotkey::fn_key())))
    });
    card().child(row(
        t("Dictation key"),
        move || {
            if recording.get() {
                t("Press the combination, or Fn alone. Esc cancels, Delete leaves dictation without a key")
            } else {
                t("Press it to record another. Fn: hold to talk, let go to stop (set macOS's \"Press 🌐 key to\" to Do Nothing, System Settings → Keyboard). Any other: press to start, again to stop")
            }
            .to_owned()
        },
        view().flex_row().items_center().gap(sp(S2)).child(reset).child(field),
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
        .when(!first, |v| v.border_t_1().border_color(BORDER()))
        .child(
            view()
                .flex_1()
                .gap(px(2.0))
                .child(text(title).text_size(tx(TEXT_BASE)).text_color(FG()))
                .child(text(hint).text_size(tx(TEXT_SM)).text_color(FG_MUTED())),
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
            .text_color(FG_MUTED()),
            false,
        ))
}

/// What a new conversation starts with: a model of one of the agents and,
/// when it has levels, how hard it thinks. "Last used" leaves it as it was:
/// the model the previous conversation was started with.
pub(crate) fn default_model_card() -> View {
    // The model chosen, as the agents offer it now.
    let chosen = computed(|| {
        let d = store::default_model()?;
        let model = store::provider(&d.provider).and_then(|p| store::offered_models(&p).into_iter().find(|m| m.id == d.model).map(|m| (p.label.clone(), m)));
        Some((d, model))
    });
    let label = computed(move || match chosen.get() {
        None => t("Last used").to_owned(),
        Some((_, Some((agent, m)))) => format!("{} · {agent}", m.name),
        Some((d, None)) => d.model,
    });
    card()
        .child(row(
            t("Default model"),
            t("What a new conversation starts with. It can still be changed in the conversation"),
            dropdown(picker_trigger(Some(Icon::Cpu), move || label.get()))
                .align(nsh_core::overlay::Align::End)
                .child(dynamic(move || {
                    let current = store::default_model();
                    let providers: Vec<_> = store::providers().into_iter().filter(|p| p.available()).collect();
                    Some(untracked(move || {
                        let mut list = view().child(menu_row(
                            t("Last used"),
                            None,
                            Some(t("The model the last conversation was started with").into()),
                            current.is_none(),
                            || store::set_default_model(None),
                        ));
                        for p in &providers {
                            let models = store::offered_models(p);
                            if models.is_empty() {
                                continue;
                            }
                            list = list.child(menu_divider()).child(menu_heading(p.label.clone()));
                            for m in models {
                                let (pid, mid) = (p.id.clone(), m.id.clone());
                                let checked = current.as_ref().is_some_and(|d| d.provider == pid && d.model == mid);
                                // The level it had, where the new model has it too.
                                let kept = current.as_ref().and_then(|d| d.thinking.clone());
                                list = list.child(menu_row(m.name.clone(), None, None, checked, move || {
                                    store::set_default_model(Some(DefaultModel { provider: pid.clone(), model: mid.clone(), thinking: kept.clone() }))
                                }));
                            }
                        }
                        menu_scroll(420.0, list).into_element()
                    }))
                })),
            true,
        ))
        // How hard it thinks, when the model chosen has levels.
        .child(dynamic(move || {
            let (d, Some((_, model))) = chosen.get()? else { return None };
            if model.levels.is_empty() {
                return None;
            }
            Some(untracked(move || {
                let now = d.thinking.clone().filter(|t| model.levels.iter().any(|(id, _)| id == t)).or(model.default_level.clone());
                let shown = model.levels.iter().find(|(id, _)| Some(id) == now.as_ref()).map(|(_, l)| l.clone()).unwrap_or_default();
                let mut menu = dropdown(picker_trigger(Some(Icon::Brain), shown)).align(nsh_core::overlay::Align::End);
                for (id, level) in &model.levels {
                    let (on, pick) = (Some(id) == now.as_ref(), DefaultModel { thinking: Some(id.clone()), ..d.clone() });
                    menu = menu.child(menu_row(level.clone(), None, None, on, move || store::set_default_model(Some(pick.clone()))));
                }
                row(t("Reasoning"), t("How hard the default model thinks"), menu, false)
            }))
        }))
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
                return Some(untracked(|| spinner(20.0).into_element()));
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
            return Some(untracked(|| spinner(20.0).into_element()));
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
                        .border_color(BORDER())
                        .child(text(t("Extra system prompt")).text_size(tx(TEXT_BASE)))
                        .child(text(t("Appended to every agent's instructions")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
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
