//! The composer: the input over a toolbar of agent controls
//! (provider and model, thinking, mode) with the send / stop disc in the
//! corner. One composer serves a live agent (`Target::Agent`, controls
//! change the agent) and a new one (`Target::Form`, controls fill the form).

use crate::i18n::{t, tf};
use nsh_base::{
    button,
    rich_editor::view::{EditorSession, PartStyles, RichEditorClasses, RichEditorPastedFiles, RichEditorStyles, rich_editor},
};
use nsh_core::{element::{FontWeight, StyleRefinement}, prelude::*};
use nsh_base::text_view::PartStyle;
use crate::ui::kit::text;

use super::{chips, composer_doc::{self, Chip, Kind}};
use crate::ui::kit::*;
use crate::store::{self, Control};

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Target {
    Agent,
    Form,
}

/// The composer's document as the rest of the composer reads it: which
/// editor (`key`, the agent's or the form's) and a revision bumped on every
/// change of its text or caret, which what derives from it tracks.
#[derive(Clone, Copy)]
struct Doc {
    key: Memo<Option<String>>,
    rev: Signal<u64>,
}

impl Doc {
    /// The editor, untracked.
    fn session(&self) -> Option<EditorSession> {
        self.key.get_untracked().and_then(|k| composer_doc::existing(&k))
    }

    /// The editor, tracking its changes.
    fn tracked(&self) -> Option<EditorSession> {
        self.rev.get();
        self.key.get().and_then(|k| composer_doc::existing(&k))
    }

    fn bump(&self) {
        self.rev.update(|r| *r += 1);
    }
}

