//! Over the composer, as DSH Go's: while the agent works, a pill of what it
//! does now (its step, the command or file, how long the turn has run);
//! and the pet, Sophon's eclipse, standing on whatever is topmost there, in
//! the mood of the conversation: typing while the agent works, a hand up
//! while it waits for the user, a hop when it is done, asleep otherwise.

use nsh_core::prelude::*;

use super::timeline::{VERBS_EN, VERBS_ZH, short_count, tokens_in, tool_glyph, tool_verb};
use crate::daemon::Item;
use crate::i18n::t;
use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;

thread_local! {
    /// When each agent's running turn was first seen running here.
    static TURN_STARTED: std::cell::RefCell<std::collections::HashMap<String, std::time::Instant>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// The pet's moods, each a strip of frames (`assets/pet/gen_pet.py`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mood {
    Typing,
    Happy,
    Waiting,
    Sleeping,
}

impl Mood {
    /// Its strip, how many frames and how long each shows (ms).
    fn strip(self) -> (ImageAsset, usize, f32) {
        match self {
            Mood::Typing => (crate::PET_TYPING, 4, 150.0),
            Mood::Happy => (crate::PET_HAPPY, 6, 110.0),
            Mood::Waiting => (crate::PET_WAITING, 4, 240.0),
            Mood::Sleeping => (crate::PET_SLEEPING, 4, 650.0),
        }
    }
}

/// How large the pet shows: its 32 pixels a side, one and a half points
/// each (its strips are drawn six to a pixel: three device pixels apiece
/// on a 2x screen).
const PET: f32 = 48.0;
/// How far the pet reaches up over what is above its block (its frame less
/// the few points its feet stand into it).
pub(crate) const PET_REACH: f32 = PET - 6.0;
/// How long it hops for joy once a turn ends (ms).
const HAPPY_FOR: u64 = 4000;

/// What the agent does now, for the pill: its step's kind (for the glyph),
/// a word for it and its detail.
fn now_doing(agent: &crate::daemon::Agent) -> (String, String, String) {
    if let Some(p) = agent.pending.first() {
        let word = if p.kind == "question" { t("Waiting for your answer") } else { t("Waiting for approval") };
        return (String::new(), word.to_owned(), p.title.clone());
    }
    let count = store::item_count();
    for i in (0..count).rev() {
        match store::item_at(i) {
            Some(Item::User { .. }) | None => break,
            Some(Item::Tool { kind, name, status, summary, .. }) if status == "running" => {
                return (kind.clone(), tool_verb(&kind, &name), crate::fold::short_path(&summary));
            }
            Some(Item::Reasoning { .. }) => return ("thinking".into(), t("Thinking").to_owned(), String::new()),
            Some(Item::Assistant { .. }) => return ("said".into(), t("Writing a reply").to_owned(), String::new()),
            Some(_) => {}
        }
    }
    (String::new(), String::new(), String::new())
}

