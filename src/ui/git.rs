//! Git in the workspace header: ahead / behind, the pull request and its
//! checks, and the actions menu (Paseo's git split button).

use crate::i18n::{t, tf};
use nsh_core::{element::FontWeight, prelude::*};
use super::kit::text;

use super::kit::*;
use crate::{daemon::connection::GitAction, store};

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
                text(format!("↑{} ↓{}", st.ahead, st.behind)).text_size(px(TEXT_SM)).font_family(|| MONO).text_color(FG_MUTED)
            }))
        }))
        .child(dynamic(move || {
            let pr = git.with(|g| g.pr.clone())?;
            Some(untracked(move || pr_badge(pr.clone())))
        }))
        .child(when(move || git.with(|g| g.status.as_ref().is_some_and(|s| s.is_git)), move || actions_menu(git)))
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

fn actions_menu(git: Memo<store::Git>) -> impl IntoElement {
    dropdown(
        view()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(CONTROL_SM))
            .px(px(S3))
            .rounded(px(R_FULL))
            .border_1()
            .border_color(BORDER_STRONG)
            .hover(|s| s.bg(highlight()))
            .child(icon(Icon::GitBranch, 14.0).text_color(FG_MUTED))
            .child(
                text(move || git.with(|g| g.busy.map(|b| format!("{b}…")).unwrap_or_else(|| t("Git").into())))
                    .text_size(px(TEXT_SM))
                    .font_weight(FontWeight(500)),
            )
            .child(icon(Icon::ChevronDown, 13.0).text_color(FG_FAINT)),
    )
    .align(nsh_core::overlay::Align::End)
    .child(dynamic(move || {
        let g = git.get();
        Some(untracked(move || {
            let st = g.status.clone().unwrap_or_default();
            let has_pr = g.pr.as_ref().is_some_and(|p| !p.merged && !p.state.eq_ignore_ascii_case("closed"));
            let mut m = view()
                .child(menu_heading(st.branch.clone().unwrap_or_else(|| t("Detached").into())))
                .child(
                    menu_row(t("Commit all changes…"), Some(Icon::GitCommitHorizontal), None, false, || {
                        store::open_modal(store::Modal::Commit)
                    })
                    .disabled(!st.dirty),
                )
                .child(
                    menu_row(
                        t("Push"),
                        Some(Icon::ArrowUp),
                        (st.ahead > 0).then(|| tf("{} commits to push", &[&st.ahead.to_string()]).into()),
                        false,
                        || store::git_action(GitAction::Push),
                    )
                    .disabled(!st.has_remote),
                )
                .child(
                    menu_row(
                        t("Pull"),
                        Some(Icon::ArrowDown),
                        (st.behind > 0).then(|| tf("{} commits behind", &[&st.behind.to_string()]).into()),
                        false,
                        || store::git_action(GitAction::Pull),
                    )
                    .disabled(!st.has_remote),
                )
                .child(menu_row(t("Switch branch…"), Some(Icon::GitBranch), None, false, || {
                    store::search_branches("");
                    store::open_modal(store::Modal::SwitchBranch)
                }))
                .child(menu_row(t("Stash changes"), Some(Icon::Archive), None, false, || store::git_action(GitAction::Stash)).disabled(!st.dirty))
                .child(menu_row(t("Pop latest stash"), Some(Icon::Undo2), None, false, || store::git_action(GitAction::PopStash)))
                .child(menu_divider());
            if has_pr {
                let url = g.pr.as_ref().map(|p| p.url.clone()).unwrap_or_default();
                m = m
                    .child(menu_row(t("Open pull request"), Some(Icon::ExternalLink), None, false, move || {
                        let _ = open_url(url.clone());
                    }))
                    .child(menu_row(t("Squash and merge"), Some(Icon::GitMerge), None, false, || {
                        store::git_action(GitAction::MergePr("squash"))
                    }))
                    .child(menu_row(t("Merge pull request"), Some(Icon::GitMerge), None, false, || {
                        store::git_action(GitAction::MergePr("merge"))
                    }));
            } else {
                m = m.child(
                    menu_row(t("Create pull request"), Some(Icon::GitPullRequest), None, false, || {
                        store::git_action(GitAction::CreatePr)
                    })
                    .disabled(!st.has_remote),
                );
            }
            if let Some(base) = st.base_ref.clone().filter(|_| st.paseo_worktree) {
                m = m.child(menu_row(
                    t("Merge into base"),
                    Some(Icon::GitMerge),
                    Some(tf("Into {}", &[&base]).into()),
                    false,
                    || store::git_action(GitAction::MergeIntoBase),
                ));
            }
            m.into_element()
        }))
    }))
}