pub(crate) fn composer(target: Target, placeholder: &'static str) -> AnyElement {
    let live = target == Target::Agent;
    let key = computed(move || match target {
        Target::Agent => store::selected_id().map(|id| format!("agent:{id}")),
        Target::Form => Some("form".to_owned()),
    });
    let doc = Doc { key, rev: signal(0_u64) };
    // The draft as text: the agent's kept draft, or the form's prompt. The
    // editor writes it on every change; a change from elsewhere (the form
    // cleared once its agent starts) replaces the editor's text.
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
    effect(move || {
        let want = text_value.get();
        let Some(key) = key.get() else { return };
        untracked(|| {
            if let Some(session) = composer_doc::existing(&key)
                && composer_doc::outgoing(session.state().doc()).text != want
            {
                composer_doc::set_text(&session, &want);
            }
        });
    });
    // A chip whose attachment went (sent, or its upload failed) goes too.
    effect(move || {
        let known = store::known_attachments();
        let Some(key) = key.get() else { return };
        untracked(|| {
            if let Some(session) = composer_doc::existing(&key) {
                composer_doc::remove_chips(&session, |id| !known.contains(id));
            }
            chips::forget_images_except(&known);
        });
    });
    let changed = move || {
        doc.bump();
        let Some(session) = doc.session() else { return };
        let out = composer_doc::outgoing(session.state().doc());
        if text_value.get_untracked() != out.text {
            set_text(&out.text);
        }
        store::sync_attachments(&out.attachments);
    };
    let submit = move || {
        let Some(session) = doc.session() else { return };
        let out = composer_doc::outgoing(session.state().doc());
        store::sync_attachments(&out.attachments);
        match target {
            Target::Agent => {
                if let Some(id) = store::selected_id()
                    && store::send_message(&out.message)
                {
                    store::set_draft(&id, "");
                    // A queued message leaves its attachments for the one
                    // sent next; their chips stay.
                    composer_doc::keep_only_chips(&session, &store::pending_attachment_ids());
                }
            }
            Target::Form => {
                // As it is sent: its chips named where they stand.
                set_text(&out.message);
                store::submit_form();
            }
        }
    };
    composer_doc::register_submit(if live { "agent" } else { "form" }, submit);
    let running = computed(move || live && store::selected_agent().is_some_and(|a| a.is_running()));
    let busy = computed(move || !live && store::form().busy);
    let empty = computed(move || doc.tracked().is_none_or(|s| composer_doc::outgoing(s.state().doc()).is_empty()));

    if MOBILE {
        // One glass card, as Lody's: the text on top, a bar under it with
        // "+" (attachments), the agent's settings as one chip (model,
        // thinking, mode; pressed, their sheets) and the send disc. Plain
        // buttons inside: the glass is the card's alone.
        let settings = computed(move || settings_summary(live));
        return view()
            .w_full()
            .child(queue_track(live))
            .child(suggest_panel(target, doc))
            .child(chips::preview_layer(key))
            .child(
                liquid_glass(view())
                    .rounded(px(24.0))
                    .px(sp(S3))
                    .pt(sp(S3))
                    .pb(sp(S2))
                    .gap(sp(S2))
                    .child(editor(doc, text_value, placeholder, TEXT_LG, 160.0, changed).px(sp(S1)))
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap(sp(S2))
                            .child(
                                nsh_base::button()
                                    .on_press(move || {
                                        let Some(key) = key.get_untracked() else { return };
                                        dismiss_keyboard();
                                        let (camera, images, files) = (key.clone(), key.clone(), key);
                                        let mut items = vec![
                                            SheetItem::new(t("Photo Library"), move || pick_images_into(images.clone())).icon(Icon::Image),
                                            SheetItem::new(t("Choose Files"), move || pick_files_into(files.clone())).icon(Icon::Folder),
                                        ];
                                        // First, where there is a camera.
                                        if camera_available() {
                                            items.insert(0, SheetItem::new(t("Take Photo"), move || take_photo_into(camera.clone())).icon(Icon::Camera));
                                        }
                                        show_sheet(None, items);
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
                                    .px(sp(S1))
                                    .active(|s| s.opacity(0.6))
                                    .child(icon(Icon::Sparkles, 15.0).text_color(FG_MUTED))
                                    .child(
                                        text(move || settings.get())
                                            .text_size(tx(TEXT_BASE))
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

    let with_key = move |f: fn(String)| move || {
        if let Some(k) = key.get_untracked() {
            f(k)
        }
    };
    // Files dragged from Finder attach wherever in the window they are let
    // go (`dropped`, on the window's root): the card says so while they
    // are over the window.
    let over = computed(|| global::<Dropping>().with(|d| d.0));
    view()
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH))
        .self_center()
        .child(queue_track(live))
        .child(suggest_panel(target, doc))
        .child(chips::preview_layer(key))
        .child(glass_strong(view())
        .w_full()
        .rounded(px(24.0))
        .border_color(move || if over.get() { ACCENT.into() } else { glass_edge() })
        .child(editor(doc, text_value, placeholder, TEXT_CONTENT, 220.0, changed).mx(sp(S4)).mt(px(14.0)).mb(sp(S1)))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .px(sp(S2))
                .pb(sp(S2))
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
                            .w(sp(CONTROL_SM))
                            .h(sp(CONTROL_SM))
                            .items_center()
                            .justify_center()
                            .rounded(px(R_FULL))
                            .hover(|s| s.bg(highlight()))
                            .label(t("Attach"))
                            .child(icon(Icon::Paperclip, 16.0).text_color(FG_MUTED)),
                    )
                    .child(menu_row(t("Choose images…"), Some(Icon::Image), None, false, with_key(pick_images_into)))
                    .child(menu_row(t("Choose files…"), Some(Icon::File), None, false, with_key(pick_files_into)))

                )
                .child(mic_button(doc))
                .child(model_picker(live)),
                )
                .child(view().flex_shrink_0().child(disc(running, busy, empty, submit))),
        ))
        .into_element()
}

