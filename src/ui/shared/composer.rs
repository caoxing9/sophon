//! The composer: the input over a toolbar of agent controls
//! (provider and model, thinking, mode) with the send / stop disc in the
//! corner. One composer serves a live agent (`Target::Agent`, controls
//! change the agent) and a new one (`Target::Form`, controls fill the form).

use crate::i18n::{t, tf};
use nsh_base::{
    button,
    rich_editor::view::{EditorSession, PartStyles, RichEditorClasses, RichEditorPastedFiles, RichEditorStyles, rich_editor},
};
use nsh_core::{element::{FontWeight, Interactive as _, StyleRefinement}, prelude::*};
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
    // What the agent guesses comes next (Claude Code's prompt suggestion),
    // while it is not working: the empty composer offers it, Tab takes it.
    let suggestion = computed(move || {
        if !live {
            return None;
        }
        store::selected_agent().filter(|a| !a.is_running()).and_then(|a| a.prompt_suggestion)
    });
    effect(move || {
        let offered = suggestion.get();
        if let Some(key) = key.get() {
            composer_doc::set_suggestion(&key, offered);
        }
    });
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
        let mut out = composer_doc::outgoing(session.state().doc());
        // The conversations mentioned, their words after the message.
        for id in &out.conversations {
            if let Some(context) = store::conversation_context(id) {
                out.message.push_str("\n\n");
                out.message.push_str(&context);
            }
        }
        store::sync_attachments(&out.attachments);
        // One of Claude Code's interactive commands: Sophon's to do, not
        // the headless agent's (`slash`).
        if let Some(native) = crate::slash::native(&out.message) {
            set_text("");
            run_native(native, live);
            return;
        }
        match target {
            Target::Agent => {
                // The notes on replies go ahead of the words.
                let notes = store::selected_id().map(|id| untracked(|| store::notes(&id))).unwrap_or_default();
                let message = crate::notes::with_notes(&notes, &out.message);
                if let Some(id) = store::selected_id()
                    && store::send_message(&message)
                {
                    store::clear_sent_notes(&id);
                    store::set_draft(&id, "");
                    // Its attachments went with it (sent, or queued with
                    // it): their chips go from the composer.
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
            .child(elsewhere_bar(live))
            .child(queue_track(live))
            .child(suggest_panel(target, doc))
            .child(chips::preview_layer(key))
            .child(
                liquid_glass(view())
                    .rounded(px(24.0))
                    .px(sp(S3))
                    .pt(sp(S3))
                    .pb(sp(S2))
                    .gap(sp(S3))
                    // The text takes the card's upper part whole, a line
                    // and a half at least: a tap a little under the line
                    // lands in it, not on the bar.
                    .child(
                        view()
                            .child(editor(doc, text_value, placeholder, suggestion, TEXT_LG, 160.0, changed).px(sp(S1)).min_h(px(34.0)))
                            .child(hold_to_talk(doc, empty)),
                    )
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
                                        let (camera, images, files, pasted) = (key.clone(), key.clone(), key.clone(), key);
                                        let mut items = vec![
                                            SheetItem::new(t("Paste"), move || paste_into(pasted.clone())).icon(Icon::ClipboardPaste),
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
                                    .bg(tint(FG(), 0.07))
                                    .items_center()
                                    .justify_center()
                                    .active(|s| s.opacity(0.6))
                                    .child(icon(Icon::Plus, 18.0).text_color(FG())),
                            )
                            .child(
                                nsh_base::button()
                                    .on_press(move || options_sheet(live))
                                    .accessibility_label(t("Agent settings"))
                                    // As wide as what it says, no wider: the
                                    // bar's width under the text was the
                                    // chip's, and a tap meant for the text
                                    // opened the agent's settings.
                                    .flex_shrink(1.0)
                                    .min_w(px(0.0))
                                    .h(px(36.0))
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .px(sp(S1))
                                    .active(|s| s.opacity(0.6))
                                    .child(icon(Icon::Sparkles, 15.0).text_color(FG_MUTED()))
                                    .child(
                                        text(move || settings.get())
                                            .text_size(tx(TEXT_BASE))
                                            .text_color(FG_MUTED())
                                            .number_of_lines(1)
                                            .flex_shrink(1.0),
                                    ),
                            )
                            .child(view().flex_1())
                            // The context, beside Send (a press opens the
                            // usage page; the phone has no pointer to rest).
                            .child(when(move || live, context_meter_ring))
                            .child(disc(running, busy, empty, submit)),
                    )
                    // While it listens, the whole card is the listening:
                    // over the rest, so last.
                    .child(listening_card(doc)),
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
    // No send button: Return sends. Stop shows while the agent runs and
    // nothing is typed (typed, Return steers, interrupts or queues).
    let stoppable = computed(move || running.get() && empty.get());
    let _ = (busy, submit);
    // One line, as Codex's: attach, the text, dictation (and stop). The
    // agent's settings and its context under the card, at the right.
    view()
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH()))
        .self_center()
        .child(elsewhere_bar(live))
        .child(queue_track(live))
        .child(suggest_panel(target, doc))
        .child(chips::preview_layer(key))
        .child(glass_strong(view())
        .w_full()
        // A press anywhere on the card that nothing else takes is a press
        // on the input: the text was a strip in the middle of what looks
        // like one field.
        .test_id("composer-card")
        .on_press(move || {
            if let Some(session) = doc.session() {
                session.focus();
            }
        })
        .rounded(px(22.0))
        .border_color(move || if over.get() { ACCENT().into() } else { glass_edge() })
        .px(sp(S2))
        .py(sp(S2))
        // The notes on replies waiting for this message, over its text.
        .child(super::notes::composer_notes(live))
        .child(view().w_full().flex_row().items_end().gap(sp(S1))
        .child(
            nsh_base::button()
                .on_press(with_key(pick_any_into))
                .accessibility_label(t("Attach"))
                .w(sp(CONTROL_SM))
                .h(sp(CONTROL_SM))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(R_FULL))
                .hover(|s| s.bg(highlight()))
                .child(icon(Icon::Plus, 18.0).text_color(FG_MUTED())),
        )
        .child(view().flex_1().min_w(px(0.0)).min_h(sp(CONTROL_SM)).justify_center().child(editor(doc, text_value, placeholder, suggestion, TEXT_BASE, 220.0, changed).mx(sp(S1))))
        .child(view().flex_shrink_0().child(mic_button(doc)))
        .child(when(stoppable, || {
            button()
                .on_press(store::stop_selected)
                .accessibility_label(t("Stop"))
                .w(sp(CONTROL_SM))
                .h(sp(CONTROL_SM))
                .flex_shrink_0()
                .rounded(px(R_FULL))
                .bg(FG())
                .items_center()
                .justify_center()
                .hover(|s| s.opacity(0.85))
                .child(view().w(px(9.0)).h(px(9.0)).rounded(px(2.0)).bg(SURFACE0()))
        }))))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_end()
                .gap(px(2.0))
                .pt(sp(S1))
                .px(sp(S2))
                // Where the agent works, at the left.
                .when(live, |v| v.child(super::agent::context_bar()))
                .child(model_picker(live))
                // What the agent may do without asking, beside its model.
                .child(super::model_picker::access_mode(live))
                .child(when(move || live, context_meter_ring)),
        )
        .into_element()
}

