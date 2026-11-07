//! The composer, as Paseo's: the input over a toolbar of agent controls
//! (provider and model, thinking, mode) with the send / stop disc in the
//! corner. One composer serves a live agent (`Target::Agent`, controls
//! change the agent) and a new one (`Target::Form`, controls fill the form).

use crate::i18n::{t, tf};
use nsh_base::{button, input};
use nsh_core::{element::FontWeight, prelude::*};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store::{self, Control};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Target {
    Agent,
    Form,
}

pub(crate) fn composer(target: Target, placeholder: &'static str) -> AnyElement {
    let live = target == Target::Agent;
    // The text: the agent's kept draft, or the form's prompt.
    let text_value = computed(move || match target {
        Target::Agent => store::selected_id().map(|id| store::draft(&id)).unwrap_or_default(),
        Target::Form => store::form().prompt,
    });
    let set_text = move |v: &str| match target {
        Target::Agent => {
            if let Some(id) = store::selected_id() {
                store::set_draft(&id, v);
            }
        }
        Target::Form => store::edit_form(|f| f.prompt = v.to_owned()),
    };
    let submit = move || match target {
        Target::Agent => {
            if let Some(id) = store::selected_id()
                && store::send_message(&store::draft(&id))
            {
                store::set_draft(&id, "");
            }
        }
        Target::Form => store::submit_form(),
    };
    let running = computed(move || live && store::selected_agent().is_some_and(|a| a.is_running()));
    let busy = computed(move || !live && store::form().busy);
    let empty = computed(move || text_value.with(|t| t.trim().is_empty()) && store::pending_images().is_empty());
    let height = signal(22.0_f32);

    if MOBILE {
        // One glass card, as Lody's: the text on top, a bar under it with
        // "+" (attachments), the agent's settings as one chip (model,
        // thinking, mode; pressed, their sheets) and the send disc. Plain
        // buttons inside: the glass is the card's alone.
        let settings = computed(move || settings_summary(live));
        return view()
            .w_full()
            .child(queue_track(live))
            .child(image_tray())
            .child(suggest_panel(target, text_value, set_text))
            .child(
                liquid_glass(view())
                    .rounded(px(24.0))
                    .px(px(S3))
                    .pt(px(S3))
                    .pb(px(S2))
                    .gap(px(S2))
                    .child(
                        input()
                            .value(move || text_value.get())
                            .multiline(true)
                            .submit_behavior(SubmitBehavior::Newline)
                            .on_change(move |v: &str| set_text(v))
                            .on_content_size_change(move |size: &Size<Points>| {
                                height.set(size.height.clamp(px(22.0), px(160.0)).0);
                            })
                            .caret_color(|| color(ACCENT))
                            .selection_color(|| color(tint(ACCENT, 0.3)))
                            .h(move || px(height.get() + 2.0))
                            .text_size(px(TEXT_LG))
                            .line_height(px(22.0))
                            .text_color(FG)
                            .px(px(S1)),
                    )
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap(px(S2))
                            .child(
                                nsh_base::button()
                                    // Photos are the one attachment: straight to the picker.
                                    .on_press(|| {
                                        dismiss_keyboard();
                                        pick_images_into_composer();
                                    })
                                    .accessibility_label(t("Attach"))
                                    .w(px(36.0))
                                    .h(px(36.0))
                                    .rounded(px(10.0))
                                    .bg(tint(FG, 0.07))
                                    .items_center()
                                    .justify_center()
                                    .active(|s| s.opacity(0.6))
                                    .child(icon(Icon::Plus, 18.0).text_color(FG)),
                            )
                            .child(
                                nsh_base::button()
                                    .on_press(move || options_sheet(live))
                                    .accessibility_label(t("Agent settings"))
                                    .flex_1()
                                    .min_w(px(0.0))
                                    .h(px(36.0))
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .px(px(S1))
                                    .active(|s| s.opacity(0.6))
                                    .child(icon(Icon::Sparkles, 15.0).text_color(FG_MUTED))
                                    .child(
                                        text(move || settings.get())
                                            .text_size(px(TEXT_BASE))
                                            .text_color(FG_MUTED)
                                            .number_of_lines(1)
                                            .flex_shrink(1.0),
                                    ),
                            )
                            .child(disc(running, busy, empty, submit)),
                    ),
            )
            .into_element();
    }

    view()
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH))
        .self_center()
        .child(queue_track(live))
        .child(image_tray())
        .child(suggest_panel(target, text_value, set_text))
        .child(glass_strong(view())
        .w_full()
        .rounded(px(24.0))
        .child(
            input()
                .value(move || text_value.get())
                .multiline(true)
                .submit_behavior(SubmitBehavior::Submit)
                .on_change(move |v: &str| set_text(v))
                .on_submit(move |_: &str| submit())
                .on_content_size_change(move |size: &Size<Points>| {
                    height.set(size.height.clamp(px(22.0), px(220.0)).0);
                })
                .placeholder(placeholder)
                .caret_color(|| color(ACCENT))
                .selection_color(|| color(tint(ACCENT, 0.3)))
                .h(move || px(height.get() + 4.0))
                .text_size(px(TEXT_CONTENT))
                .line_height(px(22.0))
                .text_color(FG)
                .mx(px(S4))
                .mt(px(14.0))
                .mb(px(S1)),
        )
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(S2))
                .px(px(S2))
                .pb(px(S2))
                .child(
                    // The controls give up width first (their labels shorten);
                    // the send / stop disc keeps its place in the corner.
                    view()
                        .flex_row()
                        .flex_wrap()
                        .items_center()
                        .gap(px(2.0))
                        .flex_1()
                        .min_w(px(0.0))
                .child(
                    dropup(
                        view()
                            .w(px(CONTROL_SM))
                            .h(px(CONTROL_SM))
                            .items_center()
                            .justify_center()
                            .rounded(px(R_FULL))
                            .hover(|s| s.bg(highlight()))
                            .label(t("Attach"))
                            .child(icon(Icon::Paperclip, 16.0).text_color(FG_MUTED)),
                    )
                    .child(menu_row(t("Choose images…"), Some(Icon::Image), None, false, pick_images_into_composer))
                    .child(menu_row(t("Paste image"), Some(Icon::Copy), None, false, paste_image_into_composer)),
                )
                .child(mic_button(text_value, set_text))
                .child(model_picker(live))
                .child(thinking_picker(live))
                .child(mode_picker(live))
                .child(feature_controls(live)),
                )
                .child(view().flex_shrink_0().child(disc(running, busy, empty, submit))),
        ))
        .into_element()
}