/// The pill over the composer while the agent works.
pub(crate) fn strip() -> impl IntoElement {
    let agent = computed(store::selected_agent);
    let running = computed(move || agent.with(|a| a.as_ref().is_some_and(|a| a.is_running())));
    let id = computed(store::selected_id);
    // The turn's start, kept per agent while it runs.
    effect(move || {
        let (Some(id), on) = (id.get(), running.get()) else { return };
        TURN_STARTED.with_borrow_mut(|m| {
            if on {
                m.entry(id).or_insert_with(std::time::Instant::now);
            } else {
                m.remove(&id);
            }
        });
    });
    let doing = computed(move || {
        store::item_count();
        agent.with(|a| a.as_ref().map(now_doing).unwrap_or_default())
    });
    // What the turn has written so far: everything after the user's last
    // message (the service says the turn's tokens only at its end).
    let written = computed(move || {
        let mut tokens = 0;
        for i in (0..store::item_count()).rev() {
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
        let id = id.get_untracked().unwrap_or_default();
        Some(untracked(move || {
            let started = TURN_STARTED.with_borrow(|m| m.get(&id).copied()).unwrap_or_else(std::time::Instant::now);
            // A word for the turn while no step has one, by when it began.
            let seed = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                format!("{started:?}{id}").hash(&mut h);
                h.finish() as usize
            };
            let idle_word = if crate::i18n::is_zh() { VERBS_ZH[seed % VERBS_ZH.len()] } else { VERBS_EN[seed % VERBS_EN.len()] };
            let tick = signal(0u64);
            spawn(async move {
                loop {
                    nsh_core::element::timer(std::time::Duration::from_secs(1)).await;
                    tick.update(|t| *t += 1);
                }
            });
            let clock = move || {
                tick.get();
                let secs = started.elapsed().as_secs();
                let time = if secs >= 3600 { format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60) } else { format!("{}:{:02}", secs / 60, secs % 60) };
                match written.get() {
                    0 => time,
                    n => format!("{time} · ↓{}", short_count(n)),
                }
            };
            glass_strong(view())
                .rounded(px(R_FULL))
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .min_h(px(36.0))
                .pl(px(10.0))
                .pr(px(14.0))
                .mb(sp(S2))
                .child(dynamic(move || {
                    let kind = doing.with(|(k, ..)| k.clone());
                    Some(untracked(move || match kind.as_str() {
                        "" | "thinking" | "said" => spark(14.0).into_element(),
                        k => tool_glyph(k, "", 20.0).into_element(),
                    }))
                }))
                .child(
                    text(move || doing.with(|(_, word, _)| if word.is_empty() { format!("{idle_word}…") } else { word.clone() }))
                        .text_size(tx(TEXT_XS))
                        .font_weight(FontWeight(600))
                        .text_color(FG())
                        .flex_shrink_0(),
                )
                .child(
                    text(move || doing.with(|(.., detail)| detail.clone()))
                        .text_size(tx(TEXT_XS))
                        .font_family(mono_font)
                        .text_color(FG_MUTED())
                        .number_of_lines(1)
                        .flex_1(),
                )
                .child(text(clock).text_size(tx(TEXT_XS)).text_color(FG_FAINT()).flex_shrink_0())
                .into_element()
        }))
    }))
}

/// The pet, standing on the top edge of the block it is put in (the
/// composer and what is over it). Laid over what is above, taking no room
/// and no presses.
pub(crate) fn pet() -> impl IntoElement {
    let agent = computed(store::selected_agent);
    let running = computed(move || agent.with(|a| a.as_ref().is_some_and(|a| a.is_running())));
    let waiting = computed(move || {
        agent.with(|a| {
            a.as_ref().is_some_and(|a| !a.pending.is_empty() || matches!(a.attention_reason.as_deref(), Some("permission" | "needs_input")))
        })
    });
    // A turn seen to end here: a hop for a moment.
    let happy = signal(false);
    let was = std::cell::Cell::new(None::<(Option<String>, bool)>);
    effect(move || {
        let now = (store::selected_id(), running.get());
        let before = was.replace(Some(now.clone()));
        if let Some((id, on)) = before
            && id == now.0
            && on
            && !now.1
        {
            happy.set(true);
            set_timeout(std::time::Duration::from_millis(HAPPY_FOR), move || happy.set(false));
        } else if now.1 {
            happy.set(false);
        }
    });
    let mood = computed(move || {
        if waiting.get() {
            Mood::Waiting
        } else if running.get() {
            Mood::Typing
        } else if happy.get() {
            Mood::Happy
        } else {
            Mood::Sleeping
        }
    });
    view()
        .absolute()
        // Its feet a few points into its frame: they touch the edge below.
        .top(px(-PET_REACH))
        .left(px(if MOBILE { S4 } else { S6 + S3 }))
        .w(px(PET))
        .h(px(PET))
        .pointer_events(nsh_core::PointerEvents::None)
        .child(dynamic(move || {
            let mood = mood.get();
            Some(untracked(move || sprite(mood).into_element()))
        }))
}

/// A mood's frames, one after another, round and round: its strip slid
/// along behind a window a frame wide.
fn sprite(mood: Mood) -> View {
    let (strip, frames, ms) = mood.strip();
    let at = animated_value(0.0);
    if !reduce_motion() {
        Animation::sequence((0..frames).flat_map(|i| {
            [
                Animation::timing(&at, -(i as f32) * PET, nsh_core::element::Timing::new(0.0, Easing::Linear)),
                Animation::delay(ms),
            ]
        }))
        .looped(None)
        .start();
    }
    view()
        .w(px(PET))
        .h(px(PET))
        .overflow_hidden()
        .child(view().bind_translate_x(&at).child(image(strip).w(px(PET * frames as f32)).h(px(PET))))
}
