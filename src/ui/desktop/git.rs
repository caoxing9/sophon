//! Git in the workspace header: ahead / behind, and the pull request with
//! its checks. The actions (commit, push, pull, …) are the command
//! palette's: the agents do most of them, and a button for them was noise.

use crate::i18n::{t, tf};
use nsh_core::{element::FontWeight, prelude::*};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store;

pub(crate) fn git_controls() -> impl IntoElement {
    let git = computed(store::git);
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .child(dynamic(move || {
            let st = git.with(|g| g.status.clone()).filter(|s| s.is_git && s.has_remote)?;
            if st.ahead + st.behind == 0 {
                return None;
            }
            Some(untracked(move || {
                text(format!("↑{} ↓{}", st.ahead, st.behind)).text_size(px(TEXT_SM)).font_family(mono_font).text_color(FG_MUTED)
            }))
        }))
        .child(dynamic(move || {
            let pr = git.with(|g| g.pr.clone())?;
            Some(untracked(move || pr_badge(pr.clone())))
        }))
}

fn pr_badge(pr: crate::daemon::connection::PullRequest) -> impl IntoElement {
    let (tone, state) = if pr.merged {
        (DOT_RUNNING, t("Merged"))
    } else if pr.state.eq_ignore_ascii_case("closed") {
        (DANGER, t("Closed"))
    } else if pr.draft {
        (FG_MUTED, t("Draft"))
    } else {
        (SUCCESS, if crate::i18n::is_zh() { "开启" } else { t("Open") })
    };
    let failing = pr.checks.iter().filter(|(_, s)| matches!(s.as_str(), "failure" | "failed" | "error")).count();
    let pending = pr.checks.iter().filter(|(_, s)| matches!(s.as_str(), "pending" | "in_progress" | "queued")).count();
    let checks = if pr.checks.is_empty() {
        None
    } else if failing > 0 {
        Some((DANGER, tf("{} failing", &[&failing.to_string()])))
    } else if pending > 0 {
        Some((WARNING, tf("{} running", &[&pending.to_string()])))
    } else {
        Some((SUCCESS, t("Checks passed").to_owned()))
    };
    let url = pr.url.clone();
    nsh_base::button()
        .on_press(move || {
            let _ = open_url(url.clone());
        })
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(26.0))
        .px(px(S2))
        .rounded(px(R_FULL))
        .bg(tint(tone, 0.1))
        .hover(|s| s.opacity(0.85))
        .child(icon(Icon::GitPullRequest, 14.0).text_color(tone))
        .child(
            text(match pr.number {
                Some(n) => format!("#{n} {state}"),
                None => state.to_owned(),
            })
            .text_size(px(TEXT_SM))
            .font_weight(FontWeight(500))
            .text_color(tone),
        )
        .when_some(checks, |v, (c, label)| {
            v.child(view().w(px(1.0)).h(px(12.0)).bg(tint(tone, 0.3)))
                .child(text(label).text_size(px(TEXT_SM)).text_color(c))
        })
}