/// The agent's settings in a line: model · thinking · mode.
fn settings_summary(live: bool) -> String {
    let c = store::controls(live);
    let provider = store::provider(&c.provider);
    let model = c
        .model
        .as_deref()
        .and_then(|m| provider.as_ref().and_then(|p| p.model(m)).map(|m| m.label.clone()))
        .or_else(|| provider.as_ref().map(|p| p.label.clone()))
        .unwrap_or_default();
    let thinking = thinking_options(live);
    let thinking = c.thinking.as_deref().and_then(|t| thinking.iter().find(|o| o.id == t)).or(thinking.first()).map(|o| o.label.clone());
    let mode = c.mode.as_deref().and_then(|m| c.modes.iter().find(|o| o.id == m)).or(c.modes.first()).map(|o| o.label.clone());
    [Some(model), thinking, mode].into_iter().flatten().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}

/// The agent's settings, as a sheet: each opens its choices.
fn options_sheet(live: bool) {
    let c = store::controls(live);
    let provider = store::provider(&c.provider);
    let model = c
        .model
        .as_deref()
        .and_then(|m| provider.as_ref().and_then(|p| p.model(m)).map(|m| m.label.clone()))
        .or_else(|| provider.as_ref().map(|p| p.label.clone()))
        .unwrap_or_default();
    let mut items = vec![SheetItem::new(tf("Model · {}", &[&model]), move || model_sheet(live)).icon(Icon::Bot)];
    let thinking = thinking_options(live);
    if !thinking.is_empty() {
        let current = c.thinking.as_deref().and_then(|t| thinking.iter().find(|o| o.id == t)).or(thinking.first()).map(|o| o.label.clone()).unwrap_or_default();
        items.push(SheetItem::new(tf("Thinking · {}", &[&current]), move || {
            let c = store::controls(live);
            choice_sheet(t("Thinking"), thinking_options(live), c.thinking, move |id| store::set_control(live, Control::Thinking(id)))
        }).icon(Icon::Brain));
    }
    if !c.modes.is_empty() {
        let current = c.mode.as_deref().and_then(|m| c.modes.iter().find(|o| o.id == m)).or(c.modes.first()).map(|o| o.label.clone()).unwrap_or_default();
        items.push(SheetItem::new(tf("Mode · {}", &[&current]), move || {
            let c = store::controls(live);
            choice_sheet(t("Mode"), c.modes.clone(), c.mode, move |id| store::set_control(live, Control::Mode(id)))
        }).icon(Icon::Layers));
    }
    show_sheet(None, items);
}

