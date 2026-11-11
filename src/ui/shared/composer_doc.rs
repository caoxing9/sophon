//! The composer's document: text with inline chips, edited in NSH's
//! `RichEditor` (ProseMirror's model). An `attachment` chip stands for an
//! image or file waiting in the store (by id: the bytes never enter the
//! document), a `mention` chip for an `@` path. The message is the
//! document read in order ([`outgoing`]): text as typed, a mention as
//! `@path`, attachments out of the text and into the message's images and
//! files, in the chips' order.
//!
//! Each composer (an agent's, the new-agent form's) keeps one
//! [`EditorSession`] for as long as the app runs, so a draft keeps its
//! chips, caret and undo history while another agent is open.

use std::{cell::RefCell, collections::HashMap, time::Duration};

use nsh_base::rich_editor::{
    commands::command,
    extensions::{HistoryOptions, history, keymap, kit, starter},
    model::{AttrSpec, Attrs, Fragment, Node, NodeSpec, Schema, SchemaSpec, TextRender},
    state::{EditorState, EditorStateConfig, Selection, TextSelection, Transaction},
    view::EditorSession,
};
use nsh_core::prelude::*;
use serde_json::Value;

use crate::ui::kit::MOBILE;

/// The chip's node types, and the attributes they carry.
pub(crate) const ATTACHMENT: &str = "attachment";
pub(crate) const MENTION: &str = "mention";

/// What a chip's attachment is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Image,
    File,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Image => "image",
            Kind::File => "file",
        }
    }
}

/// An attachment chip to insert.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Chip {
    pub(crate) id: String,
    pub(crate) kind: Kind,
    pub(crate) name: String,
    pub(crate) mime: String,
}

/// The schema: one or more paragraphs of text, line breaks and the two
/// chips. Built from Rust specs (not JSON) since the first node type is
/// the default block and JSON objects lose their key order.
pub(crate) fn schema() -> Schema {
    let attr = |default: Value| {
        let mut a = AttrSpec::default();
        a.default = Some(default);
        a
    };
    let mut doc = NodeSpec::default();
    doc.content = Some("paragraph+".into());
    let mut paragraph = NodeSpec::default();
    paragraph.content = Some("inline*".into());
    paragraph.group = Some("block".into());
    let mut text = NodeSpec::default();
    text.group = Some("inline".into());
    // Chips are atoms Backspace deletes whole. An arrow stops on one,
    // selecting it (its outline shows it), as on a picture in a document:
    // the next press goes on, Backspace or typing takes it away.
    let chip = |attrs: Vec<(String, AttrSpec)>| {
        let mut n = NodeSpec::default();
        n.inline = true;
        n.group = Some("inline".into());
        n.atom = true;
        n.selectable = true;
        n.attrs = attrs;
        n
    };
    let attachment = chip(vec![
        ("id".into(), attr(Value::from(""))),
        ("kind".into(), attr(Value::from("file"))),
        ("name".into(), attr(Value::from(""))),
        ("mime".into(), attr(Value::from(""))),
    ]);
    let mut mention = chip(vec![("path".into(), attr(Value::from(""))), ("dir".into(), attr(Value::Bool(false)))]);
    // What a copied mention reads as, and what it sends.
    mention.render_text = Some(TextRender::Template("@{path}".into()));
    let mut spec = SchemaSpec::default();
    spec.nodes = vec![
        ("doc".into(), doc),
        ("paragraph".into(), paragraph),
        ("text".into(), text),
        starter::nodes::hard_break(),
        (ATTACHMENT.into(), attachment),
        (MENTION.into(), mention),
    ];
    Schema::new(spec).expect("the composer's schema is valid")
}

fn attr_str<'a>(node: &'a Node, name: &str) -> &'a str {
    node.attrs().get(name).and_then(Value::as_str).unwrap_or("")
}

/// A chip's attachment id, if `node` is an attachment chip.
pub(crate) fn attachment_id(node: &Node) -> Option<&str> {
    (node.ty().name() == ATTACHMENT).then(|| attr_str(node, "id"))
}

/// What a document sends: its text, and its attachments' ids in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Outgoing {
    pub(crate) text: String,
    pub(crate) attachments: Vec<String>,
    /// The text as it is sent: each attachment chip a `[name]` where it
    /// stood, so the agent reads where a file belongs in the sentence and
    /// the sent message shows its chips in place ([`segments`]).
    pub(crate) message: String,
}

