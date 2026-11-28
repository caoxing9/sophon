//! Over the composer, as DSH Go's: a line as wide as it, the pet at its
//! start, Sophon's eclipse in pixels, in the conversation's mood (typing
//! while the agent works, a hand up while it waits for the user, a hop when
//! a turn ends, asleep otherwise); beside it, while the agent works, what
//! it does now (its step, the command or file, how long the turn has run).
//! No box, and nothing laid over the conversation.

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
/// The line's height: the pet's body, its frame's empty sky above it.
const BAR_H: f32 = 34.0;
/// A line of the live feed under it (`Quiet`), and how many show.
const FEED_LINE: f32 = 17.0;
const FEED_SHOWN: usize = 2;
/// The line's height with the feed: its words, then the feed.
const LIVE_H: f32 = 22.0 + FEED_LINE * FEED_SHOWN as f32 + 4.0;
/// How far under its feet the pet's frame goes (empty).
const PET_FEET: f32 = 5.0;
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
                let what = if kind == "shell" { command_shown(&summary) } else { crate::fold::short_path(&summary) };
                return (kind.clone(), tool_verb(&kind, &name), what);
            }
            Some(Item::Reasoning { .. }) => return ("thinking".into(), t("Thinking").to_owned(), String::new()),
            Some(Item::Assistant { .. }) => return ("said".into(), t("Writing a reply").to_owned(), String::new()),
            Some(_) => {}
        }
    }
    (String::new(), String::new(), String::new())
}

/// A command as the line has room for: its first line, without the `cd
/// <folder> &&` an agent puts before it (the folder is the conversation's).
fn command_shown(command: &str) -> String {
    let first = command.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or_default();
    let mut rest = first;
    while let Some(after) = rest.strip_prefix("cd ").and_then(|r| r.split_once("&&")).map(|(_, a)| a.trim_start()) {
        rest = after;
    }
    rest.to_owned()
}

/// The line over the composer, as wide as it and lined up with it: the
/// pet, and while the agent works, what it does now (its step, the file or
/// command, how long the turn has run). No box: words beside the pet, which
/// has room of its own, so nothing is covered.
pub(crate) fn bar() -> impl IntoElement {
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
    // Quietly, the conversation shows no steps: what the agent writes and
    // runs scrolls by here instead, as DSH Go's strip does, so the work is
    // seen going on.
    let live = computed(move || running.get() && store::tool_display() == crate::fold::ToolDisplay::Quiet);
    view()
        .w_full()
        .max_w(px(CONTENT_MAX_WIDTH()))
        .self_center()
        .flex_row()
        .items_end()
        .gap(sp(S2))
        .h(move || Length::from(px(if live.get() { LIVE_H } else { BAR_H })))
        .px(sp(S2))
        .child(pet(agent, running))
        .child(view().flex_1().min_w(px(0.0)).h_full().justify_end().pb(px(6.0)).child(dynamic(move || {
            if !running.get() {
                return None;
            }
            let quiet = live.get();
            let id = id.get_untracked().unwrap_or_default();
            Some(untracked(move || {
                let line = doing_line(agent, id);
                // From the foot up, as a log: what it does now on the line by
                // the composer, what it thought and did before over it, each
                // new line pushing the rest up.
                if quiet { view().justify_end().gap(px(2.0)).child(feed()).child(line).into_element() } else { line.into_element() }
            }))
        })))
}

/// A line of the live feed: a thought.
#[derive(Clone, Debug, PartialEq)]
struct FeedLine {
    words: String,
}

/// What the turn has thought so far, as lines (Markdown's marks and code
/// left out): its words read on in the conversation, and its calls are
/// not here (a command line is long, and it crowded the feed out): the
/// one going on is the line by the composer, the rest under the turn's
/// line in the conversation. How many there are, and the last `n`.
fn feed_lines(n: usize) -> (usize, Vec<FeedLine>) {
    let mut all: Vec<FeedLine> = Vec::new();
    let count = store::item_count();
    let start = (0..count).rev().find(|&i| matches!(store::item_at(i), Some(Item::User { .. }))).map_or(0, |i| i + 1);
    for i in start..count {
        // Its words read on in the conversation: here its thinking.
        if let Some(Item::Reasoning { text }) = store::item_at(i) {
            let mut fence = false;
            for line in text.lines() {
                let t = line.trim();
                if t.starts_with("```") {
                    fence = !fence;
                    continue;
                }
                if fence || t.is_empty() {
                    continue;
                }
                let words = sophon_push::excerpt(t);
                if !words.trim().is_empty() {
                    all.push(FeedLine { words: sophon_push::clip(&words, 160) });
                }
            }
        }
    }
    let total = all.len();
    let last = all.split_off(total.saturating_sub(n));
    (total, last)
}