/// Options as a sheet, the current one checked.
fn choice_sheet(title: &str, options: Vec<crate::daemon::model::Opt>, current: Option<String>, pick: impl Fn(String) + Clone + 'static) {
    let items = options
        .into_iter()
        .map(|o| {
            let on = current.as_deref() == Some(o.id.as_str());
            let pick = pick.clone();
            let id = o.id.clone();
            SheetItem::new(o.label.clone(), move || pick(id.clone())).checked(on)
        })
        .collect();
    show_sheet(Some(title.to_owned()), items);
}

/// The models of the agent's provider (a new agent: every available
/// provider's), the current one checked.
fn model_sheet(live: bool) {
    let c = store::controls(live);
    let mut items = Vec::new();
    for p in store::providers().into_iter().filter(|p| p.enabled && p.available()) {
        if live && p.id != c.provider {
            continue;
        }
        for m in &p.models {
            let checked = c.provider == p.id && c.model.as_deref() == Some(m.id.as_str());
            let (pid, mid) = (p.id.clone(), m.id.clone());
            let label = if live { m.label.clone() } else { format!("{} · {}", p.label, m.label) };
            items.push(SheetItem::new(label, move || {
                if !live {
                    store::set_control(false, Control::Provider(pid.clone()));
                }
                store::set_control(live, Control::Model(mid.clone()));
            }).checked(checked));
        }
    }
    show_sheet(Some(t("Model").into()), items);
}

/// What an @ mention or a leading / asks for, from the composer text.
#[derive(Clone, PartialEq)]
enum Trigger {
    Slash(String),
    Mention(String),
}

fn trigger_of(text: &str) -> Option<Trigger> {
    if let Some(rest) = text.strip_prefix('/')
        && !rest.contains(char::is_whitespace)
    {
        return Some(Trigger::Slash(rest.to_owned()));
    }
    let last = text.rsplit(char::is_whitespace).next()?;
    last.strip_prefix('@').map(|q| Trigger::Mention(q.to_owned()))
}

