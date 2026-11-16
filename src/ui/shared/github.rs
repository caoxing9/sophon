//! Pull requests (`crate::github`): the inbox of the open ones that
//! involve the user, and one pull request's page, its summary, code and
//! checks, with its actions (merge, draft, close) and "Fix with agent"
//! when its checks fail. The desktop shows them as screens
//! (`Screen::Inbox`, `Screen::PullRequest`); the phone pushes them.
//!
//! After monocode's GitHub inbox (hardbeat920/monocode).

use std::rc::Rc;

use nsh_base::{DiffLayout, button, diff_view, text_view};
use nsh_core::prelude::*;

use crate::github::{Check, CheckState, Detail, Inbox, InboxItem, Op};
use crate::i18n::{t, tf};
use crate::store::{self, Screen};
use crate::ui::kit::text;
use crate::ui::kit::*;

fn ago(at: &str) -> String {
    crate::ui::mobile::time::ago(at)
}

/// A state's word and color.
fn state_of(d: &Detail) -> (&'static str, AdaptiveColor) {
    if d.merged() {
        (t("Merged"), DOT_RUNNING)
    } else if !d.open() {
        (t("Closed"), DANGER)
    } else if d.draft {
        (t("Draft"), FG_MUTED)
    } else if crate::i18n::is_zh() {
        ("开启", SUCCESS)
    } else {
        (t("Open"), SUCCESS)
    }
}

fn chip(word: impl Into<SharedString>, tone: AdaptiveColor) -> View {
    view()
        .px(sp(S2))
        .h(px(22.0))
        .rounded(px(R_FULL))
        .bg(tint(tone, 0.12))
        .items_center()
        .justify_center()
        .child(text(word.into()).text_size(tx(TEXT_XS)).font_weight(FontWeight(500)).text_color(tone))
}

fn message(words: impl Into<SharedString>) -> View {
    view().py(sp(S8)).items_center().child(text(words.into()).text_size(tx(TEXT_BASE)).text_color(FG_MUTED).text_center())
}

/// What the inbox shows the first time it is asked for: rows as its own
/// will be, their words still grey bars.
fn skeleton() -> View {
    let bar = |w: f32, h: f32| view().w(px(w)).h(px(h)).rounded(px(R_FULL)).bg(tint(FG, 0.08));
    let mut c = card();
    for (i, w) in [240.0, 190.0, 260.0, 170.0, 220.0].into_iter().enumerate() {
        c = c.child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S3))
                .px(sp(S4))
                .py(sp(S3))
                .when(i > 0, |v| v.border_t(px(1.0)).border_color(BORDER))
                .child(view().w(px(16.0)).h(px(16.0)).rounded(px(R_FULL)).bg(tint(FG, 0.08)))
                .child(view().flex_1().min_w(px(0.0)).gap(px(6.0)).child(bar(w, 12.0)).child(bar(140.0, 9.0))),
        );
    }
    view()
        .gap(sp(S2))
        .label(t("Loading pull requests…"))
        .child(bar(160.0, 12.0))
        .child(c)
}

fn failure(words: String) -> View {
    view()
        .py(sp(S6))
        .items_center()
        .gap(sp(S2))
        .child(icon(Icon::CircleAlert, 20.0).text_color(DANGER))
        .child(text(words).text_size(tx(TEXT_SM)).text_color(DANGER).text_center().max_w(px(520.0)))
}

// ------------------------------------------------------------------- inbox

