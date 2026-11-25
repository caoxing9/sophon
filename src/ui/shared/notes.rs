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

/// Where words are chosen in a reply and the words.
pub(crate) type Chosen = Option<(Anchor, String)>;

/// Where chosen words stand in their reply's coordinates: the top of
/// their first line, the bottom of their last, and their middle across.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Anchor {
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    pub(crate) center: f32,
}

impl Anchor {
    pub(crate) fn of(b: Bounds<Points>) -> Self {
        Anchor { top: b.origin.y.0, bottom: b.origin.y.0 + b.size.height.0, center: b.origin.x.0 + b.size.width.0 / 2.0 }
    }

    /// The top of a box `h` tall that goes over the words, clear of them,
    /// or under them where there is no room above.
    fn top_for(&self, h: f32) -> f32 {
        let above = self.top - h - GAP;
        if above >= 0.0 { above } else { self.bottom + GAP }
    }
}

/// Between the words and what goes over them.
const GAP: f32 = 6.0;

fn count_label(n: usize) -> String {
    if n == 1 { t("1 note").to_owned() } else { tf("{} notes", &[&n.to_string()]) }
}

fn drafts_label(n: usize) -> String {
    if n == 1 { t("1 draft").to_owned() } else { tf("{} drafts", &[&n.to_string()]) }
}

/// The chip that stands for notes: "2 notes · 1 draft", pressed,
/// `on_press`.
fn chip_of(notes: usize, drafts: usize, small: bool, on_press: impl Fn() + 'static) -> impl IntoElement {
    let label = match (notes, drafts) {
        (n, 0) => count_label(n),
        (0, d) => drafts_label(d),
        (n, d) => format!("{} · {}", count_label(n), drafts_label(d)),
    };
    nsh_base::button()
        .on_press(on_press)
        .accessibility_label(label.clone())
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
        .when(notes > 0, |v| v.child(text(count_label(notes)).text_size(tx(if small { TEXT_SM } else { TEXT_BASE })).text_color(FG())))
        .when(notes > 0 && drafts > 0, |v| v.child(text("·").text_size(tx(TEXT_SM)).text_color(FG_FAINT())))
        .when(drafts > 0, |v| v.child(text(drafts_label(drafts)).text_size(tx(if small { TEXT_SM } else { TEXT_BASE })).text_color(FG_MUTED())))
}

/// One note as a list shows it: its words quoted, under them the comment.
/// A draft's is dashed, its comment fainter after a pencil.
fn note_row(note: &Note, remove: Option<Box<dyn Fn()>>) -> View {
    let draft = note.draft;
    let comment = (!note.comment.is_empty() || draft).then(|| {
        view()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .when(draft, |v| v.child(icon(Icon::Pencil, 12.0).text_color(FG_FAINT())))
            .child(
                text(note.comment.clone())
                    .text_size(tx(TEXT_SM))
                    .line_height(px(18.0))
                    .text_color(if draft { FG_MUTED() } else { FG() })
                    .flex_1(),
            )
    });
    let mut row = view()
        .flex_row()
        .items_start()
        .gap(sp(S2))
        .px(sp(S2))
        .py(px(6.0))
        .rounded(px(R_LG))
        .when(!draft, |v| v.bg(tint(FG(), 0.04)))
        .when(draft, |v| v.border_1().border_dashed().border_color(BORDER_STRONG()))
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
                .when_some(comment, |v, c| v.child(c)),
        );
    if let Some(remove) = remove {
        row = row.child(icon_btn(Icon::X, Sz::Sm, t("Remove note"), move || remove()));
    }
    row
}

/// A draft being written on in the composer's list: its words quoted and
/// a field. Return makes it a note, Escape drops it, let go it stays a
/// draft with what was typed.
fn draft_editor(index: usize, note: Note, editing: Signal<Option<usize>>) -> View {
    let typed = signal(note.comment.clone());
    let settled = signal(false);
    let done = move || {
        settled.set(true);
        editing.set(None);
    };
    let (quote, kept) = (note.quote.clone(), note.quote.clone());
    on_key("escape", move |_| {
        if !settled.get_untracked() {
            if let Some(id) = store::selected_id() {
                store::remove_note(&id, index);
            }
            done();
        }
    });
    view()
        .gap(px(3.0))
        .px(sp(S2))
        .py(px(6.0))
        .rounded(px(R_LG))
        .border_1()
        .border_dashed()
        .border_color(BORDER_STRONG())
        .child(
            view()
                .border_l(px(2.0))
                .border_color(BORDER_STRONG())
                .pl(sp(S2))
                .child(text(quote).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED()).number_of_lines(2)),
        )
        .child(
            view().flex_row().items_center().gap(px(6.0)).child(icon(Icon::Pencil, 12.0).text_color(FG_FAINT())).child(
                nsh_base::input()
                    .auto_focus(true)
                    .flex_1()
                    .h(px(20.0))
                    .text_size(tx(TEXT_SM))
                    .text_color(FG())
                    .caret_color(|| color(ACCENT()))
                    .placeholder(t("Reply"))
                    .value(move || typed.get())
                    .on_change(move |v: &str| typed.set(v.to_owned()))
                    .on_submit(move |v: &str| {
                        if settled.get_untracked() {
                            return;
                        }
                        if let Some(id) = store::selected_id() {
                            store::set_note(&id, index, Note { quote: kept.clone(), comment: v.trim().to_owned(), draft: false });
                        }
                        done();
                    })
                    .on_blur(move || {
                        if settled.get_untracked() {
                            return;
                        }
                        if let Some(id) = store::selected_id() {
                            let comment = typed.get_untracked().trim().to_owned();
                            store::set_note(&id, index, Note { quote: note.quote.clone(), comment, draft: true });
                        }
                        done();
                    }),
            ),
        )
}