/// The slash-command or file list over the composer.
fn suggest_panel(target: Target, text_value: Memo<String>, set_text: impl Fn(&str) + Copy + 'static) -> impl IntoElement {
    let trigger = computed(move || text_value.with(|t| trigger_of(t)));
    view().child(dynamic(move || {
        let trigger = trigger.get()?;
        let rows: Vec<(Icon, String, String, String)> = match &trigger {
            Trigger::Slash(prefix) => {
                let agent = store::selected_id()?;
                store::commands(&agent)
                    .into_iter()
                    .filter(|c| c.name.to_lowercase().starts_with(&prefix.to_lowercase()))
                    .take(8)
                    .map(|c| {
                        let insert = format!("/{} ", c.name);
                        (if c.skill { Icon::Sparkles } else { Icon::Slash }, format!("/{}", c.name), c.description, insert)
                    })
                    .collect()
            }
            Trigger::Mention(query) => {
                let cwd = match target {
                    Target::Agent => store::current_workspace().map(|w| w.directory),
                    Target::Form => Some(store::form().path),
                }?;
                store::suggestions(&cwd, query)
                    .into_iter()
                    .take(8)
                    .map(|(path, dir)| {
                        let rel = path.strip_prefix(&cwd).map(|p| p.trim_start_matches('/').to_owned()).unwrap_or(path);
                        (if dir { Icon::Folder } else { Icon::FileText }, rel.clone(), String::new(), rel)
                    })
                    .collect()
            }
        };
        if rows.is_empty() {
            return None;
        }
        Some(untracked(move || {
            let mut list = card().mb(px(S2)).p(px(S1)).bg(POPOVER).shadow_md();
            for (glyph, label, detail, insert) in rows.clone() {
                let trig = trigger.clone();
                list = list.child(
                    button()
                        .on_press(move || {
                            let text = text_value.get_untracked();
                            let next = match &trig {
                                Trigger::Slash(_) => insert.clone(),
                                Trigger::Mention(q) => {
                                    let cut = text.len() - q.len() - 1;
                                    format!("{}@{} ", &text[..cut], insert)
                                }
                            };
                            set_text(&next);
                        })
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .h(px(CONTROL_MD))
                        .px(px(S2))
                        .rounded(px(R_LG))
                        .hover(|s| s.bg(highlight()))
                        .child(icon(glyph, 15.0).text_color(FG_MUTED))
                        .child(text(label).text_size(px(TEXT_BASE)).font_family(|| MONO).text_color(FG).flex_shrink_0())
                        .child(text(detail).text_size(px(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1).flex_1()),
                );
            }
            list
        }))
    }))
}

/// Messages waiting for the running turn to end.
fn queue_track(live: bool) -> impl IntoElement {
    let queued = computed(move || if live { store::selected_id().map(|id| store::queue(&id)).unwrap_or_default() } else { vec![] });
    view().child(dynamic(move || {
        let list = queued.get();
        if list.is_empty() {
            return None;
        }
        Some(untracked(move || {
            let mut track = card().mb(px(S2)).px(px(S3)).py(px(S2)).gap(px(S1)).child(section_label(t("Queued")));
            for (i, msg) in list.clone().into_iter().enumerate() {
                track = track.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .child(icon(Icon::Inbox, 14.0).text_color(FG_FAINT))
                        .child(text(msg).text_size(px(TEXT_BASE)).number_of_lines(1).flex_1())
                        .child(icon_btn(Icon::X, Sz::Sm, t("Remove from queue"), move || {
                            if let Some(id) = store::selected_id() {
                                store::unqueue(&id, i);
                            }
                        })),
                );
            }
            track
        }))
    }))
}

/// The trailing control: send, or stop while the agent runs.
fn disc(running: Memo<bool>, busy: Memo<bool>, empty: Memo<bool>, submit: impl Fn() + Clone + 'static) -> impl IntoElement {
    view().child(dynamic(move || {
        let stop = running.get() && empty.get();
        let submit = submit.clone();
        Some(untracked(move || {
            if stop {
                button()
                    .on_press(store::stop_selected)
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded(px(R_FULL))
                    .bg(FG)
                    .items_center()
                    .justify_center()
                    .hover(|s| s.opacity(0.85))
                    .child(view().w(px(10.0)).h(px(10.0)).rounded(px(2.0)).bg(SURFACE0))
                    .into_element()
            } else {
                button()
                    .on_press(submit)
                    .disabled(move || empty.get() || busy.get())
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded(px(R_FULL))
                    .bg(move || if empty.get() { SURFACE3 } else { ACCENT })
                    .items_center()
                    .justify_center()
                    .hover(|s| s.opacity(0.88))
                    .child(icon(Icon::ArrowUp, 17.0).text_color(move || if empty.get() { FG_FAINT } else { ACCENT_FG }))
                    .into_element()
            }
        }))
    }))
}

fn model_picker(live: bool) -> impl IntoElement {
    let label = computed(move || {
        let c = store::controls(live);
        let provider = store::provider(&c.provider);
        let p_label = provider.as_ref().map(|p| p.label.clone()).unwrap_or_else(|| c.provider.clone());
        let m_label = c
            .model
            .as_deref()
            .and_then(|m| provider.as_ref().and_then(|p| p.model(m)).map(|m| m.label.clone()).or(Some(m.to_owned())));
        match m_label {
            Some(m) => format!("{p_label} · {m}"),
            None => p_label,
        }
    });
    dropup(picker_trigger(Some(Icon::Bot), move || label.get())).child(model_rows(live, true))
}

