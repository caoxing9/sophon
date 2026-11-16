//! Pull requests on the phone (`ui::shared::github`): the inbox, pushed from
//! the Projects tab, and one pull request, pushed from the inbox or from a
//! conversation's actions.

use crate::i18n::t;
use nsh_base::nav_page;
use nsh_core::{
    element::{FontWeight, Screen},
    prelude::*,
};

use super::{Route, open_chat, push, top_bar};
use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;
use crate::ui::shared::github::{inbox_list, pull_request};

pub(crate) fn inbox_page() -> Screen {
    store::refresh_inbox();
    nav_page().key("inbox").header_visible(false).title(t("Pull requests")).child(
        view()
            .flex_1()
            .child(top_bar(
                text(t("Pull requests")).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)),
                super::glass_button(Icon::RotateCw, t("Refresh"), store::refresh_inbox),
            ))
            .child(
                scroll().flex_1().child(
                    view().px_safe_area().px(sp(S4)).pt(sp(S2)).pb_safe_area().pb(sp(S6)).child(inbox_list(|item: crate::github::InboxItem| {
                        push(Route::PullRequest(item.repo.clone(), item.number, None));
                    })),
                ),
            ),
    )
}

pub(crate) fn pull_request_page(repo: String, number: u64, agent: Option<String>) -> Screen {
    store::refresh_pull_request(&repo, number);
    let (r, from) = (repo.clone(), agent.clone());
    nav_page().key(format!("pr:{repo}#{number}")).header_visible(false).title(format!("#{number}")).child(
        view()
            .flex_1()
            .child(top_bar(
                text(format!("#{number}")).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)),
                super::glass_button(Icon::RotateCw, t("Refresh"), move || store::refresh_pull_request(&r, number)),
            ))
            .child(view().flex_1().px_safe_area().px(sp(S4)).pb_safe_area().child(pull_request(repo, number, agent, move |went: Option<String>| {
                // To where its prompt was put, to read and send.
                match (went, from.clone()) {
                    (Some(ws), Some(a)) => open_chat(&ws, Some(store::Tab::Agent(a))),
                    _ => push(Route::NewChat),
                }
            }))),
    )
}