/// The text field: the composer's `RichEditor`, made again for each
/// editor (`key`), growing with its text up to `max_height`, then
/// scrolling.
fn editor(
    doc: Doc,
    text_value: Memo<String>,
    placeholder: &'static str,
    suggestion: Memo<Option<String>>,
    size: f32,
    max_height: f32,
    changed: impl Fn() + Copy + 'static,
) -> View {
    let key = doc.key;
    let mut styles = RichEditorStyles::default();
    // At the size the user set (`tx`), the line with it.
    let typography = |s: StyleRefinement| s.text_size(tx(size)).line_height(tx(22.0)).text_color(FG());
    styles.root = Some(PartStyle(typography(StyleRefinement::default()).into()));
    styles.caret = Some(PartStyle(StyleRefinement::default().bg(ACCENT()).into()));
    styles.selection = Some(PartStyle(StyleRefinement::default().bg(tint(ACCENT(), 0.3)).into()));
    styles.placeholder = Some(PartStyle(StyleRefinement::default().text_color(FG_FAINT()).into()));
    // What an input method is composing (pinyin before it becomes
    // characters): a light accent wash behind it, as the system's own
    // fields mark it, not the editor's underline under each piece.
    styles.composition = Some(PartStyle(StyleRefinement::default().no_underline().bg(tint(ACCENT(), 0.14)).into()));
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
            let el = rich_editor()
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
                .max_h(px(max_height))
                // Clear of the guess's Tab key at the end of the line.
                .pr(move || Length::from(px(if suggestion.with(Option::is_some) { 44.0 } else { 0.0 })));
            // The agent's guess at the next message stands where the
            // placeholder does.
            el.placeholder(move || SharedString::from(suggestion.get().unwrap_or_else(|| placeholder.to_owned())))
        }))
    }))
    // The guess's key at the end of the line while it shows: pressed (the
    // phone has no Tab), it is taken as Tab takes it.
    .child(when(
        move || suggestion.with(Option::is_some) && text_value.with(|t| t.trim().is_empty()),
        move || {
            nsh_base::button()
                .on_press(move || {
                    if let Some(key) = key.get_untracked() {
                        composer_doc::take_suggestion(&key);
                    }
                })
                .accessibility_label(t("Use the suggestion"))
                .absolute()
                .top(px(0.0))
                .right(px(0.0))
                .h(px(22.0))
                .px(px(6.0))
                .rounded(px(R_SM))
                .items_center()
                .justify_center()
                .bg(tint(FG(), 0.06))
                .hover(|s| s.bg(tint(FG(), 0.1)))
                .child(text(if MOBILE { t("Use") } else { "Tab" }).text_size(tx(TEXT_XS)).font_weight(FontWeight(500)).text_color(FG_MUTED()))
        },
    ))
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
/// Does what an interactive command of Claude Code does, Sophon's way.
fn run_native(native: crate::slash::Native, live: bool) {
    use crate::slash::Native;
    match native {
        Native::Model => model_sheet(live),
        Native::Permissions => options_sheet(live),
        Native::Clear => store::open_tab(store::Tab::Draft),
        Native::Resume if MOBILE => store::go_home(),
        Native::Resume => store::open_history(),
        Native::Usage if MOBILE => store::notify(t("Usage is on the home screen's Usage tab").into()),
        Native::Usage => store::open_usage(),
        Native::Status => store::notify(status_words(live)),
        Native::Config if MOBILE => store::notify(t("Settings are on the home screen's Settings tab").into()),
        Native::Config => store::open_settings(),
        Native::Memory if MOBILE => store::notify(t("AGENTS.md is in the conversation's files").into()),
        Native::Memory => store::open_file(memory_file()),
        Native::Mcp => store::open_mcp(),
        // The phone has no terminal: run on the computer.
        Native::Terminal(cmd) if MOBILE => store::notify(tf("Run “claude /{}” in a terminal on the computer", &[cmd])),
        // This Mac's: a Terminal window in the project. Another computer's:
        // a terminal of its service, in the workspace.
        Native::Terminal(cmd) if cfg!(target_os = "macos") && store::on_this_mac() => match store::current_workspace() {
            Some(w) => store::run_in_terminal(&crate::slash::terminal_command(&w.directory, cmd)),
            None => store::notify(t("Open a workspace to run it in a terminal").into()),
        },
        Native::Terminal(cmd) => store::run_in_workspace_terminal(crate::slash::terminal_line(cmd)),
    }
}

/// The project's instructions for its agents: `AGENTS.md`, which every
/// agent reads (a `CLAUDE.md` points Claude Code to it); a project with
/// only a `CLAUDE.md`, that.
fn memory_file() -> &'static str {
    let has = |name: &str| store::dir_entries(".").is_some_and(|list| list.iter().any(|e| !e.is_dir && e.name == name));
    if !has("AGENTS.md") && has("CLAUDE.md") { "CLAUDE.md" } else { "AGENTS.md" }
}

/// `/status`: the computer, its service, and the agent's model and mode.
fn status_words(live: bool) -> String {
    let host = match store::conn() {
        store::Conn::Connected { hostname, version, .. } => format!("{hostname} · {}", tf("service {}", &[&version])),
        _ => t("Not connected").to_owned(),
    };
    let c = store::controls(live);
    let model = c.model.as_deref().map(model_name).unwrap_or_else(|| c.provider.clone());
    let mode = c.mode.unwrap_or_default();
    [host, model, mode].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" · ")
}

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

/// The rows the slash-command or file list shows before it scrolls.
const MENU_ROWS: usize = 8;