/// The inbox's list: the pull requests that involve the user in the
/// filter's state (open, drafts, merged, closed), by repository, the
/// projects here first, and in each the ones asking for a review first.
/// `open`: a press.
pub(crate) fn inbox_list(open: impl Fn(InboxItem) + Clone + 'static) -> impl IntoElement {
    let filter = computed(store::inbox_filter);
    let key = computed(move || format!("inbox:{}", filter.get().state));
    let answer = computed(move || store::github(&key.get()));
    let loading = computed(move || store::github_loading(&key.get()));
    view()
        .gap(sp(S4))
        .child(
            view().flex_row().child(segmented(
                vec![
                    (("open", false), t("Open").into()),
                    (("open", true), t("Draft").into()),
                    (("merged", false), t("Merged").into()),
                    (("closed", false), t("Closed").into()),
                ],
                move || filter.with(|f| (f.state, f.drafts)),
                |(state, drafts): (&'static str, bool)| store::set_inbox_filter(state, drafts),
            )),
        )
        .child(dynamic(move || {
            let answer = answer.get();
            let busy = loading.get();
            let f = filter.get();
            let open = open.clone();
            Some(untracked(move || match answer {
                None if busy => skeleton().into_element(),
                None => message(t("Pull requests show here")).into_element(),
                Some(Err(e)) => failure(e).into_element(),
                Some(Ok(v)) => {
                    let inbox = Inbox::from_json(&v);
                    let items: Vec<InboxItem> = inbox.items.iter().filter(|i| !f.drafts || i.draft).cloned().collect();
                    if items.is_empty() {
                        return message(match (f.state, f.drafts) {
                            (_, true) => t("No drafts"),
                            ("merged", _) => t("No merged pull requests"),
                            ("closed", _) => t("No closed pull requests"),
                            _ => t("No open pull requests involve you"),
                        })
                        .into_element();
                    }
                    let tone = match f.state {
                        "merged" => DOT_RUNNING,
                        "closed" => DANGER,
                        _ => SUCCESS,
                    };
                    let mut list = view().gap(sp(S6));
                    for (repo, group) in by_repository(&inbox, items) {
                        let local = inbox.local_of(&repo).map(|p| p.rsplit('/').next().unwrap_or(p).to_owned());
                        let mut c = card();
                        for (i, item) in group.iter().cloned().enumerate() {
                            c = c.child(inbox_row(item, i > 0, tone, open.clone()));
                        }
                        list = list.child(view().gap(sp(S2)).child(group_head(&repo, local, group.len())).child(c));
                    }
                    // As many as asked for: there may be more.
                    let limit = store::inbox_limit(f.state);
                    if inbox.items.len() as u32 >= limit && limit < crate::github::INBOX_MAX {
                        list = list.child(view().items_center().child(btn(
                            if busy { t("Loading…") } else { t("Load more") },
                            Variant::Secondary,
                            Sz::Md,
                            None,
                            store::inbox_load_more,
                        )));
                    }
                    list.into_element()
                }
            }))
        }))
}

/// The pull requests by repository: the repositories with a project here
/// first, then by their latest; in each, the ones asking for a review
/// first, then the latest.
pub(crate) fn by_repository(inbox: &Inbox, mut items: Vec<InboxItem>) -> Vec<(String, Vec<InboxItem>)> {
    items.sort_by(|a, b| b.review_requested.cmp(&a.review_requested).then(b.updated_at.cmp(&a.updated_at)));
    let mut groups: Vec<(String, Vec<InboxItem>)> = Vec::new();
    for item in items {
        match groups.iter_mut().find(|(r, _)| *r == item.repo) {
            Some((_, g)) => g.push(item),
            None => groups.push((item.repo.clone(), vec![item])),
        }
    }
    let latest = |g: &[InboxItem]| g.iter().map(|i| i.updated_at.clone()).max().unwrap_or_default();
    groups.sort_by(|(ra, a), (rb, b)| {
        let (la, lb) = (inbox.local_of(ra).is_some(), inbox.local_of(rb).is_some());
        lb.cmp(&la).then(latest(b).cmp(&latest(a)))
    });
    groups
}

/// A repository's heading: its project here by name (else the
/// repository), the repository after it, and how many.
fn group_head(repo: &str, local: Option<String>, count: usize) -> View {
    let (name, rest) = match local {
        Some(n) if !repo.ends_with(&format!("/{n}")) => (n, repo.to_owned()),
        Some(n) => (n, repo.to_owned()),
        None => (repo.to_owned(), String::new()),
    };
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .px(sp(S1))
        .child(icon(if rest.is_empty() { Icon::GitPullRequest } else { Icon::Folder }, 14.0).text_color(FG_MUTED))
        .child(text(name).text_size(tx(TEXT_SM)).font_weight(FontWeight(600)))
        .when(!rest.is_empty(), |v| v.child(text(rest).text_size(tx(TEXT_XS)).text_color(FG_FAINT)))
        .child(text(count.to_string()).text_size(tx(TEXT_XS)).text_color(FG_FAINT))
}

fn inbox_row(item: InboxItem, divider: bool, tone: AdaptiveColor, open: impl Fn(InboxItem) + 'static) -> impl IntoElement {
    let tone = if item.draft { FG_MUTED } else { tone };
    let pressed = item.clone();
    button()
        .on_press(move || open(pressed.clone()))
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .px(sp(S4))
        .py(sp(S3))
        .when(divider, |v| v.border_t(px(1.0)).border_color(BORDER))
        .hover(|s| s.bg(highlight()))
        .active(|s| s.opacity(0.7))
        .child(icon(Icon::GitPullRequest, 16.0).text_color(tone))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(2.0))
                .child(text(item.title.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)).number_of_lines(1))
                .child(
                    text(format!("#{} · {} · {}", item.number, item.author, ago(&item.updated_at)))
                        .text_size(tx(TEXT_XS))
                        .text_color(FG_MUTED)
                        .number_of_lines(1),
                ),
        )
        .when(item.review_requested, |v| v.child(chip(t("Review requested"), WARNING)))
        .when(item.draft, |v| v.child(chip(t("Draft"), FG_MUTED)))
}

