//! An agent's conversation: user bubbles, Markdown replies, tool calls,
//! thinking, todos, errors; pending permissions above the composer.

use crate::i18n::{t, tf};
use nsh_base::{MarkdownOptions, text_view};
use nsh_core::{
    element::{AdaptiveColor, FontWeight, Position, View},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::{
    daemon::{
        Item, Permission,
        model::{Question, plan_of, questions_of},
    },
    fold::{Group, Row},
    store,
};

#[derive(Clone, Copy, PartialEq)]
enum RowKind {
    User,
    Assistant,
    Reasoning,
    Tool,
    /// A run of tool calls, summed up.
    Group,
    Todo,
    Error,
    Notice,
}

impl RowKind {
    fn of(item: &Item) -> Self {
        match item {
            Item::User { .. } => Self::User,
            Item::Assistant { .. } => Self::Assistant,
            Item::Reasoning { .. } => Self::Reasoning,
            Item::Tool { .. } => Self::Tool,
            Item::Todo { .. } => Self::Todo,
            Item::Error { .. } => Self::Error,
            Item::Notice { .. } => Self::Notice,
        }
    }
}

pub(crate) fn timeline() -> impl IntoElement {
    let loaded = computed(store::timeline_loaded);
    let running = computed(|| store::selected_agent().is_some_and(|a| a.is_running()));
    view().flex_1().child(dynamic(move || {
        let ready = loaded.get();
        Some(untracked(move || {
            if !ready {
                return view()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .pb(px(96.0))
                    .child(loader(None))
                    .into_element();
            }
            // Rows, not items: a run of tool calls is one row while folded.
            let rows = computed(store::display_rows);
            let count = rows.with_untracked(Vec::len);
            let list = element_ref();
            store::set_timeline_list(list.clone());
            view().flex_1().child(virtual_list(move || rows.with(Vec::len), move |at: Position| row(at, rows, running))
                // On the phone, a drag down the conversation takes the
                // keyboard with it, as Messages does.
                .keyboard_dismiss_mode(if MOBILE { KeyboardDismissMode::Interactive } else { KeyboardDismissMode::None })
                .element_ref(&list)
                .keys(move || rows.with(|r| store::row_keys(r)))
                .estimated_item_size(px(80.0))
                .initial_scroll_index(count.saturating_sub(1))
                .follow_end(px(40.0))
                .content_padding_top(px(S4))
                // The phone's composer floats over the end; the desktop's
                // sits below the list.
                .content_padding_bottom(px(if MOBILE { 200.0 } else { S4 }))
                .footer(thinking(running))
                // Earlier pages come as the conversation nears its start.
                .on_load_more_at_start(store::load_older)
                .load_more_at_start_threshold(1.0)
                .flex_1())
                // Earlier pages load by themselves as the start nears: a
                // small loader says so while one is on its way, no button.
                .child(when(store::loading_older, || {
                    view().absolute().top(px(S2)).left(px(0.0)).right(px(0.0)).items_center().child(eclipse(24.0))
                }))
                .when(!MOBILE, |v| v.child(turn_rail()))
                .into_element()
        }))
    }))
}

/// The conversation's turns down its left edge, as T3 Code's: a tick per
/// message of the user's (longer for a longer one); under the pointer, a
/// card with the message and how the reply begins; a press goes to it.
fn turn_rail() -> impl IntoElement {
    let turns = computed(store::turns);
    view()
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .left(px(S1))
        .justify_center()
        .pointer_events(PointerEvents::BoxNone)
        .child(dynamic(move || {
            let list = turns.get();
            // One message needs no map.
            if list.len() < 2 {
                return None;
            }
            Some(untracked(move || {
                // The tick under the pointer, whose card shows beside it.
                let over = signal(None::<usize>);
                let mut rail = view().items_start();
                // The latest turns: a rail longer than the window helps no one.
                let skip = list.len().saturating_sub(60);
                for (row, message, reply) in list.into_iter().skip(skip) {
                    let width = 8.0 + (message.chars().count().min(40) as f32) * 0.35;
                    let card = glass_strong(view())
                        .absolute()
                        .left(px(30.0))
                        .top(px(-14.0))
                        .w(px(320.0))
                        .p(px(S3))
                        .gap(px(S1))
                        .rounded(px(R_XL))
                        .shadow_lg()
                        .pointer_events(PointerEvents::None)
                        .child(text(message).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)).text_color(FG).number_of_lines(1))
                        .when(!reply.is_empty(), |v| {
                            v.child(text(reply).text_size(px(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED).number_of_lines(3))
                        })
                        .opacity(move || if over.get() == Some(row) { 1.0_f32 } else { 0.0 })
                        .transition_opacity();
                    rail = rail.child(
                        view()
                            .on_mouse_enter(move |_: &MouseEvent| over.set(Some(row)))
                            .on_mouse_leave(move |_: &MouseEvent| {
                                if over.get_untracked() == Some(row) {
                                    over.set(None);
                                }
                            })
                            .child(
                                nsh_base::button()
                                    .on_press(move || store::scroll_timeline_to(row))
                                    .h(px(9.0))
                                    .w(px(26.0))
                                    .justify_center()
                                    .child(
                                        view()
                                            .h(px(2.0))
                                            .w(px(width))
                                            .rounded(px(1.0))
                                            .bg(move || if over.get() == Some(row) { FG } else { tint(FG, 0.22) })
                                            .transition_colors(),
                                    ),
                            )
                            .child(card),
                    );
                }
                rail.into_element()
            }))
        }))
}