/// The slash-command or file list over the composer.
fn suggest_panel(target: Target, doc: Doc) -> impl IntoElement {
    let trigger = computed(move || doc.tracked().and_then(|s| trigger_at(&s)).map(|(t, _)| t));
    // A new query: the keyboard's place back to the first, the list back
    // if Escape put it away.
    effect(move || {
        let _ = trigger.get();
        global::<composer_doc::MenuSelected>().set(composer_doc::MenuSelected(0));
        global::<composer_doc::MenuClosed>().set(composer_doc::MenuClosed(false));
    });
    cleanup(|| composer_doc::set_menu(None));
    view().child(dynamic(move || {
        let Some(trigger) = trigger.get().filter(|_| !global::<composer_doc::MenuClosed>().with(|c| c.0)) else {
            composer_doc::set_menu(None);
            return None;
        };
        // (icon, label, detail, what goes in, a folder, the label in code type)
        let rows: Vec<(Option<Icon>, String, String, String, bool, bool)> = match &trigger {
            Trigger::Slash(prefix) => {
                // A conversation's, or a new one's by its provider and folder.
                let list = match (target, store::selected_id()) {
                    (_, Some(agent)) => store::commands(&agent),
                    (Target::Form, None) => {
                        let f = store::form();
                        if f.provider.is_empty() || f.path.is_empty() {
                            return None;
                        }
                        store::draft_commands(&f.provider, &f.path)
                    }
                    (Target::Agent, None) => return None,
                };
                list.into_iter()
                    .filter(|c| c.name.to_lowercase().starts_with(&prefix.to_lowercase()) && !crate::slash::hidden(&c.name))
                    .map(|c| {
                        let insert = format!("/{} ", c.name);
                        // Where it opens, when Sophon does it.
                        let detail = match crate::slash::hint(&c.name) {
                            Some(h) if c.description.is_empty() => h.to_owned(),
                            Some(h) => format!("{} · {h}", c.description),
                            None => c.description,
                        };
                        // No icon: the name's own `/` says what it is.
                        (None, format!("/{}", c.name), detail, insert, false, true)
                    })
                    .collect()
            }
            Trigger::Mention(query) => {
                let cwd = match target {
                    Target::Agent => store::current_workspace().map(|w| w.directory),
                    Target::Form => Some(store::form().path),
                }?;
                // Other conversations first, named as the sidebar names
                // them, with their project and when they last moved, the
                // latest first; then files.
                let q = query.to_lowercase();
                let here = store::selected_id();
                let mut found: Vec<(crate::daemon::Agent, String)> = store::agents()
                    .into_iter()
                    .filter(|a| Some(&a.id) != here.as_ref() && !a.archived && a.parent_id.is_none() && !store::is_hidden_run(a))
                    .map(|a| {
                        let name = store::conversation_name(&a);
                        (a, name)
                    })
                    .filter(|(a, name)| q.is_empty() || name.to_lowercase().contains(&q) || a.title.to_lowercase().contains(&q))
                    .collect();
                found.sort_by(|x, y| y.0.updated_at.cmp(&x.0.updated_at));
                let mut rows: Vec<(Option<Icon>, String, String, String, bool, bool)> = found
                    .into_iter()
                    .take(if q.is_empty() { 4 } else { 5 })
                    .map(|(a, name)| {
                        let place = store::project_name_of(&a).unwrap_or_else(|| crate::daemon::model::folder_name(&a.cwd));
                        let when = crate::ui::mobile::time::ago(&a.updated_at);
                        let detail = if when.is_empty() { place } else { format!("{place} · {when}") };
                        (Some(Icon::Bot), name, detail, format!("{}{}", composer_doc::CONVERSATION, a.id), false, false)
                    })
                    .collect();
                rows.extend(store::suggestions(&cwd, query).into_iter().take(9usize.saturating_sub(rows.len())).map(|(path, dir)| {
                    let rel = path.strip_prefix(&cwd).map(|p| p.trim_start_matches('/').to_owned()).unwrap_or(path);
                    (Some(if dir { Icon::Folder } else { Icon::FileText }), rel.clone(), String::new(), rel, dir, true)
                }));
                rows
            }
        };
        if rows.is_empty() {
            composer_doc::set_menu(None);
            return None;
        }
        // Choosing row `i`, by a press or by the keyboard.
        let choose = {
            let rows = rows.clone();
            std::rc::Rc::new(move |i: usize| {
                let Some((_, _, _, insert, dir, _)) = rows.get(i).cloned() else { return };
                // Where the caret is now: the list may be a keystroke
                // behind it.
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
        };
        composer_doc::set_menu(Some(composer_doc::Menu { count: rows.len(), pick: choose.clone() }));
        Some(untracked(move || {
            let selected = computed(|| global::<composer_doc::MenuSelected>().with(|s| s.0));
            // Eight rows show at a time; the rest scroll, and the keyboard's
            // row is scrolled into the window when it leaves it.
            let row_h = (CONTROL_MD * crate::store::ui_scale().1).round();
            let scroller = element_ref();
            let first = signal(0usize);
            effect({
                let scroller = scroller.clone();
                move || {
                    let sel = selected.get();
                    let top = first.get_untracked();
                    let top = if sel < top { sel } else if sel >= top + MENU_ROWS { sel + 1 - MENU_ROWS } else { return };
                    first.set(top);
                    scroller.scroll_to(nsh_core::geometry::point(px(0.0), px(top as f32 * row_h)), false);
                }
            });
            let mut list = view().flex_col();
            for (i, (glyph, label, detail, _, _, code)) in rows.clone().into_iter().enumerate() {
                let choose = choose.clone();
                list = list.child(
                    button()
                        .on_press(move || choose(i))
                        .bg(move || if selected.get() == i { highlight() } else { tint(FG(), 0.0) })
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .h(sp(CONTROL_MD))
                        .px(sp(S2))
                        .rounded(px(R_LG))
                        .hover(|s| s.bg(highlight()))
                        .when_some(glyph, |v, glyph| v.child(icon(glyph, 15.0).text_color(FG_MUTED())))
                        .child(if code {
                            text(label).text_size(tx(TEXT_BASE)).font_family(mono_font).text_color(FG()).flex_shrink_0()
                        } else {
                            // A conversation's name in the text's own type, as
                            // the sidebar shows it, cut to leave its detail.
                            text(label).text_size(tx(TEXT_BASE)).text_color(FG()).number_of_lines(1).flex_shrink(1.0)
                        })
                        .child(text(detail).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).number_of_lines(1).flex_shrink_0().when(!code, |t| t.ml(sp(S2))).when(code, |t| t.flex_1())),
                );
            }
            card()
                .mb(sp(S2))
                .p(sp(S1))
                .bg(POPOVER())
                .child(scroll().max_h(px(row_h * MENU_ROWS as f32)).element_ref(&scroller).child(list))
        }))
    }))
}

/// Messages waiting for the running turn to end.
/// Over the composer of a conversation whose Claude Code session runs in
/// the background on this Mac: messages from here fail while it does, so
/// it is said before one is sent, with a way to close it there.
fn elsewhere_bar(live: bool) -> impl IntoElement {
    let session = computed(move || {
        let agent = store::selected_agent().filter(|a| live && a.provider == "claude" && !a.is_running())?;
        agent.session_id.filter(|s| crate::elsewhere::in_background(s))
    });
    view().child(dynamic(move || {
        let session = session.get()?;
        Some(untracked(move || {
            let short = crate::elsewhere::short(&session);
            card()
                .mb(sp(S2))
                .px(sp(S3))
                .py(sp(S2))
                .flex_row()
                .items_center()
                .gap(sp(S3))
                .child(icon(Icon::Terminal, 15.0).text_color(FG_MUTED()))
                .child(
                    text(tf("Open in a Claude Code background session ({}): messages sent here fail until it is closed there", &[&short]))
                        .text_size(tx(TEXT_SM))
                        .text_color(FG_MUTED())
                        .flex_1(),
                )
                .child(btn(t("Close it there"), Variant::Outline, Sz::Sm, None, move || {
                    store::open_modal(store::Modal::CloseElsewhere { session: session.clone() })
                }))
        }))
    }))
}

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
                        .child(icon(Icon::Inbox, 14.0).text_color(FG_FAINT()))
                        // Its words and its attachments' chips where they
                        // stand, as the composer had them: one line.
                        .child({
                            let mut thumbs: Vec<(String, Option<String>)> = msg
                                .images
                                .iter()
                                .map(|i| (i.name.clone(), Some(format!("data:{};base64,{}", i.mime, i.data))))
                                .collect();
                            thumbs.extend(msg.attachments.iter().map(|a| (a.get("fileName").and_then(serde_json::Value::as_str).unwrap_or_default().to_owned(), None)));
                            // Clipped at its end (one line); room above and below,
                            // so a chip, lifted to sit level with the words, keeps
                            // its edge.
                            let mut line = view().flex_row().items_center().flex_1().min_w(px(0.0)).py(px(2.0)).overflow_hidden();
                            for segment in composer_doc::segments(&msg.text) {
                                line = match segment {
                                    composer_doc::Segment::Text(t) => {
                                        line.child(text(t.replace('\n', " ")).text_size(tx(TEXT_BASE)).number_of_lines(1).flex_shrink(1.0))
                                    }
                                    composer_doc::Segment::Chip(name) => {
                                        let thumb = thumbs.iter().position(|(n, _)| n.replace(['[', ']', '\n'], " ").trim() == name).map(|i| thumbs.remove(i).1).flatten();
                                        line.child(super::timeline::sent_chip(name, thumb).flex_shrink_0())
                                    }
                                };
                            }
                            for (name, thumb) in thumbs {
                                line = line.child(super::timeline::sent_chip(name, thumb).flex_shrink_0());
                            }
                            line
                        })
                        // Back into the composer, attachments and all, to be
                        // changed and sent again.
                        .child(btn(t("Edit"), Variant::Ghost, Sz::Sm, Some(Icon::Pencil), move || {
                            if let Some(id) = store::selected_id() {
                                store::edit_queued(&id, i);
                            }
                        }))
                        // Into the running turn now, without waiting for it
                        // to end.
                        .child(btn(t("Steer"), Variant::Ghost, Sz::Sm, Some(Icon::CircleArrowUp), move || {
                            if let Some(id) = store::selected_id() {
                                store::steer_queued(&id, i);
                            }
                        }))
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
                    .bg(FG())
                    .items_center()
                    .justify_center()
                    .hover(|s| s.opacity(0.85))
                    .child(view().w(px(10.0)).h(px(10.0)).rounded(px(2.0)).bg(SURFACE0()))
                    .into_element()
            } else {
                button()
                    .on_press(submit)
                    .disabled(move || empty.get() || busy.get())
                    .w(px(32.0))
                    .h(px(32.0))
                    .rounded(px(R_FULL))
                    .bg(move || if empty.get() { SURFACE3() } else { ACCENT() })
                    .items_center()
                    .justify_center()
                    .hover(|s| s.opacity(0.88))
                    .child(icon(Icon::ArrowUp, 17.0).text_color(move || if empty.get() { FG_FAINT() } else { ACCENT_FG() }))
                    .into_element()
            }
        }))
    }))
}