impl Outgoing {
    /// Nothing to send: no text but white space, no attachment.
    pub(crate) fn is_empty(&self) -> bool {
        self.text.trim().is_empty() && self.attachments.is_empty()
    }
}

/// The message `doc` makes: paragraphs and line breaks as newlines, a
/// mention as `@path`, attachment chips out of the text (their ids, in
/// order, in `attachments`).
pub(crate) fn outgoing(doc: &Node) -> Outgoing {
    let mut out = Outgoing::default();
    for (i, block) in doc.content().children().iter().enumerate() {
        if i > 0 {
            out.text.push('\n');
            out.message.push('\n');
        }
        for inline in block.content().children() {
            if let Some(t) = inline.text() {
                out.text.push_str(t);
                out.message.push_str(t);
                continue;
            }
            match inline.ty().name() {
                ATTACHMENT => {
                    out.attachments.push(attr_str(inline, "id").to_owned());
                    let name = attr_str(inline, "name").replace(['[', ']', '\n'], " ");
                    out.message.push_str(&format!("[{}]", name.trim()));
                }
                MENTION => {
                    let mention = format!("@{}", attr_str(inline, "path"));
                    out.text.push_str(&mention);
                    out.message.push_str(&mention);
                }
                // A line break, or anything with a text of its own.
                _ => {
                    let t = inline.text_content();
                    out.text.push_str(&t);
                    out.message.push_str(&t);
                }
            }
        }
    }
    out
}

/// A piece of a sent message: text, or an attachment's `[name]`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Segment {
    Text(String),
    Chip(String),
}