/// The desktop's inbox screen.
pub(crate) fn inbox_page() -> impl IntoElement {
    scroll().flex_1().child(
        view()
            .w_full()
            .max_w(px(860.0))
            .self_center()
            .px(sp(S6))
            .py(sp(S8))
            .gap(sp(S6))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .child(text(t("Pull requests")).text_size(tx(TEXT_2XL)).font_weight(FontWeight(500)).flex_1())
                    .child(btn(t("Refresh"), Variant::Secondary, Sz::Md, Some(Icon::RotateCw), store::refresh_inbox)),
            )
            .child(inbox_list(|item: InboxItem| store::open_pull_request(&item.repo, item.number, None))),
    )
}

// ------------------------------------------------------------ pull request

#[derive(Clone, Copy, PartialEq)]
enum PrTab {
    Summary,
    Code,
    Checks,
}

/// The project folder here of `repo`, as the inbox found it.
fn local_root(repo: &str) -> Option<String> {
    crate::github::STATES.iter().find_map(|state| {
        let v = store::github(&format!("inbox:{state}"))?.ok()?;
        Inbox::from_json(&v).local_of(repo).map(str::to_owned)
    })
}

/// The desktop's pull request screen (`Screen::PullRequest`).
pub(crate) fn pull_request_screen() -> impl IntoElement {
    let target = computed(|| match store::screen() {
        Screen::PullRequest { repo, number, agent } => Some((repo, number, agent)),
        _ => None,
    });
    view().flex_1().child(dynamic(move || {
        let (repo, number, agent) = target.get()?;
        Some(untracked(move || {
            let back: Rc<dyn Fn()> = match agent.clone().and_then(|a| store::agent(&a)) {
                Some(a) => Rc::new(move || {
                    if let Some(ws) = &a.workspace_id {
                        store::open_workspace(ws);
                        store::open_tab(store::Tab::Agent(a.id.clone()));
                    }
                }),
                None => Rc::new(store::open_inbox),
            };
            view()
                .flex_1()
                .w_full()
                .max_w(px(1100.0))
                .self_center()
                .px(sp(S6))
                .pt(sp(S6))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), move || back())),
                )
                .child(pull_request(repo, number, agent, |_| {}))
                .into_element()
        }))
    }))
}