fn row(at: Position, rows: Memo<Vec<Row>>, running: Memo<bool>) -> impl IntoElement {
    let shown = computed(move || rows.with(|r| r.get(at.index()).cloned()));
    // The item's index, for a row that shows one.
    let index = computed(move || match shown.get() {
        Some(Row::Item { index, .. }) => Some(index),
        _ => None,
    });
    let grouped = computed(move || matches!(shown.get(), Some(Row::Item { grouped: true, .. })));
    let group = computed(move || match shown.get() {
        Some(Row::Group(g)) => Some(g),
        _ => None,
    });
    let item = computed(move || index.get().and_then(store::item_at));
    let kind = computed(move || {
        if group.with(Option::is_some) {
            return Some(RowKind::Group);
        }
        item.with(|i| i.as_ref().map(RowKind::of))
    });
    let streaming = computed(move || running.get() && index.get().is_some_and(|i| i + 1 == store::item_count()));
    let text_of = move || {
        item.with(|i| match i {
            Some(Item::User { text, .. } | Item::Assistant { text } | Item::Reasoning { text }) => text.clone(),
            Some(Item::Error { message } | Item::Notice { message }) => message.clone(),
            _ => String::new(),
        })
    };
    let hit = computed(move || {
        let f = store::find();
        if !f.open || f.query.trim().is_empty() {
            return 0u8;
        }
        let Some(index) = index.get() else { return 0 };
        let m = store::find_matches();
        match m.iter().position(|&i| i == index) {
            Some(p) if p == f.current => 2,
            Some(_) => 1,
            None => 0,
        }
    });
    view()
        .px(px(S6))
        .py(px(6.0))
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH + 2.0 * S6))
        .self_center()
        .rounded(px(R_LG))
        .bg(move || match hit.get() {
            2 => tint(ACCENT, 0.14),
            1 => tint(DOT_WARNING, 0.10),
            _ => tint(ACCENT, 0.0),
        })
        .child(dynamic(
        move || {
            let kind = kind.get()?;
            Some(untracked(move || match kind {
                // The phone: no button on the bubble; a tap opens its
                // actions, copying among them.
                RowKind::User if MOBILE => user_row(item, nsh_base::button()
                    .px(px(14.0))
                    .py(px(9.0))
                    .rounded(px(18.0))
                    .bg(tint(ACCENT, 0.10))
                    .active(|s| s.opacity(0.7))
                    .on_press(move || message_sheet(item))
                    .accessibility_label(t("Message actions"))
                    .child(user_text(item, false)))
                    .into_element(),
                // The desktop: its actions show while the pointer is on it.
                RowKind::User => {
                    let hover = signal(false);
                    user_row(item, view()
                    .on_mouse_enter(move |_: &MouseEvent| hover.set(true))
                    .on_mouse_leave(move |_: &MouseEvent| hover.set(false))
                    .child(user_actions(item).opacity(move || if hover.get() { 1.0_f32 } else { 0.0 }).transition_opacity())
                    .px(px(14.0))
                    .py(px(9.0))
                    .rounded(px(18.0))
                    .bg(tint(ACCENT, 0.10))
                    .child(user_text(item, true)))
                    .into_element()
                }
                RowKind::Assistant => text_view()
                    // While searching, the words found are marked in the
                    // text itself, not only the message they are in.
                    .markdown(move || {
                        let body = text_of();
                        let f = store::find();
                        Some(SharedString::from(if f.open && hit.get() > 0 { mark_matches(&body, &f.query) } else { body }))
                    })
                    .options(move || MarkdownOptions { streaming: streaming.get(), ..MarkdownOptions::default() })
                    .selectable(true)
                    .gap_3()
                    .text_size(px(TEXT_CONTENT))
                    .line_height(px(24.0))
                    .text_color(FG)
                    .into_element(),
                RowKind::Reasoning => view()
                    .flex_row()
                    .gap(px(S2))
                    .child(icon(Icon::Brain, 15.0).text_color(FG_FAINT).mt(px(2.0)))
                    .child(
                        text(move || text_of().trim().to_owned())
                            .text_size(px(13.0))
                            .line_height(px(19.0))
                            .italic()
                            .number_of_lines(3)
                            .text_color(FG_MUTED)
                            .flex_1(),
                    )
                    .into_element(),
                // A call under an open group sits indented under its
                // summary, past the summary's chevron.
                RowKind::Tool => view().pl(move || px(if grouped.get() { S4 } else { 0.0 })).child(tool_row(item)).into_element(),
                RowKind::Group => group_row(group).into_element(),
                RowKind::Todo => todo_card(item).into_element(),
                RowKind::Error => view()
                    .flex_row()
                    .gap(px(S2))
                    .px(px(S3))
                    .py(px(S2))
                    .rounded(px(R_LG))
                    .bg(tint(DANGER, 0.08))
                    .child(icon(Icon::CircleAlert, 16.0).text_color(DANGER))
                    .child(
                        text(text_of)
                            .text_size(px(TEXT_BASE))
                            .text_color(DANGER)
                            .selectable(true)
                            .flex_1(),
                    )
                    .into_element(),
                RowKind::Notice => text(text_of)
                    .text_size(px(TEXT_SM))
                    .text_color(FG_FAINT)
                    .text_center()
                    .into_element(),
            }))
        },
    ))
}

/// A user message on the right: the images it carried as thumbnails over
/// its bubble (no bubble when it was images alone).
/// `markdown` with each run of `query` (any case) inside `<mark>`, which
/// the text view shows highlighted. Code (fenced or inline), a link's
/// address and HTML tags are left alone: marks there would show as text or
/// break the link.
pub(crate) fn mark_matches(markdown: &str, query: &str) -> String {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return markdown.to_owned();
    }
    let mut out = String::with_capacity(markdown.len() + 32);
    let mut fenced = false;
    for line in markdown.split_inclusive('\n') {
        let fence = line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~");
        if fence {
            fenced = !fenced;
        }
        let lower = line.to_lowercase();
        // Lowercasing that moves bytes (rare) would misplace the marks.
        if fenced || fence || lower.len() != line.len() {
            out.push_str(line);
            continue;
        }
        // Where marks may not go: `code`, `](address)`, `<tag>`.
        let mut closed = vec![false; line.len()];
        let bytes = line.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let end = match bytes[i] {
                b'`' => line[i + 1..].find('`').map(|e| i + 1 + e + 1),
                b']' if bytes.get(i + 1) == Some(&b'(') => line[i..].find(')').map(|e| i + e + 1),
                b'<' => line[i..].find('>').map(|e| i + e + 1),
                _ => None,
            };
            match end {
                Some(end) => {
                    closed[i..end].iter_mut().for_each(|c| *c = true);
                    i = end;
                }
                None => i += 1,
            }
        }
        let mut at = 0;
        while let Some(found) = lower[at..].find(&needle) {
            let (start, end) = (at + found, at + found + needle.len());
            if closed[start..end].iter().any(|c| *c) || !line.is_char_boundary(start) || !line.is_char_boundary(end) {
                out.push_str(&line[at..end.min(line.len())]);
            } else {
                out.push_str(&line[at..start]);
                out.push_str("<mark>");
                out.push_str(&line[start..end]);
                out.push_str("</mark>");
            }
            at = end;
        }
        out.push_str(&line[at..]);
    }
    out
}