/// A model the provider does not list (a default the daemon keeps, from
/// another Claude Code version) by a short name: `claude-opus-5-5` as
/// `Opus 5.5`; any other id as it is.
pub(crate) fn model_name(id: &str) -> String {
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
pub(crate) fn model_picker(live: bool) -> impl IntoElement {
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

/// The status bar under the desktop's conversation (as monocode's): the
/// provider's mark, then its 5-hour and weekly limits, each a ring, a word
/// and a percentage with the time until it starts again; and a refresh.
/// (The context is the composer's, beside Send: `context_meter_ring`.) Amber from 80%,
/// red from 90%; a meter opens the usage page. One with nothing to say is
/// left out. `live`: the open agent's (else a new conversation's).
pub(crate) fn usage_bar(live: bool) -> impl IntoElement {
    let readings = computed(move || {
        // The time left moves on each minute.
        let now = store::minute().max(crate::usage::now());
        let mut out: Vec<(&'static str, f64, Option<i64>)> = Vec::new();
        let provider = store::controls(live).provider;
        if let Some(p) = store::usage().and_then(|l| l.into_iter().find(|p| p.id == provider)) {
            for (word, length) in [(t("5h"), 5 * 3600), (t("Week"), 7 * 86_400)] {
                if let Some((pct, resets)) = limit_window(&p.windows, length) {
                    out.push((word, pct, resets.map(|at| at - now)));
                }
            }
        }
        (provider, out)
    });
    view().flex_row().items_center().h(px(30.0)).px(sp(S3)).gap(sp(S2)).border_t(px(1.0)).border_color(BORDER()).child(dynamic(move || {
        let (provider, list) = readings.get();
        Some(untracked(move || {
            let glyph = match provider.as_str() {
                "codex" => Icon::Codex,
                _ => Icon::Claude,
            };
            let mut row = view().flex_row().items_center().gap(sp(S4)).child(icon(glyph, 13.0).text_color(FG_MUTED()));
            if list.is_empty() {
                row = row.child(text(t("No usage reported yet")).text_size(tx(TEXT_XS)).text_color(FG_FAINT()));
            }
            for (word, pct, left) in list {
                let tone = meter_tone(pct);
                let mut meter = button()
                    .on_press(store::open_usage)
                    .accessibility_label(t("Usage"))
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .px(px(4.0))
                    .h(px(22.0))
                    .rounded(px(R_MD))
                    .hover(|s| s.bg(highlight()))
                    .child(ring(pct).text_color(tone))
                    .child(text(word).text_size(tx(TEXT_XS)).text_color(FG_MUTED()))
                    .child(text(format!("{:.0}%", pct.clamp(0.0, 100.0))).text_size(tx(TEXT_XS)).font_family(mono_font).text_color(tone));
                if let Some(secs) = left.filter(|s| *s > 0) {
                    meter = meter.child(text(time_left(secs)).text_size(tx(TEXT_XS)).font_family(mono_font).text_color(FG_FAINT()));
                }
                row = row.child(meter);
            }
            row.child(icon_btn(Icon::RotateCw, Sz::Sm, t("Refresh"), store::refresh_usage)).into_element()
        }))
    }))
}

/// What is left of each agent's plan this week, at the top right (as T3
/// Code's): "This week left", then each agent with a reading, its ring and
/// what is left of its weekly limit (else of its 5-hour one). Green while
/// there is plenty, amber under 25%, red under 10%. The chevron opens each
/// one's windows and when they start again; a press on the readings opens
/// the usage page.
pub(crate) fn usage_summary() -> impl IntoElement {
    let readings = computed(|| {
        let now = store::minute().max(crate::usage::now());
        store::usage()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| {
                let week = limit_window(&p.windows, 7 * 86_400);
                let hours = limit_window(&p.windows, 5 * 3600);
                let (used, _) = week.or(hours)?;
                let name = match p.id.as_str() {
                    "codex" => "Codex".to_owned(),
                    "claude" => "Claude".to_owned(),
                    _ if !p.name.is_empty() => p.name.clone(),
                    _ => p.id.clone(),
                };
                let windows: Vec<(&'static str, f64, Option<i64>)> = [(t("5h"), hours), (t("Week"), week)]
                    .into_iter()
                    .filter_map(|(word, w)| w.map(|(pct, resets)| (word, pct, resets.map(|at| at - now))))
                    .collect();
                Some((p.id.clone(), name, (100.0 - used).clamp(0.0, 100.0), windows))
            })
            .collect::<Vec<_>>()
    });
    view().child(dynamic(move || {
        let list = readings.get();
        if list.is_empty() {
            return None;
        }
        Some(untracked(move || {
            let left_tone = |left: f64| if left < 10.0 { DANGER } else if left < 25.0 { WARNING } else { SUCCESS };
            let mut shown = button()
                .on_press(store::open_usage)
                .accessibility_label(t("Usage"))
                .flex_row()
                .items_center()
                .gap(sp(S3))
                .h(px(28.0))
                .pl(sp(S3))
                .pr(sp(S1))
                .child(text(t("This week left")).text_size(tx(TEXT_XS)).text_color(FG_FAINT()));
            for (_, name, left, _) in &list {
                shown = shown.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(5.0))
                        .child(ring(*left).text_color(left_tone(*left)))
                        .child(text(name.clone()).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG()))
                        .child(text(format!("{left:.0}%")).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(left_tone(*left))),
                );
            }
            // Each one's windows, and when they start again.
            let mut card = view().w(px(300.0)).p(sp(S2)).gap(sp(S1));
            for (id, name, _, windows) in list.clone() {
                let mut block = view().px(sp(S2)).py(sp(S2)).gap(px(6.0)).child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(crate::ui::shared::model_picker::mark(&id, 14.0))
                        .child(text(name).text_size(tx(TEXT_SM)).font_weight(FontWeight(600)).flex_1()),
                );
                for (word, used, left) in windows {
                    let rest = (100.0 - used).clamp(0.0, 100.0);
                    block = block.child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap(sp(S2))
                            .child(text(word).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).w(px(42.0)))
                            .child(
                                view()
                                    .flex_1()
                                    .h(px(5.0))
                                    .rounded(px(R_FULL))
                                    .bg(SURFACE3())
                                    .child(view().h_full().w(relative((rest / 100.0) as f32)).rounded(px(R_FULL)).bg(left_tone(rest))),
                            )
                            .child(text(format!("{rest:.0}%")).text_size(tx(TEXT_XS)).font_family(mono_font).text_color(left_tone(rest)).w(px(36.0)).text_right())
                            .child(text(left.filter(|s| *s > 0).map(time_left).unwrap_or_default()).text_size(tx(TEXT_XS)).font_family(mono_font).text_color(FG_FAINT()).w(px(54.0)).text_right()),
                    );
                }
                card = card.child(block);
            }
            card = card.child(menu_divider()).child(menu_row(t("Open usage"), Some(Icon::Activity), None, false, store::open_usage)).child(menu_row(
                t("Refresh"),
                Some(Icon::RotateCw),
                None,
                false,
                store::refresh_usage,
            ));
            view()
                .flex_row()
                .items_center()
                .rounded(px(R_FULL))
                .border_1()
                .border_color(BORDER())
                .bg(SURFACE0())
                .hover(|s| s.bg(highlight()))
                .child(shown)
                .child(
                    dropdown(view().w(px(24.0)).h(px(24.0)).mr(px(2.0)).items_center().justify_center().rounded(px(R_FULL)).hover(|s| s.bg(highlight())).child(icon(Icon::ChevronDown, 13.0).text_color(FG_MUTED())))
                        .align(nsh_core::overlay::Align::End)
                        .child(card),
                )
                .into_element()
        }))
    }))
}