/// The models to pick from, as menu rows (in a scroll of their own when
/// `scrolled`).
fn model_rows(live: bool, scrolled: bool) -> impl IntoElement {
    view().child(dynamic(move || {
        let c = store::controls(live);
        let providers = store::providers();
        Some(untracked(move || {
            let mut list = view();
            for p in providers.iter().filter(|p| p.enabled) {
                // A live agent keeps its provider; it lists only its models.
                if live && p.id != c.provider {
                    continue;
                }
                list = list.child(menu_heading(if p.available() {
                    p.label.clone()
                } else {
                    tf("{} · not available", &[&p.label])
                }));
                if p.models.is_empty() {
                    let id = p.id.clone();
                    list = list.child(
                        menu_row(p.label.clone(), None, None, c.provider == p.id, move || {
                            store::set_control(live, Control::Provider(id.clone()))
                        })
                        .disabled(!p.available() || live),
                    );
                    continue;
                }
                for m in &p.models {
                    let (pid, mid) = (p.id.clone(), m.id.clone());
                    let checked = c.provider == p.id && c.model.as_deref() == Some(m.id.as_str());
                    list = list.child(
                        menu_row(m.label.clone(), None, Some(m.description.clone().into()), checked, move || {
                            if !live {
                                store::set_control(false, Control::Provider(pid.clone()));
                            }
                            store::set_control(live, Control::Model(mid.clone()));
                        })
                        .disabled(!p.available()),
                    );
                }
            }
            if scrolled { menu_scroll(420.0, list).into_element() } else { list.into_element() }
        }))
    }))
}

fn thinking_options(live: bool) -> Vec<crate::daemon::model::Opt> {
    let c = store::controls(live);
    store::provider(&c.provider)
        .and_then(|p| c.model.as_deref().and_then(|m| p.model(m).cloned()).or_else(|| p.default_model().cloned()))
        .map(|m| m.thinking)
        .unwrap_or_default()
}

fn thinking_picker(live: bool) -> impl IntoElement {
    let has = computed(move || !thinking_options(live).is_empty());
    let label = computed(move || {
        let c = store::controls(live);
        let opts = thinking_options(live);
        c.thinking
            .as_deref()
            .and_then(|t| opts.iter().find(|o| o.id == t))
            .or(opts.first())
            .map(|o| o.label.clone())
            .unwrap_or_default()
    });
    when(has, move || {
        dropup(picker_trigger(Some(Icon::Brain), move || label.get())).child(thinking_rows(live))
    })
}

fn thinking_rows(live: bool) -> impl IntoElement {
    view().child(dynamic(move || {
            let c = store::controls(live);
            let opts = thinking_options(live);
            if opts.is_empty() {
                return None;
            }
            Some(untracked(move || {
                let mut list = view().child(menu_heading(t("Thinking")));
                for o in &opts {
                    let id = o.id.clone();
                    list = list.child(menu_row(
                        o.label.clone(),
                        None,
                        Some(o.description.clone().into()),
                        c.thinking.as_deref() == Some(o.id.as_str()),
                        move || store::set_control(live, Control::Thinking(id.clone())),
                    ));
                }
                list.into_element()
            }))
    }))
}

fn mode_picker(live: bool) -> impl IntoElement {
    let has = computed(move || !store::controls(live).modes.is_empty());
    let label = computed(move || {
        let c = store::controls(live);
        c.mode
            .as_deref()
            .and_then(|m| c.modes.iter().find(|o| o.id == m))
            .or(c.modes.first())
            .map(|o| o.label.clone())
            .unwrap_or_default()
    });
    when(has, move || {
        dropup(picker_trigger(Some(Icon::Layers), move || label.get())).child(mode_rows(live))
    })
}

fn mode_rows(live: bool) -> impl IntoElement {
    view().child(dynamic(move || {
            let c = store::controls(live);
            if c.modes.is_empty() {
                return None;
            }
            Some(untracked(move || {
                let mut list = view().child(menu_heading(t("Mode")));
                for o in &c.modes {
                    let id = o.id.clone();
                    list = list.child(menu_row(
                        o.label.clone(),
                        None,
                        Some(o.description.clone().into()),
                        c.mode.as_deref() == Some(o.id.as_str()),
                        move || store::set_control(live, Control::Mode(id.clone())),
                    ));
                }
                list.into_element()
            }))
    }))
}