/// A sent message's text with its attachments' chips where they stood in
/// the composer (`[name]` in the text, `composer_doc::segments`): a
/// picture's chip carries its thumbnail and shows it large under the
/// pointer; a message from the host's history has the names alone.
fn user_text(item: Memo<Option<Item>>, selectable: bool) -> impl IntoElement {
    use crate::ui::shared::composer_doc::{Segment, is_image_name, segments};
    view().child(dynamic(move || {
        let (body, images) = item.with(|i| match i {
            Some(Item::User { text, images, .. }) => (text.clone(), images.clone()),
            _ => Default::default(),
        });
        let f = store::find();
        Some(untracked(move || {
            let mut line = text("").text_size(px(TEXT_CONTENT)).line_height(px(22.0)).text_color(FG).selectable(selectable);
            let mut pictures = images.into_iter();
            let needle = if f.open { f.query.trim().to_lowercase() } else { String::new() };
            for segment in segments(&body) {
                line = match segment {
                    // The words a search found, marked as the replies' are.
                    Segment::Text(t) if !needle.is_empty() && t.to_lowercase().len() == t.len() => {
                        let lower = t.to_lowercase();
                        let mut at = 0;
                        while let Some(found) = lower[at..].find(&needle) {
                            let (start, end) = (at + found, at + found + needle.len());
                            if !t.is_char_boundary(start) || !t.is_char_boundary(end) {
                                break;
                            }
                            line = line.child(text(t[at..start].to_owned())).child(text(t[start..end].to_owned()).bg(tint(DOT_WARNING, 0.35)));
                            at = end;
                        }
                        line.child(text(t[at..].to_owned()))
                    }
                    Segment::Text(t) => line.child(text(t)),
                    Segment::Chip(name) => {
                        let thumb = if is_image_name(&name) { pictures.next() } else { None };
                        line.child(sent_chip(name, thumb))
                    }
                };
            }
            line.into_element()
        }))
    }))
}

/// An attachment's chip in a sent message, as the composer's: its
/// thumbnail (a file's glyph) and its name.
fn sent_chip(name: String, thumb: Option<String>) -> View {
    let chip = view()
        .flex_row()
        .items_center()
        .gap(px(5.0))
        .h(px(22.0))
        .pl(px(4.0))
        .pr(px(7.0))
        .mx(px(2.0))
        .rounded(px(R_MD))
        .bg(SURFACE0)
        .border_1()
        .border_color(tint(ACCENT, 0.35))
        .align_middle();
    let chip = match &thumb {
        Some(uri) => chip.child(
            image(SharedString::from(uri.clone())).w(px(16.0)).h(px(16.0)).object_fit(nsh_core::ResizeMode::Cover).rounded(px(3.0)),
        ),
        None => chip.child(icon(if crate::ui::shared::composer_doc::is_image_name(&name) { Icon::Image } else { Icon::File }, 13.0).text_color(FG_MUTED)),
    };
    let chip_name = name.clone();
    let chip = chip.child(text(name).text_size(px(TEXT_SM)).text_color(FG).number_of_lines(1));
    // A picture's chip opens it large (and shows it under the pointer).
    match thumb {
        Some(uri) => {
            let (open_name, open_uri) = (chip_name, uri.clone());
            chip.hover(|s| s.border_color(ACCENT))
                .on_press(move |_: &PressEvent| store::open_modal(store::Modal::Picture { name: open_name.clone(), uri: open_uri.clone() }))
                .tooltip(
                    glass_strong(view()).p(px(S1)).rounded(px(R_LG)).child(
                        image(SharedString::from(uri)).w(px(280.0)).h(px(280.0)).object_fit(nsh_core::ResizeMode::Contain).rounded(px(R_MD)),
                    ),
                )
        }
        None => chip,
    }
}

fn user_row(item: Memo<Option<Item>>, bubble: impl IntoElement + 'static) -> impl IntoElement {
    let (images, has_text) = item.with_untracked(|i| match i {
        Some(Item::User { images, text, .. }) => {
            // The pictures the text names show as chips in it; the rest (a
            // message sent before chips were named) stay above it.
            let named = crate::ui::shared::composer_doc::segments(text)
                .iter()
                .filter(|s| matches!(s, crate::ui::shared::composer_doc::Segment::Chip(n) if crate::ui::shared::composer_doc::is_image_name(n)))
                .count();
            (images.iter().skip(named).cloned().collect::<Vec<_>>(), !text.trim().is_empty())
        }
        _ => (vec![], true),
    });
    let mut thumbs = view().flex_row().flex_wrap().justify_end().gap(px(6.0));
    for uri in &images {
        thumbs = thumbs.child(
            image(SharedString::from(uri.clone()))
                .w(px(132.0))
                .h(px(132.0))
                .object_fit(nsh_core::ResizeMode::Cover)
                .rounded(px(14.0))
                .border_1()
                .border_color(tint(BORDER, 0.8)),
        );
    }
    view()
        .self_end()
        .items_end()
        .gap(px(6.0))
        .max_w(relative(0.82))
        .when(!images.is_empty(), |v| v.child(thumbs))
        .when(has_text, |v| v.child(bubble))
}