/// The live feed: the turn's last lines, the newest at the foot, growing
/// as it is written; each new one slides the rest up (a quick spring), the
/// older fainter.
fn feed() -> View {
    let lines = computed(|| feed_lines(FEED_SHOWN + 1));
    let slide = animated_value(0.0);
    let seen = std::cell::Cell::new(None::<usize>);
    effect(move || {
        let total = lines.with(|(n, _)| *n);
        let before = seen.replace(Some(total));
        if before.is_some_and(|b| total > b) && !reduce_motion() {
            slide.set(FEED_LINE);
            let _ = slide.spring(0.0, Spring { stiffness: 420.0, damping: 36.0, ..Spring::default() });
        }
    });
    view().max_h(px(FEED_LINE * FEED_SHOWN as f32)).overflow_hidden().justify_end().child(view().bind_translate_y(&slide).child(dynamic(move || {
        let (_, list) = lines.get();
        Some(untracked(move || {
            let n = list.len();
            let mut column = view();
            for (i, line) in list.into_iter().enumerate() {
                let age = n - 1 - i;
                let words = text(line.words).text_size(tx(TEXT_XS)).line_height(px(FEED_LINE)).text_color(FG_FAINT()).number_of_lines(1);
                column = column.child(view().h(px(FEED_LINE)).opacity([1.0_f32, 0.55, 0.25][age.min(2)]).child(words));
            }
            column.into_element()
        }))
    })))
}

/// What the agent does now, one line of words.
fn doing_line(agent: Memo<Option<crate::daemon::Agent>>, id: String) -> View {
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
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .child(dynamic(move || {
            let kind = doing.with(|(k, ..)| k.clone());
            Some(untracked(move || match kind.as_str() {
                "" | "thinking" | "said" => spark(13.0).into_element(),
                k => tool_glyph(k, "", 18.0).into_element(),
            }))
        }))
        .child(
            text(move || doing.with(|(_, word, _)| if word.is_empty() { format!("{idle_word}…") } else { word.clone() }))
                .text_size(tx(TEXT_SM))
                .font_weight(FontWeight(500))
                .text_color(FG_MUTED())
                .flex_shrink_0(),
        )
        .child(
            text(move || doing.with(|(.., detail)| detail.clone()))
                .text_size(tx(TEXT_XS))
                .font_family(mono_font)
                .text_color(FG_FAINT())
                .number_of_lines(1)
                .flex_1(),
        )
        .child(text(clock).text_size(tx(TEXT_XS)).text_color(FG_FAINT()).flex_shrink_0())
}

/// The pet in its room at the line's start, in the conversation's mood.
fn pet(agent: Memo<Option<crate::daemon::Agent>>, running: Memo<bool>) -> impl IntoElement {
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
    // Its frame is taller than the line: the empty sky over its head
    // reaches up past it, its feet on the line's foot.
    view().w(px(PET)).h(px(BAR_H)).flex_shrink_0().child(
        view().absolute().bottom(px(-PET_FEET)).left(px(0.0)).w(px(PET)).h(px(PET)).pointer_events(nsh_core::PointerEvents::None).child(dynamic(
            move || {
                let mood = mood.get();
                Some(untracked(move || sprite(mood).into_element()))
            },
        )),
    )
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

#[cfg(test)]
mod tests {
    use super::command_shown;

    #[test]
    fn a_command_is_shown_without_its_cd_and_past_its_first_line() {
        assert_eq!(command_shown("cd /Users/a/wt && cargo test --lib"), "cargo test --lib");
        assert_eq!(command_shown("cd /a && cd b && python3 - <<'EOF'\nprint(1)\nEOF"), "python3 - <<'EOF'");
        assert_eq!(command_shown("npm test"), "npm test");
        assert_eq!(command_shown("cd /a"), "cd /a", "a cd alone is the command");
    }
}
