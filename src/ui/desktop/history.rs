//! History, a sessions screen: every session, archived included,
//! newest first and searchable; opening an archived one restores it.

use crate::i18n::t;
use nsh_base::button;
use nsh_core::{
    element::{FontWeight, Position},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::{daemon::Agent, store};

pub(crate) fn history(compact: bool) -> impl IntoElement {
    let state = computed(store::history);
    let query = signal(store::history().0);
    view()
        .flex_1()
        .child(
            view()
                .w_full()
                .max_w(px(820.0))
                .self_center()
                .px(sp(S6))
                .pt(sp(S8))
                .pb(sp(S4))
                .gap(sp(S4))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home)))
                        .child(text(t("History")).text_size(tx(TEXT_2XL)).font_weight(FontWeight(500))),
                )
                .child(
                    field()
                        .value(move || query.get())
                        .on_change(move |v: &str| {
                            query.set(v.to_owned());
                            store::search_history(v);
                        })
                        .placeholder(t("Search agents, workspaces, branches and projects")),
                ),
        )
        .child(dynamic(move || {
            let (_, agents, loaded) = state.get();
            Some(untracked(move || {
                if !loaded {
                    return loading().into_element();
                }
                if agents.is_empty() {
                    return view()
                        .p(sp(S8))
                        .items_center()
                        .child(text(t("No sessions yet")).text_size(tx(TEXT_BASE)).text_color(FG_MUTED))
                        .into_element();
                }
                let count = agents.len();
                virtual_list(move || count, move |at: Position| {
                    let agent = computed(move || state.with(|(_, a, _)| a.get(at.index()).cloned()));
                    session_row(agent)
                })
                .keys(move || state.with(|(_, a, _)| a.iter().map(|a| a.id.clone()).collect::<Vec<_>>()))
                .estimated_item_size(px(60.0))
                .content_padding_bottom(sp(S8))
                .flex_1()
                .into_element()
            }))
        }))
}

fn session_row(agent: Memo<Option<Agent>>) -> impl IntoElement {
    let label = move |f: fn(&Agent) -> String| move || agent.with(|a| a.as_ref().map(f).unwrap_or_default());
    view().w_full().max_w(px(820.0)).self_center().px(sp(S4)).child(
        button()
            .on_press(move || {
                if let Some(a) = agent.get_untracked() {
                    store::open_session(&a);
                }
            })
            .flex_row()
            .items_center()
            .gap(sp(S3))
            .px(sp(S3))
            .py(sp(S2))
            .rounded(px(R_LG))
            .hover(|s| s.bg(highlight()))
            .child(status_dot(move || {
                agent.with(|a| a.as_ref().map(|a| if a.archived { Status::Idle } else { Status::of_agent(&a.status, !a.pending.is_empty()) }).unwrap_or(Status::Idle))
            }))
            .child(
                view()
                    .flex_1()
                    .gap(px(2.0))
                    .child(text(label(|a| a.title.clone())).text_size(tx(TEXT_BASE)).number_of_lines(1))
                    .child(
                        text(label(|a| {
                            let date = a.updated_at.get(..10).unwrap_or_default();
                            format!("{} · {} · {date}", a.provider, crate::daemon::model::folder_name(&a.cwd))
                        }))
                        .text_size(tx(TEXT_SM))
                        .text_color(FG_MUTED)
                        .number_of_lines(1),
                    ),
            )
            .child(when(move || agent.with(|a| a.as_ref().is_some_and(|a| a.archived)), || {
                badge(t("Archived"), FG_MUTED)
            })),
    )
}