/// One pull request: its head (state, title, branches, size), its actions,
/// and its Summary, Code and Checks. `agent`: the conversation it was
/// opened from; `fixed`: after "Fix with agent" put its prompt somewhere
/// (the workspace of that conversation, or `None`: a new conversation's
/// form), for the phone to go there.
pub(crate) fn pull_request(repo: String, number: u64, agent: Option<String>, fixed: impl Fn(Option<String>) + Clone + 'static) -> impl IntoElement {
    let view_key = Op::View { repo: repo.clone(), number }.key();
    let checks_key = Op::Checks { repo: repo.clone(), number }.key();
    let (vk, ck) = (view_key.clone(), checks_key.clone());
    let detail = computed(move || store::github(&vk));
    let busy = computed(move || store::github_loading(&view_key));
    let checks = computed(move || store::github(&ck).and_then(Result::ok).map(|v| crate::github::checks_of(&v)));
    let tab = signal(PrTab::Summary);
    let repo2 = repo.clone();
    view().flex_1().min_h(px(0.0)).child(dynamic(move || {
        let detail = detail.get();
        let (repo, agent, fixed) = (repo2.clone(), agent.clone(), fixed.clone());
        Some(untracked(move || match detail {
            None => message(t("Loading the pull request…")).into_element(),
            Some(Err(e)) => failure(e).into_element(),
            Some(Ok(v)) => {
                let d = Detail::from_json(&v);
                view()
                    .flex_1()
                    .min_h(px(0.0))
                    .gap(sp(S4))
                    .pt(sp(S2))
                    .child(head(&repo, &d))
                    .child(actions(&repo, &d, busy))
                    .child(tabs(tab, checks))
                    .child(body(repo, d, tab, checks, agent, fixed))
                    .into_element()
            }
        }))
    }))
}

fn head(repo: &str, d: &Detail) -> View {
    let (state, tone) = state_of(d);
    let mut labels = view().flex_row().flex_wrap().gap(sp(S1));
    for l in &d.labels {
        labels = labels.child(chip(l.clone(), FG_MUTED));
    }
    view()
        .gap(sp(S2))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .child(icon(Icon::GitPullRequest, 15.0).text_color(tone))
                .child(text(format!("{repo} #{}", d.number)).text_size(tx(TEXT_SM)).text_color(FG_MUTED))
                .child(chip(state, tone)),
        )
        .child(text(d.title.clone()).text_size(tx(TEXT_XL)).font_weight(FontWeight(600)).selectable(true))
        .child(
            text(format!(
                "{} · {} ← {} · +{} −{} · {} · {}",
                d.author,
                d.base,
                d.head,
                d.additions,
                d.deletions,
                tf("{} files", &[&d.changed_files.to_string()]),
                ago(&d.updated_at)
            ))
            .text_size(tx(TEXT_SM))
            .text_color(FG_MUTED),
        )
        .when(!d.labels.is_empty(), |v| v.child(labels))
}