/// A hint line under a composer (errors, where it starts).
pub(crate) fn hint(message: impl Fn() -> Option<(String, bool)> + 'static) -> impl IntoElement {
    let message = computed(message);
    view().w_full().max_w(px(CONTENT_MAX_WIDTH)).self_center().px(px(S2)).child(dynamic(move || {
        let (m, error) = message.get()?;
        Some(untracked(move || {
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon(if error { Icon::CircleAlert } else { Icon::Info }, 14.0).text_color(if error {
                    DANGER
                } else {
                    FG_FAINT
                }))
                .child(
                    text(m.clone())
                        .text_size(px(TEXT_SM))
                        .font_weight(FontWeight(400))
                        .text_color(if error { DANGER } else { FG_MUTED })
                        .selectable(true)
                        .flex_shrink(1.0),
                )
        }))
    }))
}

fn pick_images_into_composer() {
    spawn(async move {
        let mut req = nsh_core::picker::PickRequest::default();
        req.limit = 6;
        req.max_dimension = Some(nsh_core::geometry::DevicePixels(2048));
        let picked = pick_images(req).await;
        use base64::Engine;
        let images = picked
            .into_iter()
            .map(|f| crate::daemon::connection::ImageInput {
                name: f.name.clone(),
                mime: f.mime.clone(),
                data: base64::engine::general_purpose::STANDARD.encode(f.data.bytes()),
            })
            .collect::<Vec<_>>();
        if !images.is_empty() {
            store::add_images(images);
        }
    });
}

/// The images waiting to go with the message.
fn image_tray() -> impl IntoElement {
    let images = computed(|| store::pending_images().into_iter().map(|i| i.name).collect::<Vec<_>>());
    view().child(dynamic(move || {
        let names = images.get();
        if names.is_empty() {
            return None;
        }
        Some(untracked(move || {
            let mut row = view().flex_row().flex_wrap().gap(px(S2)).mb(px(S2));
            for (i, name) in names.clone().into_iter().enumerate() {
                row = row.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .h(px(30.0))
                        .pl(px(S2))
                        .pr(px(2.0))
                        .rounded(px(R_LG))
                        .border_1()
                        .border_color(BORDER)
                        .bg(SURFACE1)
                        .child(icon(Icon::Image, 14.0).text_color(FG_MUTED))
                        .child(text(name).text_size(px(TEXT_SM)).number_of_lines(1).max_w(px(180.0)))
                        .child(icon_btn(Icon::X, Sz::Sm, t("Remove image"), move || store::remove_image(i))),
                );
            }
            row
        }))
    }))
}

/// How full the agent's context window is, and what it cost.
pub(crate) fn context_meter() -> impl IntoElement {
    let usage = computed(|| store::selected_agent().map(|a| (a.context_used, a.context_max, a.cost_usd)));
    view().child(dynamic(move || {
        let (Some(used), Some(max), cost) = usage.get()? else { return None };
        if max == 0 {
            return None;
        }
        let pct = (used as f64 / max as f64 * 100.0).round() as u32;
        let tone = if pct >= 85 { DANGER } else if pct >= 65 { WARNING } else { FG_MUTED };
        Some(untracked(move || {
            let label = match cost {
                Some(c) if c > 0.0 => format!("{pct}% · ${c:.2}"),
                _ => format!("{pct}%"),
            };
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .px(px(S2))
                .child(
                    view().w(px(28.0)).h(px(4.0)).rounded(px(R_FULL)).bg(SURFACE3).child(
                        view().h_full().w(relative((pct.min(100) as f32) / 100.0)).rounded(px(R_FULL)).bg(tone),
                    ),
                )
                .child(text(label).text_size(px(TEXT_XS)).font_family(|| MONO).text_color(tone))
        }))
    }))
}