/// The notes waiting in the composer for the next message: a chip ("2
/// notes · 1 draft"), and pressed, their list (each may go; a draft,
/// pressed, is written on again).
pub(crate) fn composer_notes(live: bool) -> impl IntoElement {
    let open = signal(false);
    let editing = signal(None::<usize>);
    let list = computed(move || if live { store::selected_id().map(|id| store::notes(&id)).unwrap_or_default() } else { Vec::new() });
    dynamic(move || {
        let notes = list.get();
        if notes.is_empty() {
            open.set(false);
            return None;
        }
        Some(untracked(move || {
            let drafts = notes.iter().filter(|n| n.draft).count();
            let sent = notes.len() - drafts;
            let toggle = move || {
                editing.set(None);
                open.update(|o| *o = !*o);
            };
            let mut col = view().w_full().gap(px(6.0)).px(sp(S1)).pb(sp(S1)).child(view().flex_row().child(chip_of(sent, drafts, false, toggle)));
            col = col.child(when(move || open.get(), move || {
                let notes = notes.clone();
                dynamic(move || {
                    let at = editing.get();
                    let notes = notes.clone();
                    Some(untracked(move || {
                        let mut items = view().gap(px(6.0));
                        for (i, note) in notes.iter().enumerate() {
                            if at == Some(i) && note.draft {
                                items = items.child(draft_editor(i, note.clone(), editing));
                                continue;
                            }
                            let row = note_row(
                                note,
                                Some(Box::new(move || {
                                    if let Some(id) = store::selected_id() {
                                        store::remove_note(&id, i);
                                    }
                                    editing.set(None);
                                })),
                            );
                            items = items.child(if note.draft {
                                nsh_base::button().on_press(move || editing.set(Some(i))).accessibility_label(t("Draft · not sent")).child(row).into_element()
                            } else {
                                row.into_element()
                            });
                        }
                        items.into_element()
                    }))
                })
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
        .child(chip_of(n, 0, true, move || open.update(|o| *o = !*o)))
        .child(when(move || open.get(), move || {
            let mut items = view().gap(px(6.0)).max_w(px(480.0));
            for note in &notes {
                items = items.child(note_row(note, None));
            }
            items
        }))
}

const PILL_W: f32 = 80.0;
const PILL_H: f32 = 28.0;
const CARD_W: f32 = 440.0;
/// The card as tall as it is with three lines quoted, where it goes over
/// the words.
const CARD_H: f32 = 124.0;

/// Over a reply: "Note" centered over the words chosen in it (`chosen`),
/// and the card a press on it opens there (`open`). Return in the card
/// makes a note; Escape drops it; let go with words typed, it stays a
/// draft in the composer, let go empty, it goes.
pub(crate) fn selection_layer(chosen: Signal<Chosen>, open: Signal<Chosen>) -> impl IntoElement {
    // The reply's width: the pill and the card stay inside it.
    let width = signal(0.0_f32);
    let left_in = move |center: f32, w: f32| (center - w / 2.0).min(width.get() - w).max(0.0);
    view()
        .on_layout(move |b: &Bounds<Points>| width.set(b.size.width.0))
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .pointer_events(nsh_core::PointerEvents::BoxNone)
        .child(dynamic(move || {
            let (at, words) = chosen.get()?;
            if open.with(Option::is_some) {
                return None;
            }
            Some(untracked(move || {
                nsh_base::button()
                    .on_press(move || {
                        open.set(Some((at, words.clone())));
                        chosen.set(None);
                    })
                    .accessibility_label(t("Note"))
                    .absolute()
                    .left(px(left_in(at.center, PILL_W)))
                    .top(px(at.top_for(PILL_H)))
                    .flex_row()
                    .items_center()
                    .gap(px(6.0))
                    .h(px(PILL_H))
                    .px(px(10.0))
                    .rounded(px(R_LG))
                    .bg(POPOVER())
                    .border_1()
                    .border_color(BORDER())
                    // Solid, so the chosen words under it don't show through.
                    .hover(|s| s.bg(SURFACE3()).border_color(BORDER_STRONG()))
                    .child(icon(Icon::MessageSquare, 13.0).text_color(FG()))
                    .child(text(t("Note")).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG()))
                    .into_element()
            }))
        }))
        .child(dynamic(move || {
            let (at, words) = open.get()?;
            Some(untracked(move || {
                let typed = signal(String::new());
                // Return or Escape settled it: the blur as it closes adds
                // nothing.
                let settled = signal(false);
                let close = move || {
                    settled.set(true);
                    open.set(None);
                };
                on_key("escape", move |_| close());
                let (quote, kept) = (words.clone(), words.clone());
                view()
                    .absolute()
                    .left(px(left_in(at.center, CARD_W.min(width.get_untracked()))))
                    .top(px(at.top_for(CARD_H)))
                    .w(px(CARD_W))
                    .max_w(move || px(width.get().max(240.0)))
                    .px(sp(S4))
                    .py(sp(S3))
                    .gap(sp(S2))
                    .rounded(px(16.0))
                    .bg(POPOVER())
                    .border_1()
                    .border_color(BORDER())
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
                                if settled.get_untracked() {
                                    return;
                                }
                                store::add_note(Note { quote: words.clone(), comment: v.trim().to_owned(), draft: false });
                                close();
                            })
                            .on_blur(move || {
                                if settled.get_untracked() {
                                    return;
                                }
                                let comment = typed.get_untracked().trim().to_owned();
                                if !comment.is_empty() {
                                    store::add_note(Note { quote: kept.clone(), comment, draft: true });
                                }
                                close();
                            }),
                    )
                    .into_element()
            }))
        }))
}