/// Merge (three ways, each asked twice), draft or ready, close or reopen,
/// and GitHub's page.
fn actions(repo: &str, d: &Detail, busy: Memo<bool>) -> View {
    let confirm = signal(None::<&'static str>);
    let (open, merged, draft, conflicting) = (d.open(), d.merged(), d.draft, d.mergeable.eq_ignore_ascii_case("CONFLICTING"));
    let (repo, number, url) = (repo.to_owned(), d.number, d.url.clone());
    let act = {
        let repo = repo.clone();
        move |action: &'static str| {
            let repo = repo.clone();
            move || store::pull_request_act(&repo, number, action)
        }
    };
    let ask = move |action: &'static str| move || confirm.set(Some(action));
    let mut row = view().flex_row().flex_wrap().items_center().gap(sp(S2));
    if open && !draft && !conflicting {
        row = row.child(
            dropdown(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .h(px(32.0))
                    .px(sp(S3))
                    .rounded(px(R_MD))
                    .bg(ACCENT)
                    .label(t("Merge pull request"))
                    .child(icon(Icon::GitMerge, 14.0).text_color(ACCENT_FG))
                    .child(text(t("Merge pull request")).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(ACCENT_FG))
                    .child(icon(Icon::ChevronDown, 12.0).text_color(ACCENT_FG)),
            )
            .child(menu_row(t("Create a merge commit"), Some(Icon::GitMerge), None, false, ask("merge")))
            .child(menu_row(t("Squash and merge"), Some(Icon::GitMerge), None, false, ask("squash")))
            .child(menu_row(t("Rebase and merge"), Some(Icon::GitMerge), None, false, ask("rebase"))),
        );
    }
    if open {
        row = if draft {
            row.child(btn(t("Ready for review"), Variant::Secondary, Sz::Md, Some(Icon::Eye), act("ready")))
        } else {
            row.child(btn(t("Convert to draft"), Variant::Secondary, Sz::Md, None, ask("draft")))
        };
        row = row.child(btn(t("Close pull request"), Variant::Secondary, Sz::Md, None, ask("close")));
    } else if !merged {
        row = row.child(btn(t("Reopen"), Variant::Secondary, Sz::Md, Some(Icon::RotateCw), act("reopen")));
    }
    row = row
        .child(btn(t("Open on GitHub"), Variant::Ghost, Sz::Md, Some(Icon::ExternalLink), move || {
            let _ = open_url(url.clone());
        }))
        .child(when(move || busy.get(), || eclipse(16.0)));
    let note = if conflicting && open {
        Some(t("This branch has conflicts: resolve them before merging"))
    } else if draft && open {
        Some(t("A draft is merged once it is ready for review"))
    } else {
        None
    };
    view()
        .gap(sp(S2))
        .child(row)
        .when_some(note, |v, n| v.child(text(n).text_size(tx(TEXT_SM)).text_color(WARNING)))
        // The question an action asks before it is done.
        .child(dynamic(move || {
            let action = confirm.get()?;
            let go = act(action);
            Some(untracked(move || {
                let question = match action {
                    "merge" => tf("Merge #{} with a merge commit?", &[&number.to_string()]),
                    "squash" => tf("Squash and merge #{}?", &[&number.to_string()]),
                    "rebase" => tf("Rebase and merge #{}?", &[&number.to_string()]),
                    "draft" => tf("Convert #{} to a draft?", &[&number.to_string()]),
                    _ => tf("Close #{} without merging?", &[&number.to_string()]),
                };
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .p(sp(S3))
                    .rounded(px(R_LG))
                    .bg(tint(WARNING, 0.1))
                    .child(text(question).text_size(tx(TEXT_SM)).flex_1())
                    .child(btn(t("Cancel"), Variant::Secondary, Sz::Sm, None, move || confirm.set(None)))
                    .child(btn(t("Confirm"), if action == "close" { Variant::Destructive } else { Variant::Primary }, Sz::Sm, None, move || {
                        confirm.set(None);
                        go();
                    }))
                    .into_element()
            }))
        }))
}

fn tabs(tab: Signal<PrTab>, checks: Memo<Option<Vec<Check>>>) -> View {
    let checks_word = computed(move || {
        let failing = checks.with(|c| c.as_ref().map(|c| c.iter().filter(|c| c.state == CheckState::Failed).count()).unwrap_or(0));
        if failing > 0 { tf("Checks · {} failing", &[&failing.to_string()]) } else { t("Checks").to_owned() }
    });
    view().child(dynamic(move || {
        let word = checks_word.get();
        Some(untracked(move || {
            segmented(
                vec![(PrTab::Summary, t("Summary").into()), (PrTab::Code, t("Code").into()), (PrTab::Checks, word.into())],
                move || tab.get(),
                move |t| tab.set(t),
            )
        }))
    }))
}