fn tool_icon(kind: &str, name: &str) -> Icon {
    match kind {
        "shell" => Icon::Terminal,
        "read" => Icon::FileText,
        "edit" | "write" => Icon::Pencil,
        "search" => Icon::Search,
        "fetch" => Icon::Globe,
        "sub_agent" => Icon::Bot,
        "plan" => Icon::ListTodo,
        _ if name.to_lowercase().contains("todo") => Icon::ListTodo,
        _ => Icon::Wrench,
    }
}

/// A user message's actions on the phone: copy it, or rewind to it.
fn message_sheet(item: Memo<Option<Item>>) {
    let Some(Item::User { text: body, message_id, .. }) = item.get_untracked() else { return };
    let mut items = vec![SheetItem::new(t("Copy"), move || set_clipboard(body.clone())).icon(Icon::Copy)];
    if let Some(id) = message_id {
        let (a, b, c) = (id.clone(), id.clone(), id);
        items.push(SheetItem::new(t("Rewind conversation to here"), move || store::rewind(&a, "conversation")).icon(Icon::Undo2));
        items.push(SheetItem::new(t("Rewind files to here"), move || store::rewind(&b, "files")).icon(Icon::FileText));
        items.push(SheetItem::new(t("Rewind both to here"), move || store::rewind(&c, "both")).icon(Icon::RotateCw));
    }
    show_sheet(None, items);
}

