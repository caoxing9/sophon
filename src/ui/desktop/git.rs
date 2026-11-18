//! Git in the workspace header: ahead / behind, and the pull request with
//! its checks; pressed, the pull request's sheet: each check, and a merge.
//! The other actions (commit, push, pull, …) are the command palette's:
//! the agents do most of them, and a button for them was noise.

use crate::i18n::t;
use nsh_core::{element::FontWeight, prelude::*};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::daemon::connection::PullRequest;
use crate::store;
use crate::ui::shared::modals::sheet;
use crate::ui::shared::pull_request::{self, checks_summary, pr_state};

pub(crate) fn git_controls() -> impl IntoElement {
    let git = computed(store::git);
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .child(dynamic(move || {
            let st = git.with(|g| g.status.clone()).filter(|s| s.is_git && s.has_remote)?;
            if st.ahead + st.behind == 0 {
                return None;
            }
            Some(untracked(move || {
                text(format!("↑{} ↓{}", st.ahead, st.behind)).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(FG_MUTED)
            }))
        }))
        .child(dynamic(move || {
            let pr = git.with(|g| g.pr.clone())?;
            Some(untracked(move || pr_badge(pr.clone())))
        }))
}

fn pr_badge(pr: PullRequest) -> impl IntoElement {
    let (tone, state) = pr_state(&pr);
    let checks = checks_summary(&pr);
    nsh_base::button()
        .on_press(|| {
            store::open_modal(store::Modal::PullRequest);
            store::watch_pull_request(|| matches!(store::modal(), Some(store::Modal::PullRequest)));
        })
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(26.0))
        .px(sp(S2))
        .rounded(px(R_FULL))
        .bg(tint(tone, 0.1))
        .hover(|s| s.opacity(0.85))
        .child(icon(Icon::GitPullRequest, 14.0).text_color(tone))
        .child(
            text(match pr.number {
                Some(n) => format!("#{n} {state}"),
                None => state.to_owned(),
            })
            .text_size(tx(TEXT_SM))
            .font_weight(FontWeight(500))
            .text_color(tone),
        )
        .when_some(checks, |v, (c, label)| {
            v.child(view().w(px(1.0)).h(px(12.0)).bg(tint(tone, 0.3)))
                .child(text(label).text_size(tx(TEXT_SM)).text_color(c))
        })
}


/// The open workspace's pull request, as a sheet ([`pull_request`]).
pub(crate) fn pull_request_sheet() -> View {
    sheet(t("Pull request")).w(px(560.0)).child(pull_request::details(true))
}