fn body(repo: String, d: Detail, tab: Signal<PrTab>, checks: Memo<Option<Vec<Check>>>, agent: Option<String>, fixed: impl Fn(Option<String>) + Clone + 'static) -> View {
    view().flex_1().min_h(px(0.0)).child(dynamic(move || {
        let which = tab.get();
        let (repo, d, agent, fixed) = (repo.clone(), d.clone(), agent.clone(), fixed.clone());
        Some(untracked(move || match which {
            PrTab::Summary => summary(&d).into_element(),
            PrTab::Code => code(repo, d.number).into_element(),
            PrTab::Checks => checks_tab(repo, d, checks, agent, fixed).into_element(),
        }))
    }))
}

fn summary(d: &Detail) -> View {
    let body = d.body.trim().to_owned();
    if body.is_empty() {
        return message(t("No description"));
    }
    view().flex_1().min_h(px(0.0)).child(scroll().flex_1().child(
        view().pb(sp(S8)).child(
            text_view()
                .links(nsh_base::TextViewLinks::Open)
                .markdown(SharedString::from(body))
                .selectable(true)
                .code_block_actions(vec![nsh_base::TextViewCodeAction::new("copy").with_label(t("Copy"))])
                .gap_3()
                .text_size(tx(TEXT_CONTENT))
                .line_height(px(24.0))
                .text_color(FG),
        ),
    ))
}

/// The diff, asked for the first time Code opens.
fn code(repo: String, number: u64) -> View {
    let key = Op::Diff { repo: repo.clone(), number }.key();
    if store::github(&key).is_none() && !store::github_loading(&key) {
        store::github_fetch(Op::Diff { repo, number });
    }
    let diff = computed(move || store::github(&key));
    view().flex_1().min_h(px(0.0)).child(dynamic(move || {
        let diff = diff.get();
        Some(untracked(move || match diff {
            None => message(t("Loading changes…")).into_element(),
            Some(Err(e)) => failure(e).into_element(),
            Some(Ok(v)) => {
                let patch = v.get("patch").and_then(|p| p.as_str()).unwrap_or_default().to_owned();
                let truncated = v.get("truncated").and_then(|t| t.as_bool()).unwrap_or(false);
                if patch.trim().is_empty() {
                    return message(t("No changes")).into_element();
                }
                view()
                    .flex_1()
                    .min_h(px(0.0))
                    .gap(sp(S2))
                    .when(truncated, |v| v.child(text(t("This diff is long and was cut: GitHub has all of it")).text_size(tx(TEXT_SM)).text_color(WARNING)))
                    .child(
                        diff_view()
                            .patch(SharedString::from(patch))
                            .layout(if MOBILE { DiffLayout::Unified } else { DiffLayout::Split })
                            .text_size(tx(TEXT_CODE))
                            .flex_1(),
                    )
                    .into_element()
            }
        }))
    }))
}