/// Whether `name` is how Claude Code's own terminal names a pasted
/// picture in the message (`[Image #1]`): a session started there and
/// imported has these.
fn is_terminal_image(name: &str) -> bool {
    name.strip_prefix("Image #").is_some_and(|n| !n.is_empty() && n.len() <= 4 && n.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether `name` is a picture's, by its extension.
pub(crate) fn is_image_name(name: &str) -> bool {
    is_terminal_image(name) || name.rsplit_once('.').is_some_and(|(_, ext)| {
        matches!(ext.to_ascii_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "heic" | "heif" | "bmp" | "tiff" | "avif")
    })
}

/// A sent message's text and the attachments named in it (`[image.png]`: a
/// file's name in brackets, on one line), in order.
pub(crate) fn segments(text: &str) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut rest = text;
    let mut plain = String::new();
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        let name = after.find(']').map(|close| &after[..close]).filter(|n| {
            let ext = n.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
            is_terminal_image(n) || !n.is_empty()
                && n.len() <= 160
                && !n.contains(['[', '\n'])
                && n.trim() == *n
                && (1..=8).contains(&ext.len())
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
        });
        match name {
            Some(name) => {
                plain.push_str(&rest[..open]);
                if !plain.is_empty() {
                    out.push(Segment::Text(std::mem::take(&mut plain)));
                }
                out.push(Segment::Chip(name.to_owned()));
                rest = &after[name.len() + 1..];
            }
            None => {
                plain.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    plain.push_str(rest);
    if !plain.is_empty() {
        out.push(Segment::Text(plain));
    }
    out
}

/// Paragraphs of `text`, a line each.
fn paragraphs(schema: &Schema, text: &str) -> Fragment {
    let ty = schema.node_type("paragraph").expect("paragraph");
    let blocks: Vec<Node> = text
        .split('\n')
        .map(|line| {
            let content = if line.is_empty() { Fragment::empty() } else { Fragment::from(vec![schema.text(line, vec![])]) };
            ty.create(None, content, vec![]).expect("a paragraph")
        })
        .collect();
    Fragment::from(blocks)
}

/// A document of plain `text` (a draft kept as text).
pub(crate) fn doc_from_text(schema: &Schema, text: &str) -> Node {
    schema.top_node_type().create(None, paragraphs(schema, text), vec![]).expect("a document")
}

thread_local! {
    /// The composers' editors, by composer key (`"form"`, `"agent:<id>"`).
    static SESSIONS: RefCell<HashMap<String, EditorSession>> = RefCell::new(HashMap::new());
}

/// The composer `key`'s editor, made with `text` the first time.
pub(crate) fn session(key: &str, text: impl FnOnce() -> String) -> EditorSession {
    if let Some(s) = existing(key) {
        return s;
    }
    let s = new_session(key, &text());
    SESSIONS.with_borrow_mut(|m| m.insert(key.to_owned(), s.clone()));
    s
}

/// The composer `key`'s editor, if it was made.
pub(crate) fn existing(key: &str) -> Option<EditorSession> {
    SESSIONS.with_borrow(|m| m.get(key).cloned())
}

/// The composer kind a key names (`"form"`, `"agent"`): what Enter submits.
pub(crate) fn target_of(key: &str) -> &str {
    key.split(':').next().unwrap_or(key)
}

thread_local! {
    /// What Enter submits, per composer kind (the composer on screen
    /// registers it).
    static SUBMITS: RefCell<HashMap<String, std::rc::Rc<dyn Fn()>>> = RefCell::new(HashMap::new());
}

/// Makes `submit` what Enter does in composers of `target` while it is
/// mounted.
pub(crate) fn register_submit(target: &'static str, submit: impl Fn() + 'static) {
    let f: std::rc::Rc<dyn Fn()> = std::rc::Rc::new(submit);
    SUBMITS.with_borrow_mut(|m| m.insert(target.to_owned(), f.clone()));
    cleanup(move || {
        SUBMITS.with_borrow_mut(|m| {
            if m.get(target).is_some_and(|g| std::rc::Rc::ptr_eq(g, &f)) {
                m.remove(target);
            }
        })
    });
}

thread_local! {
    /// The app's shortcuts (`nsh/keymap` syntax) for the editor to run
    /// ([`set_app_keys`]).
    static APP_KEYS: RefCell<Vec<(String, std::rc::Rc<dyn Fn()>)>> = const { RefCell::new(Vec::new()) };
}

/// The app's shortcuts, which a focused editor would otherwise swallow:
/// NSH's `RichEditor` binds every Command and Control key while it has the
/// keyboard and drops those its keymap has no command for. Set before the
/// composers' sessions are made.
pub(crate) fn set_app_keys(keys: Vec<(String, std::rc::Rc<dyn Fn()>)>) {
    APP_KEYS.with_borrow_mut(|k| *k = keys);
}

/// `shift+mod+t` (`nsh/keymap`) as the editor's keymap names it:
/// `Shift-Mod-t`.
fn editor_key_name(keystroke: &str) -> String {
    keystroke
        .split('+')
        .map(|part| match part {
            "mod" => "Mod",
            "ctrl" => "Ctrl",
            "shift" => "Shift",
            "alt" => "Alt",
            "cmd" => "Meta",
            key => key,
        })
        .collect::<Vec<_>>()
        .join("-")
}

fn new_session(key: &str, text: &str) -> EditorSession {
    let schema = schema();
    let mut bindings = kit::history_keys();
    bindings.extend(kit::hard_break_keys(&schema, false));
    if !MOBILE {
        // Enter sends, Shift+Enter breaks the line (bound above). The
        // keymap runs inside the editor's key handling, so the send waits
        // for the next turn rather than editing the session under it.
        let target = target_of(key).to_owned();
        bindings.push((
            "Enter".into(),
            command(move |_, dispatch| {
                if dispatch.is_some() {
                    let target = target.clone();
                    set_timeout(Duration::ZERO, move || {
                        if let Some(submit) = SUBMITS.with_borrow(|m| m.get(&target).cloned()) {
                            submit();
                        }
                    });
                }
                true
            }),
        ));
    }
    // Escape in an agent's composer: stops the agent, then takes the
    // queued messages back (`store::escape_composer`).
    if key.starts_with("agent:") {
        bindings.push((
            "Escape".into(),
            command(move |_, dispatch| {
                if dispatch.is_some() {
                    set_timeout(Duration::ZERO, crate::store::escape_composer);
                }
                true
            }),
        ));
    }
    // The app's shortcuts, run after the key as the send is.
    // Commands are `Send`: each holds its shortcut's place in the list.
    let names: Vec<String> = APP_KEYS.with_borrow(|k| k.iter().map(|(name, _)| editor_key_name(name)).collect());
    for (i, name) in names.into_iter().enumerate() {
        bindings.push((
            name,
            command(move |_, dispatch| {
                if dispatch.is_some() {
                    set_timeout(Duration::ZERO, move || {
                        if let Some(action) = APP_KEYS.with_borrow(|k| k.get(i).map(|(_, a)| a.clone())) {
                            action();
                        }
                    });
                }
                true
            }),
        ));
    }
    let doc = doc_from_text(&schema, text);
    let plugins = vec![keymap(bindings), history(HistoryOptions::default())];
    // The caret at the end, where one continues a draft.
    let end = Selection::at_end(&doc);
    let state = EditorState::create(EditorStateConfig::new(schema).doc(doc).selection(end).plugins(plugins))
        .expect("the composer's state");
    EditorSession::new(state, crate::ui::shared::chips::node_views(key))
}

fn dispatch(session: &EditorSession, tr: Transaction) {
    if let Err(e) = session.dispatch(tr) {
        log::warn!("composer: an edit was refused: {e}");
    }
}

/// Replaces the document with plain `text`, the caret at its end.
pub(crate) fn set_text(session: &EditorSession, text: &str) {
    let state = session.state();
    let mut tr = state.tr();
    let size = state.doc().content().size();
    if tr.replace_with(0, size, paragraphs(state.schema(), text)).is_err() {
        return;
    }
    let end = Selection::at_end(tr.doc());
    tr.set_selection(end);
    dispatch(session, tr);
}

/// Replaces the document with the attachment chips among `keep` (in their
/// order), text gone: what stays after a message was queued, whose
/// attachments go with the message that is sent next.
pub(crate) fn keep_only_chips(session: &EditorSession, keep: &[String]) {
    let state = session.state();
    let mut chips = vec![];
    state.doc().descendants(&mut |n, _, _, _| {
        if let Some(id) = attachment_id(n)
            && keep.iter().any(|k| k == id)
        {
            chips.push(n.clone());
        }
        true
    });
    let schema = state.schema();
    let Ok(paragraph) = schema.node_type("paragraph").expect("paragraph").create(None, Fragment::from(chips), vec![]) else {
        return;
    };
    let mut tr = state.tr();
    if tr.replace_with(0, state.doc().content().size(), Fragment::from(vec![paragraph])).is_err() {
        return;
    }
    let end = Selection::at_end(tr.doc());
    tr.set_selection(end);
    dispatch(session, tr);
}

/// Inserts attachment chips at the selection, the caret after them.
pub(crate) fn insert_chips(session: &EditorSession, chips: &[Chip]) {
    if chips.is_empty() {
        return;
    }
    let state = session.state();
    let Some(ty) = state.schema().node_type(ATTACHMENT) else { return };
    let mut tr = state.tr();
    for c in chips {
        let mut attrs = Attrs::new();
        attrs.insert("id".into(), Value::from(c.id.clone()));
        attrs.insert("kind".into(), Value::from(c.kind.as_str()));
        attrs.insert("name".into(), Value::from(c.name.clone()));
        attrs.insert("mime".into(), Value::from(c.mime.clone()));
        let Ok(node) = ty.create(Some(attrs), Fragment::empty(), vec![]) else { continue };
        tr.replace_selection_with(node, false);
    }
    tr.scroll_into_view();
    dispatch(session, tr);
}

/// Replaces `from..to` (an `@query`) with a mention chip of `path` and a
/// space, the caret after it.
pub(crate) fn insert_mention(session: &EditorSession, from: usize, to: usize, path: &str, dir: bool) {
    let state = session.state();
    let schema = state.schema();
    let Some(ty) = schema.node_type(MENTION) else { return };
    let mut attrs = Attrs::new();
    attrs.insert("path".into(), Value::from(path));
    attrs.insert("dir".into(), Value::Bool(dir));
    let Ok(node) = ty.create(Some(attrs), Fragment::empty(), vec![]) else { return };
    let mut tr = state.tr();
    if tr.replace_with(from, to, Fragment::from(vec![node, schema.text(" ", vec![])])).is_err() {
        return;
    }
    let caret = TextSelection::create(tr.doc(), from + 2, None);
    tr.set_selection(caret);
    tr.scroll_into_view();
    dispatch(session, tr);
}

/// Replaces `from..to` with `text`, the caret after it.
pub(crate) fn replace_text(session: &EditorSession, from: usize, to: usize, text: &str) {
    let state = session.state();
    let size = state.doc().content().size();
    let (from, to) = (from.min(size), to.min(size));
    let mut tr = state.tr();
    tr.insert_text(text, Some(from), Some(to));
    tr.scroll_into_view();
    dispatch(session, tr);
}

/// Deletes the attachment chips whose ids `remove` says to, as one edit.
pub(crate) fn remove_chips(session: &EditorSession, remove: impl Fn(&str) -> bool) {
    let state = session.state();
    let mut at = vec![];
    state.doc().descendants(&mut |n, pos, _, _| {
        if let Some(id) = attachment_id(n)
            && remove(id)
        {
            at.push(pos);
        }
        true
    });
    if at.is_empty() {
        return;
    }
    let mut tr = state.tr();
    // From the end, so earlier positions stay where they are.
    for pos in at.into_iter().rev() {
        let _ = tr.delete(pos, pos + 1);
    }
    dispatch(session, tr);
}

/// The text before a caret, from the document's start (blocks separated
/// by newlines, chips and line breaks as U+FFFC), and the caret's position;
/// none for a selection.
pub(crate) fn before_caret(state: &EditorState) -> Option<(String, usize)> {
    let sel = state.selection();
    if !sel.empty() {
        return None;
    }
    let head = sel.head();
    Some((state.doc().text_between(0, head, Some("\n"), Some(OBJECT)), head))
}

/// What an inline leaf reads as in [`before_caret`]'s text.
pub(crate) const OBJECT: &str = "\u{FFFC}";

/// The ids of a chips' document's attachments, for a test and the store.
#[cfg(test)]
fn sample_schema_doc(schema: &Schema) -> Node {
    schema
        .node_from_json(&serde_json::json!({
            "type": "doc",
            "content": [
                {"type": "paragraph", "content": [
                    {"type": "text", "text": "Look at "},
                    {"type": "mention", "attrs": {"path": "src/main.rs", "dir": false}},
                    {"type": "text", "text": " and "},
                    {"type": "attachment", "attrs": {"id": "img-1", "kind": "image", "name": "shot.png", "mime": "image/png"}},
                    {"type": "hardBreak"},
                    {"type": "text", "text": "then"},
                ]},
                {"type": "paragraph", "content": [
                    {"type": "attachment", "attrs": {"id": "file-2", "kind": "file", "name": "a.pdf", "mime": "application/pdf"}},
                    {"type": "text", "text": "this"},
                ]},
            ],
        }))
        .expect("a valid document")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_shortcuts_are_named_as_the_editor_keymap_names_keys() {
        assert_eq!(editor_key_name("mod+k"), "Mod-k");
        assert_eq!(editor_key_name("shift+ctrl+`"), "Shift-Ctrl-`");
        assert_eq!(editor_key_name("mod+,"), "Mod-,");
    }

    #[test]
    fn a_document_sends_its_text_mentions_and_attachments_in_order() {
        let schema = schema();
        let doc = sample_schema_doc(&schema);
        assert_eq!(
            outgoing(&doc),
            Outgoing {
                text: "Look at @src/main.rs and \nthen\nthis".into(),
                attachments: vec!["img-1".into(), "file-2".into()],
                message: "Look at @src/main.rs and [shot.png]\nthen\n[a.pdf]this".into(),
            }
        );
    }

    #[test]
    fn a_sent_message_reads_back_as_text_and_chips() {
        use Segment::{Chip, Text};
        assert_eq!(
            segments("Look at [shot.png] and [林晗 简历.pdf]this"),
            [Text("Look at ".into()), Chip("shot.png".into()), Text(" and ".into()), Chip("林晗 简历.pdf".into()), Text("this".into())]
        );
        // Brackets that name no file stay text: code, a list, a link.
        assert_eq!(segments("a[0] = [1, 2] and [x](y.md)"), [Text("a[0] = [1, 2] and [x](y.md)".into())]);
        assert_eq!(segments("[image.png]"), [Chip("image.png".into())]);
        assert_eq!(segments("plain"), [Text("plain".into())]);
        assert!(is_image_name("Shot.PNG") && !is_image_name("a.pdf"));
    }

    #[test]
    fn plain_text_round_trips_through_a_document() {
        let schema = schema();
        let text = "first line\n\nthird";
        assert_eq!(outgoing(&doc_from_text(&schema, text)).text, text);
        assert!(outgoing(&doc_from_text(&schema, "")).is_empty());
    }

    #[test]
    fn the_text_before_the_caret_reads_chips_as_objects() {
        let schema = schema();
        let doc = sample_schema_doc(&schema);
        let mut state = EditorState::create(EditorStateConfig::new(schema).doc(doc)).unwrap();
        // After "then": 1 + "Look at " (8) + mention + " and " (5) + chip + break + "then" (4).
        let mut tr = state.tr();
        let at = TextSelection::create(tr.doc(), 1 + 8 + 1 + 5 + 1 + 1 + 4, None);
        tr.set_selection(at);
        state = state.apply(tr).unwrap();
        let (text, head) = before_caret(&state).unwrap();
        assert_eq!(text, format!("Look at {OBJECT} and {OBJECT}{OBJECT}then"));
        assert_eq!(head, 21);
    }
}