/// The open agent's context beside Send: a ring and its percentage; under
/// the pointer, a card: "Context", "6% used", a bar, and the tokens
/// (`6.1万 / 100万`, `61K / 1M`). Amber from 80%, red from 90%.
fn context_meter_ring() -> impl IntoElement {
    use nsh_core::element::Interactive as _;
    let usage = computed(|| store::selected_agent().and_then(|a| Some((a.context_used?, a.context_max.filter(|m| *m > 0)?))));
    view().child(dynamic(move || {
        let (used, max) = usage.get()?;
        let pct = (used as f64 / max as f64 * 100.0).clamp(0.0, 100.0);
        let tone = meter_tone(pct);
        Some(untracked(move || {
            let tip = view()
                .w(px(260.0))
                .gap(sp(S2))
                .px(sp(S4))
                .py(sp(S3))
                .rounded(px(R_LG))
                .bg(SURFACE0())
                .border_1()
                .border_color(BORDER())
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .child(text(t("Context")).text_size(tx(TEXT_SM)).font_weight(FontWeight(600)).flex_1())
                        .child(text(tf("{}% used", &[&format!("{pct:.0}")])).text_size(tx(TEXT_XS)).text_color(FG_MUTED())),
                )
                .child(
                    view()
                        .h(px(5.0))
                        .rounded(px(R_FULL))
                        .bg(SURFACE3())
                        .child(view().h_full().w(relative((pct / 100.0) as f32)).rounded(px(R_FULL)).bg(if pct >= 80.0 { tone } else { FG_MUTED() })),
                )
                .child(text(format!("{} / {}", tokens(used), tokens(max))).text_size(tx(TEXT_XS)).text_color(FG_MUTED()));
            view().tooltip(tip.into_element()).child(
                button()
                    .on_press(store::open_usage)
                    .accessibility_label(tf("{}% context used", &[&format!("{pct:.0}")]))
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .h(px(28.0))
                    .px(px(6.0))
                    .rounded(px(R_FULL))
                    .hover(|s| s.bg(highlight()))
                    .child(ring(pct).text_color(tone))
                    .child(text(format!("{pct:.0}%")).text_size(tx(TEXT_XS)).font_family(mono_font).text_color(tone)),
            )
        }))
    }))
}