/// The text field: the composer's `RichEditor`, made again for each
/// editor (`key`), growing with its text up to `max_height`, then
/// scrolling.
fn editor(
    doc: Doc,
    text_value: Memo<String>,
    placeholder: &'static str,
    size: f32,
    max_height: f32,
    changed: impl Fn() + Copy + 'static,
) -> View {
    let key = doc.key;
    let mut styles = RichEditorStyles::default();
    let typography = |s: StyleRefinement| s.text_size(px(size)).line_height(px(22.0)).text_color(FG);
    styles.root = Some(PartStyle(typography(StyleRefinement::default()).into()));
    styles.caret = Some(PartStyle(StyleRefinement::default().bg(ACCENT).into()));
    styles.selection = Some(PartStyle(StyleRefinement::default().bg(tint(ACCENT, 0.3)).into()));
    styles.placeholder = Some(PartStyle(StyleRefinement::default().text_color(FG_FAINT).into()));
    let mut nodes = PartStyles::default();
    nodes.0.insert("paragraph".into(), typography(StyleRefinement::default()).into());
    // The editor wraps each chip in a box of its node's part: that box is
    // the inline attachment, so its alignment centres the chip on the
    // line's x height (CSS's `vertical-align: middle`) instead of standing
    // it on the baseline. Chips are no taller than the line, so lines with
    // and without them are the same height.
    let mut classes = RichEditorClasses::default();
    classes.nodes = Some(
        [composer_doc::ATTACHMENT, composer_doc::MENTION].into_iter().map(|chip| (chip.to_owned(), "align-middle".to_owned())).collect(),
    );
    styles.nodes = Some(nodes);
    view().test_id("composer").child(dynamic(move || {
        let key = key.get()?;
        let styles = styles.clone();
        let classes = classes.clone();
        Some(untracked(move || {
            let session = composer_doc::session(&key, || text_value.get_untracked());
            let mut el = rich_editor()
                .editor(session)
                .styles(styles)
                .classes(classes.clone())
                .accessibility_label(SharedString::from(t("Message")))
                .on_change(move |_: &_| changed())
                .on_selection_change(move |_: &_| doc.bump())
                .on_paste_files({
                    let key = key.clone();
                    move |p: &RichEditorPastedFiles| attach_pasted(&key, p)
                })
                .w_full()
                .flex_row()
                .min_h(px(22.0))
                .max_h(px(max_height));
            if !placeholder.is_empty() {
                el = el.placeholder(SharedString::from(placeholder));
            }
            el
        }))
    }))
}
/// The agent's settings in a line: model · thinking · mode.
fn settings_summary(live: bool) -> String {
    let c = store::controls(live);
    let provider = store::provider(&c.provider);
    let model = c
        .model
        .as_deref()
        .and_then(|m| provider.as_ref().and_then(|p| p.model(m)).map(|m| m.label.clone()).or(Some(model_name(m))))
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
        .and_then(|m| provider.as_ref().and_then(|p| p.model(m)).map(|m| m.label.clone()).or(Some(model_name(m))))
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
        for m in p.models.iter().filter(|m| !store::forbidden_model(&m.id, &m.label)) {
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

/// The trigger in `before` (the text before the caret; chips and line
/// breaks read as U+FFFC): a `/command` the text starts with, or the
/// `@query` the caret ends.
fn trigger_of(before: &str) -> Option<Trigger> {
    let gap = |c: char| c.is_whitespace() || c == '\u{FFFC}';
    if let Some(rest) = before.strip_prefix('/')
        && !rest.contains(gap)
    {
        return Some(Trigger::Slash(rest.to_owned()));
    }
    let last = before.rsplit(gap).next()?;
    last.strip_prefix('@').map(|q| Trigger::Mention(q.to_owned()))
}

/// The trigger at the editor's caret, and the caret's position.
fn trigger_at(session: &EditorSession) -> Option<(Trigger, usize)> {
    let (before, head) = composer_doc::before_caret(&session.state())?;
    Some((trigger_of(&before)?, head))
}

/// The slash-command or file list over the composer.
fn suggest_panel(target: Target, doc: Doc) -> impl IntoElement {
    let trigger = computed(move || doc.tracked().and_then(|s| trigger_at(&s)).map(|(t, _)| t));
    view().child(dynamic(move || {
        let trigger = trigger.get()?;
        let rows: Vec<(Icon, String, String, String, bool)> = match &trigger {
            Trigger::Slash(prefix) => {
                let agent = store::selected_id()?;
                store::commands(&agent)
                    .into_iter()
                    .filter(|c| c.name.to_lowercase().starts_with(&prefix.to_lowercase()))
                    .take(8)
                    .map(|c| {
                        let insert = format!("/{} ", c.name);
                        (if c.skill { Icon::Sparkles } else { Icon::Slash }, format!("/{}", c.name), c.description, insert, false)
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
                        (if dir { Icon::Folder } else { Icon::FileText }, rel.clone(), String::new(), rel, dir)
                    })
                    .collect()
            }
        };
        if rows.is_empty() {
            return None;
        }
        Some(untracked(move || {
            let mut list = card().mb(sp(S2)).p(sp(S1)).bg(POPOVER).shadow_md();
            for (glyph, label, detail, insert, dir) in rows.clone() {
                list = list.child(
                    button()
                        .on_press(move || {
                            // Where the caret is now: the list may be a
                            // keystroke behind it.
                            let Some(session) = doc.session() else { return };
                            let Some((trigger, head)) = trigger_at(&session) else { return };
                            match trigger {
                                // A command replaces what was typed.
                                Trigger::Slash(_) => composer_doc::replace_text(&session, 1, head, &insert),
                                // A path becomes a chip in place of its query.
                                Trigger::Mention(q) => {
                                    let from = head - q.encode_utf16().count() - 1;
                                    composer_doc::insert_mention(&session, from, head, &insert, dir);
                                }
                            }
                            session.focus();
                        })
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .h(sp(CONTROL_MD))
                        .px(sp(S2))
                        .rounded(px(R_LG))
                        .hover(|s| s.bg(highlight()))
                        .child(icon(glyph, 15.0).text_color(FG_MUTED))
                        .child(text(label).text_size(tx(TEXT_BASE)).font_family(mono_font).text_color(FG).flex_shrink_0())
                        .child(text(detail).text_size(tx(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1).flex_1()),
                );
            }
            list
        }))
    }))
}

/// Messages waiting for the running turn to end.
fn queue_track(live: bool) -> impl IntoElement {
    let queued = computed(move || if live { store::selected_id().map(|id| store::queue(&id)).unwrap_or_default() } else { vec![] });
    let held = computed(move || live && store::selected_id().is_some_and(|id| store::queue_held(&id)));
    view().child(dynamic(move || {
        let list = queued.get();
        held.get();
        if list.is_empty() {
            return None;
        }
        Some(untracked(move || {
            // Waiting after an Escape: said, with the key that takes them back.
            let held = store::selected_id().is_some_and(|id| store::queue_held(&id));
            let mut track = card().mb(sp(S2)).px(sp(S3)).py(sp(S2)).gap(sp(S1)).child(section_label(if held {
                t("Queued · paused — Esc to edit, send to continue")
            } else {
                t("Queued")
            }));
            for (i, msg) in list.clone().into_iter().enumerate() {
                track = track.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(icon(Icon::Inbox, 14.0).text_color(FG_FAINT))
                        .child(text(msg).text_size(tx(TEXT_BASE)).number_of_lines(1).flex_1())
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

/// A model the provider does not list (a default the daemon keeps, from
/// another Claude Code version) by a short name: `claude-opus-5-5` as
/// `Opus 5.5`; any other id as it is.
fn model_name(id: &str) -> String {
    let Some(rest) = id.strip_prefix("claude-") else { return id.to_owned() };
    let mut parts = rest.split('-');
    let Some(family) = parts.next().filter(|f| !f.is_empty() && f.chars().all(|c| c.is_ascii_alphabetic())) else {
        return id.to_owned();
    };
    let version: Vec<&str> = parts.take_while(|p| p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit())).collect();
    if version.is_empty() {
        return id.to_owned();
    }
    let mut name = family.to_owned();
    name[..1].make_ascii_uppercase();
    format!("{name} {}", version.join("."))
}

/// The model picker ([`super::model_picker`]), showing the model's name.
fn model_picker(live: bool) -> impl IntoElement {
    let name = computed(move || {
        let c = store::controls(live);
        let provider = store::provider(&c.provider);
        c.model
            .as_deref()
            .map(|m| provider.as_ref().and_then(|p| p.model(m)).map(|m| m.label.clone()).unwrap_or_else(|| model_name(m)))
            .or_else(|| provider.as_ref().map(|p| p.label.clone()))
            .unwrap_or_default()
    });
    super::model_picker::model_picker(live, name)
}

pub(crate) fn thinking_options(live: bool) -> Vec<crate::daemon::model::Opt> {
    let c = store::controls(live);
    // The agent's own list first: its levels for this model.
    let local = c.model.as_deref().and_then(|m| store::local_models(&c.provider)?.into_iter().find(|x| x.id == m));
    if let Some(local) = local.filter(|l| !l.thinking.is_empty()) {
        return local
            .thinking
            .into_iter()
            .map(|(id, label)| crate::daemon::model::Opt { id, label, description: String::new() })
            .collect();
    }
    store::provider(&c.provider)
        .and_then(|p| c.model.as_deref().and_then(|m| p.model(m).cloned()).or_else(|| p.default_model().cloned()))
        .map(|m| m.thinking)
        .unwrap_or_default()
}

/// A hint line under a composer (errors, where it starts).
pub(crate) fn hint(message: impl Fn() -> Option<(String, bool)> + 'static) -> impl IntoElement {
    let message = computed(message);
    view().w_full().max_w(px(CONTENT_MAX_WIDTH)).self_center().px(sp(S2)).child(dynamic(move || {
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
                        .text_size(tx(TEXT_SM))
                        .font_weight(FontWeight(400))
                        .text_color(if error { DANGER } else { FG_MUTED })
                        .selectable(true)
                        .flex_shrink(1.0),
                )
        }))
    }))
}

/// Adds `images` to the message and their chips to composer `key`'s text,
/// at the caret.
fn insert_images(key: &str, images: Vec<crate::daemon::connection::ImageInput>) {
    let chips: Vec<Chip> = images
        .into_iter()
        .map(|img| {
            let (name, mime) = (img.name.clone(), img.mime.clone());
            Chip { id: store::add_image(img), kind: Kind::Image, name, mime }
        })
        .collect();
    insert_into(key, &chips);
}

/// Starts uploading a file and puts its chip in composer `key`'s text.
fn insert_file(key: &str, name: String, mime: String, bytes: Vec<u8>) {
    if let Some(id) = store::attach_file(name.clone(), mime.clone(), bytes) {
        insert_into(key, &[Chip { id, kind: Kind::File, name, mime }]);
    }
}

fn insert_into(key: &str, chips: &[Chip]) {
    if let Some(session) = composer_doc::existing(key) {
        composer_doc::insert_chips(&session, chips);
        if !MOBILE {
            session.focus();
        }
    }
}

fn pick_images_into(key: String) {
    store::spawn_detached(async move {
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
        insert_images(&key, images);
    });
}

/// Whether the device has a camera to take a photo with now.
fn camera_available() -> bool {
    #[cfg(target_os = "ios")]
    return crate::platform::ios_camera::available();
    #[cfg(not(target_os = "ios"))]
    false
}

/// A photo taken now with the phone's camera, attached as an image.
#[cfg_attr(not(target_os = "ios"), allow(unused_variables))]
fn take_photo_into(key: String) {
    // Once the sheet that offered it has gone: the camera's screen cannot
    // come up over one that is leaving.
    #[cfg(target_os = "ios")]
    set_timeout(std::time::Duration::from_millis(350), move || {
        // The camera's screen is UIKit's, on the main thread; the photo
        // comes back to this one (the engine's), where the composer is.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Option<Vec<u8>>>();
        crate::platform::ios_camera::take_photo(move |photo| {
            let _ = tx.send(photo);
        });
        store::spawn_detached(async move {
            use base64::Engine;
            let Some(Some(bytes)) = rx.recv().await else { return };
            let at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            insert_images(&key, vec![crate::daemon::connection::ImageInput {
                name: format!("photo-{at}.jpg"),
                mime: "image/jpeg".to_owned(),
                data: base64::engine::general_purpose::STANDARD.encode(&bytes),
            }]);
        });
    });
}

/// Files from the device: uploaded to the host, then sent as attachments.
fn pick_files_into(key: String) {
    store::spawn_detached(async move {
        let picked = pick_files(nsh_core::picker::FilePickRequest::new(true, 10)).await;
        for f in picked {
            insert_file(&key, f.name.clone(), f.mime.clone(), f.data.bytes().to_vec());
        }
    });
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
                .px(sp(S2))
                .child(
                    view().w(px(28.0)).h(px(4.0)).rounded(px(R_FULL)).bg(SURFACE3).child(
                        view().h_full().w(relative((pct.min(100) as f32) / 100.0)).rounded(px(R_FULL)).bg(tone),
                    ),
                )
                .child(text(label).text_size(tx(TEXT_XS)).font_family(mono_font).text_color(tone))
        }))
    }))
}

