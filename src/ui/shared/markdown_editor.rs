//! Instructions written as they read. An automation's prompt is
//! Markdown; here it is edited in place, as Notion's pages are: a heading
//! is a heading while it is typed (`## ` at a line's start makes one, `- `
//! a list, a backtick pair a piece of code), and there is no other view
//! of it to switch to. What is kept is still Markdown: the document is
//! read from it when the editor opens and written back on each change.
//!
//! NSH's `RichEditor` with its starter kit (Tiptap's): the kit's input
//! rules are what turn the typed marks into structure.

use std::sync::Arc;

use nsh_base::rich_editor::{
    extensions::{ExtensionManager, kit::starter_kit},
    formats::{DocFormat, MarkdownFormat},
    model::Node,
    state::{EditorState, EditorStateConfig},
    view::{EditorSession, NodeViews, RichEditorClasses, RichEditorStyles, rich_editor},
};
use nsh_base::text_view::PartStyle;
use nsh_core::{element::StyleRefinement, prelude::*};

use crate::ui::kit::*;

thread_local! {
    /// The kit, resolved once: its schema and its plugins are the same
    /// for every editor.
    static KIT: Arc<ExtensionManager> = Arc::new(ExtensionManager::with_core(vec![starter_kit()]).expect("the starter kit"));
}

/// `markdown` as a document of the kit's schema. What cannot be read as
/// Markdown is a document of its lines, each a paragraph: nothing typed
/// is lost to a syntax error.
fn doc_of(kit: &ExtensionManager, markdown: &str) -> Node {
    let schema = kit.schema();
    MarkdownFormat::default()
        .parse(schema, markdown)
        .unwrap_or_else(|_| {
            let paragraph = schema.node_type("paragraph").expect("the kit's paragraph");
            let lines: Vec<Node> = markdown
                .lines()
                .filter_map(|line| {
                    paragraph
                        .create(
                            None,
                            if line.is_empty() {
                                vec![]
                            } else {
                                vec![schema.text(line, vec![])]
                            }
                            .into(),
                            vec![],
                        )
                        .ok()
                })
                .collect();
            schema
                .top_node_type()
                .create(None, lines.into(), vec![])
                .expect("a document")
        })
}

/// `doc` as Markdown, as it is kept.
fn markdown_of(doc: &Node) -> String {
    MarkdownFormat::default().serialize(doc).unwrap_or_default()
}

/// The editor's own parts: the app's type, its caret and its selection.
fn styles() -> RichEditorStyles {
    let mut styles = RichEditorStyles::default();
    styles.root = Some(PartStyle(
        StyleRefinement::default()
            .text_size(tx(TEXT_BASE))
            .line_height(px(24.0))
            .text_color(FG)
            .into(),
    ));
    styles.caret = Some(PartStyle(StyleRefinement::default().bg(ACCENT).into()));
    styles.selection = Some(PartStyle(
        StyleRefinement::default().bg(tint(ACCENT, 0.3)).into(),
    ));
    styles.placeholder = Some(PartStyle(
        StyleRefinement::default().text_color(FG_FAINT).into(),
    ));
    // What an input method is composing: a light wash, as the composer's.
    styles.composition = Some(PartStyle(
        StyleRefinement::default()
            .no_underline()
            .bg(tint(ACCENT, 0.14))
            .into(),
    ));
    styles
}

/// How each kind of block and mark looks: a heading larger and heavier,
/// a list indented behind its markers, code in the code font on a tinted
/// ground. Classes, not typed styles: the editor merges a node's styles
/// by their declarations, and a typed one has none to merge.
fn classes() -> RichEditorClasses {
    let named = |list: &[(&str, &str)]| {
        Some(
            list.iter()
                .map(|(name, class)| ((*name).to_owned(), (*class).to_owned()))
                .collect(),
        )
    };
    let mut classes = RichEditorClasses::default();
    classes.nodes = named(&[
        ("heading", "font-semibold text-foreground"),
        (
            "heading[level=1]",
            "text-[22px] leading-[32px] mt-[14px] mb-[4px]",
        ),
        (
            "heading[level=2]",
            "text-[18px] leading-[28px] mt-[12px] mb-[2px]",
        ),
        ("heading[level=3]", "text-[16px] leading-[26px] mt-[8px]"),
        ("bulletList", "pl-[22px]"),
        ("orderedList", "pl-[22px]"),
        (
            "codeBlock",
            "font-mono text-[12px] leading-[20px] bg-muted rounded-lg px-[12px] py-[8px] my-[4px]",
        ),
        (
            "blockquote",
            "border-l-2 border-border pl-[12px] text-muted-foreground",
        ),
        ("horizontalRule", "my-[8px]"),
    ]);
    classes.marks = named(&[
        ("bold", "font-semibold"),
        ("italic", "italic"),
        ("strike", "line-through"),
        ("code", "font-mono text-[12px] bg-muted"),
        ("link", "text-primary underline"),
    ]);
    classes.list_marker = Some("text-muted-foreground".into());
    classes
}