/// Rewind from a user message (the message menu).
fn user_actions(item: Memo<Option<Item>>) -> View {
    let id = computed(move || match item.get() {
        Some(Item::User { message_id: Some(id), .. }) => Some(id),
        _ => None,
    });
    view().absolute().bottom(px(-12.0)).right(px(-4.0)).child(dynamic(move || {
        let id = id.get()?;
        Some(untracked(move || {
            let (a, b, c) = (id.clone(), id.clone(), id.clone());
            if MOBILE {
                // The phone: the system's action sheet.
                return nsh_base::button()
                    .on_press(move || {
                        let (a, b, c) = (a.clone(), b.clone(), c.clone());
                        show_sheet(
                            Some(t("Rewind to here").into()),
                            vec![
                                SheetItem::new(t("Conversation only"), move || store::rewind(&a, "conversation")).icon(Icon::Undo2),
                                SheetItem::new(t("Files only"), move || store::rewind(&b, "files")).icon(Icon::FileText),
                                SheetItem::new(t("Conversation and files"), move || store::rewind(&c, "both")).icon(Icon::RotateCw),
                            ],
                        )
                    })
                    .accessibility_label(t("Message actions"))
                    .w(px(32.0))
                    .h(px(32.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(R_FULL))
                    .bg(SURFACE0)
                    .border_1()
                    .border_color(BORDER)
                    .child(icon(Icon::Ellipsis, 14.0).text_color(FG_MUTED))
                    .into_element();
            }
            dropdown(
                view()
                    .w(px(22.0))
                    .h(px(22.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(R_FULL))
                    .bg(SURFACE0)
                    .border_1()
                    .border_color(BORDER)
                    .label(t("Message actions"))
                    .child(icon(Icon::Ellipsis, 13.0).text_color(FG_MUTED)),
            )
            .align(nsh_core::overlay::Align::End)
            .child(menu_heading(t("Rewind to here")))
            .child(menu_row(t("Conversation only"), Some(Icon::Undo2), None, false, move || store::rewind(&a, "conversation")))
            .child(menu_row(t("Files only"), Some(Icon::FileText), None, false, move || store::rewind(&b, "files")))
            .child(menu_row(t("Conversation and files"), Some(Icon::RotateCw), None, false, move || store::rewind(&c, "both")))
            .into_element()
        }))
    }))
}

/// A run of tool calls as one row: what they did ("Ran 3 commands,
/// edited 2 files"), failures and progress, and a chevron. Pressed, its
/// calls show under it, or fold away again.
fn group_row(group: Memo<Option<Group>>) -> impl IntoElement {
    let summary = computed(move || group.get().map(|g| store::group_summary(g.start, g.end)).unwrap_or_default());
    let open = computed(move || group.with(|g| g.as_ref().is_some_and(|g| g.open)));
    nsh_base::button()
        .on_press(move || {
            if let Some(g) = group.get_untracked() {
                store::set_group_open(&g.id, !g.open);
            }
        })
        .flex_row()
        .items_center()
        .gap(px(S2))
        .px(px(S2))
        .h(px(30.0))
        .rounded(px(R_LG))
        .hover(|s| s.bg(highlight()))
        .child(
            icon(Icon::ChevronRight, 13.0)
                .text_color(FG_FAINT)
                .rotate(move || Radians(if open.get() { std::f32::consts::FRAC_PI_2 } else { 0.0 }))
                .transition_transform(),
        )
        .child(
            text(move || summary.with(|s| s.describe()))
                .text_size(px(TEXT_BASE))
                .text_color(FG_MUTED)
                .number_of_lines(1)
                .flex_shrink(1.0),
        )
        // Failures show on the summary, so a folded group hides no error.
        .child(dynamic(move || {
            let failed = summary.with(|s| s.failures())?;
            Some(untracked(move || {
                text(format!("· {failed}")).text_size(px(TEXT_BASE)).text_color(DANGER).flex_shrink_0().into_element()
            }))
        }))
        .child(view().flex_1())
        .child(dynamic(move || {
            let (running, failed) = summary.with(|s| (s.running > 0, s.failed > 0));
            Some(untracked(move || {
                if running {
                    status_dot(|| Status::Running).into_element()
                } else if failed {
                    icon(Icon::X, 14.0).text_color(DANGER).into_element()
                } else {
                    icon(Icon::Check, 14.0).text_color(SUCCESS).into_element()
                }
            }))
        }))
}

/// Tool calls open under their rows, by call id: the list recycles its
/// rows, so what is open is kept here rather than in a row.
#[derive(Clone, Default)]
struct OpenTools(std::collections::HashSet<String>);

/// A tool call: its icon, name, summary and state. Pressed, on the desktop
/// it opens in place (its command and output, its diff, what it found),
/// the rows below gliding down; on the phone its detail comes up in a
/// sheet.
fn tool_row(item: Memo<Option<Item>>) -> impl IntoElement {
    let tool = computed(move || match item.get() {
        Some(Item::Tool { name, kind, status, summary, .. }) => (name, kind, status, summary),
        _ => Default::default(),
    });
    let call = computed(move || match item.get() {
        Some(Item::Tool { call_id, .. }) => call_id,
        _ => String::new(),
    });
    let open = computed(move || !MOBILE && call.with(|c| global::<OpenTools>().with(|o| o.0.contains(c))));
    let glyph = computed(move || tool.with(|(n, k, ..)| tool_icon(k, n)));
    let header = nsh_base::button()
        .on_press(move || {
            let Some(Item::Tool { call_id, name, kind, detail, error, .. }) = item.get_untracked() else { return };
            if !super::tool_detail::has_body(&detail, error.as_deref()) {
                return;
            }
            if MOBILE {
                store::open_modal(store::Modal::ToolDetail { name, kind, detail, error });
                return;
            }
            global::<OpenTools>().update(|o| {
                if !o.0.remove(&call_id) {
                    o.0.insert(call_id);
                }
            });
        })
        .flex_row()
        .items_center()
        .gap(px(S2))
        .px(px(S2))
        .h(px(30.0))
        .rounded(px(R_LG))
        .hover(|s| s.bg(highlight()))
        .child(dynamic(move || {
            let g = glyph.get();
            Some(untracked(move || icon(g, 15.0).text_color(FG_MUTED)))
        }))
        .child(
            text(move || tool.with(|(n, ..)| n.clone()))
                .text_size(px(TEXT_BASE))
                .font_weight(FontWeight(500))
                .text_color(FG)
                .flex_shrink_0(),
        )
        .child(
            text(move || tool.with(|(.., s)| s.clone()))
                .text_size(px(TEXT_CODE))
                .font_family(|| MONO)
                .text_color(FG_MUTED)
                .number_of_lines(1)
                .flex_1(),
        )
        .child(dynamic(move || {
            // A chevron that turns as the detail opens, where there is one
            // (the phone opens a sheet instead).
            let expandable = !MOBILE
                && item.with(|i| matches!(i, Some(Item::Tool { detail, error, .. }) if super::tool_detail::has_body(detail, error.as_deref())));
            expandable.then(|| {
                untracked(move || {
                    icon(Icon::ChevronRight, 13.0)
                        .text_color(FG_FAINT)
                        .rotate(move || Radians(if open.get() { std::f32::consts::FRAC_PI_2 } else { 0.0 }))
                        .transition_transform()
                        .into_element()
                })
            })
        }))
        .child(dynamic(move || {
            let status = tool.with(|(_, _, s, _)| s.clone());
            Some(untracked(move || match status.as_str() {
                "running" => status_dot(|| Status::Running).into_element(),
                "failed" => icon(Icon::X, 14.0).text_color(DANGER).into_element(),
                "canceled" => icon(Icon::CircleX, 14.0).text_color(FG_FAINT).into_element(),
                _ => icon(Icon::Check, 14.0).text_color(SUCCESS).into_element(),
            }))
        }));
    // The detail springs open under the row (the sidebar's reveal): its
    // height grows from nothing and what follows glides down with it.
    nsh_base::collapsible()
        .open(move || open.get())
        .reveal(true)
        .content(view().child(dynamic(move || {
            let Some(Item::Tool { kind, detail, error, .. }) = item.get() else { return None };
            // Indented under the row's name, past its icon.
            Some(untracked(move || {
                view()
                    .pl(px(S2 + 15.0 + S2))
                    .pr(px(S2))
                    .pt(px(S1))
                    .pb(px(S2))
                    .child(super::tool_detail::tool_body(&kind, &detail, error.as_deref()))
                    .into_element()
            }))
        })))
        .child(header)
}

fn todo_card(item: Memo<Option<Item>>) -> impl IntoElement {
    let lines = computed(move || match item.get() {
        Some(Item::Todo { lines }) => lines,
        _ => vec![],
    });
    card().p(px(S3)).gap(px(6.0)).child(section_label(t("Tasks"))).child(dynamic(move || {
        let lines = lines.get();
        Some(untracked(move || {
            let mut list = view().gap(px(6.0));
            for (done, t) in lines.clone() {
                list = list.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .child(icon(if done { Icon::CircleCheck } else { Icon::CircleDot }, 15.0).text_color(if done {
                            SUCCESS
                        } else {
                            FG_FAINT
                        }))
                        .child(
                            text(t)
                                .text_size(px(TEXT_BASE))
                                .text_color(if done { FG_MUTED } else { FG })
                                .flex_1(),
                        ),
                );
            }
            list
        }))
    }))
}

/// The first pending permission, as a card above the composer: a question
/// form, a plan to approve, or a tool to allow.
pub(crate) fn permissions() -> View {
    let pending = computed(|| store::selected_agent().map(|a| a.pending).unwrap_or_default());
    view().w_full().max_w(px(CONTENT_MAX_WIDTH)).self_center().child(dynamic(move || {
        let first = pending.get().into_iter().next()?;
        Some(untracked(move || {
            let questions = first.input.as_ref().and_then(questions_of).filter(|q| !q.is_empty());
            let plan = first.input.as_ref().and_then(plan_of);
            match (questions, plan) {
                (Some(qs), _) if first.kind == "question" || first.actions.is_empty() => {
                    question_card(first.clone(), qs).into_element()
                }
                (_, Some(plan)) => plan_card(first.clone(), plan).into_element(),
                _ => tool_card(first.clone()).into_element(),
            }
        }))
    }))
}

fn action_buttons(p: &Permission) -> View {
    let pid = p.id.clone();
    let mut buttons = view().flex_row().gap(px(S2)).flex_wrap();
    if p.actions.is_empty() {
        let (a, d) = (pid.clone(), pid.clone());
        return buttons
            .child(btn(t("Allow"), Variant::Primary, Sz::Md, Some(Icon::Check), move || {
                store::answer_permission(&a, true, None)
            }))
            .child(btn(t("Deny"), Variant::Secondary, Sz::Md, None, move || store::answer_permission(&d, false, None)));
    }
    let mut primary_used = false;
    for act in p.actions.clone() {
        let id = pid.clone();
        let label = if act.label.is_empty() { act.id.clone() } else { act.label.clone() };
        let variant = if act.allow && !primary_used {
            primary_used = true;
            Variant::Primary
        } else {
            Variant::Secondary
        };
        buttons = buttons.child(btn(label, variant, Sz::Md, None, move || {
            store::answer_permission(&id, act.allow, Some(act.id.clone()))
        }));
    }
    buttons
}

fn card_title(glyph: Icon, tone: AdaptiveColor, title: String) -> View {
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .child(icon(glyph, 16.0).text_color(tone))
        .child(text(title).text_size(px(TEXT_BASE)).font_weight(FontWeight(500)).flex_1())
}

fn tool_card(p: Permission) -> View {
    card()
        .mb(px(S2))
        .p(px(S4))
        .gap(px(S3))
        .border_color(tint(DOT_WARNING, 0.5))
        .child(card_title(Icon::TriangleAlert, WARNING, p.title.clone()))
        .when(!p.description.is_empty(), |v| {
            v.child(
                view().px(px(S3)).py(px(S2)).rounded(px(R_LG)).bg(SURFACE2).child(
                    text(p.description.clone())
                        .text_size(px(TEXT_CODE))
                        .font_family(|| MONO)
                        .line_height(px(18.0))
                        .number_of_lines(8)
                        .text_color(FG)
                        .selectable(true),
                ),
            )
        })
        .child(action_buttons(&p))
}

fn plan_card(p: Permission, plan: String) -> View {
    card()
        .mb(px(S2))
        .p(px(S4))
        .gap(px(S3))
        .child(card_title(Icon::ListTodo, ACCENT, t("Plan ready for review").into()))
        .child(
            scroll().max_h(px(320.0)).child(
                text_view()
                    .markdown(move || Some(SharedString::from(plan.clone())))
                    .selectable(true)
                    .gap_2()
                    .text_size(px(TEXT_BASE))
                    .line_height(px(21.0))
                    .text_color(FG),
            ),
        )
        .child(action_buttons(&p))
}

/// A form for an agent's questions: options as choices (one or several),
/// an "Other" answer, and Submit / Dismiss.
fn question_card(p: Permission, questions: Vec<Question>) -> View {
    // Per question: the chosen option labels and the typed answer.
    let chosen = signal(vec![Vec::<String>::new(); questions.len()]);
    let other = signal(vec![String::new(); questions.len()]);
    let qs = questions.clone();
    let answered = computed(move || {
        let (c, o) = (chosen.get(), other.get());
        (0..qs.len()).all(|i| !c[i].is_empty() || !o[i].trim().is_empty())
    });

    let mut body = view().gap(px(S4));
    for (i, q) in questions.iter().enumerate() {
        let mut options = view().flex_row().flex_wrap().gap(px(S2));
        for (label, detail) in q.options.clone() {
            let multi = q.multi;
            let (l1, l2) = (label.clone(), label.clone());
            let shown = if detail.is_empty() { label.clone() } else { format!("{label} — {detail}") };
            options = options.child(chip(shown, None, move || chosen.with(|c| c[i].contains(&l1)), move || {
                chosen.update(|c| {
                    if multi {
                        if let Some(pos) = c[i].iter().position(|x| *x == l2) {
                            c[i].remove(pos);
                        } else {
                            c[i].push(l2.clone());
                        }
                    } else {
                        c[i] = vec![l2.clone()];
                    }
                });
                if !multi {
                    other.update(|o| o[i].clear());
                }
            }));
        }
        body = body.child(
            view()
                .gap(px(S2))
                .child(section_label(q.header.clone()))
                .child(text(q.question.clone()).text_size(px(TEXT_BASE)).line_height(px(20.0)))
                .child(options)
                .child(
                    field()
                        .value(move || other.with(|o| o[i].clone()))
                        .on_change(move |v: &str| other.update(|o| o[i] = v.to_owned()))
                        .placeholder(if q.options.is_empty() { t("Your answer") } else { "Other…" }),
                ),
        );
    }

    let input = p.input.clone().unwrap_or(serde_json::Value::Null);
    let (submit_id, dismiss_id) = (p.id.clone(), p.id.clone());
    let headers: Vec<(String, bool)> = questions.iter().map(|q| (q.header.clone(), q.multi)).collect();
    card()
        .mb(px(S2))
        .p(px(S4))
        .gap(px(S4))
        .child(card_title(Icon::CircleDot, ACCENT, p.title.clone()))
        .child(scroll().max_h(px(380.0)).child(body))
        .child(
            view()
                .flex_row()
                .gap(px(S2))
                .child(
                    btn(t("Submit"), Variant::Primary, Sz::Md, Some(Icon::Check), move || {
                        let (c, o) = (chosen.get_untracked(), other.get_untracked());
                        let mut answers = serde_json::Map::new();
                        for (i, (header, multi)) in headers.iter().enumerate() {
                            let typed = o[i].trim();
                            let value = if typed.is_empty() {
                                c[i].join(", ")
                            } else if *multi {
                                c[i].iter().cloned().chain([typed.to_owned()]).collect::<Vec<_>>().join(", ")
                            } else {
                                typed.to_owned()
                            };
                            answers.insert(header.clone(), serde_json::Value::String(value));
                        }
                        let mut updated = input.clone();
                        if let Some(obj) = updated.as_object_mut() {
                            obj.insert("answers".into(), serde_json::Value::Object(answers));
                        }
                        store::answer_permission_with(&submit_id, true, None, Some(updated));
                    })
                    .disabled(move || !answered.get()),
                )
                .child(btn(t("Dismiss"), Variant::Secondary, Sz::Md, None, move || {
                    store::answer_permission(&dismiss_id, false, None)
                })),
        )
}

/// Sub-agents and background work of the open agent, as tracks above the
/// composer.
pub(crate) fn tracks() -> View {
    let todos = computed(|| {
        (0..store::item_count()).rev().find_map(|i| match store::item_at(i) {
            Some(Item::Todo { lines }) => Some(lines),
            _ => None,
        })
    });
    let subs = computed(|| store::selected_id().map(|id| store::subagents(&id)).unwrap_or_default());
    let work = computed(|| store::selected_id().map(|id| store::background(&id)).unwrap_or_default());
    view()
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH))
        .self_center()
        .child(dynamic(move || {
            let lines = todos.get()?;
            let done = lines.iter().filter(|(d, _)| *d).count();
            if lines.is_empty() || done == lines.len() {
                return None;
            }
            Some(untracked(move || {
                let next = lines.iter().find(|(d, _)| !*d).map(|(_, t)| t.clone()).unwrap_or_default();
                card()
                    .mb(px(S2))
                    .px(px(S3))
                    .py(px(S2))
                    .flex_row()
                    .items_center()
                    .gap(px(S2))
                    .child(icon(Icon::ListTodo, 15.0).text_color(ACCENT))
                    .child(text(tf("Tasks {}/{}", &[&done.to_string(), &lines.len().to_string()])).text_size(px(TEXT_SM)).font_weight(FontWeight(500)))
                    .child(text(next).text_size(px(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1).flex_1())
            }))
        }))
        .child(dynamic(move || {
            let list = subs.get();
            if list.is_empty() {
                return None;
            }
            Some(untracked(move || {
                let mut c = card().mb(px(S2)).px(px(S3)).py(px(S2)).gap(px(2.0)).child(section_label(tf("Sub-agents · {}", &[&list.len().to_string()])));
                for s in list.clone() {
                    let status = match s.status.as_str() {
                        "running" => Status::Running,
                        "failed" => Status::Failed,
                        "completed" => Status::Done,
                        _ => Status::Idle,
                    };
                    let (parent, id) = (s.parent.clone(), s.id.clone());
                    c = c.child(
                        nsh_base::button()
                            .on_press(move || store::open_tab(store::Tab::Subagent(parent.clone(), id.clone())))
                            .flex_row()
                            .items_center()
                            .gap(px(S2))
                            .h(px(30.0))
                            .px(px(S1))
                            .rounded(px(R_LG))
                            .hover(|st| st.bg(highlight()))
                            .child(status_dot(move || status))
                            .child(text(s.title.clone()).text_size(px(TEXT_BASE)).number_of_lines(1).flex_shrink(1.0))
                            .child(text(s.subtitle.clone()).text_size(px(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1).flex_1()),
                    );
                }
                c
            }))
        }))
        .child(dynamic(move || {
            let list = work.get();
            if list.is_empty() {
                return None;
            }
            Some(untracked(move || {
                let mut c = card().mb(px(S2)).px(px(S3)).py(px(S2)).gap(px(2.0)).child(section_label(t("Background")));
                for w in list.clone() {
                    let mut row = view()
                        .flex_row()
                        .items_center()
                        .gap(px(S2))
                        .h(px(30.0))
                        .child(status_dot(|| Status::Running))
                        .child(text(w.label.clone()).text_size(px(TEXT_CODE)).font_family(|| MONO).number_of_lines(1).flex_1());
                    if let Some(token) = w.stop_token.clone() {
                        let item = w.id.clone();
                        row = row.child(btn(t("Stop"), Variant::Outline, Sz::Sm, None, move || {
                            if let Some(agent) = store::selected_id() {
                                store::stop_background(&agent, &item, &token);
                            }
                        }));
                    }
                    c = c.child(row);
                }
                c
            }))
        }))
}

/// A sub-agent's conversation, read-only.
pub(crate) fn subagent_pane(parent: String, id: String) -> impl IntoElement {
    let items = {
        let id = id.clone();
        computed(move || store::subagent_items(&id))
    };
    let info = computed(move || store::subagent(&parent, &id));
    view()
        .flex_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(S2))
                .h(px(40.0))
                .px(px(S6))
                .border_b_1()
                .border_color(BORDER)
                .child(icon(Icon::Bot, 15.0).text_color(FG_MUTED))
                .child(text(move || info.with(|i| i.as_ref().map(|i| i.title.clone()).unwrap_or_default())).text_size(px(TEXT_BASE)).number_of_lines(1).flex_1())
                .child(text(move || info.with(|i| i.as_ref().map(|i| i.status.clone()).unwrap_or_default())).text_size(px(TEXT_SM)).text_color(FG_MUTED)),
        )
        .child(dynamic(move || {
            let list = items.get();
            Some(untracked(move || match list.clone() {
                None => view().flex_1().items_center().justify_center().child(loader(None)).into_element(),
                Some(list) => {
                    let mut col = view().gap(px(S2)).px(px(S6)).py(px(S4)).w_full().max_w(px(CONTENT_MAX_WIDTH + 2.0 * S6)).self_center();
                    for item in list {
                        col = col.child(static_row(item));
                    }
                    scroll().flex_1().child(col).into_element()
                }
            }))
        }))
}

