//! A pull request, as the desktop's sheet and the phone's page show it:
//! its title and branches, each check (pressed, its log), whether it can
//! merge, and a merge. Squash, as this repository's history is; the
//! desktop's palette merges otherwise.

use crate::daemon::connection::{GitAction, PrCheck, PullRequest};
use crate::i18n::{t, tf};
use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;
use nsh_core::{element::FontWeight, prelude::*};

/// The pull request's state, in its colour.
pub(crate) fn pr_state(pr: &PullRequest) -> (AdaptiveColor, &'static str) {
    if pr.merged {
        (DOT_RUNNING, t("Merged"))
    } else if pr.state.eq_ignore_ascii_case("closed") {
        (DANGER, t("Closed"))
    } else if pr.draft {
        (FG_MUTED, t("Draft"))
    } else {
        (SUCCESS, if crate::i18n::is_zh() { "开启" } else { t("Open") })
    }
}

fn failed(c: &PrCheck) -> bool {
    matches!(c.status.as_str(), "failure" | "failed" | "error")
}

fn running(c: &PrCheck) -> bool {
    matches!(c.status.as_str(), "pending" | "in_progress" | "queued")
}

/// The checks in a few words: failing, running, or passed.
pub(crate) fn checks_summary(pr: &PullRequest) -> Option<(AdaptiveColor, String)> {
    let failing = pr.checks.iter().filter(|c| failed(c)).count();
    let pending = pr.checks.iter().filter(|c| running(c)).count();
    if pr.checks.is_empty() {
        None
    } else if failing > 0 {
        Some((DANGER, tf("{} failing", &[&failing.to_string()])))
    } else if pending > 0 {
        Some((WARNING, tf("{} running", &[&pending.to_string()])))
    } else {
        Some((SUCCESS, t("Checks passed").to_owned()))
    }
}

/// The open workspace's pull request, kept up to date; `close` adds a
/// Close button (the sheet's: the phone's page goes back by itself).
pub(crate) fn details(close: bool) -> View {
    let git = computed(store::git);
    view().child(dynamic(move || {
        let (pr, busy) = git.with(|g| (g.pr.clone(), g.busy));
        Some(untracked(move || match pr {
            None => view()
                .gap(sp(S3))
                .child(text(t("No pull request for this branch")).text_size(tx(TEXT_SM)).text_color(FG_MUTED))
                .when(close, |v| {
                    v.child(view().flex_row().justify_end().child(btn(t("Close"), Variant::Secondary, Sz::Md, None, store::close_modal)))
                })
                .into_element(),
            Some(pr) => body(pr, busy, close).into_element(),
        }))
    }))
}

fn body(pr: PullRequest, busy: Option<&'static str>, close: bool) -> View {
    let (tone, state) = pr_state(&pr);
    let open = !pr.merged && !pr.state.eq_ignore_ascii_case("closed");
    let conflicting = pr.mergeable.eq_ignore_ascii_case("CONFLICTING");
    // Why not to merge yet, or a caution, in its colour.
    let blocker = if !open {
        None
    } else if pr.draft {
        Some((FG_MUTED, t("A draft: mark it ready on GitHub to merge it")))
    } else if conflicting {
        Some((DANGER, t("It conflicts with its base: resolve that first")))
    } else if pr.checks.iter().any(failed) {
        Some((DANGER, t("Some checks failed")))
    } else if pr.checks.iter().any(running) {
        Some((WARNING, t("Checks are still running")))
    } else if pr.review.as_deref() == Some("changes_requested") {
        Some((WARNING, t("A reviewer asked for changes")))
    } else {
        None
    };
    let mut v = view().gap(sp(S4));
    // #59 · Open, the title, base ← head.
    v = v.child(
        view()
            .gap(sp(S1))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .child(icon(Icon::GitPullRequest, 16.0).text_color(tone))
                    .child(
                        text(pr.number.map(|n| format!("#{n} · {state}")).unwrap_or_else(|| state.to_owned()))
                            .text_size(tx(TEXT_SM))
                            .font_weight(FontWeight(500))
                            .text_color(tone),
                    ),
            )
            .child(text(pr.title.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)).line_height(px(20.0)))
            .when(!pr.base.is_empty(), |v| {
                v.child(
                    text(format!("{} ← {}", pr.base, pr.head))
                        .text_size(tx(TEXT_XS))
                        .font_family(mono_font)
                        .text_color(FG_FAINT)
                        .number_of_lines(1),
                )
            }),
    );
    // The checks, failing first, then running.
    if !pr.checks.is_empty() {
        let mut checks = pr.checks.clone();
        checks.sort_by_key(|c| if failed(c) { 0 } else if running(c) { 1 } else { 2 });
        let mut list = view().gap(px(2.0));
        for c in checks {
            list = list.child(check_row(c));
        }
        let (c, summary) = checks_summary(&pr).unwrap_or((FG_MUTED, String::new()));
        v = v.child(
            view()
                .gap(sp(S2))
                .child(text(summary).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(c))
                .child(scroll().max_h(px(280.0)).rounded(px(R_LG)).border_1().border_color(tint(BORDER, 0.6)).p(px(4.0)).child(list)),
        );
    }
    if let Some((c, why)) = blocker {
        v = v.child(text(why).text_size(tx(TEXT_SM)).text_color(c));
    }
    let url = pr.url.clone();
    let mut buttons = view()
        .flex_row()
        .justify_end()
        .gap(sp(S2))
        .when(close, |v| v.child(btn(t("Close"), Variant::Secondary, Sz::Md, None, store::close_modal)))
        .child(btn(t("Open in GitHub"), Variant::Secondary, Sz::Md, Some(Icon::ExternalLink), move || {
            let _ = open_url(url.clone());
        }));
    if open {
        let merging = busy.is_some();
        buttons = buttons.child(
            btn(if merging { t("Merging…") } else { t("Squash and merge") }, Variant::Primary, Sz::Md, Some(Icon::GitMerge), || {
                store::git_action(GitAction::MergePr("squash"))
            })
            .disabled(move || merging || pr.draft || conflicting),
        );
    }
    v.child(buttons)
}

/// A check: its state, name and time; pressed, its page.
fn check_row(c: PrCheck) -> impl IntoElement {
    let (glyph, tone) = if failed(&c) {
        (Icon::CircleX, DANGER)
    } else if running(&c) {
        (Icon::CircleDot, WARNING)
    } else if c.status == "success" {
        (Icon::CircleCheck, SUCCESS)
    } else {
        (Icon::Minus, FG_FAINT)
    };
    let url = c.url.clone();
    nsh_base::button()
        .on_press(move || {
            if let Some(u) = &url {
                let _ = open_url(u.clone());
            }
        })
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .h(px(30.0))
        .px(sp(S2))
        .rounded(px(R_MD))
        .hover(|s| s.bg(highlight()))
        .child(icon(glyph, 14.0).text_color(tone))
        .child(text(c.name.clone()).text_size(tx(TEXT_SM)).number_of_lines(1).flex_1().min_w(px(0.0)))
        .when_some(c.duration.clone(), |v, d| v.child(text(d).text_size(tx(TEXT_XS)).font_family(mono_font).text_color(FG_FAINT)))
        .when(c.url.is_some(), |v| v.child(icon(Icon::ExternalLink, 12.0).text_color(FG_FAINT)))
}