/// The editor of `value`, as tall as what is written between `least` and
/// `most`, and scrolling past that: `value` is read once, when it is made,
/// and follows every change after.
pub(crate) fn markdown_editor(
    value: Signal<String>,
    placeholder: &'static str,
    least: f32,
    most: f32,
) -> impl IntoElement {
    let kit = KIT.with(Arc::clone);
    let doc = doc_of(&kit, &value.get_untracked());
    let state = EditorState::create(
        EditorStateConfig::new(kit.schema().clone())
            .doc(doc)
            .plugins(kit.plugins()),
    )
    .expect("the editor's state");
    let session = EditorSession::new(state, NodeViews::new());
    let read = session.clone();
    let mut el = rich_editor()
        .editor(session)
        .styles(styles())
        .classes(classes())
        .accessibility_label(SharedString::from(crate::i18n::t("Instructions")))
        .on_change(move |_: &_| {
            let written = markdown_of(read.state().doc());
            if value.with_untracked(|kept| *kept != written) {
                value.set(written);
            }
        })
        .w_full()
        // A row, as the composer's: its document is then as tall as what
        // is written, and the editor with it up to `most`.
        .flex_row()
        .min_h(px(least))
        .max_h(px(most))
        .px(sp(S4))
        .py(sp(S3));
    if !placeholder.is_empty() {
        el = el.placeholder(SharedString::from(placeholder));
    }
    el
}

#[cfg(test)]
mod tests {
    use super::{KIT, doc_of, markdown_of};
    use std::sync::Arc;

    /// Read and written back, a prompt is what it was: nothing an agent
    /// reads is changed by having been opened in the editor.
    fn round_trip(markdown: &str) -> String {
        let kit = KIT.with(Arc::clone);
        markdown_of(&doc_of(&kit, markdown))
    }

    #[test]
    fn what_a_prompt_names_comes_back_as_it_was() {
        let written = round_trip(
            "有人请求我 review：{{url}}\n\n## 步骤\n\n1. 了解 PR：`gh pr view {{url}} --json title,body`\n   - 已合并：不 review。\n2. 读 `git show FETCH_HEAD:<路径>`，结论写「没有问题」。\n\n- 不要 push_force，也不要改 a_b_c。",
        );
        for kept in [
            "{{url}}",
            "## 步骤",
            "`gh pr view {{url}} --json title,body`",
            "`git show FETCH_HEAD:<路径>`",
            "1. 了解 PR",
            "- 已合并",
        ] {
            assert!(written.contains(kept), "{kept} in:\n{written}");
        }
        // A second pass changes nothing more: what was written is stable.
        assert_eq!(round_trip(&written), written);
    }

    #[test]
    fn the_shipped_prompt_survives_the_editor() {
        let prompt = include_str!("../../../plugins/github-pr-watch/automations/review/prompt.md");
        let written = round_trip(prompt);
        // Every command and placeholder an agent acts on is there, letter for letter.
        let code: Vec<&str> = prompt.split('`').skip(1).step_by(2).collect();
        assert!(code.len() > 10);
        for span in code {
            assert!(
                written.contains(&format!("`{span}`")),
                "`{span}` in:\n{written}"
            );
        }
        for name in [
            "{{url}}",
            "{{repo}}",
            "{{number}}",
            "{{author}}",
            "{{rule}}",
        ] {
            assert_eq!(
                written.matches(name).count(),
                prompt.matches(name).count(),
                "{name} in:\n{written}"
            );
        }
        assert_eq!(round_trip(&written), written);
    }
}