fn static_row(item: Item) -> AnyElement {
    match item {
        Item::User { text: t, .. } => {
            let item = computed(move || Some(Item::User { text: t.clone(), message_id: None, images: vec![] }));
            view()
                .self_end()
                .max_w(relative(0.82))
                .px(px(14.0))
                .py(px(9.0))
                .rounded(px(18.0))
                .bg(SURFACE2)
                .child(user_text(item, true))
                .into_element()
        }
        Item::Assistant { text: t } => text_view()
            .markdown(move || Some(SharedString::from(t.clone())))
            .selectable(true)
            .gap_3()
            .text_size(px(TEXT_CONTENT))
            .line_height(px(24.0))
            .text_color(FG)
            .into_element(),
        Item::Tool { name, kind, summary, .. } => view()
            .flex_row()
            .items_center()
            .gap(px(S2))
            .px(px(S2))
            .h(px(30.0))
            .child(icon(tool_icon(&kind, &name), 15.0).text_color(FG_MUTED))
            .child(text(name).text_size(px(TEXT_BASE)).font_weight(FontWeight(500)))
            .child(text(summary).text_size(px(TEXT_CODE)).font_family(|| MONO).text_color(FG_MUTED).number_of_lines(1).flex_1())
            .into_element(),
        Item::Reasoning { text: t } => text(t).text_size(px(13.0)).italic().number_of_lines(3).text_color(FG_MUTED).into_element(),
        Item::Error { message } => text(message).text_size(px(TEXT_BASE)).text_color(DANGER).into_element(),
        Item::Notice { message } => text(message).text_size(px(TEXT_SM)).text_color(FG_FAINT).into_element(),
        Item::Todo { lines } => text(lines.iter().map(|(d, t)| format!("{} {t}", if *d { "☑" } else { "☐" })).collect::<Vec<_>>().join("\n"))
            .text_size(px(TEXT_BASE))
            .into_element(),
    }
}

