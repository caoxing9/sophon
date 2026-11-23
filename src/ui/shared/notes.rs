//! Notes on a reply, drawn: words chosen in a reply offer "Note" beside
//! them; pressed, a card opens there with the words quoted and a field
//! for what to say about them. The notes wait in the composer as an
//! "N notes" chip, go ahead of the next message (`crate::notes`), and
//! show as the same chip on the message sent.

use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};

use crate::{
    i18n::{t, tf},
    notes::Note,
    store,
    ui::kit::{text, *},
};

/// Where words are chosen in a reply (in its view's coordinates) and the
/// words.
pub(crate) type Chosen = Option<(f32, f32, String)>;

fn count_label(n: usize) -> String {
    if n == 1 { t("1 note").to_owned() } else { tf("{} notes", &[&n.to_string()]) }
}

/// The chip that stands for notes: "N notes", pressed, `on_press`.
fn chip(n: usize, small: bool, on_press: impl Fn() + 'static) -> impl IntoElement {
    nsh_base::button()
        .on_press(on_press)
        .accessibility_label(count_label(n))
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(if small { 24.0 } else { 28.0 }))
        .px(px(10.0))
        .rounded(px(9.0))
        .border_1()
        .border_color(BORDER())
        .bg(tint(FG(), 0.04))
        .hover(|s| s.bg(tint(FG(), 0.08)))
        .child(icon(Icon::MessageSquare, 13.0).text_color(FG_MUTED()))
        .child(text(count_label(n)).text_size(tx(if small { TEXT_SM } else { TEXT_BASE })).text_color(FG()))
}

/// One note as a list shows it: its words quoted, under them the comment.
fn note_row(note: &Note, remove: Option<Box<dyn Fn()>>) -> View {
    let mut row = view()
        .flex_row()
        .items_start()
        .gap(sp(S2))
        .px(sp(S2))
        .py(px(6.0))
        .rounded(px(R_LG))
        .bg(tint(FG(), 0.04))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(3.0))
                .child(
                    view()
                        .border_l(px(2.0))
                        .border_color(BORDER_STRONG())
                        .pl(sp(S2))
                        .child(text(note.quote.clone()).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED()).number_of_lines(2)),
                )
                .when(!note.comment.is_empty(), |v| v.child(text(note.comment.clone()).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(FG()))),
        );
    if let Some(remove) = remove {
        row = row.child(icon_btn(Icon::X, Sz::Sm, t("Remove note"), move || remove()));
    }
    row
}

/// The notes waiting in the composer for the next message: a chip, and
/// pressed, their list (each may go).
pub(crate) fn composer_notes(live: bool) -> impl IntoElement {
    let open = signal(false);
    let list = computed(move || if live { store::selected_id().map(|id| store::notes(&id)).unwrap_or_default() } else { Vec::new() });
    dynamic(move || {
        let notes = list.get();
        if notes.is_empty() {
            open.set(false);
            return None;
        }
        Some(untracked(move || {
            let n = notes.len();
            let mut col = view().w_full().gap(px(6.0)).px(sp(S1)).pb(sp(S1)).child(view().flex_row().child(chip(n, false, move || open.update(|o| *o = !*o))));
            col = col.child(when(move || open.get(), move || {
                let mut items = view().gap(px(6.0));
                for (i, note) in notes.iter().enumerate() {
                    items = items.child(note_row(
                        note,
                        Some(Box::new(move || {
                            if let Some(id) = store::selected_id() {
                                store::remove_note(&id, i);
                            }
                        })),
                    ));
                }
                items
            }));
            col.into_element()
        }))
    })
}

/// A sent message's notes: the chip over its bubble, pressed, their list.
pub(crate) fn sent_notes(notes: Vec<Note>) -> View {
    let open = signal(false);
    let n = notes.len();
    view()
        .items_end()
        .gap(px(6.0))
        .child(chip(n, true, move || open.update(|o| *o = !*o)))
        .child(when(move || open.get(), move || {
            let mut items = view().gap(px(6.0)).max_w(px(480.0));
            for note in &notes {
                items = items.child(note_row(note, None));
            }
            items
        }))
}

/// Over a reply: "Note" where words are chosen in it (`chosen`), and the
/// card a press on it opens there (`open`).
pub(crate) fn selection_layer(chosen: Signal<Chosen>, open: Signal<Chosen>) -> impl IntoElement {
    view()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .pointer_events(nsh_core::PointerEvents::BoxNone)
        .child(dynamic(move || {
            let (x, y, words) = chosen.get()?;
            if open.with(Option::is_some) {
                return None;
            }
            Some(untracked(move || {
                nsh_base::button()
                    .on_press(move || {
                        open.set(Some((x, y, words.clone())));
                        chosen.set(None);
                    })
                    .accessibility_label(t("Note"))
                    .absolute()
                    .left(px((x - 34.0).max(0.0)))
                    .top(px(y - 40.0))
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .h(px(28.0))
                    .px(px(10.0))
                    .rounded(px(R_LG))
                    .bg(POPOVER())
                    .border_1()
                    .border_color(BORDER())
                    .shadow_md()
                    // Solid, so the chosen words under it don't show through.
                    .hover(|s| s.bg(SURFACE3()).border_color(BORDER_STRONG()))
                    .child(icon(Icon::MessageSquare, 13.0).text_color(FG()))
                    .child(text(t("Note")).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG()))
                    .into_element()
            }))
        }))
        .child(dynamic(move || {
            let (x, y, words) = open.get()?;
            Some(untracked(move || {
                let typed = signal(String::new());
                let close = move || open.set(None);
                on_key("escape", move |_| close());
                let quote = words.clone();
                view()
                    .absolute()
                    .left(px((x - 40.0).max(0.0)))
                    // Above the words when there is room, else under them.
                    .top(px(if y > 140.0 { y - 128.0 } else { y + 16.0 }))
                    .w(px(440.0))
                    .px(sp(S4))
                    .py(sp(S3))
                    .gap(sp(S2))
                    .rounded(px(16.0))
                    .bg(POPOVER())
                    .border_1()
                    .border_color(BORDER())
                    .shadow_lg()
                    .child(
                        view()
                            .border_l(px(2.0))
                            .border_color(BORDER_STRONG())
                            .pl(sp(S2))
                            .child(text(quote).text_size(tx(TEXT_BASE)).line_height(px(20.0)).text_color(FG_MUTED()).number_of_lines(3)),
                    )
                    .child(
                        nsh_base::input()
                            .auto_focus(true)
                            .h(px(28.0))
                            .text_size(tx(TEXT_CONTENT))
                            .text_color(FG())
                            .caret_color(|| color(ACCENT()))
                            .placeholder(t("Reply"))
                            .value(move || typed.get())
                            .on_change(move |v: &str| typed.set(v.to_owned()))
                            .on_submit(move |v: &str| {
                                store::add_note(Note { quote: words.clone(), comment: v.trim().to_owned() });
                                close();
                            })
                            // Let go empty, it closes; with words, it stays.
                            .on_blur(move || {
                                if typed.with_untracked(|t| t.trim().is_empty()) {
                                    close();
                                }
                            }),
                    )
                    .into_element()
            }))
        }))
}