#[derive(Clone, Default)]
struct Dictating(bool);

thread_local! {
    static SPEECH: std::cell::RefCell<Option<std::rc::Rc<std::cell::RefCell<nsh_core::element::SpeechRecognition>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Dictation: speech to text into the composer at the caret, with the
/// system recognizer. Each result is the whole transcript so far, so it
/// replaces what the previous one inserted.
fn mic_button(doc: Doc) -> impl IntoElement {
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
            let Some(session) = doc.session() else { return };
            let state = session.state();
            let at = state.selection().from();
            // A space from the text before, unless at a line's start.
            let spaced = at > 1
                && state.doc().text_between(at - 1, at, Some("\n"), Some(composer_doc::OBJECT)).chars().all(|c| !c.is_whitespace());
            let inserted = std::rc::Rc::new(std::cell::Cell::new(0usize));
            let speech = std::rc::Rc::new(std::cell::RefCell::new(recognize_speech(nsh_core::speech::SpeechRequest::default())));
            on.update(|d| d.0 = true);
            SPEECH.with_borrow_mut(|s| *s = Some(speech.clone()));
            spawn(async move {
                // Borrowed only while polled, so Stop can reach the session.
                while let Some(ev) = std::future::poll_fn(|cx| speech.borrow_mut().poll_next(cx)).await {
                    match ev {
                        nsh_core::speech::SpeechEvent::Result { text, .. } => {
                            let text = if spaced { format!(" {text}") } else { text };
                            composer_doc::replace_text(&session, at, at + inserted.get(), &text);
                            inserted.set(text.encode_utf16().count());
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
        .w(sp(CONTROL_SM))
        .h(sp(CONTROL_SM))
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

/// Cmd+V / Ctrl+V of files: chips at the caret: images (a screenshot) as images, other files uploaded to the
/// host, and files copied in Finder read and uploaded too (the host may be
/// another computer). Text pastes into the editor itself.
fn attach_pasted(key: &str, e: &RichEditorPastedFiles) {
    use base64::Engine;
    let mut images = vec![];
    for f in &e.files {
        if f.mime.starts_with("image/") {
            images.push(crate::daemon::connection::ImageInput {
                name: f.name.clone(),
                mime: f.mime.clone(),
                data: base64::engine::general_purpose::STANDARD.encode(&f.data),
            });
        } else {
            insert_file(key, f.name.clone(), f.mime.clone(), f.data.clone());
        }
    }
    insert_images(key, images);
    attach_uris(key, &e.uris, true);
}

/// Files are being dragged over the window.
#[derive(Clone, Default, PartialEq)]
struct Dropping(bool);

/// Files are over the window (`true`), or left it.
pub(crate) fn dragging_over(over: bool) {
    global::<Dropping>().set(Dropping(over));
}

/// Files let go anywhere in the window: attached to the composer on screen
/// (the open conversation's, or the new conversation's).
pub(crate) fn dropped(uris: &[String]) {
    dragging_over(false);
    let key = match (store::screen(), store::tab()) {
        (store::Screen::Workspace(_), store::Tab::Agent(id)) => Some(format!("agent:{id}")),
        (store::Screen::Workspace(_), store::Tab::Draft) | (store::Screen::NewWorkspace, _) => Some("form".to_owned()),
        _ => None,
    };
    match key.filter(|k| composer_doc::existing(k).is_some()) {
        Some(key) => attach_uris(&key, uris, false),
        None => store::notify(t("Open a conversation to attach files to").to_owned()),
    }
}

/// Files by their `file:` addresses, pasted (copied in Finder) or dropped
/// on the composer: read here and attached, images as images, the rest
/// uploaded to the host (which may be another computer). Folders are left
/// out. `pasted`: one that cannot be read may be on the clipboard itself.
#[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
fn attach_uris(key: &str, uris: &[String], pasted: bool) {
    use base64::Engine;
    #[cfg(not(target_arch = "wasm32"))]
    for uri in uris {
        let Some(path) = uri.strip_prefix("file://") else { continue };
        let path = std::path::PathBuf::from(percent_decode(path));
        if path.is_dir() {
            continue;
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            // A file inside another app's own folder (a picture copied in
            // WeChat) is not the app's to read: the picture itself is on
            // the clipboard beside its address.
            Err(_) => match pasted.then(clipboard_image).flatten() {
                Some((mime, data)) => {
                    insert_images(key, vec![crate::daemon::connection::ImageInput {
                        name,
                        mime: mime.to_owned(),
                        data: base64::engine::general_purpose::STANDARD.encode(&data),
                    }]);
                    continue;
                }
                None => {
                    store::notify(if pasted { t("Could not read the pasted file") } else { t("Could not read the dropped file") }.to_owned());
                    continue;
                }
            },
        };
        let mime = nsh_core::picker::mime_for_file(&name, &bytes).to_owned();
        if mime.starts_with("image/") {
            insert_images(key, vec![crate::daemon::connection::ImageInput {
                name,
                mime,
                data: base64::engine::general_purpose::STANDARD.encode(&bytes),
            }]);
        } else {
            insert_file(key, name, mime, bytes);
        }
    }
}

/// The image on the clipboard, as the JPEG or PNG the copying app offers.
#[cfg(target_os = "macos")]
fn clipboard_image() -> Option<(&'static str, Vec<u8>)> {
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;
    let board = NSPasteboard::generalPasteboard();
    [("image/jpeg", "public.jpeg"), ("image/png", "public.png")]
        .into_iter()
        .find_map(|(mime, kind)| board.dataForType(&NSString::from_str(kind)).map(|data| (mime, data.to_vec())))
}

#[cfg(not(any(target_os = "macos", target_arch = "wasm32")))]
fn clipboard_image() -> Option<(&'static str, Vec<u8>)> {
    None
}

/// A `file:` URL's path with its `%XX` escapes decoded.
#[cfg(not(target_arch = "wasm32"))]
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    #[test]
    fn decodes_file_url_paths() {
        assert_eq!(super::percent_decode("/Users/a/My%20Shot%E5%9B%BE.png"), "/Users/a/My Shot图.png");
        assert_eq!(super::percent_decode("/tmp/100%"), "/tmp/100%");
    }
}

#[cfg(test)]
mod model_name_tests {
    #[test]
    fn a_claude_id_reads_as_its_family_and_version() {
        assert_eq!(super::model_name("claude-opus-5-5"), "Opus 5.5");
        assert_eq!(super::model_name("claude-haiku-4-5-20251001"), "Haiku 4.5");
        assert_eq!(super::model_name("claude-sonnet-5"), "Sonnet 5");
        assert_eq!(super::model_name("gpt-6-sol"), "gpt-6-sol");
    }
}