/// The find bar over the conversation (⌘F).
pub(crate) fn find_bar() -> impl IntoElement {
    let open = computed(|| store::find().open);
    when(open, || {
        let count = computed(|| store::find_matches().len());
        let cur = computed(|| store::find().current);
        let asked = computed(|| !store::find().query.trim().is_empty());
        // The field keeps the keys: Return goes to the next match again and
        // again, and the arrows hand the focus back after a press.
        let field = element_ref();
        let step = {
            let field = field.clone();
            move |forward: bool| {
                store::find_step(forward);
                field.focus();
            }
        };
        let (previous, next) = (step.clone(), step);
        // A compact panel over the conversation's top right corner, as a
        // browser's or an editor's (the phone: across its top): it covers
        // nothing that the search moves to, and pushes nothing down.
        let panel = glass_strong(view())
            .flex_row()
            .items_center()
            .gap(px(S1))
            .h(px(40.0))
            .pl(px(S3))
            .pr(px(S1))
            .rounded(px(R_XL))
            .shadow_lg();
        let panel = if MOBILE { panel.mx(px(S2)).mb(px(S2)) } else { panel.absolute().top(px(S2)).right(px(S6)).w(px(360.0)) };
        panel
            .child(icon(Icon::Search, 15.0).text_color(FG_MUTED))
            .child(
                nsh_core::element::input()
                    .value(|| store::find().query)
                    .on_change(|v: &str| store::set_find_query(v))
                    .on_submit(|_: &str| store::find_step(true))
                    .submit_behavior(SubmitBehavior::Submit)
                    .element_ref(&field)
                    .placeholder(t("Find in conversation"))
                    .auto_focus(true)
                    .text_size(px(TEXT_BASE))
                    .text_color(FG)
                    .caret_color(|| color(ACCENT))
                    .h(px(28.0))
                    .ml(px(S1))
                    .flex_1()
                    .min_w(px(0.0)),
            )
            .child(
                text(move || match (count.get(), asked.get()) {
                    (0, true) => t("No results").to_owned(),
                    (0, false) => String::new(),
                    (n, _) => format!("{} / {n}", cur.get().min(n - 1) + 1),
                })
                .text_size(px(TEXT_SM))
                .text_color(FG_MUTED)
                .tabular_nums()
                .number_of_lines(1)
                .flex_shrink_0(),
            )
            .child(view().w(px(1.0)).h(px(18.0)).mx(px(S1)).bg(BORDER))
            .child(icon_btn(Icon::ChevronUp, Sz::Sm, t("Previous match"), move || previous(false)))
            .child(icon_btn(Icon::ChevronDown, Sz::Sm, t("Next match"), move || next(true)))
            .child(icon_btn(Icon::X, Sz::Sm, t("Close find"), store::toggle_find))
    })
}