fn checks_tab(repo: String, d: Detail, checks: Memo<Option<Vec<Check>>>, agent: Option<String>, fixed: impl Fn(Option<String>) + Clone + 'static) -> View {
    view().flex_1().min_h(px(0.0)).child(scroll().flex_1().child(view().pb(sp(S8)).gap(sp(S3)).child(dynamic(move || {
        let list = checks.get();
        let (repo, d, agent, fixed) = (repo.clone(), d.clone(), agent.clone(), fixed.clone());
        Some(untracked(move || {
            let Some(list) = list else { return message(t("Loading checks…")).into_element() };
            if list.is_empty() {
                return message(t("No checks reported")).into_element();
            }
            let failing: Vec<Check> = list.iter().filter(|c| c.state == CheckState::Failed).cloned().collect();
            let mut v = view().gap(sp(S3));
            if !failing.is_empty() {
                let root = local_root(&repo);
                // Somewhere to work on it: the conversation it came from,
                // or its project here.
                let can_fix = agent.is_some() || root.is_some();
                let prompt = crate::github::fix_prompt(&repo, &d, &failing);
                v = v.child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S3))
                        .p(sp(S3))
                        .rounded(px(R_LG))
                        .bg(tint(DANGER, 0.08))
                        .child(icon(Icon::CircleX, 16.0).text_color(DANGER))
                        .child(text(tf("{} checks failing", &[&failing.len().to_string()])).text_size(tx(TEXT_SM)).flex_1())
                        .when(can_fix, |row| {
                            row.child(btn(t("Fix with agent"), Variant::Primary, Sz::Sm, Some(Icon::Sparkles), move || {
                                let went = store::fix_with_agent(prompt.clone(), agent.as_deref(), root.as_deref());
                                fixed(went);
                            }))
                        }),
                );
            }
            let mut c = card();
            for (i, check) in list.into_iter().enumerate() {
                c = c.child(check_row(check, i > 0));
            }
            v.child(c).into_element()
        }))
    }))))
}

fn check_row(c: Check, divider: bool) -> impl IntoElement {
    let (glyph, tone, word) = match c.state {
        CheckState::Passed => (Icon::CircleCheck, SUCCESS, t("Passed")),
        CheckState::Failed => (Icon::CircleX, DANGER, t("Failed")),
        CheckState::Running => (Icon::CircleDot, WARNING, t("Running")),
        CheckState::Skipped => (Icon::Minus, FG_FAINT, t("Skipped")),
    };
    let url = c.url.clone();
    button()
        .on_press(move || {
            if !url.is_empty() {
                let _ = open_url(url.clone());
            }
        })
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .px(sp(S4))
        .py(sp(S3))
        .when(divider, |v| v.border_t(px(1.0)).border_color(BORDER))
        .hover(|s| s.bg(highlight()))
        .child(icon(glyph, 15.0).text_color(tone))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .child(text(c.name.clone()).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).number_of_lines(1))
                .when(!c.workflow.is_empty(), |v| v.child(text(c.workflow.clone()).text_size(tx(TEXT_XS)).text_color(FG_MUTED))),
        )
        .child(text(word).text_size(tx(TEXT_XS)).text_color(tone))
        .when(!c.url.is_empty(), |v| v.child(icon(Icon::ExternalLink, 12.0).text_color(FG_FAINT)))
}

#[cfg(test)]
mod tests {
    use super::by_repository;
    use crate::github::{Inbox, InboxItem};

    fn item(repo: &str, number: u64, updated: &str, review: bool) -> InboxItem {
        InboxItem { repo: repo.into(), number, title: String::new(), url: String::new(), draft: false, author: String::new(), updated_at: updated.into(), review_requested: review }
    }

    #[test]
    fn pull_requests_group_by_repository_projects_here_first() {
        let inbox = Inbox { items: vec![], local: vec![("caoxing9/sophon".into(), "/Users/me/sophon".into())] };
        let items = vec![
            item("teableio/teable-ee", 1, "2026-11-16T05:00:00Z", false),
            item("caoxing9/sophon", 61, "2026-11-16T02:00:00Z", false),
            item("teableio/teable-ee", 2, "2026-11-10T00:00:00Z", true),
            item("caoxing9/sophon", 63, "2026-11-16T04:00:00Z", false),
        ];
        let groups = by_repository(&inbox, items);
        let shape: Vec<(String, Vec<u64>)> = groups.into_iter().map(|(r, g)| (r, g.iter().map(|i| i.number).collect())).collect();
        assert_eq!(shape, vec![
            ("caoxing9/sophon".to_owned(), vec![63, 61]),
            // Asked to review first, though older.
            ("teableio/teable-ee".to_owned(), vec![2, 1]),
        ]);
    }
}