/// A count of tokens, short, as the language writes it: `6.1万` / `100万`,
/// `61K` / `1M`.
fn tokens(n: u64) -> String {
    tokens_in(n, crate::i18n::is_zh())
}

fn tokens_in(n: u64, zh: bool) -> String {
    if zh && n >= 10_000 {
        let w = n as f64 / 10_000.0;
        let s = if w >= 100.0 { format!("{w:.0}") } else { format!("{w:.1}") };
        format!("{}万", s.trim_end_matches(".0"))
    } else {
        crate::usage::compact(n)
    }
}

/// Seconds until a reset, short: `7h 38m`, `2d 3h`, `12m`.
fn time_left(secs: i64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3600, secs % 3600 / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{}m", m.max(1))
    }
}

/// The used percentage of the provider's window of `length` seconds (the
/// one for every model, not one model's own), and when it resets (Unix
/// seconds).
fn limit_window(windows: &[crate::daemon::connection::UsageWindow], length: i64) -> Option<(f64, Option<i64>)> {
    use crate::usage::{self, limits};
    windows
        .iter()
        .filter(|w| w.used.is_some() && limits::window_length(&w.id, &w.label) == Some(length))
        // The shortest id is the unscoped one (`weekly`, not `weekly_model_…`).
        .min_by_key(|w| w.id.len())
        .map(|w| {
            (w.used.unwrap_or(0.0), w.resets_at.as_deref().and_then(usage::parse::rfc3339))
        })
}

fn meter_tone(pct: f64) -> AdaptiveColor {
    if pct >= 90.0 {
        DANGER
    } else if pct >= 80.0 {
        WARNING
    } else {
        FG_MUTED()
    }
}

/// A 14-point ring filled to `pct`, in the text color (its track a faint
/// one of the same).
fn ring(pct: f64) -> nsh_core::element::Svg {
    let markup = ring_markup(pct);
    svg(move || nsh_core::element::ImageSrc::svg_markup(&markup)).w(px(14.0)).h(px(14.0)).flex_shrink_0()
}

