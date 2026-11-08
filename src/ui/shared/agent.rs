//! An agent's pane, on both platforms: the bar with its actions, the
//! conversation, and the composer floating over it; and the draft of a new
//! agent in the open workspace.

use crate::i18n::{t, tf};
use nsh_core::{element::FontWeight, prelude::*};

use super::{
    composer::{self, Target},
    timeline,
};
use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;

/// `bar`: the thin bar of the agent's actions (the phone has them in its
/// header instead).
pub(crate) fn agent_pane(bar: bool) -> impl IntoElement {
    let agent = computed(store::selected_agent);
    // The composer floats over the conversation, which scrolls under the
    // glass.
    view()
        .flex_1()
        .when(bar, |v| v.child(agent_bar(agent)))
        .child(timeline::find_bar())
        // On the phone a tap on the conversation (not on something in it
        // that takes taps) puts the keyboard away, as Messages does.
        .child(
            view()
                .flex_1()
                .when(MOBILE, |v| v.on_press(|_: &PressEvent| dismiss_keyboard()))
                .child(timeline::timeline()),
        )
        .child(
            view()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .px(px(if MOBILE { S2 } else { S6 }))
                .pb(px(if MOBILE { S2 } else { S4 }))
                .child(timeline::tracks())
                .child(timeline::permissions())
                .child(composer::composer(Target::Agent, t("Message the agent…"))),
        )
}

/// A thin bar over the conversation: the agent's provider and actions.
fn agent_bar(agent: Memo<Option<crate::daemon::Agent>>) -> impl IntoElement {
    let label = computed(move || {
        agent.with(|a| {
            a.as_ref()
                .map(|a| {
                    let provider = store::provider(&a.provider).map(|p| p.label).unwrap_or_else(|| a.provider.clone());
                    format!("{provider} · {}", Status::of_agent(&a.status, !a.pending.is_empty()).label())
                })
                .unwrap_or_default()
        })
    });
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .h(px(36.0))
        .px(px(S6))
        .child(text(move || label.get()).text_size(px(TEXT_SM)).text_color(FG_FAINT))
        .child(composer::context_meter())
        .child(view().flex_1())
        .child(
            dropup_or_down_outline(),
        )
        .child(icon_btn(Icon::Search, Sz::Sm, t("Find in conversation"), store::toggle_find))
        .child(
            dropdown(
                view()
                    .w(px(26.0))
                    .h(px(26.0))
                    .items_center()
                    .justify_center()
                    .rounded(px(R_FULL))
                    .hover(|s| s.bg(highlight()))
                    .child(icon(Icon::Ellipsis, 16.0).text_color(FG_MUTED)),
            )
            .align(nsh_core::overlay::Align::End)
            .child(agent_actions(agent)),
        )
}

/// The open agent's actions, as menu rows: fork, rename, stop, archive.
pub(crate) fn agent_actions(agent: Memo<Option<crate::daemon::Agent>>) -> impl IntoElement {
    view().child(dynamic(move || {
        let a = agent.get()?;
        Some(untracked(move || {
            let (stop, archive, rename) = (a.is_running(), a.id.clone(), a.id.clone());
            let title = a.title.clone();
            let mut menu = view()
                .child(menu_row(t("Fork into new agent"), Some(Icon::GitCompare), None, false, store::fork_selected))
                .child(menu_row(t("Rename"), Some(Icon::Pencil), None, false, move || {
                    store::open_modal(store::Modal::RenameAgent { id: rename.clone(), name: title.clone() })
                }));
            if stop {
                menu = menu.child(menu_row(t("Stop"), Some(Icon::Square), None, false, store::stop_selected));
            }
            menu.child(menu_row_danger(t("Archive agent"), Icon::Archive, move || store::archive_agent(&archive)))
                .into_element()
        }))
    }))
}

/// A new agent in this workspace.
pub(crate) fn draft_pane() -> AnyElement {
    let form = computed(store::form);
    if MOBILE {
        // The phone: an empty conversation (the header already says
        // "New agent"): the mark and where it runs in the middle, the
        // composer at the foot where the conversation's sits.
        let line = move || {
            form.with(|f| {
                f.error.clone().unwrap_or_else(|| {
                    if f.busy {
                        t("Starting…").into()
                    } else {
                        let dir = f.path.trim_end_matches('/').rsplit('/').next().unwrap_or(&f.path).to_owned();
                        tf("Runs in {}", &[&dir])
                    }
                })
            })
        };
        return view()
            .flex_1()
            .child(
                view()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap(px(S3))
                    .child(mark(48.0, Lift::Flat))
                    .child(
                        text(line)
                            .px(px(S8))
                            .text_size(px(TEXT_SM))
                            .text_color(move || if form.with(|f| f.error.is_some()) { DANGER } else { FG_MUTED })
                            .text_center()
                            .number_of_lines(2),
                    ),
            )
            .child(view().px(px(S2)).pb(px(S2)).child(composer::composer(Target::Form, t("What should the agent do?"))))
            .into_element();
    }
    view()
        .flex_1()
        .justify_center()
        .px(px(if MOBILE { S3 } else { S6 }))
        .pb(px(S8))
        .gap(px(S3))
        .child(
            view()
                .w_full()
                .max_w(px(CONTENT_MAX_WIDTH))
                .self_center()
                .px(px(S2))
                .child(text(t("New agent")).text_size(px(TEXT_2XL)).font_weight(FontWeight(500))),
        )
        .child(composer::composer(Target::Form, t("What should the agent do?")))
        .child(composer::hint(move || {
            form.with(|f| {
                f.error.clone().map(|e| (e, true)).or_else(|| {
                    if f.busy {
                        Some((t("Starting…").into(), false))
                    } else if let Some((_, from)) = &f.context {
                        Some((tf("Continues from “{}” · runs in {}", &[&from, &f.path]), false))
                    } else {
                        Some((tf("Runs in {}", &[&f.path]), false))
                    }
                })
            })
        }))
        .into_element()
}

/// The outline: the conversation's prompts, to jump to.
pub(crate) fn dropup_or_down_outline() -> impl IntoElement {
    dropdown(
        view()
            .w(px(CONTROL_SM))
            .h(px(CONTROL_SM))
            .items_center()
            .justify_center()
            .rounded(px(R_FULL))
            .hover(|s| s.bg(highlight()))
            .label(t("Outline"))
            .child(icon(Icon::ListTodo, 15.0).text_color(FG_MUTED)),
    )
    .align(nsh_core::overlay::Align::End)
    .child(dynamic(|| {
        let prompts = store::prompts();
        Some(untracked(move || {
            let mut list = view().child(menu_heading(t("Your messages")));
            if prompts.is_empty() {
                list = list.child(menu_row(t("No messages yet"), None, None, false, || {}).disabled(true));
            }
            for (row, text) in prompts.clone() {
                list = list.child(menu_row(text, None, None, false, move || store::scroll_timeline_to(row)));
            }
            menu_scroll(380.0, list).into_element()
        }))
    }))
}