/// While the agent runs and nothing it says is streaming: the mark
/// breathing at the foot of the conversation, so a sent message never sits
/// there without an answer in sight.
fn thinking(running: Memo<bool>) -> impl IntoElement {
    let state = computed(move || {
        if !running.get() {
            return None;
        }
        let last = store::item_count().checked_sub(1).and_then(store::item_at);
        match last {
            // Its words are on screen already.
            Some(Item::Assistant { .. }) => None,
            Some(Item::User { .. }) | None => Some(t("Thinking…")),
            Some(_) => Some(t("Working…")),
        }
    });
    view().child(dynamic(move || {
        let label = state.get()?;
        Some(untracked(move || {
            // The words alone, breathing so they read as live.
            let breath = animated_value(1.0);
            let half = nsh_core::element::Timing::new(900.0, Easing::EaseInOut);
            Animation::sequence([Animation::timing(&breath, 0.45, half.clone()), Animation::timing(&breath, 1.0, half)])
                .looped(None)
                .start();
            view()
                .w_full()
                .max_w(px(CONTENT_MAX_WIDTH + 2.0 * S6))
                .self_center()
                // The rows' own inset, so the words line up with the text.
                .px(px(S6))
                .py(px(S3))
                .child(text(label).text_size(px(TEXT_SM)).text_color(FG_MUTED).bind_opacity(&breath))
                .into_element()
        }))
    }))
}

#[cfg(test)]
mod find_tests {
    use super::mark_matches;

    #[test]
    fn a_search_marks_its_words_in_the_text_but_not_in_code_or_addresses() {
        assert_eq!(mark_matches("Run the Tests now", "tests"), "Run the <mark>Tests</mark> now");
        assert_eq!(mark_matches("简历要改两处，简历源文件", "简历"), "<mark>简历</mark>要改两处，<mark>简历</mark>源文件");
        // Inline code, a link's address and fenced code keep their text.
        assert_eq!(mark_matches("use `cargo test` to test", "test"), "use `cargo test` to <mark>test</mark>");
        assert_eq!(mark_matches("[test docs](https://x.dev/test)", "test"), "[<mark>test</mark> docs](https://x.dev/test)");
        assert_eq!(mark_matches("a test\n```\ntest()\n```\ntest", "test"), "a <mark>test</mark>\n```\ntest()\n```\n<mark>test</mark>");
        assert_eq!(mark_matches("nothing here", "zzz"), "nothing here");
        assert_eq!(mark_matches("same", " "), "same");
    }
}