/// The provider's features: toggles as chips, choices as menus.
fn feature_controls(live: bool) -> impl IntoElement {
    let list = computed(move || store::features(live));
    view().flex_row().items_center().gap(px(2.0)).child(dynamic(move || {
        let features = list.get();
        if features.is_empty() {
            return None;
        }
        Some(untracked(move || {
            let mut row = view().flex_row().items_center().gap(px(2.0));
            for f in features.clone() {
                match f.value.clone() {
                    Ok(on) => {
                        let id = f.id.clone();
                        row = row.child(
                            nsh_base::button()
                                .on_press(move || store::set_feature(live, &id, (!on).into()))
                                .flex_row()
                                .items_center()
                                .gap(px(5.0))
                                .h(px(CONTROL_SM))
                                .px(px(S2))
                                .rounded(px(R_FULL))
                                .bg(if on { tint(ACCENT, 0.12) } else { tint(ACCENT, 0.0) })
                                .hover(|s| s.bg(highlight()))
                                .child(icon(Icon::Sparkles, 14.0).text_color(if on { ACCENT } else { FG_MUTED }))
                                .child(text(f.label.clone()).text_size(px(TEXT_SM)).font_weight(FontWeight(500)).text_color(if on { ACCENT } else { FG_MUTED })),
                        );
                    }
                    Err((value, options)) => {
                        let current = value
                            .as_deref()
                            .and_then(|v| options.iter().find(|o| o.id == v))
                            .map(|o| o.label.clone())
                            .unwrap_or_else(|| f.label.clone());
                        let mut menu = dropup(picker_trigger(Some(Icon::Settings2), current)).child(menu_heading(f.label.clone()));
                        for o in options {
                            let (id, oid) = (f.id.clone(), o.id.clone());
                            menu = menu.child(menu_row(
                                o.label.clone(),
                                None,
                                Some(o.description.clone().into()),
                                value.as_deref() == Some(o.id.as_str()),
                                move || store::set_feature(live, &id, oid.clone().into()),
                            ));
                        }
                        row = row.child(menu);
                    }
                }
            }
            row
        }))
    }))
}

#[derive(Clone, Default)]
struct Dictating(bool);

thread_local! {
    static SPEECH: std::cell::RefCell<Option<std::rc::Rc<std::cell::RefCell<nsh_core::element::SpeechRecognition>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Dictation: speech to text into the composer, with the system recognizer.
fn mic_button(text_value: Memo<String>, set_text: impl Fn(&str) + Copy + 'static) -> impl IntoElement {
    let on = global::<Dictating>();
    nsh_base::button()
        .on_press(move || {
            if on.with_untracked(|d| d.0) {
                SPEECH.with_borrow(|s| {
                    if let Some(s) = s {
                        s.borrow().stop();
                    }
                });
                return;
            }
            let base = text_value.get_untracked();
            let session = std::rc::Rc::new(std::cell::RefCell::new(recognize_speech(nsh_core::speech::SpeechRequest::default())));
            on.update(|d| d.0 = true);
            SPEECH.with_borrow_mut(|s| *s = Some(session.clone()));
            spawn(async move {
                // Borrowed only while polled, so Stop can reach the session.
                while let Some(ev) = std::future::poll_fn(|cx| session.borrow_mut().poll_next(cx)).await {
                    match ev {
                        nsh_core::speech::SpeechEvent::Result { text, .. } => {
                            let joined = if base.trim().is_empty() { text } else { format!("{} {text}", base.trim_end()) };
                            set_text(&joined);
                        }
                        nsh_core::speech::SpeechEvent::End { error } => {
                            if let Some(e) = error {
                                store::notify(tf("Dictation stopped: {}", &[&format!("{e:?}")]));
                            }
                            break;
                        }
                        _ => {}
                    }
                }
                on.update(|d| d.0 = false);
                SPEECH.with_borrow_mut(|s| *s = None);
            });
        })
        .accessibility_label(t("Dictate"))
        .w(px(CONTROL_SM))
        .h(px(CONTROL_SM))
        .items_center()
        .justify_center()
        .rounded(px(R_FULL))
        .bg(move || if on.with(|d| d.0) { tint(DANGER, 0.14) } else { tint(DANGER, 0.0) })
        .hover(|s| s.bg(highlight()))
        .child(dynamic(move || {
            let active = on.with(|d| d.0);
            Some(untracked(move || icon(if active { Icon::MicOff } else { Icon::Mic }, 16.0).text_color(if active { DANGER } else { FG_MUTED })))
        }))
}

/// The image on the clipboard, attached to the message.
fn paste_image_into_composer() {
    spawn(async move {
        let Some(img) = clipboard_image().await else {
            store::notify(t("No image on the clipboard").into());
            return;
        };
        match fetch(img.url.as_str()).await {
            Ok(resp) if resp.ok() => {
                use base64::Engine;
                let mime = resp.header("content-type").unwrap_or("image/png").to_owned();
                store::add_images(vec![crate::daemon::connection::ImageInput {
                    name: t("Pasted image").into(),
                    mime,
                    data: base64::engine::general_purpose::STANDARD.encode(&resp.body),
                }]);
            }
            _ => store::notify(t("Could not read the pasted image").into()),
        }
    });
}