fn ring_markup(pct: f64) -> String {
    let r = 6.0_f64;
    let c = 2.0 * std::f64::consts::PI * r;
    let on = c * pct.clamp(0.0, 100.0) / 100.0;
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="{r}" fill="none" stroke="currentColor" stroke-opacity="0.22" stroke-width="2.5"/><circle cx="8" cy="8" r="{r}" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-dasharray="{on:.2} {c:.2}" transform="rotate(-90 8 8)"/></svg>"#
    )
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
    view().w_full().max_w(px(CONTENT_MAX_WIDTH())).self_center().px(sp(S2)).child(dynamic(move || {
        let (m, error) = message.get()?;
        Some(untracked(move || {
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon(if error { Icon::CircleAlert } else { Icon::Info }, 14.0).text_color(if error {
                    DANGER
                } else {
                    FG_FAINT()
                }))
                .child(
                    text(m.clone())
                        .text_size(tx(TEXT_SM))
                        .font_weight(FontWeight(400))
                        .text_color(if error { DANGER } else { FG_MUTED() })
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

/// A sent message's pictures (data URIs) back in conversation `agent`'s
/// composer, as chips after its text: a moment later, once the text given
/// back has reached the editor (it would be written over otherwise).
pub(crate) fn put_back_images(agent: &str, uris: &[String]) {
    let images: Vec<_> = uris
        .iter()
        .enumerate()
        .filter_map(|(i, uri)| {
            let (head, data) = uri.strip_prefix("data:")?.split_once(";base64,")?;
            let ext = head.rsplit('/').next().unwrap_or("png");
            Some(crate::daemon::connection::ImageInput { name: format!("image-{}.{ext}", i + 1), mime: head.to_owned(), data: data.to_owned() })
        })
        .collect();
    if images.is_empty() {
        return;
    }
    let key = format!("agent:{agent}");
    set_timeout(std::time::Duration::from_millis(30), move || insert_images(&key, images));
}

/// Starts uploading a file and puts its chip in composer `key`'s text.
fn insert_file(key: &str, name: String, mime: String, bytes: Vec<u8>) {
    if let Some(id) = store::attach_file(name.clone(), mime.clone(), bytes) {
        insert_into(key, &[Chip { id, kind: Kind::File, name, mime }]);
    }
}

/// Attaches a message of the open conversation as context: its words,
/// quoted (`> `), at the top of the composer, which takes the keyboard.
pub(crate) fn attach_as_context(message: &str) {
    let body = match message.find(crate::ui::shared::timeline::CONTEXT_MARK) {
        Some(at) => &message[..at],
        None => message,
    };
    let body = body.trim();
    if body.is_empty() {
        return;
    }
    let quote: Vec<String> = body.lines().map(|l| if l.is_empty() { ">".to_owned() } else { format!("> {l}") }).collect();
    let Some(id) = store::selected_id() else { return };
    let Some(session) = composer_doc::existing(&format!("agent:{id}")) else { return };
    composer_doc::prepend_quote(&session, &quote.join("\n"));
    session.focus();
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

/// Anything from the computer, in one dialog: a picture goes with the
/// message as a picture, any other file is uploaded as an attachment.
fn pick_any_into(key: String) {
    store::spawn_detached(async move {
        use base64::Engine;
        let picked = pick_files(nsh_core::picker::FilePickRequest::new(true, 10)).await;
        for f in picked {
            let bytes = f.data.bytes().to_vec();
            let mime = if f.mime.starts_with("image/") { f.mime.clone() } else { nsh_core::picker::mime_for_file(&f.name, &bytes).to_owned() };
            if mime.starts_with("image/") {
                insert_images(&key, vec![crate::daemon::connection::ImageInput {
                    name: f.name.clone(),
                    mime,
                    data: base64::engine::general_purpose::STANDARD.encode(&bytes),
                }]);
            } else {
                insert_file(&key, f.name.clone(), mime, bytes);
            }
        }
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

#[derive(Clone, Default)]
struct Dictating(bool);

thread_local! {
    static SPEECH: std::cell::RefCell<Option<std::rc::Rc<std::cell::RefCell<nsh_extra::speech::SpeechRecognition>>>> =
        const { std::cell::RefCell::new(None) };
    /// The composer on screen, for the dictation shortcut.
    static MIC_DOC: std::cell::Cell<Option<Doc>> = const { std::cell::Cell::new(None) };
}

/// The phone's push to talk, over its empty composer while the keyboard
/// is down, unmarked: held, dictation listens (a moment in, so a tap stays a tap)
/// and stops when it is let go; a tap brings the keyboard, as a tap on the
/// text does. With text in it, or the keyboard up, the text is the
/// editor's to touch (and the keyboard has its own microphone).
fn hold_to_talk(doc: Doc, empty: Memo<bool>) -> impl IntoElement {
    let listening = computed(move || global::<Dictating>().with(|d| d.0));
    // Kept while it listens: the words it puts in make the composer not
    // empty, and the finger still on it must be heard letting go.
    let shown = computed(move || MOBILE && (listening.get() || (empty.get() && nsh_core::element::keyboard_height().0 <= 0.0)));
    when(shown, move || {
        let held = signal(0u64);
        let started = signal(false);
        let release = move |tap: bool| {
            held.update(|h| *h += 1);
            if started.get_untracked() {
                started.set(false);
                if dictating() {
                    dictate(doc);
                }
            } else if tap && let Some(session) = doc.session() {
                session.focus();
            }
        };
        view()
            .absolute()
            .top(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .claim_touches(TouchClaim::on_start().ask(|r: &ClaimRequest| r.claim()))
            .keep_touches(true)
            .on_responder_grant(move |_: &ResponderEvent| {
                let n = held.get_untracked() + 1;
                held.set(n);
                set_timeout(std::time::Duration::from_millis(300), move || {
                    if held.get_untracked() == n && !dictating() {
                        started.set(true);
                        dictate(doc);
                    }
                });
            })
            .on_responder_release(move |_: &ResponderEvent| release(true))
            .on_responder_terminate(move |_: &ResponderEvent| release(false))
    })
}

/// The phone's card while dictation listens, as Codex's voice mode: over
/// the whole composer, a wave that moves, what it has heard so far (the
/// words go into the text under it) and how to stop. It takes no touch:
/// the finger holding the text keeps its gesture.
fn listening_card(doc: Doc) -> impl IntoElement {
    let listening = computed(move || global::<Dictating>().with(|d| d.0));
    let heard = computed(move || {
        let text = doc.tracked().map(|s| composer_doc::outgoing(s.state().doc()).text).unwrap_or_default();
        // The end of it, as it comes.
        let chars: Vec<char> = text.trim().chars().collect();
        let tail: String = chars[chars.len().saturating_sub(60)..].iter().collect();
        if chars.len() > 60 { format!("…{tail}") } else { tail }
    });
    when(listening, move || {
        // Five bars, each rising and falling at its own pace.
        let mut wave = view().flex_row().items_center().gap(px(4.0)).h(px(28.0));
        for (i, ms) in [420.0_f32, 300.0, 520.0, 360.0, 460.0].into_iter().enumerate() {
            let level = animated_value(0.35);
            let half = nsh_core::element::Timing::new(ms, Easing::EaseInOut);
            Animation::sequence([Animation::timing(&level, 1.0, half.clone()), Animation::timing(&level, 0.3 + 0.1 * (i % 3) as f32, half)])
                .looped(None)
                .start();
            wave = wave.child(
                view().w(px(4.0)).h(px(26.0)).rounded(px(R_FULL)).bg(ACCENT()).bind_transform(nsh_core::TransformKind::ScaleY, &level),
            );
        }
        view()
            .absolute()
            .top(px(0.0))
            .bottom(px(0.0))
            .left(px(0.0))
            .right(px(0.0))
            .rounded(px(24.0))
            .bg(SURFACE0())
            .pointer_events(PointerEvents::None)
            .px(sp(S4))
            .py(sp(S3))
            .gap(sp(S2))
            .justify_center()
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S3))
                    .child(wave)
                    .child(text(t("Listening…")).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)).text_color(FG()))
                    .child(view().flex_1())
                    .child(text(t("Let go to stop")).text_size(tx(TEXT_SM)).text_color(FG_FAINT())),
            )
            .child(when(move || heard.with(|h| !h.is_empty()), move || {
                text(move || heard.get()).text_size(tx(TEXT_BASE)).text_color(FG_MUTED()).number_of_lines(2)
            }))
    })
}

/// Dictation is listening (for push to talk: Fn on macOS, the phone's
/// held composer).
pub(crate) fn dictating() -> bool {
    global::<Dictating>().with_untracked(|d| d.0)
}

/// The dictation shortcut: starts or stops dictating into the composer on
/// screen, as its microphone button does.
pub(crate) fn toggle_dictation() {
    if let Some(doc) = MIC_DOC.with(std::cell::Cell::get) {
        dictate(doc);
    }
}

/// The language dictation listens for: Sophon's own when it speaks
/// Chinese (the system's may be English on a Mac set up in English), the
/// system's otherwise.
fn dictation_lang() -> Option<String> {
    crate::i18n::is_zh().then(|| "zh-CN".to_owned())
}

/// Starts dictating into `doc` at its caret, or stops.
fn dictate(doc: Doc) {
    let on = global::<Dictating>();
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
    let mut request = nsh_extra::speech::SpeechRequest::default();
    request.lang = dictation_lang();
    let speech = std::rc::Rc::new(std::cell::RefCell::new(nsh_extra::speech::recognize(request)));
    on.update(|d| d.0 = true);
    SPEECH.with_borrow_mut(|s| *s = Some(speech.clone()));
    spawn(async move {
        // Borrowed only while polled, so Stop can reach the session.
        while let Some(ev) = std::future::poll_fn(|cx| speech.borrow_mut().poll_next(cx)).await {
            match ev {
                nsh_extra::speech::SpeechEvent::Result { text, .. } => {
                    // Chinese runs on without spaces.
                    let text = if spaced && !text.starts_with(|c: char| !c.is_ascii()) { format!(" {text}") } else { text };
                    composer_doc::replace_text(&session, at, at + inserted.get(), &text);
                    inserted.set(text.encode_utf16().count());
                }
                nsh_extra::speech::SpeechEvent::End { error } => {
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
}

/// Dictation: speech to text into the composer at the caret, with the
/// system recognizer. Each result is the whole transcript so far, so it
/// replaces what the previous one inserted.
fn mic_button(doc: Doc) -> impl IntoElement {
    let on = global::<Dictating>();
    MIC_DOC.with(|d| d.set(Some(doc)));
    nsh_base::button()
        .on_press(move || dictate(doc))
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
            Some(untracked(move || icon(if active { Icon::MicOff } else { Icon::Mic }, 16.0).text_color(if active { DANGER } else { FG_MUTED() })))
        }))
}

/// Cmd+V / Ctrl+V of files: chips at the caret: images (a screenshot) as images, other files uploaded to the
/// host, and files copied in Finder read and uploaded too (the host may be
/// another computer). Text pastes into the editor itself.
/// The phone's Paste (the attach sheet's first item). The composer's
/// editor draws its text itself over an input the system's edit menu does
/// not reach, so a long press offers no Paste there. A picture on the
/// clipboard is attached; else its text goes in at the caret, in place of
/// what is selected.
fn paste_into(key: String) {
    store::spawn_detached(async move {
        if clipboard_has_image().await
            && let Some(image) = nsh_core::prelude::clipboard_image().await
            && let Some((mime, data)) = data_uri(&image.url)
        {
            let ext = mime.strip_prefix("image/").unwrap_or("png").to_owned();
            insert_images(&key, vec![crate::daemon::connection::ImageInput { name: format!("pasted.{ext}"), mime, data }]);
            return;
        }
        let text = clipboard_string().await;
        if text.is_empty() {
            store::notify(t("Nothing to paste").to_owned());
            return;
        }
        let Some(session) = composer_doc::existing(&key) else { return };
        let sel = session.state().selection().clone();
        composer_doc::replace_text(&session, sel.from(), sel.to(), &text);
        session.focus();
    });
}

/// The type and base64 data of a `data:<type>;base64,<data>` URI.
fn data_uri(uri: &str) -> Option<(String, String)> {
    let (head, data) = uri.strip_prefix("data:")?.split_once(',')?;
    let mime = head.strip_suffix(";base64")?;
    mime.starts_with("image/").then(|| (mime.to_owned(), data.to_owned()))
}

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
    dropped_files(uris, &[]);
}

/// A drop's files: by their addresses (`uris`, Finder's), or as their
/// contents (`files`: what a drag only carries, a picture out of a chat
/// app or a browser, or a file it promised and wrote once dropped).
pub(crate) fn dropped_files(uris: &[String], files: &[nsh_core::picker::PickedFile]) {
    dragging_over(false);
    let key = match (store::screen(), store::tab()) {
        (store::Screen::Workspace(_), store::Tab::Agent(id)) => Some(format!("agent:{id}")),
        (store::Screen::Workspace(_), store::Tab::Draft) | (store::Screen::NewWorkspace, _) => Some("form".to_owned()),
        _ => None,
    };
    let Some(key) = key.filter(|k| composer_doc::existing(k).is_some()) else {
        store::notify(t("Open a conversation to attach files to").to_owned());
        return;
    };
    attach_uris(&key, uris, false);
    attach_contents(&key, files);
}

/// Files dropped as their contents: images as images, the rest uploaded.
fn attach_contents(key: &str, files: &[nsh_core::picker::PickedFile]) {
    use base64::Engine;
    for file in files {
        if file.status != nsh_core::picker::PickedStatus::Read {
            store::notify(tf("“{}” is too large to attach", &[&file.name]));
            continue;
        }
        let bytes = file.data.bytes();
        let mime = if file.mime.is_empty() { nsh_core::picker::mime_for_file(&file.name, bytes).to_owned() } else { file.mime.clone() };
        if mime.starts_with("image/") {
            insert_images(key, vec![crate::daemon::connection::ImageInput {
                name: file.name.clone(),
                mime,
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
            }]);
        } else {
            insert_file(key, file.name.clone(), mime, bytes.to_vec());
        }
    }
}

/// `text` typed into composer `key` at its caret.
#[cfg(not(target_arch = "wasm32"))]
fn insert_text(key: &str, text: &str) {
    let Some(session) = composer_doc::existing(key) else { return };
    let at = session.state().selection().from();
    composer_doc::replace_text(&session, at, at, text);
}

/// Files by their `file:` addresses, pasted (copied in Finder) or dropped
/// on the composer: read here and attached, images as images, the rest
/// uploaded to the host (which may be another computer). A folder is named
/// by its path in the text, as a terminal takes it, when the host is this
/// Mac (the agent reads it there); another computer cannot reach it.
/// `pasted`: one that cannot be read may be on the clipboard itself.
#[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
fn attach_uris(key: &str, uris: &[String], pasted: bool) {
    use base64::Engine;
    #[cfg(not(target_arch = "wasm32"))]
    for uri in uris {
        let Some(path) = uri.strip_prefix("file://") else { continue };
        let path = std::path::PathBuf::from(percent_decode(path));
        if path.is_dir() {
            if store::on_this_mac() {
                insert_text(key, &format!("{} ", path.display()));
            } else {
                store::notify(t("A folder on this computer cannot be sent to the other one").to_owned());
            }
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

#[cfg(test)]
mod paste_tests {
    use super::data_uri;

    #[test]
    fn a_pasted_picture_s_data_uri_gives_its_type_and_data() {
        assert_eq!(data_uri("data:image/png;base64,iVBOR"), Some(("image/png".into(), "iVBOR".into())));
        assert_eq!(data_uri("data:text/plain;base64,aGk="), None);
        assert_eq!(data_uri("data:image/png,raw"), None);
        assert_eq!(data_uri("file:///tmp/x.png"), None);
    }
}

#[cfg(test)]
mod meter_tests {
    use super::{limit_window, ring_markup};
    use crate::daemon::connection::UsageWindow;

    fn w(id: &str, label: &str, used: Option<f64>) -> UsageWindow {
        UsageWindow { id: id.into(), label: label.into(), used, resets_at: None }
    }

    #[test]
    fn the_5h_and_weekly_meters_take_the_provider_s_own_windows() {
        let claude = [w("five_hour", "Session", Some(42.0)), w("weekly_model_fable", "Weekly · Fable", Some(86.0)), w("weekly", "Weekly", Some(55.0))];
        assert_eq!(limit_window(&claude, 5 * 3600).map(|x| x.0), Some(42.0));
        // The weekly for every model, not one model's own.
        assert_eq!(limit_window(&claude, 7 * 86_400).map(|x| x.0), Some(55.0));
        // Nothing reported: no meter.
        assert_eq!(limit_window(&[w("weekly", "Weekly", None)], 7 * 86_400), None);
        assert_eq!(limit_window(&[w("credits", "Credits", Some(3.0))], 5 * 3600), None);
    }

    #[test]
    fn tokens_read_as_the_language_writes_them() {
        use super::tokens_in;
        assert_eq!((tokens_in(61_234, true), tokens_in(1_000_000, true), tokens_in(140_000, true), tokens_in(9_500, true)), ("6.1万".into(), "100万".into(), "14万".into(), crate::usage::compact(9_500)));
        assert_eq!(tokens_in(61_234, false), crate::usage::compact(61_234));
    }

    #[test]
    fn the_time_left_is_short() {
        use super::time_left;
        assert_eq!((time_left(7 * 3600 + 38 * 60), time_left(2 * 86_400 + 3 * 3600 + 5), time_left(30), time_left(12 * 60)), ("7h 38m".into(), "2d 3h".into(), "1m".into(), "12m".into()));
    }

    #[test]
    fn a_ring_is_filled_to_its_percentage() {
        assert!(ring_markup(0.0).contains("stroke-dasharray=\"0.00 "));
        assert!(ring_markup(50.0).contains("stroke-dasharray=\"18.85 37.70\""));
        assert!(ring_markup(150.0).contains("stroke-dasharray=\"37.70 37.70\""));
    }
}
