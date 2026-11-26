//! An agent's conversation: user bubbles, Markdown replies, tool calls,
//! thinking, todos, errors; pending permissions above the composer.

// Presses and long presses on plain views (the phone's bubbles).
use nsh_core::element::Interactive as _;
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
    Retry,
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
            Item::Retry { .. } => Self::Retry,
        }
    }
}

pub(crate) fn timeline() -> impl IntoElement {
    // A list of its own for each conversation: it opens at that
    // conversation's end, wherever the last one was left.
    let loaded = computed(|| (store::timeline_loaded(), store::selected_id()));
    let running = computed(|| store::selected_agent().is_some_and(|a| a.is_running()));
    view().flex_1().child(dynamic(move || {
        let (ready, _) = loaded.get();
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
            let list = element_ref();
            store::set_timeline_list(list.clone());
            // Its rows measure themselves once shown, and the list goes on
            // to its end as they do: a few moves in the first quarter
            // second; and this device's copy of it, shown first, grows when
            // the host's comes. It shows once the host's is there and it
            // stays at its end (most of a second at most), so a
            // conversation opens where it ends. A change of its rows (the
            // host's copy, a reply streaming in) lets go of nsh's hold on
            // the end, so meanwhile it is sent there again.
            // Rows or pages changed: once they are laid out, whether the
            // list fills the view yet (`store::fill_timeline`).
            effect(move || {
                rows.with(|_| ());
                store::timeline_pages();
                set_timeout(std::time::Duration::from_millis(150), store::fill_timeline);
            });
            let settled = signal(false);
            let gone = std::rc::Rc::new(std::cell::Cell::new(false));
            {
                let gone = gone.clone();
                cleanup(move || gone.set(true));
            }
            store::spawn_detached(async move {
                let (mut last, mut steady) = (f32::NAN, 0);
                for _ in 0..45 {
                    nsh_core::element::timer(std::time::Duration::from_millis(16)).await;
                    if gone.get() {
                        return;
                    }
                    let (from_end, content) = store::timeline_place();
                    if from_end.is_finite() && from_end > 2.0 {
                        store::timeline_to_end_now();
                    }
                    // A reply streaming in grows it on: at its end is enough.
                    let streaming = untracked(|| store::selected_agent().is_some_and(|a| a.is_running()));
                    steady = if from_end <= 2.0 && (content == last || streaming) && !untracked(store::timeline_syncing) { steady + 1 } else { 0 };
                    last = content;
                    if steady >= 3 {
                        break;
                    }
                }
                if !gone.get() {
                    settled.set(true);
                }
            });
            // On the desktop the whole conversation is one selectable area:
            // a selection runs from reply to reply, as on a web page (each
            // reply its own area stopped it at the reply's edge).
            view().flex_1().opacity(move || if settled.get() { 1.0_f32 } else { 0.0 }).transition_opacity().when(!MOBILE, |v| v.selectable(true).on_text_selection_change(list_chosen)).child(virtual_list(move || rows.with(Vec::len), move |at: Position| row(at, rows, running))
                // On the phone, a drag down the conversation takes the
                // keyboard with it, as Messages does.
                .keyboard_dismiss_mode(if MOBILE { KeyboardDismissMode::Interactive } else { KeyboardDismissMode::None })
                .element_ref(&list)
                .keys(move || rows.with(|r| store::row_keys(r)))
                .estimated_item_size(px(80.0))
                // The turns' rail down the left is the desktop's map of the
                // conversation: no scroll bar beside it.
                .scrollbar(MOBILE)
                // At its end, the last row at the bottom, kept there while
                // the rows measure (nsh pins it): no jumps as they do.
                .initial_scroll_at_end()
                .follow_end(px(FOLLOW_END))
                // How far from its end the list is, for what depends on
                // its keeping the end in place (`store::set_group_open`).
                .event_throttle(100.0)
                .on_scroll(|e: &ScrollEvent| store::set_timeline_scroll(e.offset.y.0, e.content_size.height.0, e.size.height.0))
                // A drag is the user's, whatever the content does meanwhile.
                .on_scroll_drag_start(|_: &nsh_core::Point<nsh_core::Points>| store::timeline_scrolled_by_user())
                .content_padding_top(sp(S4))
                // The phone's composer floats over the end; the desktop's
                // sits below the list.
                .content_padding_bottom(px(if MOBILE { 200.0 } else { S4 }))
                .footer(thinking(running))
                // Earlier pages come as the conversation nears its start.
                // Not while it opens: an earlier page put before the rows
                // keeps what shows in place, away from the end.
                .on_load_more_at_start(move || {
                    if settled.get_untracked() {
                        store::load_older();
                    }
                })
                .load_more_at_start_threshold(1.0)
                .flex_1())
                // Earlier pages load by themselves as the start nears: a
                // small loader says so while one is on its way, no button.
                .child(when(store::loading_older, || {
                    view().absolute().top(sp(S2)).left(px(0.0)).right(px(0.0)).items_center().child(eclipse(24.0))
                }))
                .when(!MOBILE, |v| v.child(turn_rail()))
                .child(jump_buttons())
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
        .left(sp(S1))
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
                        .p(sp(S3))
                        .gap(sp(S1))
                        .rounded(px(R_XL))
                        .pointer_events(PointerEvents::None)
                        .child(text(message).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)).text_color(FG()).number_of_lines(1))
                        .when(!reply.is_empty(), |v| {
                            v.child(text(reply).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED()).number_of_lines(3))
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
                                            .bg(move || if over.get() == Some(row) { FG() } else { tint(FG(), 0.22) })
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
    let last = computed(move || index.get().is_some_and(|i| i + 1 == store::item_count()));
    let streaming = computed(move || running.get() && last.get());
    let text_of = move || {
        item.with(|i| match i {
            Some(Item::User { text, .. } | Item::Assistant { text, .. } | Item::Reasoning { text }) => text.clone(),
            Some(Item::Error { message }) => message.clone(),
            Some(Item::Notice { message }) => crate::i18n::tr(message),
            Some(retry @ Item::Retry { .. }) => retry_words(retry, streaming.get()),
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
    // The end of a turn (worked out with the rows): the reply's actions
    // show under it alone, not under each piece said between tool calls.
    let turn_end = computed(move || matches!(shown.get(), Some(Row::Item { turn_end: true, .. })));
    view()
        .px(sp(S6))
        // Close together: a turn's steps and words read as one piece.
        .py(px(if MOBILE { 6.0 } else { 2.0 }))
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH() + 2.0 * S6))
        .self_center()
        .rounded(px(R_LG))
        .bg(move || match hit.get() {
            2 => tint(ACCENT(), 0.14),
            1 => tint(ACCENT(), 0.06),
            _ => tint(ACCENT(), 0.0),
        })
        .child(dynamic(
        move || {
            let kind = kind.get()?;
            Some(untracked(move || match kind {
                // The phone: no button on the bubble; a tap opens its
                // actions, copying among them.
                RowKind::User if MOBILE => user_row(item, move || view().child(dynamic(move || {
                    let selecting = index.get().is_some() && selecting_text() == index.get();
                    Some(untracked(move || {
                        let bubble = view().px(px(14.0)).py(px(9.0)).rounded(px(18.0)).bg(user_bubble());
                        if selecting {
                            select_in_place(bubble.child(user_text(item, true))).items_end().into_element()
                        } else {
                            bubble
                                .active(|s| s.opacity(0.7))
                                .on_press(move || message_sheet(item, index.get_untracked()))
                                // As other chat apps: a long press, the message's actions.
                                .on_long_press(move |_: &nsh_core::element::PressEvent| message_sheet(item, index.get_untracked()))
                                .label(t("Message actions"))
                                .child(user_text(item, false))
                                .into_element()
                        }
                    }))
                })))
                    .into_element(),
                // The desktop: its actions show while the pointer is on it.
                // They have a line of their own under the bubble, kept for
                // them whether shown or not: nothing after the message is
                // covered. And the pointer is "on it" over the bubble and
                // that line alike, so reaching for an action does not lose it.
                RowKind::User => {
                    let hover = signal(false);
                    user_row(
                        item,
                        move || view()
                            .items_end()
                            .gap(px(4.0))
                            .on_mouse_enter(move |_: &MouseEvent| hover.set(true))
                            .on_mouse_leave(move |_: &MouseEvent| hover.set(false))
                            .child(view().px(px(14.0)).py(px(9.0)).rounded(px(18.0)).bg(user_bubble()).selectable(true).on_text_selection_change(take_copy).child(user_text(item, true)))
                            .child(user_actions(item).opacity(move || if hover.get() { 1.0_f32 } else { 0.0 }).transition_opacity()),
                    )
                    .into_element()
                }
                // A copy button floats at its corner: under the pointer on
                // the desktop, always on the phone (no pointer, and a press
                // there selects).
                // The phone: no button on the reply; a long press opens its
                // actions (copy, select text), as other chat apps do.
                RowKind::Assistant if MOBILE => view().child(dynamic(move || {
                    let selecting = index.get().is_some() && selecting_text() == index.get();
                    Some(untracked(move || {
                        let reply = text_view()
                            .links(nsh_base::TextViewLinks::Open)
                            // A file's (Codex links the files it names): opened here.
                            .on_link_press(super::file_links::press_link)
                            .code_block_actions(vec![nsh_base::TextViewCodeAction::new("copy").with_label(crate::i18n::t("Copy"))])
                            .markdown(move || Some(SharedString::from(text_of())))
                            .options(move || MarkdownOptions { streaming: streaming.get(), ..MarkdownOptions::default() })
                            .gap_3()
                            .text_size(tx(TEXT_CONTENT))
                            .line_height(px(24.0))
                            .text_color(FG());
                        if selecting {
                            select_in_place(view().child(reply)).into_element()
                        } else {
                            view()
                                .on_long_press(move |_: &nsh_core::element::PressEvent| reply_sheet(text_of(), index.get_untracked()))
                                .label(t("Message actions"))
                                .child(reply)
                                .into_element()
                        }
                    }))
                }))
                    .into_element(),
                RowKind::Assistant => {
                    let hover = signal(false);
                    let chosen = signal(Chosen::default());
                    // Words chosen in it offer a note over them; a press on
                    // that opens the note's card there.
                    let pointer = signal((0.0_f32, 0.0_f32));
                    // Where the press that chose them began: the pointer
                    // as it last moved with no button down.
                    let pressed_at = signal((0.0_f32, 0.0_f32));
                    // Where this reply stands on the page (the pointer's
                    // page position less its own): what puts a selection's
                    // page box in the reply's coordinates.
                    let origin = signal(None::<(f32, f32)>);
                    // On the desktop the words are chosen in the list's one
                    // area: they are this reply's while the pointer is on it
                    // (where a selection is let go).
                    if !MOBILE {
                        effect(move || {
                            let all = global::<ListChosen>().with(|c| c.0.clone());
                            let here = hover.get_untracked() || all.words.trim().is_empty() || all.cleared.is_some();
                            chosen.set(if here { all } else { Chosen::default() });
                        });
                    }
                    let offer = signal(super::notes::Chosen::None);
                    let noting = signal(super::notes::Chosen::None);
                    effect(move || {
                        let c = chosen.get();
                        if c.cleared.is_none() && !c.words.trim().is_empty() {
                            let anchor = match (c.page, origin.get_untracked()) {
                                (Some(b), Some((x, y))) => {
                                    let mut b = b;
                                    b.origin.x.0 -= x;
                                    b.origin.y.0 -= y;
                                    super::notes::Anchor::of(b)
                                }
                                // Where the platform does not place them: the
                                // line the drag began on, or let go on if it
                                // went up.
                                _ => {
                                    let (up, down) = (pointer.get_untracked(), pressed_at.get_untracked());
                                    let (x, y) = if up.1 < down.1 { up } else { down };
                                    super::notes::Anchor { top: y - 12.0, bottom: y + 12.0, center: x }
                                }
                            };
                            offer.set(Some((anchor, c.words.trim().to_owned())));
                        } else {
                            // A moment later: the press on "Note" itself
                            // lets the words go first.
                            set_timeout(std::time::Duration::from_millis(400), move || {
                                if chosen.with_untracked(|c| c.cleared.is_some() || c.words.trim().is_empty()) {
                                    offer.set(None);
                                }
                            });
                        }
                    });
                    // One piece of the reply as Markdown, as the whole is shown.
                    let piece = move |markdown: Box<dyn Fn() -> String>| {
                        text_view()
                            // A link opens in the browser (or the app it belongs to).
                            .links(nsh_base::TextViewLinks::Open)
                            // A file's (Codex links the files it names): opened here.
                            .on_link_press(super::file_links::press_link)
                            // A copy button on each code block (under the pointer on the
                            // desktop, always on the phone).
                            .code_block_actions(vec![nsh_base::TextViewCodeAction::new("copy").with_label(crate::i18n::t("Copy"))])
                            // While searching, the words found are marked in the
                            // text itself, not only the message they are in.
                            .markdown(move || {
                                // A file's path written as code, a chip of the file.
                                let here = store::on_this_mac();
                                let body = super::file_links::link_paths(&markdown(), |p| !here || super::file_links::exists_here(p));
                                let f = store::find();
                                Some(SharedString::from(if f.open && hit.get() > 0 { mark_matches(&body, &f.query) } else { body }))
                            })
                            .options(move || MarkdownOptions { streaming: streaming.get(), ..MarkdownOptions::default() })
                            // A file's path, a chip that opens it.
                            .extensions(vec![super::file_links::extension()])
                            .render_custom(super::file_links::chip)
                            .selectable(true)
                            .gap_3()
                            .text_size(tx(TEXT_CONTENT))
                            .line_height(px(24.0))
                            .text_color(FG())
                    };
                    // Written, a reply with quotes in it shows each quote apart,
                    // with a copy button of its own (a reply drafted for someone
                    // else is often one): while it is written it is one piece.
                    let pieces = computed(move || if streaming.get() { Vec::new() } else { quote_pieces(&text_of()) });
                    view()
                        .on_mouse_enter(move |_: &MouseEvent| hover.set(true))
                        .on_mouse_leave(move |_: &MouseEvent| hover.set(false))
                        .on_mouse_move(move |e: &MouseEvent| {
                            let at = (e.position.x.0, e.position.y.0);
                            pointer.set(at);
                            origin.set(Some((e.page_position.x.0 - at.0, e.page_position.y.0 - at.1)));
                            if e.pressed_button.is_none() {
                                pressed_at.set(at);
                            }
                        })
                        .child(copy_area_choosing(chosen, dynamic(move || {
                            let list = pieces.get();
                            Some(untracked(move || {
                                if !list.iter().any(|p| p.quoted.is_some()) {
                                    return piece(Box::new(text_of)).into_element();
                                }
                                let mut column = view().gap(sp(S3));
                                for p in list {
                                    let shown = p.markdown.clone();
                                    let el = piece(Box::new(move || shown.clone()));
                                    column = column.child(match p.quoted {
                                        Some(words) => quote_with_copy(el, words).into_element(),
                                        None => el.into_element(),
                                    });
                                }
                                column.into_element()
                            }))
                        })))
                        // Under the reply (as T3 Code's): copy it, fork the
                        // conversation, quote it in the next message, and when
                        // it was written; there under the pointer once written.
                        .child(when(turn_end, move || reply_actions(item, text_of, chosen, computed(move || !streaming.get() && hover.get()))))
                        // Over the reply, so last.
                        .child(super::notes::selection_layer(offer, noting))
                        .into_element()
                }
                // The agent's thinking, folded to one line ("Thinking"):
                // a press shows it whole, another folds it again.
                RowKind::Reasoning => {
                    // Under an open group, indented as its calls are.
                    let indent = move || px(if grouped.get() { S4 } else { 0.0 });
                    let key = move || text_of().chars().take(80).collect::<String>();
                    let open = computed(move || global::<OpenThoughts>().with(|o| o.0.contains(&key())));
                    view()
                        .pl(indent)
                        .child(
                            nsh_base::button()
                                .on_press(move || {
                                    let k = key();
                                    global::<OpenThoughts>().update(|o| {
                                        if !o.0.remove(&k) {
                                            o.0.insert(k);
                                        }
                                    });
                                })
                                .accessibility_label(t("Thinking"))
                                .flex_row()
                                .items_center()
                                .gap(sp(S2))
                                .h(px(24.0))
                                .self_start()
                                .group()
                                .child(icon(Icon::Brain, 15.0).text_color(FG_FAINT()))
                                .child(
                                    text(move || if streaming.get() { t("Thinking…") } else { t("Thinking") }.to_owned())
                                        .text_size(px(13.0))
                                        .text_color(FG_FAINT())
                                        .group_hover(|s| s.text_color(FG_MUTED())),
                                )
                                .child(
                                    icon(Icon::ChevronRight, 12.0)
                                        .text_color(FG_FAINT())
                                        .rotate(move || Radians(if open.get() { std::f32::consts::FRAC_PI_2 } else { 0.0 }))
                                        .transition_transform(),
                                ),
                        )
                        .child(when(open, move || {
                            view().selectable(true).on_text_selection_change(take_copy).pl(px(15.0 + S2)).pt(px(2.0)).child(
                                text(move || text_of().trim().to_owned())
                                    .text_size(px(13.0))
                                    .line_height(px(19.0))
                                    .italic()
                                    .text_color(FG_MUTED()),
                            )
                        }))
                        .into_element()
                }
                // A call under an open group sits indented under its
                // summary, past the summary's chevron.
                RowKind::Tool => view().pl(move || px(if grouped.get() { S4 } else { 0.0 })).child(tool_row(item)).into_element(),
                RowKind::Group => group_row(group).into_element(),
                RowKind::Todo => todo_card(item).into_element(),
                // A line where the reply would be, not a red block: the mark
                // says it failed, the words say why, quietly.
                RowKind::Error => view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .min_h(px(26.0))
                    .child(icon(Icon::CircleAlert, 14.0).text_color(DANGER))
                    .child(
                        view()
                            .selectable(true)
                            .on_text_selection_change(take_copy)
                            .flex_1()
                            .child(text(text_of).text_size(tx(TEXT_SM)).line_height(px(19.0)).text_color(FG_MUTED())),
                    )
                    // The turn ended on it: its message sent again.
                    .child(when(computed(move || last.get() && !running.get()), move || retry_button(DANGER)))
                    .into_element(),
                // Claude Code retrying a failed request: which attempt, why;
                // the last message sent again at a press, not after the wait.
                RowKind::Retry => view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .px(sp(S3))
                    .py(sp(S1))
                    .min_h(px(34.0))
                    .rounded(px(R_LG))
                    .bg(tint(WARNING, 0.08))
                    .child(icon(Icon::RotateCw, 14.0).text_color(WARNING))
                    .child(text(text_of).text_size(tx(TEXT_SM)).text_color(WARNING).flex_1())
                    .child(when(last, move || retry_button(WARNING)))
                    .into_element(),
                RowKind::Notice => text(text_of)
                    .text_size(tx(TEXT_SM))
                    .text_color(FG_FAINT())
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
/// Where another conversation's words begin in a message that refers to
/// it (`store::conversation_context`).
/// A user message as copied: what was written, without another
/// conversation's words sent along with it (`store::conversation_context`).
fn message_words(text: &str) -> String {
    let text = crate::notes::body(text);
    match text.find(CONTEXT_MARK) {
        Some(at) => text[..at].trim_end().to_owned(),
        None => text.to_owned(),
    }
}

/// Text chosen in the conversation is what ⌘C copies. The composer keeps
/// the keyboard through clicks elsewhere (as a desktop field does), and
/// with it the Edit menu's Copy, which then copied nothing: it gives the
/// keyboard up once something is selected here.
fn take_copy(e: &nsh_core::element::TextSelectionEvent) {
    if !e.text.is_empty() {
        crate::ui::shared::composer_doc::blur_all();
    }
}

/// `content` in a selectable area that reports its selection
/// ([`take_copy`]): a rich text view reports none of its own.
/// A part of a reply: its Markdown, and (a quote's) its words without the
/// quote's marks, to copy.
#[derive(Clone, Debug, PartialEq)]
struct QuotePiece {
    markdown: String,
    quoted: Option<String>,
}

/// `text` cut where its quotes begin and end (a run of lines that start
/// with `>`, outside code blocks), each part kept as it was written.
fn quote_pieces(text: &str) -> Vec<QuotePiece> {
    let mut pieces: Vec<QuotePiece> = Vec::new();
    let mut fence: Option<&str> = None;
    let mut push = |line: &str, quote: bool| {
        let words = quote.then(|| {
            let rest = line.trim_start().strip_prefix('>').unwrap_or("");
            rest.strip_prefix(' ').unwrap_or(rest).to_owned()
        });
        match pieces.last_mut() {
            Some(last) if last.quoted.is_some() == quote => {
                last.markdown.push('\n');
                last.markdown.push_str(line);
                if let (Some(all), Some(w)) = (last.quoted.as_mut(), words) {
                    all.push('\n');
                    all.push_str(&w);
                }
            }
            _ => pieces.push(QuotePiece { markdown: line.to_owned(), quoted: words }),
        }
    };
    for line in text.lines() {
        let lead = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();
        // In a code block, a `>` is the code's.
        if let Some(open) = fence {
            if lead < 4 && trimmed.starts_with(open) {
                fence = None;
            }
            push(line, false);
            continue;
        }
        if lead < 4 && (trimmed.starts_with("```") || trimmed.starts_with("~~~")) {
            fence = Some(&trimmed[..3]);
            push(line, false);
            continue;
        }
        push(line, lead < 4 && trimmed.starts_with('>'));
    }
    // The words between the quotes, without the blank lines at their ends;
    // a quote's words without a trailing blank.
    pieces
        .into_iter()
        .filter_map(|mut p| {
            p.markdown = p.markdown.trim_matches('\n').to_owned();
            if let Some(q) = p.quoted.as_mut() {
                *q = q.trim().to_owned();
            }
            (!p.markdown.trim().is_empty()).then_some(p)
        })
        .collect()
}

/// A quote of a reply, a copy button at its corner under the pointer that
/// copies its words alone (not the `>` of each line).
fn quote_with_copy(quote: impl IntoElement, words: String) -> View {
    let hover = signal(false);
    view()
        .on_mouse_enter(move |_: &MouseEvent| hover.set(true))
        .on_mouse_leave(move |_: &MouseEvent| hover.set(false))
        .child(quote)
        .child(
            view()
                .absolute()
                .top(px(-2.0))
                .right(px(0.0))
                .rounded(px(R_MD))
                .bg(SURFACE0())
                .opacity(move || if hover.get() { 1.0_f32 } else { 0.0 })
                .transition_opacity()
                .pointer_events(move || if hover.get() { nsh_core::PointerEvents::Auto } else { nsh_core::PointerEvents::None })
                .child(icon_btn(Icon::Copy, Sz::Sm, t("Copy quote"), move || {
                    let _ = set_clipboard(words.clone());
                    store::notify(t("Copied").into());
                })),
        )
}

#[cfg(test)]
mod quote_tests {
    use super::{QuotePiece, quote_pieces};

    fn piece(markdown: &str, quoted: Option<&str>) -> QuotePiece {
        QuotePiece { markdown: markdown.into(), quoted: quoted.map(str::to_owned) }
    }

    #[test]
    fn a_reply_is_cut_where_its_quotes_are() {
        let text = "Use this one:\n\n> So true. Once agents can touch real data,\n> it changes.\n\nOr, lighter:\n\n>Agreed!\n";
        assert_eq!(
            quote_pieces(text),
            [
                piece("Use this one:", None),
                piece("> So true. Once agents can touch real data,\n> it changes.", Some("So true. Once agents can touch real data,\nit changes.")),
                piece("Or, lighter:", None),
                piece(">Agreed!", Some("Agreed!")),
            ]
        );
    }

    #[test]
    fn a_reply_without_quotes_is_one_piece_and_code_keeps_its_marks() {
        assert_eq!(quote_pieces("Just words.\n\nMore."), [piece("Just words.\n\nMore.", None)]);
        let code = "Run:\n\n```sh\n> not a quote\n```";
        assert_eq!(quote_pieces(code), [piece(code, None)]);
    }
}

/// The words chosen in the conversation's list (the desktop's one
/// selectable area), for the reply under the pointer to offer a note on
/// and to quote.
#[derive(Clone, Default, PartialEq)]
struct ListChosen(Chosen);

fn list_chosen(e: &nsh_core::element::TextSelectionEvent) {
    take_copy(e);
    global::<ListChosen>().update(|c| choose(&mut c.0, e));
}

/// `c` after a selection of `words` (empty: none), keeping when it was let go.
fn choose(c: &mut Chosen, e: &nsh_core::element::TextSelectionEvent) {
    if !e.text.trim().is_empty() {
        *c = Chosen { words: e.text.clone(), cleared: None, page: e.page_bounds };
    } else if c.cleared.is_none() && !c.words.is_empty() {
        c.cleared = Some(std::time::Instant::now());
    }
}

fn copy_area(content: impl IntoElement) -> View {
    view().selectable(true).on_text_selection_change(take_copy).child(content)
}

/// The words last chosen in a reply, and when the choice was let go (a
/// press on the reply's Quote button may clear it just before it lands).
#[derive(Clone, Default, PartialEq)]
struct Chosen {
    words: String,
    cleared: Option<std::time::Instant>,
    /// The box around them on the page (the window's root view), when
    /// the platform knows it.
    page: Option<Bounds<Points>>,
}

impl Chosen {
    /// What a quote takes now: the words chosen, if still chosen or let go
    /// a moment ago.
    fn words(&self) -> Option<&str> {
        let recent = self.cleared.is_none_or(|at| at.elapsed() < std::time::Duration::from_millis(600));
        (recent && !self.words.trim().is_empty()).then_some(self.words.as_str())
    }
}

/// [`copy_area`] that also keeps what is chosen in it, for Quote.
fn copy_area_choosing(chosen: Signal<Chosen>, content: impl IntoElement) -> View {
    view()
        .selectable(true)
        .on_text_selection_change(move |e: &nsh_core::element::TextSelectionEvent| {
            take_copy(e);
            chosen.update(|c| choose(c, e));
        })
        .child(content)
}

pub(crate) const CONTEXT_MARK: &str = "\n\n--- The conversation \"";

/// A sent message's bubble: the surface a step above the page, opaque
/// and without the accent's hue (a black or pale accent would vanish in
/// it, a loud one shout).
fn user_bubble() -> AdaptiveColor {
    SURFACE2()
}

fn user_text(item: Memo<Option<Item>>, selectable: bool) -> impl IntoElement {
    use crate::ui::shared::composer_doc::{Segment, is_image_name, segments};
    view().child(dynamic(move || {
        let (body, images) = item.with(|i| match i {
            Some(Item::User { text, images, .. }) => (text.clone(), images.clone()),
            _ => Default::default(),
        });
        // Another conversation's words sent with it (`store::conversation_
        // context`) are the agent's to read, not the bubble's to show.
        // Its notes show as a chip above it (`user_row`).
        let body = crate::notes::body(&body).to_owned();
        let body = match body.find(CONTEXT_MARK) {
            Some(at) => body[..at].trim_end().to_owned(),
            None => body,
        };
        let f = store::find();
        Some(untracked(move || {
            let mut line = text("").text_size(tx(TEXT_CONTENT)).line_height(px(22.0)).text_color(FG()).selectable(selectable);
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
                            line = line.child(text(t[at..start].to_owned())).child(text(t[start..end].to_owned()).bg(tint(ACCENT(), 0.28)));
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
        .translate_y(px(crate::ui::shared::chips::CHIP_LIFT))
        .rounded(px(R_MD))
        .bg(SURFACE0())
        .border_1()
        .border_color(tint(ACCENT(), 0.35));
    let chip = match &thumb {
        Some(uri) => chip.child(
            image(SharedString::from(uri.clone())).w(px(16.0)).h(px(16.0)).object_fit(nsh_core::ResizeMode::Cover).rounded(px(3.0)),
        ),
        None => chip.child(icon(if crate::ui::shared::composer_doc::is_image_name(&name) { Icon::Image } else { Icon::File }, 13.0).text_color(FG_MUTED())),
    };
    let chip_name = name.clone();
    let chip = chip.child(text(crate::ui::shared::chips::chip_name(&name)).text_size(tx(TEXT_SM)).text_color(FG()).number_of_lines(1));
    // A picture's chip opens it large (and shows it under the pointer).
    let chip = match thumb {
        Some(uri) => {
            let (open_name, open_uri) = (chip_name, uri.clone());
            chip.hover(|s| s.border_color(ACCENT()))
                .on_press(move |_: &PressEvent| store::open_modal(store::Modal::Picture { name: open_name.clone(), uri: open_uri.clone() }))
                .tooltip(
                    glass_strong(view()).p(sp(S1)).rounded(px(R_LG)).child(
                        image(SharedString::from(uri)).w(px(280.0)).h(px(280.0)).object_fit(nsh_core::ResizeMode::Contain).rounded(px(R_MD)),
                    ),
                )
        }
        None => chip,
    };
    // The room beside it is the box's the line lays out (as the composer's
    // editor wraps its chips): a margin of the chip's own is not counted.
    view().pl(px(1.0)).pr(px(5.0)).align_middle().child(chip)
}

fn user_row(item: Memo<Option<Item>>, bubble: impl Fn() -> View + 'static) -> impl IntoElement {
    // Read as the row's message goes: a row is kept while the message in
    // its place changes (its text follows in the bubble; its notes and
    // pictures must too).
    let parts = computed(move || item.with(|i| match i {
        Some(Item::User { images, text, .. }) => {
            // The pictures the text names show as chips in it; the rest (a
            // message sent before chips were named) stay above it.
            let named = crate::ui::shared::composer_doc::segments(text)
                .iter()
                .filter(|s| matches!(s, crate::ui::shared::composer_doc::Segment::Chip(n) if crate::ui::shared::composer_doc::is_image_name(n)))
                .count();
            let images = images.iter().skip(named).cloned().collect::<Vec<_>>();
            (crate::notes::split(text).0, images, !crate::notes::body(text).trim().is_empty())
        }
        _ => (Vec::new(), vec![], true),
    }));
    let notes = computed(move || parts.with(|p| p.0.clone()));
    let images = computed(move || parts.with(|p| p.1.clone()));
    let has_text = computed(move || parts.with(|p| p.2));
    view()
        .self_end()
        .items_end()
        .gap(px(6.0))
        .max_w(relative(0.82))
        .child(dynamic(move || {
            let notes = notes.get();
            (!notes.is_empty()).then(|| untracked(move || super::notes::sent_notes(notes).into_element()))
        }))
        .child(dynamic(move || {
            let images = images.get();
            (!images.is_empty()).then(|| {
                untracked(move || {
                    let mut thumbs = view().flex_row().flex_wrap().justify_end().gap(px(6.0));
                    for uri in &images {
                        thumbs = thumbs.child(
                            image(SharedString::from(uri.clone()))
                                .w(px(132.0))
                                .h(px(132.0))
                                .object_fit(nsh_core::ResizeMode::Cover)
                                .rounded(px(14.0))
                                .border_1()
                                .border_color(tint(BORDER(), 0.8)),
                        );
                    }
                    thumbs.into_element()
                })
            })
        }))
        // None for a message of notes or pictures alone.
        .child(when(move || has_text.get(), bubble))
}

fn tool_icon(kind: &str, name: &str) -> Icon {
    match kind {
        "shell" => Icon::SquareTerminal,
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

/// The message selected in place on the phone (its item's index), after
/// its "Select text": its words selectable where they are, all of them
/// selected, until the selection is cleared or Done.
#[derive(Clone, Copy, Default, PartialEq)]
struct SelectingText(Option<usize>);

fn selecting_text() -> Option<usize> {
    global::<SelectingText>().get().0
}

fn select_text(index: Option<usize>) {
    global::<SelectingText>().set(SelectingText(index));
}

/// A message being selected in place: `content` a selectable area, all of
/// it selected as it shows (handles on its ends and the system's menu:
/// Copy, Select All), and Done below. A tap that clears the selection is
/// done too.
fn select_in_place(content: View) -> View {
    let area = element_ref();
    let selected_once = signal(false);
    let select = area.clone();
    // Once its texts are laid out.
    set_timeout(std::time::Duration::from_millis(120), move || select.command("selectAll", nsh_core::JsonValue::Null));
    view()
        .gap(px(6.0))
        .child(
            content
                .selectable(true)
                .element_ref(&area)
                .on_text_selection_change(move |e: &nsh_core::element::TextSelectionEvent| {
                    if !e.text.is_empty() {
                        selected_once.set(true);
                    } else if selected_once.get_untracked() {
                        select_text(None);
                    }
                }),
        )
        .child(view().flex_row().justify_end().child(btn(t("Done"), Variant::Secondary, Sz::Sm, None, || select_text(None))))
}

/// A user message's actions on the phone: copy it, select in it, or rewind
/// to it.
fn message_sheet(item: Memo<Option<Item>>, index: Option<usize>) {
    let Some(Item::User { text: body, message_id, .. }) = item.get_untracked() else { return };
    let words = message_words(&body);
    let quoted = words.clone();
    let mut items = vec![
        SheetItem::new(t("Copy"), move || {
            let _ = set_clipboard(words.clone());
            store::notify(t("Copied").into());
        })
        .icon(Icon::Copy),
        SheetItem::new(t("Select text"), move || select_text(index)).icon(Icon::FileText),
        SheetItem::new(t("Attach as context"), move || crate::ui::shared::composer::attach_as_context(&quoted)).icon(Icon::TextQuote),
    ];
    if let Some(id) = message_id {
        let (a, b, c) = (id.clone(), id.clone(), id);
        items.push(SheetItem::new(t("Rewind conversation to here"), move || store::rewind(&a, "conversation")).icon(Icon::Undo2));
        items.push(SheetItem::new(t("Rewind files to here"), move || store::rewind(&b, "files")).icon(Icon::FileText));
        items.push(SheetItem::new(t("Rewind both to here"), move || store::rewind(&c, "both")).icon(Icon::RotateCw));
    }
    show_sheet(None, items);
}

/// A reply's actions on the phone (a long press): copy it whole, or select
/// part of it where it is.
fn reply_sheet(body: String, index: Option<usize>) {
    if body.trim().is_empty() {
        return;
    }
    let quoted = body.clone();
    show_sheet(None, vec![
        SheetItem::new(t("Copy"), move || {
            let _ = set_clipboard(body.clone());
            store::notify(t("Copied").into());
        })
        .icon(Icon::Copy),
        SheetItem::new(t("Select text"), move || select_text(index)).icon(Icon::FileText),
        SheetItem::new(t("Attach as context"), move || crate::ui::shared::composer::attach_as_context(&quoted)).icon(Icon::TextQuote),
    ]);
}

/// Rewind from a user message (the message menu).
/// The row under a written reply: copy, fork, quote, and its time, each
/// named under the pointer.
fn reply_actions(item: Memo<Option<Item>>, text_of: impl Fn() -> String + Copy + 'static, chosen: Signal<Chosen>, shown: Memo<bool>) -> impl IntoElement {
    let at = computed(move || match item.get() {
        Some(Item::Assistant { at, .. }) => at.as_deref().and_then(sent_at),
        _ => None,
    });
    let tip = |label: &'static str| text(label).text_size(tx(TEXT_XS)).px(sp(S2)).py(px(3.0)).rounded(px(R_MD)).bg(SURFACE0()).border_1().border_color(BORDER()).into_element();
    // Lit (the accent) while words in the reply are chosen: it quotes those.
    let lit_action = move |glyph: Icon, label: &'static str, lit: Memo<bool>, press: Box<dyn Fn()>| {
        view().tooltip(tip(label)).child(
            nsh_base::button()
                .on_press(move || press())
                .accessibility_label(label)
                .w(px(26.0))
                .h(px(26.0))
                .rounded(px(R_MD))
                .items_center()
                .justify_center()
                .hover(|s| s.bg(highlight()))
                .child(icon(glyph, 14.0).text_color(move || if lit.get() { ACCENT() } else { FG_MUTED() })),
        )
    };
    let action = move |glyph: Icon, label: &'static str, press: Box<dyn Fn()>| lit_action(glyph, label, computed(|| false), press);
    view()
        .flex_row()
        .items_center()
        .gap(px(2.0))
        .h(px(28.0))
        .mt(px(2.0))
        .ml(px(-6.0))
        .opacity(move || if shown.get() { 1.0_f32 } else { 0.0 })
        .transition_opacity()
        .pointer_events(move || if shown.get() { nsh_core::PointerEvents::Auto } else { nsh_core::PointerEvents::None })
        .child(action(
            Icon::Copy,
            t("Copy message"),
            Box::new(move || {
                let _ = set_clipboard(text_of());
                store::notify(t("Copied").into());
            }),
        ))
        .child(action(Icon::GitBranch, t("Fork conversation"), Box::new(store::fork_selected)))
        .child(lit_action(
            Icon::TextQuote,
            t("Quote in a message"),
            computed(move || chosen.with(|c| c.cleared.is_none() && !c.words.trim().is_empty())),
            Box::new(move || {
                let quoted = chosen.with_untracked(|c| c.words().map(str::to_owned)).unwrap_or_else(text_of);
                crate::ui::shared::composer::attach_as_context(&quoted);
            }),
        ))
        .child(text(move || at.get().unwrap_or_default()).text_size(tx(TEXT_XS)).text_color(FG_FAINT()).ml(sp(S1)))
}

/// When a message was sent, short: its time today ("8:38"), else its day
/// too ("11/20 8:38"), else its date.
fn sent_at(at: &str) -> Option<String> {
    let utc = crate::usage::parse::rfc3339(at)?;
    let local = utc + crate::usage::local_offset(utc);
    let now = crate::usage::now();
    let today = (now + crate::usage::local_offset(now)).div_euclid(86_400);
    let day = local.div_euclid(86_400);
    let (h, m) = (local.rem_euclid(86_400) / 3600, local.rem_euclid(3600) / 60);
    let (year, month, date) = crate::usage::civil(day);
    let (this_year, _, _) = crate::usage::civil(today);
    Some(if day == today {
        format!("{h}:{m:02}")
    } else if year == this_year {
        format!("{month}/{date} {h}:{m:02}")
    } else {
        format!("{year}/{month}/{date}")
    })
}

fn user_actions(item: Memo<Option<Item>>) -> View {
    let id = computed(move || match item.get() {
        Some(Item::User { message_id: Some(id), .. }) => Some(id),
        _ => None,
    });
    // The desktop, under the bubble (as T3 Code's): when it was sent, then
    // edit it (the conversation goes back to before it, its words in the
    // composer), copy it, and the other rewinds, each named under the
    // pointer. The phone copies from the bubble's own sheet.
    if !MOBILE {
        let body = computed(move || match item.get() {
            Some(Item::User { text, images, at, .. }) => Some((message_words(&text), images, at)),
            _ => None,
        });
        return view().h(px(28.0)).child(dynamic(move || {
            let (words, pictures, at) = body.get()?;
            let id = id.get();
            Some(untracked(move || {
                let tip = |label: &'static str| {
                    text(label).text_size(tx(TEXT_XS)).px(sp(S2)).py(px(3.0)).rounded(px(R_MD)).bg(SURFACE0()).border_1().border_color(BORDER()).into_element()
                };
                let action = |glyph: Icon, label: &'static str, press: Box<dyn Fn()>| {
                    view().tooltip(tip(label)).child(
                        nsh_base::button()
                            .on_press(move || press())
                            .accessibility_label(label)
                            .w(px(26.0))
                            .h(px(26.0))
                            .rounded(px(R_MD))
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(highlight()))
                            .child(icon(glyph, 14.0).text_color(FG_MUTED())),
                    )
                };
                let mut row = view().flex_row().items_center().gap(px(2.0));
                if let Some(at) = at.as_deref().and_then(sent_at) {
                    row = row.child(text(at).text_size(tx(TEXT_XS)).text_color(FG_FAINT()).mr(sp(S1)));
                }
                if let Some(id) = id.clone() {
                    let (again, said) = (id.clone(), words.clone());
                    row = row.child(action(Icon::Pencil, t("Edit message"), Box::new(move || store::edit_sent(&again, &said, &pictures))));
                }
                let copied = words.clone();
                row = row.child(action(
                    Icon::Copy,
                    t("Copy message"),
                    Box::new(move || {
                        let _ = set_clipboard(copied.clone());
                        store::notify(t("Copied").into());
                    }),
                ));
                if let Some(id) = id {
                    let (a, b, c) = (id.clone(), id.clone(), id);
                    row = row.child(
                        dropdown(
                            view()
                                .w(px(26.0))
                                .h(px(26.0))
                                .rounded(px(R_MD))
                                .items_center()
                                .justify_center()
                                .hover(|s| s.bg(highlight()))
                                .label(t("Message actions"))
                                .child(icon(Icon::Ellipsis, 14.0).text_color(FG_MUTED())),
                        )
                        .align(nsh_core::overlay::Align::End)
                        .child(menu_heading(t("Rewind to here")))
                        .child(menu_row(t("Conversation only"), Some(Icon::Undo2), None, false, move || store::rewind(&a, "conversation")))
                        .child(menu_row(t("Files only"), Some(Icon::FileText), None, false, move || store::rewind(&b, "files")))
                        .child(menu_row(t("Conversation and files"), Some(Icon::RotateCw), None, false, move || store::rewind(&c, "both"))),
                    );
                }
                row.into_element()
            }))
        }));
    }
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
                    .bg(SURFACE0())
                    .border_1()
                    .border_color(BORDER())
                    .child(icon(Icon::Ellipsis, 14.0).text_color(FG_MUTED()))
                    .into_element();
            }
            dropdown(
                view()
                    .w(px(22.0))
                    .h(px(22.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(R_FULL))
                    .bg(SURFACE0())
                    .border_1()
                    .border_color(BORDER())
                    .label(t("Message actions"))
                    .child(icon(Icon::Ellipsis, 13.0).text_color(FG_MUTED())),
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
/// To the start and to the end of the conversation: small glass discs
/// at the list's right, there while it is being scrolled (and under the
/// pointer), each only while the list is more than a screen from that end,
/// fading in and out; still, the conversation is left clear. The end's sits just above the
/// composer (on the phone, above its floating card).
fn jump_buttons() -> impl IntoElement {
    let ends = computed(store::timeline_ends);
    // Under the pointer the disc stays, so it can be reached once the
    // scrolling has stopped.
    let held = signal(false);
    let disc = move |glyph: Icon, label: &'static str, go: fn(), shown: Memo<bool>| {
        nsh_base::button()
            .on_press(go)
            .accessibility_label(label)
            .w(px(30.0))
            .h(px(30.0))
            .rounded(px(R_FULL))
            .items_center()
            .justify_center()
            .bg(glass_fill_strong())
            .backdrop_blur(px(GLASS_BLUR))
            .backdrop_saturate(1.7)
            .border_1()
            .border_color(glass_edge())
            .group()
            .active(|s| s.opacity(0.75))
            .opacity(move || if shown.get() { 1.0_f32 } else { 0.0 })
            .transition_opacity()
            .pointer_events(move || if shown.get() { PointerEvents::Auto } else { PointerEvents::None })
            .child(icon(glyph, 15.0).text_color(FG_MUTED()).group_hover(|s| s.text_color(FG())))
    };
    let live = move || store::timeline_scrolling() || held.get();
    let to_start = computed(move || live() && ends.with(|e| e.away_from_start));
    let to_end = computed(move || live() && ends.with(|e| e.away_from_end));
    // At the right, past the text's lines (centred they lay over the text).
    // The row is as wide as the list: it takes no pointer of its own, or
    // a strip across the conversation's first and last lines could not be
    // pressed or selected from.
    let row = || view().absolute().left(px(0.0)).right(px(0.0)).items_end().pr(sp(S6)).pointer_events(PointerEvents::BoxNone);
    view()
        .absolute()
        .top(px(0.0))
        .bottom(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .pointer_events(PointerEvents::BoxNone)
        .child(row().top(sp(S3)).child(hover_keeps(held, to_start, disc(Icon::ArrowUp, t("Go to the start"), store::jump_timeline_start, to_start))))
        .child(row().bottom(px(if MOBILE { 196.0 } else { S3 })).child(hover_keeps(held, to_end, disc(Icon::ArrowDown, t("Go to the end"), store::jump_timeline_end, to_end))))
}

/// `el`, `held` while the pointer is over it. Hidden, it is not there to
/// the pointer either: the text under it is pressed and selected through.
fn hover_keeps(held: Signal<bool>, shown: Memo<bool>, el: impl IntoElement) -> View {
    view()
        .pointer_events(move || if shown.get() { PointerEvents::Auto } else { PointerEvents::None })
        .on_mouse_enter(move |_: &MouseEvent| held.set(true))
        .on_mouse_leave(move |_: &MouseEvent| held.set(false))
        .child(el)
}

/// Within this of its end, the conversation's list keeps its end in place.
pub(crate) const FOLLOW_END: f32 = 40.0;

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
        .gap(sp(S2))
        .px(sp(S2))
        .h(px(26.0))
        // Set a point below the middle: centred, a row reads as floating;
        // a little low, it sits on its line as writing does.
        .pt(px(2.0))
        .rounded(px(R_LG))
        .hover(|s| s.bg(highlight()))
        .child(
            icon(Icon::ChevronRight, 13.0)
                .text_color(FG_FAINT())
                .rotate(move || Radians(if open.get() { std::f32::consts::FRAC_PI_2 } else { 0.0 }))
                .transition_transform(),
        )
        // What the run did, as an icon and a count per kind; the words
        // are in the tooltip.
        .child(dynamic(move || {
            let counts = summary.with(|s| s.counts());
            let tip = summary.with(|s| match s.failures() {
                Some(f) => format!("{} · {f}", s.describe()),
                None => s.describe(),
            });
            Some(untracked(move || {
                let mut row = view().flex_row().items_center().gap(sp(S3)).flex_shrink(1.0);
                for (kind, n) in counts {
                    row = row.child(count_chip(tool_icon(kind, ""), n, FG_MUTED()));
                }
                view()
                    .tooltip(
                        glass_strong(view())
                            .px(sp(S2))
                            .py(sp(S1))
                            .rounded(px(R_LG))
                            .child(text(tip).text_size(tx(TEXT_XS)))
                            .into_element(),
                    )
                    .child(row)
                    .into_element()
            }))
        }))
        .child(view().flex_1())
        .child(dynamic(move || {
            let (running, failed) = summary.with(|s| (s.running > 0, s.failed));
            // Failures are counted here, beside the row's status, so a
            // folded group hides no error and the red shows once.
            Some(untracked(move || {
                let mut end = view().flex_row().items_center().gap(sp(S2)).flex_shrink_0();
                if failed > 0 {
                    end = end.child(count_chip(Icon::X, failed, DANGER_MUTED));
                }
                if running {
                    end = end.child(status_dot(|| Status::Running));
                } else if failed == 0 {
                    end = end.child(icon(Icon::Check, 14.0).text_color(SUCCESS));
                }
                end.into_element()
            }))
        }))
}

/// A kind of call on a folded row: its icon and how many there were.
fn count_chip(glyph: Icon, n: usize, color: AdaptiveColor) -> impl IntoElement {
    view()
        .flex_row()
        .items_center()
        .gap(px(4.0))
        .flex_shrink_0()
        .child(icon(glyph, 14.0).text_color(color))
        // A digit stands above its line's middle and outgrows a 13 pt
        // glyph: one size down and nudged, it sits level with the icon.
        .child(text(n.to_string()).text_size(tx(TEXT_XS)).text_color(color).mt(px(1.0)))
}

/// Thoughts shown whole, by how they begin: the list recycles its rows,
/// and an index moves as earlier pages load.
#[derive(Clone, Default)]
struct OpenThoughts(std::collections::HashSet<String>);

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
        .gap(sp(S2))
        .px(sp(S2))
        .h(px(26.0))
        .rounded(px(R_LG))
        .hover(|s| s.bg(highlight()))
        .child(dynamic(move || {
            let g = glyph.get();
            Some(untracked(move || icon(g, 15.0).text_color(FG_MUTED())))
        }))
        .child(
            text(move || tool.with(|(n, ..)| n.clone()))
                .text_size(tx(TEXT_BASE))
                .font_weight(FontWeight(500))
                .text_color(FG())
                .flex_shrink_0(),
        )
        .child(
            text(move || tool.with(|(.., s)| s.clone()))
                .text_size(tx(TEXT_CODE))
                .font_family(mono_font)
                .text_color(FG_MUTED())
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
                        .text_color(FG_FAINT())
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
                "canceled" => icon(Icon::CircleX, 14.0).text_color(FG_FAINT()).into_element(),
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
                    .pr(sp(S2))
                    .pt(sp(S1))
                    .pb(sp(S2))
                    .child(super::tool_detail::tool_body(&kind, &detail, error.as_deref()))
                    .into_element()
            }))
        })))
        .child(header)
}

fn todo_card(item: Memo<Option<Item>>) -> impl IntoElement {
    let lines = computed(move || match item.get() {
        Some(Item::Todo { lines, .. }) => lines,
        _ => vec![],
    });
    card().p(sp(S3)).gap(px(6.0)).child(section_label(t("Tasks"))).child(dynamic(move || {
        let lines = lines.get();
        Some(untracked(move || {
            let mut list = view().gap(px(6.0));
            for crate::daemon::model::TodoLine { done, text: t, .. } in lines.clone() {
                list = list.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(icon(if done { Icon::CircleCheck } else { Icon::CircleDot }, 15.0).text_color(if done {
                            SUCCESS
                        } else {
                            FG_FAINT()
                        }))
                        .child(
                            text(t)
                                .text_size(tx(TEXT_BASE))
                                .text_color(if done { FG_MUTED() } else { FG() })
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
    view().w_full().max_w(px(CONTENT_MAX_WIDTH())).self_center().child(dynamic(move || {
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
    let mut buttons = view().flex_row().gap(sp(S2)).flex_wrap();
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
        // The host's own words for its answers, in the language shown.
        let label = if act.label.is_empty() { act.id.clone() } else { crate::i18n::tr(&act.label) };
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
        .gap(sp(S2))
        .child(icon(glyph, 16.0).text_color(tone))
        .child(text(title).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)).flex_1())
}

fn tool_card(p: Permission) -> View {
    card()
        .mb(sp(S2))
        .p(sp(S4))
        .gap(sp(S3))
        .border_color(tint(DOT_WARNING, 0.5))
        .child(card_title(Icon::TriangleAlert, WARNING, p.title.clone()))
        .when(!p.description.is_empty(), |v| {
            v.child(
                view().px(sp(S3)).py(sp(S2)).rounded(px(R_LG)).bg(SURFACE2()).child(
                    text(p.description.clone())
                        .text_size(tx(TEXT_CODE))
                        .font_family(mono_font)
                        .line_height(px(18.0))
                        .number_of_lines(8)
                        .text_color(FG())
                        .selectable(true),
                ),
            )
        })
        .child(action_buttons(&p))
}

fn plan_card(p: Permission, plan: String) -> View {
    card()
        .mb(sp(S2))
        .p(sp(S4))
        .gap(sp(S3))
        .child(card_title(Icon::ListTodo, ACCENT(), t("Plan ready for review").into()))
        .child(
            scroll().max_h(px(320.0)).child(copy_area(
                text_view()
                    // A link opens in the browser (or the app it belongs to).
                    .links(nsh_base::TextViewLinks::Open)
                    // A file's (Codex links the files it names): opened here.
                    .on_link_press(super::file_links::press_link)
                    // A copy button on each code block (under the pointer on the
                    // desktop, always on the phone).
                    .code_block_actions(vec![nsh_base::TextViewCodeAction::new("copy").with_label(crate::i18n::t("Copy"))])
.markdown(move || Some(SharedString::from(plan.clone())))
                    .selectable(true)
                    .gap_2()
                    .text_size(tx(TEXT_BASE))
                    .line_height(px(21.0))
                    .text_color(FG()),
            )),
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

    let mut body = view().gap(sp(S4));
    for (i, q) in questions.iter().enumerate() {
        let mut options = view().flex_row().flex_wrap().gap(sp(S2));
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
                .gap(sp(S2))
                .child(section_label(q.header.clone()))
                .child(text(q.question.clone()).text_size(tx(TEXT_BASE)).line_height(px(20.0)))
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
        .mb(sp(S2))
        .p(sp(S4))
        .gap(sp(S4))
        .child(card_title(Icon::CircleDot, ACCENT(), p.title.clone()))
        .child(scroll().max_h(px(380.0)).child(body))
        .child(
            view()
                .flex_row()
                .gap(sp(S2))
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

/// The most a track's list takes over the composer before it scrolls:
/// three 30pt rows and the gaps between them.
const SUBAGENT_ROWS_MAX_H: f32 = 3.0 * 30.0 + 2.0 * 2.0;

/// The tasks a folded todo list shows, as Claude Code's: so many at
/// most, the rest one line ("… +2 pending, 3 completed").
const TODO_SHOWN: usize = 3;

/// One part of the line over the composer.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Track {
    Todo,
    Subagents,
    Background,
}

/// The agent's todo list, its sub-agents and its background work, as one
/// line over the composer: a part for each there is ("Tasks · 3/7 done",
/// "Sub-agents 2 · 1 running", "Background tasks 1"). A press on a part
/// opens its list under the line, one at a time.
pub(crate) fn tracks() -> View {
    let todos = computed(|| {
        (0..store::item_count()).rev().find_map(|i| match store::item_at(i) {
            Some(Item::Todo { lines, .. }) => Some(lines),
            _ => None,
        })
    });
    let subs = computed(|| store::selected_id().map(|id| store::subagents(&id)).unwrap_or_default());
    let work = computed(|| store::selected_id().map(|id| store::background(&id)).unwrap_or_default());
    // The part opened, as the user left it (`None`: not chosen yet, the
    // sub-agents' while one runs).
    let chosen = signal(None::<Option<Track>>);
    // The whole todo list, past the three a folded one shows.
    let all_todos = signal(false);
    let parts = computed(move || {
        let mut parts = Vec::new();
        if todos.with(|t| t.as_ref().is_some_and(|l| !l.is_empty() && !l.iter().all(|l| l.done))) {
            parts.push(Track::Todo);
        }
        // Over the composer while one runs. Once all have ended they are
        // out of the conversation's way: the agent's menu lists them again.
        let shown = subs.with(|l| {
            !l.is_empty() && (l.iter().any(|s| s.status == "running") || l.first().is_some_and(|s| store::subagents_shown(&s.parent)))
        });
        if shown {
            parts.push(Track::Subagents);
        }
        if work.with(|w| !w.is_empty()) {
            parts.push(Track::Background);
        }
        parts
    });
    let open = computed(move || {
        let parts = parts.get();
        let running = subs.with(|l| l.iter().any(|s| s.status == "running"));
        let want = chosen.get().unwrap_or(running.then_some(Track::Subagents));
        want.filter(|t| parts.contains(t))
    });
    view().w_full().max_w(px(CONTENT_MAX_WIDTH())).self_center().child(dynamic(move || {
        let parts = parts.get();
        if parts.is_empty() {
            return None;
        }
        let open = open.get();
        Some(untracked(move || {
            let mut line = view().flex_row().items_center().gap(sp(S1)).h(sp(CONTROL_SM)).min_w(px(0.0));
            for part in parts.iter().copied() {
                line = line.child(track_tab(part, open == Some(part), todos, subs, work, move || {
                    chosen.set(Some(if open == Some(part) { None } else { Some(part) }));
                }));
            }
            let list = open.map(|part| {
                let list = match part {
                    Track::Todo => todo_rows(todos.get_untracked().unwrap_or_default(), all_todos).into_element(),
                    Track::Subagents => subagent_rows(subs.get_untracked()).into_element(),
                    Track::Background => background_rows(work.get_untracked()).into_element(),
                };
                scroll().max_h(px(SUBAGENT_ROWS_MAX_H + 26.0)).child(list)
            });
            // On the phone it floats over the conversation: a card keeps it
            // readable. On the desktop the line is bare, as the composer's
            // own row under it; only a list opened gets a card, under it.
            if MOBILE {
                let mut c = card().mb(sp(S2)).p(px(4.0)).child(line);
                if let Some(list) = list {
                    c = c.child(view().mt(px(4.0)).pt(px(2.0)).border_t_1().border_color(BORDER()).child(list));
                }
                return c.into_element();
            }
            view()
                .mb(sp(S1))
                .child(line.px(px(4.0)))
                .when_some(list, |v, list| v.child(card().mt(px(2.0)).p(px(4.0)).child(list)))
                .into_element()
        }))
    }))
}

/// A part of the tracks' line: what it is and how many, pressed, its list.
fn track_tab(
    part: Track,
    open: bool,
    todos: Memo<Option<Vec<crate::daemon::model::TodoLine>>>,
    subs: Memo<Vec<crate::daemon::connection::Subagent>>,
    work: Memo<Vec<crate::daemon::connection::BackgroundItem>>,
    on_press: impl Fn() + 'static,
) -> impl IntoElement {
    let tab = nsh_base::button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(sp(CONTROL_SM))
        .px(sp(S2))
        .rounded(px(R_FULL))
        .min_w(px(0.0))
        .bg(move || if open { highlight() } else { tint(FG(), 0.0) })
        .hover(|st| st.bg(highlight()));
    let tab = match part {
        Track::Todo => {
            let lines = todos.get_untracked().unwrap_or_default();
            let done = lines.iter().filter(|l| l.done).count();
            // Under way, in its own wording; else the next to do.
            let now = lines.iter().find_map(|l| l.doing.clone()).or_else(|| lines.iter().find(|l| !l.done).map(|l| l.text.clone()));
            tab.flex_shrink(1.0)
                .child(icon(Icon::ListTodo, 14.0).text_color(ACCENT()))
                .child(
                    text(tf("Tasks · {}/{} done", &[&done.to_string(), &lines.len().to_string()]))
                        .text_size(tx(TEXT_SM))
                        .font_weight(FontWeight(500)).text_color(tint(FG(), 0.8))
                        .flex_shrink_0(),
                )
                .when_some(now, |v, now| v.child(text(now).text_size(tx(TEXT_SM)).text_color(FG_FAINT()).number_of_lines(1).flex_shrink(1.0)))
        }
        Track::Subagents => {
            let list = subs.get_untracked();
            let running = list.iter().filter(|s| s.status == "running").count();
            tab.flex_shrink_0()
                .child(status_dot(move || if running > 0 { Status::Running } else { Status::Done }))
                .child(text(tf("Sub-agents {}", &[&list.len().to_string()])).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(tint(FG(), 0.8)))
                .when(running > 0, |v| {
                    v.child(text(tf("· {} running", &[&running.to_string()])).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
                })
        }
        Track::Background => tab
            .flex_shrink_0()
            .child(status_dot(|| Status::Running))
            .child(text(format!("{} {}", t("Background task"), work.get_untracked().len())).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(tint(FG(), 0.8))),
    };
    tab.child(icon(if open { Icon::ChevronDown } else { Icon::ChevronRight }, 13.0).text_color(FG_FAINT()))
}

/// A todo list as Claude Code shows it: three at most when folded (under
/// way first, then to do, then done, in their own order), the rest one
/// line that, pressed, shows them all.
fn todo_rows(lines: Vec<crate::daemon::model::TodoLine>, all: Signal<bool>) -> impl IntoElement {
    dynamic(move || {
        let whole = all.get();
        let lines = lines.clone();
        Some(untracked(move || {
            let rank: Vec<usize> = (0..lines.len())
                .filter(|&i| lines[i].doing.is_some())
                .chain((0..lines.len()).filter(|&i| !lines[i].done && lines[i].doing.is_none()))
                .chain((0..lines.len()).filter(|&i| lines[i].done))
                .collect();
            let shown: Vec<usize> = if whole || lines.len() <= TODO_SHOWN { (0..lines.len()).collect() } else {
                let mut s = rank[..TODO_SHOWN].to_vec();
                s.sort_unstable();
                s
            };
            let rest: Vec<usize> = rank.iter().copied().filter(|i| !shown.contains(i)).collect();
            let mut col = view().gap(px(2.0));
            for i in shown {
                let l = &lines[i];
                let (glyph, color) = match (l.done, l.doing.is_some()) {
                    (true, _) => (Icon::Check, DOT_SUCCESS),
                    (false, true) => (Icon::Square, ACCENT()),
                    _ => (Icon::Square, FG_FAINT()),
                };
                col = col.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .h(px(26.0))
                        .px(sp(S2))
                        .child(icon(glyph, 13.0).text_color(color))
                        .child(
                            text(l.text.clone())
                                .text_size(tx(TEXT_SM))
                                .number_of_lines(1)
                                .flex_1()
                                .font_weight(FontWeight(if l.doing.is_some() { 600 } else { 400 }))
                                .text_color(if l.doing.is_some() { FG() } else { FG_MUTED() }),
                        ),
                );
            }
            if !rest.is_empty() || (whole && lines.len() > TODO_SHOWN) {
                let label = if rest.is_empty() {
                    t("Show less").to_owned()
                } else {
                    let count = |f: &dyn Fn(&crate::daemon::model::TodoLine) -> bool| rest.iter().filter(|&&i| f(&lines[i])).count();
                    let mut said = Vec::new();
                    for (n, key) in [
                        (count(&|l| l.doing.is_some()), "{} in progress"),
                        (count(&|l| !l.done && l.doing.is_none()), "{} pending"),
                        (count(&|l| l.done), "{} completed"),
                    ] {
                        if n > 0 {
                            said.push(tf(key, &[&n.to_string()]));
                        }
                    }
                    format!("… +{}", said.join(t(", ")))
                };
                col = col.child(
                    nsh_base::button()
                        .on_press(move || all.update(|a| *a = !*a))
                        .flex_row()
                        .items_center()
                        .h(px(26.0))
                        .px(sp(S2))
                        .rounded(px(R_LG))
                        .hover(|st| st.bg(highlight()))
                        .child(text(label).text_size(tx(TEXT_SM)).text_color(FG_FAINT())),
                );
            }
            col.into_element()
        }))
    })
}

/// The sub-agents' rows, running ones first; a press opens one's tab.
fn subagent_rows(mut list: Vec<crate::daemon::connection::Subagent>) -> View {
    list.sort_by_key(|s| s.status != "running");
    let mut rows = view().gap(px(2.0));
    for s in list {
        let status = match s.status.as_str() {
            "running" => Status::Running,
            "failed" => Status::Failed,
            "completed" => Status::Done,
            _ => Status::Idle,
        };
        let (parent, id) = (s.parent.clone(), s.id.clone());
        let mut row = view().flex_row().items_center().child(
            nsh_base::button()
                .on_press(move || store::open_tab(store::Tab::Subagent(parent.clone(), id.clone())))
                .flex_1()
                .min_w(px(0.0))
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .h(px(30.0))
                .px(sp(S2))
                .rounded(px(R_LG))
                .hover(|st| st.bg(highlight()))
                .child(status_dot(move || status))
                .child(text(s.title.clone()).text_size(tx(TEXT_BASE)).number_of_lines(1).flex_shrink(1.0))
                .child(text(s.subtitle.clone()).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).number_of_lines(1).flex_1()),
        );
        // One in the background stops from here: it is not among the background work.
        if let Some(token) = s.stop_token.clone().filter(|_| s.status == "running") {
            let (agent, item) = (s.parent.clone(), s.id.clone());
            row = row.child(icon_btn(Icon::X, Sz::Sm, "Stop", move || store::stop_background(&agent, &item, &token)));
        }
        rows = rows.child(row);
    }
    rows
}

/// The background work's rows: what each is for, and its stop.
fn background_rows(list: Vec<crate::daemon::connection::BackgroundItem>) -> View {
    let mut rows = view().gap(px(2.0));
    for w in list {
        let mut row = view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .h(px(30.0))
            .px(sp(S2))
            .child(status_dot(|| Status::Running))
            .child(text(if w.label.is_empty() { t("Background task").to_owned() } else { w.label.clone() }).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).number_of_lines(1).flex_1());
        if let Some(token) = w.stop_token.clone() {
            let item = w.id.clone();
            row = row.child(icon_btn(Icon::X, Sz::Sm, "Stop", move || {
                if let Some(agent) = store::selected_id() {
                    store::stop_background(&agent, &item, &token);
                }
            }));
        }
        rows = rows.child(row);
    }
    rows
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
                .gap(sp(S2))
                .h(px(40.0))
                .px(sp(S6))
                .border_b_1()
                .border_color(BORDER())
                .child(icon(Icon::Bot, 15.0).text_color(FG_MUTED()))
                .child(text(move || info.with(|i| i.as_ref().map(|i| i.title.clone()).unwrap_or_default())).text_size(tx(TEXT_BASE)).number_of_lines(1).flex_1())
                .child(text(move || info.with(|i| i.as_ref().map(|i| i.status.clone()).unwrap_or_default())).text_size(tx(TEXT_SM)).text_color(FG_MUTED())),
        )
        .child(dynamic(move || {
            let list = items.get();
            Some(untracked(move || match list.clone() {
                None => view().flex_1().items_center().justify_center().child(loader(None)).into_element(),
                Some(list) => {
                    let mut col = view().gap(sp(S2)).px(sp(S6)).py(sp(S4)).w_full().max_w(px(CONTENT_MAX_WIDTH() + 2.0 * S6)).self_center();
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
            let item = computed(move || Some(Item::User { text: t.clone(), message_id: None, images: vec![], at: None }));
            view()
                .self_end()
                .max_w(relative(0.82))
                .px(px(14.0))
                .py(px(9.0))
                .rounded(px(18.0))
                .bg(SURFACE2())
                .child(user_text(item, true))
                .into_element()
        }
        Item::Assistant { text: t, .. } => copy_area(text_view()
            // A link opens in the browser (or the app it belongs to).
            .links(nsh_base::TextViewLinks::Open)
            // A file's (Codex links the files it names): opened here.
            .on_link_press(super::file_links::press_link)
                    // A copy button on each code block (under the pointer on the
                    // desktop, always on the phone).
                    .code_block_actions(vec![nsh_base::TextViewCodeAction::new("copy").with_label(crate::i18n::t("Copy"))])
.markdown(move || Some(SharedString::from(t.clone())))
            .selectable(true)
            .gap_3()
            .text_size(tx(TEXT_CONTENT))
            .line_height(px(24.0))
            .text_color(FG()))
            .into_element(),
        Item::Tool { name, kind, summary, .. } => view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .px(sp(S2))
            .h(px(30.0))
            .child(icon(tool_icon(&kind, &name), 15.0).text_color(FG_MUTED()))
            .child(text(name).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)))
            .child(text(summary).text_size(tx(TEXT_CODE)).font_family(mono_font).text_color(FG_MUTED()).number_of_lines(1).flex_1())
            .into_element(),
        Item::Reasoning { text: t } => text(t).text_size(px(13.0)).italic().number_of_lines(3).text_color(FG_MUTED()).into_element(),
        Item::Error { message } => text(message).text_size(tx(TEXT_BASE)).text_color(DANGER).into_element(),
        Item::Notice { message } => text(message).text_size(tx(TEXT_SM)).text_color(FG_FAINT()).into_element(),
        retry @ Item::Retry { .. } => text(retry_words(&retry, false)).text_size(tx(TEXT_SM)).text_color(WARNING).into_element(),
        Item::Todo { lines, .. } => text(lines.iter().map(|l| format!("{} {}", if l.done { "☑" } else { "☐" }, l.text)).collect::<Vec<_>>().join("\n"))
            .text_size(tx(TEXT_BASE))
            .into_element(),
    }
}

/// What a retry row says: while Claude Code waits to try again, the
/// attempt that failed, of how many, and why; afterwards, how many it took.
fn retry_words(item: &Item, waiting: bool) -> String {
    let Item::Retry { attempt, max, status, error, delay_ms } = item else { return String::new() };
    let why = match status {
        Some(s) => format!("{s} {error}"),
        None => error.clone(),
    };
    let why = why.trim();
    if waiting {
        let secs = delay_ms.div_ceil(1000).max(1).to_string();
        tf("Request failed ({}) · retrying {}/{} in {} s", &[&why, &attempt.to_string(), &max.to_string(), &secs])
    } else {
        tf("Request retried {} times ({})", &[&attempt.to_string(), &why])
    }
}

/// Sends the last message again (interrupting a wait between retries).
fn retry_button(tone: AdaptiveColor) -> impl IntoElement {
    nsh_base::button()
        .on_press(store::retry_last)
        .accessibility_label(t("Retry"))
        .flex_row()
        .items_center()
        .gap(sp(S1))
        .px(sp(S2))
        .h(px(24.0))
        .rounded(px(R_FULL))
        .bg(tint(tone, 0.12))
        .active(|s| s.opacity(0.6))
        .child(icon(Icon::RotateCw, 12.0).text_color(tone))
        .child(text(t("Retry")).text_size(tx(TEXT_SM)).font_weight(FontWeight(600)).text_color(tone))
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
            .gap(sp(S1))
            .h(px(40.0))
            .pl(sp(S3))
            .pr(sp(S1))
            .rounded(px(R_XL));
        let panel = if MOBILE { panel.mx(sp(S2)).mb(sp(S2)) } else { panel.absolute().top(sp(S2)).right(sp(S6)).w(px(360.0)) };
        panel
            .child(icon(Icon::Search, 15.0).text_color(FG_MUTED()))
            .child(
                nsh_core::element::input()
                    .value(|| store::find().query)
                    .on_change(|v: &str| store::set_find_query(v))
                    .on_submit(|_: &str| store::find_step(true))
                    .submit_behavior(SubmitBehavior::Submit)
                    .element_ref(&field)
                    .placeholder(t("Find in conversation"))
                    .auto_focus(true)
                    .text_size(tx(TEXT_BASE))
                    .text_color(FG())
                    .caret_color(|| color(ACCENT()))
                    .h(px(28.0))
                    .ml(sp(S1))
                    .flex_1()
                    .min_w(px(0.0)),
            )
            .child(
                text(move || match (count.get(), asked.get()) {
                    (0, true) => t("No results").to_owned(),
                    (0, false) => String::new(),
                    (n, _) => format!("{} / {n}", cur.get().min(n - 1) + 1),
                })
                .text_size(tx(TEXT_SM))
                .text_color(FG_MUTED())
                .tabular_nums()
                .number_of_lines(1)
                .flex_shrink_0(),
            )
            .child(view().w(px(1.0)).h(px(18.0)).mx(sp(S1)).bg(BORDER()))
            .child(icon_btn(Icon::ChevronUp, Sz::Sm, t("Previous match"), move || previous(false)))
            .child(icon_btn(Icon::ChevronDown, Sz::Sm, t("Next match"), move || next(true)))
            .child(icon_btn(Icon::X, Sz::Sm, t("Close find"), store::toggle_find))
    })
}

/// While the agent runs and nothing it says is streaming: the mark
/// breathing at the foot of the conversation, so a sent message never sits
/// there without an answer in sight.
/// The words a turn goes by, one per turn, as Claude Code's status line
/// has them.
const VERBS_EN: [&str; 16] = [
    "Thinking", "Pondering", "Brewing", "Fermenting", "Percolating", "Mulling", "Simmering", "Tinkering",
    "Crafting", "Musing", "Conjuring", "Noodling", "Ruminating", "Whirring", "Cogitating", "Marinating",
];
const VERBS_ZH: [&str; 12] = ["思考中", "琢磨中", "酝酿中", "发酵中", "推敲中", "构思中", "打磨中", "冥想中", "捣鼓中", "盘算中", "斟酌中", "沉吟中"];

thread_local! {
    /// When each agent's running turn was first seen running here.
    static TURN_STARTED: std::cell::RefCell<std::collections::HashMap<String, std::time::Instant>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// About how many tokens `text` is: four letters of English to a token,
/// a character of Chinese (or other wide script) to one.
fn tokens_in(text: &str) -> usize {
    let (narrow, wide) = text.chars().fold((0usize, 0usize), |(n, w), c| if c.is_ascii() { (n + 1, w) } else { (n, w + 1) });
    narrow / 4 + wide
}

/// "3.2k", "860".
fn short_count(n: usize) -> String {
    if n >= 1000 { format!("{:.1}k", n as f64 / 1000.0) } else { n.to_string() }
}

/// "53s", "2m 05s".
fn short_time(secs: u64) -> String {
    if secs < 60 { format!("{secs}s") } else { format!("{}m {:02}s", secs / 60, secs % 60) }
}

/// While the agent works, at the conversation's foot, as Claude Code's
/// status line: the app's loading mark (the eclipse, as the sidebar has it
/// for a conversation that runs; Claude Code's star was a letter of
/// whatever font had it, small and off its place), the turn's word, how long it has run and
/// about how many tokens it has written so far (its messages, thinking and
/// calls since the user's message, counted here: the service says the
/// turn's tokens only at its end).
fn thinking(running: Memo<bool>) -> impl IntoElement {
    let agent = computed(store::selected_id);
    // The turn's start, kept per agent while it runs.
    effect(move || {
        let (Some(id), on) = (agent.get(), running.get()) else { return };
        TURN_STARTED.with_borrow_mut(|m| {
            if on {
                m.entry(id).or_insert_with(std::time::Instant::now);
            } else {
                m.remove(&id);
            }
        });
    });
    // What the turn has written: everything after the user's last message.
    let written = computed(move || {
        let count = store::item_count();
        let mut tokens = 0;
        for i in (0..count).rev() {
            match store::item_at(i) {
                Some(Item::User { .. }) | None => break,
                Some(Item::Assistant { text, .. } | Item::Reasoning { text }) => tokens += tokens_in(&text),
                Some(Item::Tool { summary, detail, .. }) => tokens += tokens_in(&summary) + tokens_in(&detail.to_string()) / 4,
                Some(_) => {}
            }
        }
        tokens
    });
    view().child(dynamic(move || {
        if !running.get() {
            return None;
        }
        let id = agent.get_untracked().unwrap_or_default();
        Some(untracked(move || {
            let started = TURN_STARTED.with_borrow(|m| m.get(&id).copied()).unwrap_or_else(std::time::Instant::now);
            // One word for the turn, by when it began.
            let seed = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                format!("{started:?}{id}").hash(&mut h);
                h.finish() as usize
            };
            let verb = if crate::i18n::is_zh() { VERBS_ZH[seed % VERBS_ZH.len()] } else { VERBS_EN[seed % VERBS_EN.len()] };
            // The time goes on with each tick.
            let tick = signal(0u64);
            spawn(async move {
                loop {
                    nsh_core::element::timer(std::time::Duration::from_secs(1)).await;
                    tick.update(|t| *t += 1);
                }
            });
            let detail = move || {
                tick.get();
                let secs = started.elapsed().as_secs();
                let tokens = written.get();
                if tokens == 0 {
                    format!("({})", short_time(secs))
                } else {
                    format!("({} · ↓ {} tokens)", short_time(secs), short_count(tokens))
                }
            };
            view()
                .w_full()
                .max_w(px(CONTENT_MAX_WIDTH() + 2.0 * S6))
                .self_center()
                // The rows' own inset, so the words line up with the text.
                .px(sp(S6))
                .py(sp(S3))
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .child(spark(14.0))
                .child(text(format!("{verb}…")).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(ACCENT()))
                .child(text(detail).text_size(tx(TEXT_SM)).text_color(FG_FAINT()))
                .into_element()
        }))
    }))
}

#[cfg(test)]
mod status_tests {
    use super::{short_count, short_time, tokens_in};

    #[test]
    fn the_status_line_counts_and_times_as_claude_code_does() {
        assert_eq!(tokens_in("abcdefgh"), 2);
        assert_eq!(tokens_in("你好"), 2);
        assert_eq!(short_count(860), "860");
        assert_eq!(short_count(3200), "3.2k");
        assert_eq!(short_time(53), "53s");
        assert_eq!(short_time(125), "2m 05s");
    }
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

#[cfg(test)]
mod copy_tests {
    use super::{CONTEXT_MARK, message_words};

    #[test]
    fn a_copied_message_leaves_another_conversation_s_words_out() {
        assert_eq!(message_words("fix the tab bar"), "fix the tab bar");
        assert_eq!(message_words(&format!("look at this{CONTEXT_MARK}other\" said …")), "look at this");
    }
}
