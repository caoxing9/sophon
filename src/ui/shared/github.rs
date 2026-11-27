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

fn chip(word: impl Into<SharedString>, tone: AdaptiveColor) -> nsh_shadcn::BadgeElement {
    let word: SharedString = word.into();
    badge(word, tone)
}

fn message(words: impl Into<SharedString>) -> View {
    view().py(sp(S8)).items_center().child(text(words.into()).text_size(tx(TEXT_BASE)).text_color(FG_MUTED()).text_center())
}

/// What the inbox shows the first time it is asked for: rows as its own
/// will be, their words still grey bars.
fn skeleton() -> View {
    let bar = |w: f32, h: f32| view().w(px(w)).h(px(h)).rounded(px(R_FULL)).bg(tint(FG(), 0.08));
    let mut c = card();
    for (i, w) in [240.0, 190.0, 260.0, 170.0, 220.0].into_iter().enumerate() {
        c = c.child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S3))
                .px(sp(S4))
                .py(sp(S3))
                .when(i > 0, |v| v.border_t(px(1.0)).border_color(BORDER()))
                .child(view().w(px(16.0)).h(px(16.0)).rounded(px(R_FULL)).bg(tint(FG(), 0.08)))
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
        .child(icon(if rest.is_empty() { Icon::GitPullRequest } else { Icon::Folder }, 14.0).text_color(FG_MUTED()))
        .child(text(name).text_size(tx(TEXT_SM)).font_weight(FontWeight(600)))
        .when(!rest.is_empty(), |v| v.child(text(rest).text_size(tx(TEXT_XS)).text_color(FG_FAINT())))
        .child(text(count.to_string()).text_size(tx(TEXT_XS)).text_color(FG_FAINT()))
}

fn inbox_row(item: InboxItem, divider: bool, tone: AdaptiveColor, open: impl Fn(InboxItem) + 'static) -> impl IntoElement {
    let tone = if item.draft { FG_MUTED() } else { tone };
    let pressed = item.clone();
    button()
        .on_press(move || open(pressed.clone()))
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .px(sp(S4))
        .py(sp(S3))
        .when(divider, |v| v.border_t(px(1.0)).border_color(BORDER()))
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
                        .text_color(FG_MUTED())
                        .number_of_lines(1),
                ),
        )
        .when(item.review_requested, |v| v.child(chip(t("Review requested"), WARNING)))
        .when(item.draft, |v| v.child(chip(t("Draft"), FG_MUTED())))
}

/// The desktop's inbox screen.
pub(crate) fn inbox_page() -> impl IntoElement {
    let filters = computed(store::desk_inbox);
    let all = computed(store::desk_inbox_items);
    let loading = computed(|| !store::github_loading_with("inbox:").is_empty());
    let answered = computed(|| crate::github::STATES.iter().any(|s| store::github(&format!("inbox:{s}")).is_some()));
    let selected = computed(store::inbox_selected);
    let shown = computed(move || {
        let f = filters.get();
        let now = crate::ui::mobile::time::now();
        all.get().into_iter().filter(|i| f.shows(i, crate::ui::mobile::time::epoch_of(&i.updated_at).map(|t| now - t))).collect::<Vec<_>>()
    });
    let unseen = computed(move || shown.with(|l| l.iter().any(store::inbox_unseen)));
    let list = view()
        .w(px(340.0))
        .flex_shrink_0()
        .border_r(px(1.0))
        .border_color(BORDER())
        // The source: GitHub (monocode's tabs of sources).
        .child(
            view().flex_row().items_center().h(px(40.0)).px(sp(S2)).border_b(px(1.0)).border_color(BORDER()).window_drag(true).child(
                view()
                    .flex_row()
                    .items_center()
                    .justify_center()
                    .gap(px(6.0))
                    .h(px(28.0))
                    .flex_1()
                    .rounded(px(R_MD))
                    .bg(highlight())
                    .child(icon(Icon::Github, 14.0).text_color(FG()))
                    .child(text("GitHub").text_size(tx(TEXT_SM)).font_weight(FontWeight(500))),
            ),
        )
        // Search, filters, all read, refresh.
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .h(px(40.0))
                .px(sp(S2))
                .border_b(px(1.0))
                .border_color(BORDER())
                .child(icon(Icon::Search, 13.0).text_color(FG_FAINT()).ml(px(6.0)))
                .child(
                    nsh_base::input()
                        .flex_1()
                        .h(px(28.0))
                        .px(px(6.0))
                        .text_size(tx(TEXT_SM))
                        .text_color(FG())
                        .caret_color(|| color(ACCENT()))
                        .placeholder(t("Filter inbox"))
                        .value(move || filters.with(|f| f.query.clone()))
                        .on_change(|v: &str| {
                            let v = v.to_owned();
                            store::set_desk_inbox(move |f| f.query = v);
                        }),
                )
                .child(filters_menu(filters, all))
                .child(tool_button(Icon::CheckCheck, t("Mark all as read"), move || unseen.get(), move || store::mark_inbox_seen(&shown.get_untracked())))
                .child(
                    nsh_base::button()
                        .on_press(store::refresh_desk_inbox)
                        .accessibility_label(t("Refresh"))
                        .w(px(26.0))
                        .h(px(26.0))
                        .rounded(px(R_MD))
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(highlight()))
                        .child(dynamic(move || {
                            let busy = loading.get();
                            Some(untracked(move || if busy { spinner(13.0).into_element() } else { icon(Icon::RotateCw, 13.0).text_color(FG_MUTED()).into_element() }))
                        })),
                ),
        )
        .child(dynamic(move || {
            let list = shown.get();
            let current = selected.get();
            let busy = loading.get();
            let any = answered.get();
            let narrowed = filters.with(|f| f.narrowed() || !f.query.trim().is_empty());
            // The first shows at once, as the list comes.
            if !list.is_empty() && !current.as_ref().is_some_and(|(r, n)| list.iter().any(|i| i.repo == *r && i.number == *n)) {
                let first = list[0].clone();
                untracked(move || {
                    store::mark_inbox_seen(std::slice::from_ref(&first));
                    store::select_inbox(&first.repo, first.number);
                });
            }
            Some(untracked(move || {
                if list.is_empty() {
                    if !any && busy {
                        return view().py(sp(S8)).items_center().child(spinner(18.0)).into_element();
                    }
                    let failed = crate::github::STATES.iter().find_map(|s| store::github(&format!("inbox:{s}")).and_then(Result::err));
                    let words = match failed {
                        Some(e) => e,
                        None if narrowed => t("No pull requests match these filters").to_owned(),
                        None => t("No pull requests involve you").to_owned(),
                    };
                    return text(words).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).px(sp(S3)).py(sp(S2)).into_element();
                }
                let mut col = view().gap(px(2.0)).p(px(6.0));
                for item in list {
                    let on = current.as_ref().is_some_and(|(r, n)| *r == item.repo && *n == item.number);
                    col = col.child(inbox_card(item, on));
                }
                scroll().flex_1().child(col).into_element()
            }))
        }));
    view()
        .flex_1()
        .flex_row()
        .min_h(px(0.0))
        .child(list)
        .child(view().flex_1().min_w(px(0.0)).child(dynamic(move || {
            let (repo, number) = selected.get()?;
            Some(untracked(move || pull_request(repo, number, None, |_| {}).into_element()))
        })))
}

/// A small square button of a toolbar, `enabled` or dimmed.
fn tool_button(glyph: Icon, label: &'static str, enabled: impl Fn() -> bool + Copy + 'static, on_press: impl Fn() + 'static) -> impl IntoElement {
    nsh_base::button()
        .on_press(move || {
            if enabled() {
                on_press();
            }
        })
        .accessibility_label(label)
        .w(px(26.0))
        .h(px(26.0))
        .rounded(px(R_MD))
        .items_center()
        .justify_center()
        .opacity(move || if enabled() { 1.0_f32 } else { 0.35 })
        .hover(|s| s.bg(highlight()))
        .child(icon(glyph, 14.0).text_color(FG_MUTED()))
}

/// The inbox's filters (monocode's menu): the states, how recent, asked
/// to review, and the repositories. It stays open while things are picked.
fn filters_menu(filters: Memo<store::DeskInbox>, all: Memo<Vec<InboxItem>>) -> impl IntoElement {
    let open = signal(false);
    let active = move || filters.with(store::DeskInbox::narrowed);
    let repos = computed(move || {
        let mut r: Vec<String> = all.with(|l| l.iter().map(|i| i.repo.clone()).collect());
        r.sort_by_key(|x| x.to_lowercase());
        r.dedup();
        r
    });
    let section = |words: &'static str| {
        text(words.to_uppercase()).text_size(tx(10.0)).font_weight(FontWeight(600)).text_color(FG_FAINT()).px(sp(S2)).pt(sp(S2)).pb(px(2.0))
    };
    let item = |label: SharedString, checked: Box<dyn Fn() -> bool>, on_press: Box<dyn Fn()>| {
        let checked: std::rc::Rc<dyn Fn() -> bool> = checked.into();
        let c = checked.clone();
        nsh_base::button()
            .on_press(move || on_press())
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .h(px(28.0))
            .px(sp(S2))
            .rounded(px(R_MD))
            .hover(|s| s.bg(highlight()))
            .child(text(label).text_size(tx(13.0)).text_color(FG()).number_of_lines(1).flex_1().min_w(px(0.0)))
            .child(icon(Icon::Check, 14.0).text_color(FG()).opacity(move || if c() { 1.0_f32 } else { 0.0 }))
    };
    let divider = || view().h(px(1.0)).my(px(4.0)).bg(tint(FG(), 0.08));
    nsh_base::popover()
        .open(move || Some(open.get()))
        .on_open_change(move |e: &nsh_base::PopoverEvent| {
            if let nsh_base::PopoverEvent::OpenChange(next, _) = e {
                open.set(*next);
            }
        })
        .align(nsh_core::overlay::Align::Start)
        .offset(px(4.0))
        .popup_style(|s: nsh_core::element::StyleRefinement| {
            s.bg(POPOVER()).rounded(px(R_LG)).border_1().border_color(tint(FG(), 0.10)).overflow_hidden().w(px(240.0))
        })
        .trigger(
            view()
                .w(px(26.0))
                .h(px(26.0))
                .rounded(px(R_MD))
                .items_center()
                .justify_center()
                .bg(move || if active() || open.get() { highlight() } else { tint(FG(), 0.0) })
                .hover(|s| s.bg(highlight()))
                .label(t("Filter inbox"))
                .child(icon(Icon::ListFilter, 13.0).text_color(move || if active() { FG() } else { FG_MUTED() })),
        )
        .child(scroll().max_h(px(480.0)).child({
            let mut col = view().p(px(4.0)).child(item(
                t("Asks for my review").into(),
                Box::new(move || filters.with(|f| f.review_only)),
                Box::new(|| store::set_desk_inbox(|f| f.review_only = !f.review_only)),
            ));
            col = col.child(section(t("Status")));
            for (i, word) in [t("Open"), t("Draft"), t("Merged"), t("Closed")].into_iter().enumerate() {
                col = col.child(item(word.into(), Box::new(move || filters.with(|f| f.status[i])), Box::new(move || store::set_desk_inbox(|f| f.status[i] = !f.status[i]))));
            }
            col = col.child(section(t("Updated")));
            for (key, word) in [("all", t("Any time")), ("day", t("Today")), ("week", t("This week")), ("month", t("This month"))] {
                col = col.child(item(
                    word.into(),
                    Box::new(move || filters.with(|f| f.time == key || (key == "all" && f.time.is_empty()))),
                    Box::new(move || store::set_desk_inbox(|f| f.time = key)),
                ));
            }
            col.child(dynamic(move || {
                let list = repos.get();
                Some(untracked(move || {
                    if list.is_empty() {
                        return view().into_element();
                    }
                    let mut v = view().child(section(t("Repositories")));
                    for repo in list {
                        let (a, b) = (repo.clone(), repo.clone());
                        v = v.child(item(
                            repo.into(),
                            Box::new(move || filters.with(|f| !f.hidden.iter().any(|h| h.eq_ignore_ascii_case(&a)))),
                            Box::new(move || {
                                let r = b.clone();
                                store::set_desk_inbox(move |f| match f.hidden.iter().position(|h| h.eq_ignore_ascii_case(&r)) {
                                    Some(i) => {
                                        f.hidden.remove(i);
                                    }
                                    None => f.hidden.push(r),
                                })
                            }),
                        ));
                    }
                    v.into_element()
                }))
            }))
            .child(when(active, move || {
                view().child(divider()).child(
                    nsh_base::button()
                        .on_press(|| {
                            store::set_desk_inbox(|f| {
                                let query = std::mem::take(&mut f.query);
                                *f = store::DeskInbox { query, ..Default::default() };
                            })
                        })
                        .h(px(28.0))
                        .px(sp(S2))
                        .rounded(px(R_MD))
                        .justify_center()
                        .hover(|s| s.bg(highlight()))
                        .child(text(t("Clear filters")).text_size(tx(13.0)).text_color(FG_MUTED())),
                )
            }))
        }))
}

/// A pull request's state as monocode marks it: icon, color, word.
fn status_mark(state: &str, draft: bool) -> (Icon, AdaptiveColor, &'static str) {
    match state.to_ascii_lowercase().as_str() {
        "merged" => (Icon::GitMerge, VIOLET, t("Merged")),
        "closed" => (Icon::GitPullRequestClosed, DANGER, t("Closed")),
        _ if draft => (Icon::GitPullRequestDraft, FG_MUTED(), t("Draft")),
        _ => (Icon::GitPullRequest, SUCCESS, if crate::i18n::is_zh() { "开启" } else { t("Open") }),
    }
}

/// A label, small.
fn label_chip(name: String) -> View {
    view()
        .px(px(6.0))
        .h(px(18.0))
        .rounded(px(R_FULL))
        .bg(tint(FG(), 0.06))
        .items_center()
        .justify_center()
        .child(text(name).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).number_of_lines(1))
}

/// A card of the inbox's list (monocode's `InboxCard`).
fn inbox_card(item: InboxItem, on: bool) -> impl IntoElement {
    let (glyph, tone, _) = status_mark(&item.state, item.draft);
    let unseen = computed({
        let item = item.clone();
        move || store::inbox_unseen(&item)
    });
    let pressed = item.clone();
    let mut labels = view().flex_row().gap(px(4.0)).flex_shrink_0();
    for l in item.labels.iter().take(2) {
        labels = labels.child(label_chip(l.clone()));
    }
    button()
        .on_press(move || {
            store::mark_inbox_seen(std::slice::from_ref(&pressed));
            store::select_inbox(&pressed.repo, pressed.number);
        })
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(R_MD))
        .when(on, |v| v.bg(highlight()))
        .hover(|s| s.bg(highlight()))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon(Icon::Github, 13.0).text_color(FG()))
                .child(icon(glyph, 12.0).text_color(tone))
                .child(text(format!("{} · #{}", t("Pull request"), item.number)).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).number_of_lines(1).flex_1().min_w(px(0.0)))
                .child(text(crate::ui::mobile::time::ago_long(&item.updated_at)).text_size(tx(TEXT_XS)).text_color(FG_FAINT()))
                .child(when(move || unseen.get(), || view().w(px(6.0)).h(px(6.0)).rounded(px(R_FULL)).bg(DOT_RUNNING))),
        )
        .child(text(item.title.clone()).text_size(tx(TEXT_SM)).font_weight(FontWeight(600)).number_of_lines(1).mt(px(4.0)))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .mt(px(4.0))
                .child(text(item.repo.clone()).text_size(tx(TEXT_XS)).text_color(FG_FAINT()).number_of_lines(1).flex_1().min_w(px(0.0)))
                .when(item.review_requested, |v| v.child(chip(t("Review requested"), WARNING)))
                .child(labels),
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
            None => view().flex_1().items_center().pt(px(80.0)).child(spinner(20.0)).into_element(),
            Some(Err(e)) => failure(e).into_element(),
            Some(Ok(v)) => {
                let d = Detail::from_json(&v);
                view()
                    .flex_1()
                    .min_h(px(0.0))
                    .child(
                        // The head, fixed over what scrolls.
                        view()
                            .border_b(px(1.0))
                            .border_color(BORDER())
                            .child(
                                view()
                                    .px(if MOBILE { sp(S1) } else { px(32.0) })
                                    .pt(px(20.0))
                                    .gap(px(10.0))
                                    .child(head(&repo, &d))
                                    .child(actions(&repo, &d, busy))
                                    .child(tabs(tab, checks)),
                            ),
                    )
                    .child(body(repo, d, tab, checks, agent, fixed))
                    .into_element()
            }
        }))
    }))
}

/// A person, as GitHub shows them: their picture and login.
fn person(login: &str, size: f32) -> View {
    view()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .flex_shrink_0()
        .child(
            view()
                .w(px(size))
                .h(px(size))
                .rounded(px(R_FULL))
                .overflow_hidden()
                .bg(tint(FG(), 0.08))
                .child(image(SharedString::from(format!("https://github.com/{login}.png?size={}", (size * 2.0) as u32))).w(px(size)).h(px(size)).object_fit(nsh_core::ResizeMode::Cover)),
        )
        .child(text(login.to_owned()).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).number_of_lines(1))
}

fn dot() -> nsh_core::element::Text {
    text("·").text_size(tx(TEXT_SM)).text_color(FG_FAINT())
}

/// Its kind, number, state and repository; the title; who, when, the
/// branches and the review (monocode's detail head).
fn head(repo: &str, d: &Detail) -> View {
    let (glyph, tone, state) = status_mark(&d.state, d.draft);
    let (review, review_tone) = match d.review_decision.as_str() {
        "APPROVED" => (t("Approved"), SUCCESS),
        "CHANGES_REQUESTED" => (t("Changes requested"), DANGER),
        "REVIEW_REQUIRED" => (t("Review required"), WARNING),
        _ => ("", FG_MUTED()),
    };
    let head_branch = d.head.clone();
    let mut meta = view().flex_row().flex_wrap().items_center().gap(px(8.0));
    if !d.author.is_empty() {
        meta = meta.child(person(&d.author, 16.0)).child(dot());
    }
    meta = if d.assignees.is_empty() {
        meta.child(text(t("Unassigned")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
    } else {
        let mut people = view().flex_row().gap(px(8.0));
        for a in &d.assignees {
            people = people.child(person(a, 16.0));
        }
        meta.child(people)
    };
    if !d.created_at.is_empty() {
        meta = meta.child(dot()).child(text(tf("Created {}", &[&crate::ui::mobile::time::ago_long(&d.created_at)])).text_size(tx(TEXT_SM)).text_color(FG_MUTED()));
    }
    meta = meta.child(dot()).child(text(tf("Updated {}", &[&crate::ui::mobile::time::ago_long(&d.updated_at)])).text_size(tx(TEXT_SM)).text_color(FG_MUTED()));
    if !d.base.is_empty() && !d.head.is_empty() {
        meta = meta.child(dot()).child(
            view()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .child(icon(Icon::GitCompare, 12.0).text_color(FG_MUTED()))
                .child(text(format!("{} ← {}", d.base, d.head)).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).number_of_lines(1))
                .child(
                    nsh_base::button()
                        .on_press(move || {
                            let _ = set_clipboard(head_branch.clone());
                            store::notify(t("Copied").into());
                        })
                        .accessibility_label(t("Copy branch name"))
                        .w(px(20.0))
                        .h(px(20.0))
                        .rounded(px(R_SM))
                        .items_center()
                        .justify_center()
                        .hover(|s| s.bg(highlight()))
                        .child(icon(Icon::Copy, 11.0).text_color(FG_FAINT())),
                ),
        );
    }
    if !review.is_empty() {
        meta = meta.child(dot()).child(text(review).text_size(tx(TEXT_SM)).text_color(review_tone));
    }
    view()
        .gap(px(10.0))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(icon(Icon::Github, 14.0).text_color(FG_MUTED()))
                .child(text(format!("{} #{}", t("Pull request"), d.number)).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
                .child(icon(glyph, 13.0).text_color(tone))
                .child(text(state).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(tone))
                .child(text(repo.to_owned()).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).number_of_lines(1)),
        )
        .child(text(d.title.clone()).text_size(tx(20.0)).font_weight(FontWeight(600)).line_height(px(26.0)).number_of_lines(2).selectable(true))
        .child(meta)
}

/// What an action asks before it is done (monocode's words).
fn action_copy(action: &str, base: &str, head: &str) -> (String, String, &'static str) {
    let source = if head.is_empty() { t("this branch").to_owned() } else { format!("“{head}”") };
    let dest = if base.is_empty() { t("the base branch").to_owned() } else { format!("“{base}”") };
    match action {
        "merge" => (t("Merge this pull request?").into(), tf("Every commit from {} will be added to {} with a merge commit.", &[&source, &dest]), t("Merge pull request")),
        "squash" => (t("Squash and merge?").into(), tf("The commits from {} will be combined into one commit on {}.", &[&source, &dest]), t("Squash and merge")),
        "rebase" => (t("Rebase and merge?").into(), tf("The commits from {} will be rebased individually onto {}.", &[&source, &dest]), t("Rebase and merge")),
        "draft" => (t("Convert to draft?").into(), t("Reviewers will see that this pull request is not ready to merge.").into(), t("Convert to draft")),
        "ready" => (t("Mark as ready for review?").into(), t("Reviewers will see that this pull request is ready for feedback.").into(), t("Ready for review")),
        "close" => (t("Close this pull request?").into(), t("The pull request will close without merging. You can reopen it later.").into(), t("Close pull request")),
        _ => (t("Reopen this pull request?").into(), t("The pull request will return to the open state.").into(), t("Reopen pull request")),
    }
}

/// An outlined action of the head.
fn outline(label: &'static str, glyph: Icon, danger: bool, on_press: impl Fn() + 'static) -> impl IntoElement {
    nsh_base::button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(28.0))
        .px(sp(S3))
        .rounded(px(R_MD))
        .border_1()
        .border_color(BORDER_STRONG())
        .hover(move |s| if danger { s.bg(tint(DANGER, 0.08)) } else { s.bg(highlight()) })
        .child(icon(glyph, 14.0).text_color(FG()))
        .child(text(label).text_size(tx(TEXT_SM)).text_color(FG()))
}

/// Merge (its way picked beside it), draft or ready, close or reopen, ask
/// an agent, GitHub's page; each asked first (monocode's actions).
fn actions(repo: &str, d: &Detail, busy: Memo<bool>) -> View {
    let confirm = signal(None::<&'static str>);
    let way = signal("merge");
    let (open, merged, draft) = (d.open(), d.merged(), d.draft);
    let (repo, number, url, base, head_ref) = (repo.to_owned(), d.number, d.url.clone(), d.base.clone(), d.head.clone());
    let ask = move |action: &'static str| move || confirm.set(Some(action));
    let mut row = view().flex_row().flex_wrap().items_center().gap(sp(S2));
    if open && !draft {
        let way_word = move || match way.get() {
            "squash" => t("Squash and merge"),
            "rebase" => t("Rebase and merge"),
            _ => t("Merge pull request"),
        };
        let pick = move |w: &'static str, label: &'static str, detail: &'static str| {
            menu_row(label, None, Some(detail.into()), way.get_untracked() == w, move || way.set(w))
        };
        row = row.child(
            view()
                .flex_row()
                .h(px(28.0))
                .rounded(px(R_MD))
                .overflow_hidden()
                .bg(FG())
                .child(
                    nsh_base::button()
                        .on_press(move || confirm.set(Some(way.get_untracked())))
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .px(sp(S3))
                        .hover(|s| s.opacity(0.85))
                        .child(icon(Icon::GitMerge, 14.0).text_color(SURFACE0()))
                        .child(text(way_word).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(SURFACE0())),
                )
                .child(dynamic(move || {
                    way.get();
                    Some(untracked(move || {
                        dropdown(
                            view()
                                .w(px(28.0))
                                .h(px(28.0))
                                .items_center()
                                .justify_center()
                                .border_l(px(1.0))
                                .border_color(tint(SURFACE0(), 0.25))
                                .label(t("Merge options"))
                                .child(icon(Icon::ChevronDown, 12.0).text_color(SURFACE0())),
                        )
                        .child(pick("merge", t("Create a merge commit"), t("All commits from this branch will be added to the base branch via a merge commit.")))
                        .child(pick("squash", t("Squash and merge"), t("The commits from this branch will be combined into one commit in the base branch.")))
                        .child(pick("rebase", t("Rebase and merge"), t("The commits from this branch will be rebased and added to the base branch.")))
                        .into_element()
                    }))
                })),
        );
    }
    if open {
        row = if draft {
            row.child(outline(t("Ready for review"), Icon::GitPullRequest, false, ask("ready")))
        } else {
            row.child(outline(t("Convert to draft"), Icon::GitPullRequestDraft, false, ask("draft")))
        };
        row = row.child(outline(t("Close pull request"), Icon::GitPullRequestClosed, true, ask("close")));
    } else if !merged {
        row = row.child(outline(t("Reopen pull request"), Icon::GitPullRequest, false, ask("reopen")));
    }
    let (ask_repo, ask_url, ask_title) = (repo.clone(), url.clone(), d.title.clone());
    row = row
        .child(outline(t("Ask"), Icon::MessageSquare, false, move || {
            let prompt = tf("About pull request {} ({}): ", &[&ask_url, &ask_title]);
            store::fix_with_agent(prompt, None, local_root(&ask_repo).as_deref());
        }))
        .child(
            nsh_base::button()
                .on_press(move || {
                    let _ = open_url(url.clone());
                })
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .h(px(28.0))
                .px(sp(S2))
                .rounded(px(R_MD))
                .hover(|s| s.bg(highlight()))
                .child(icon(Icon::ExternalLink, 14.0).text_color(FG_MUTED()))
                .child(text(t("Review on GitHub")).text_size(tx(TEXT_SM)).text_color(FG_MUTED())),
        )
        .child(when(move || busy.get(), || spinner(16.0)));
    let conflicting = open && d.mergeable.eq_ignore_ascii_case("CONFLICTING");
    view()
        .gap(sp(S2))
        .child(row)
        .when(conflicting, |v| v.child(text(t("This branch has conflicts: resolve them before merging")).text_size(tx(TEXT_SM)).text_color(WARNING)))
        // The question an action asks before it is done.
        .child(dynamic(move || {
            let action = confirm.get()?;
            let (title, detail, word) = action_copy(action, &base, &head_ref);
            let repo = repo.clone();
            Some(untracked(move || {
                let tone = match action {
                    "close" => DANGER,
                    "merge" | "squash" | "rebase" => SUCCESS,
                    _ => FG(),
                };
                view()
                    .max_w(px(380.0))
                    .p(sp(S3))
                    .gap(px(4.0))
                    .rounded(px(R_LG))
                    .border_1()
                    .border_color(BORDER_STRONG())
                    .bg(POPOVER())
                    .child(text(title).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)))
                    .child(text(detail).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
                    .child(
                        view()
                            .flex_row()
                            .justify_end()
                            .gap(sp(S2))
                            .pt(sp(S2))
                            .child(btn(t("Cancel"), Variant::Ghost, Sz::Sm, None, move || confirm.set(None)))
                            .child(
                                nsh_base::button()
                                    .on_press(move || {
                                        confirm.set(None);
                                        store::pull_request_act(&repo, number, action);
                                    })
                                    .h(px(28.0))
                                    .px(sp(S3))
                                    .rounded(px(R_MD))
                                    .items_center()
                                    .justify_center()
                                    .bg(tint(tone, 0.18))
                                    .hover(move |s| s.bg(tint(tone, 0.28)))
                                    .child(text(word).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(tone)),
                            ),
                    )
                    .into_element()
            }))
        }))
}

/// One of the head's tabs, underlined while it is picked.
fn tab_button(label: impl IntoReactive<SharedString>, on: impl Fn() -> bool + Copy + 'static, lead: Option<View>, on_press: impl Fn() + 'static) -> impl IntoElement {
    nsh_base::button()
        .on_press(on_press)
        .h(px(36.0))
        .justify_end()
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .flex_1()
                .child(text(label).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(move || if on() { FG() } else { FG_MUTED() }))
                .when_some(lead, |v, l| v.child(l)),
        )
        .child(view().h(px(2.0)).rounded(px(1.0)).bg(move || if on() { FG() } else { tint(FG(), 0.0) }))
}

fn tabs(tab: Signal<PrTab>, checks: Memo<Option<Vec<Check>>>) -> View {
    let checks_mark = move || {
        checks.with(|c| match c {
            None => None,
            Some(c) if c.is_empty() => None,
            Some(c) if c.iter().any(|c| c.state == CheckState::Failed) => Some((Icon::CircleX, DANGER)),
            Some(c) if c.iter().any(|c| c.state == CheckState::Running) => Some((Icon::LoaderCircle, WARNING)),
            Some(_) => Some((Icon::CircleCheck, SUCCESS)),
        })
    };
    view()
        .flex_row()
        .gap(px(16.0))
        .child(tab_button(t("Summary"), move || tab.get() == PrTab::Summary, None, move || tab.set(PrTab::Summary)))
        .child(tab_button(t("Code"), move || tab.get() == PrTab::Code, None, move || tab.set(PrTab::Code)))
        .child(tab_button(
            t("Checks"),
            move || tab.get() == PrTab::Checks,
            Some(view().child(dynamic(move || {
                let (glyph, tone) = checks_mark()?;
                Some(untracked(move || icon(glyph, 12.0).text_color(tone)))
            }))),
            move || tab.set(PrTab::Checks),
        ))
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
    let mut labels = view().flex_row().flex_wrap().gap(px(4.0));
    for l in &d.labels {
        labels = labels.child(label_chip(l.clone()));
    }
    let mut thread = view().gap(sp(S4));
    for c in d.comments.clone() {
        thread = thread.child(comment_view(c));
    }
    view().flex_1().min_h(px(0.0)).child(scroll().flex_1().child(
        view()
            .w_full()
            .max_w(px(1024.0))
            .self_center()
            .px(if MOBILE { sp(S1) } else { px(32.0) })
            .py(px(20.0))
            .gap(px(20.0))
            .when(!d.labels.is_empty(), |v| v.child(labels))
            .child(if body.is_empty() {
                text(t("No description")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).into_element()
            } else {
                text_view()
                    .links(nsh_base::TextViewLinks::Open)
                    .markdown(SharedString::from(body))
                    .selectable(true)
                    .code_block_actions(vec![nsh_base::TextViewCodeAction::new("copy").with_label(t("Copy"))])
                    .gap_3()
                    .text_size(tx(TEXT_CONTENT))
                    .line_height(px(24.0))
                    .text_color(FG())
                    .into_element()
            })
            .when(!d.comments.is_empty(), |v| {
                v.child(
                    view()
                        .gap(sp(S3))
                        .pt(sp(S2))
                        .border_t(px(1.0))
                        .border_color(BORDER())
                        .child(text(tf("{} comments", &[&d.comments.len().to_string()])).text_size(tx(TEXT_XS)).font_weight(FontWeight(500)).text_color(FG_MUTED()).pt(sp(S2)))
                        .child(thread),
                )
            })
            .child(comment_form(d.url.clone(), d.number))
            .pb(sp(S8)),
    ))
}

/// A comment or a review of the conversation.
fn comment_view(c: crate::github::Comment) -> impl IntoElement {
    let review = match c.review.as_str() {
        "APPROVED" => Some((t("approved"), SUCCESS)),
        "CHANGES_REQUESTED" => Some((t("requested changes"), DANGER)),
        "COMMENTED" => Some((t("reviewed"), FG_MUTED())),
        _ => None,
    };
    view()
        .gap(px(6.0))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(person(&c.author, 20.0))
                .when_some(review, |v, (w, tone)| v.child(text(w).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(tone)))
                .child(text(crate::ui::mobile::time::ago_long(&c.at)).text_size(tx(TEXT_XS)).text_color(FG_FAINT())),
        )
        .when(!c.body.trim().is_empty(), |v| {
            v.child(
                view().ml(px(28.0)).child(
                    text_view()
                        .links(nsh_base::TextViewLinks::Open)
                        .markdown(SharedString::from(c.body.trim().to_owned()))
                        .selectable(true)
                        .gap_2()
                        .text_size(tx(TEXT_SM))
                        .line_height(px(21.0))
                        .text_color(FG()),
                ),
            )
        })
}

/// A comment to add, sent as the signed-in user.
fn comment_form(url: String, number: u64) -> impl IntoElement {
    let words = signal(String::new());
    let repo = crate::github::of_url(&url).map(|(r, _)| r).unwrap_or_default();
    let send = move || {
        let body = words.get_untracked();
        if body.trim().is_empty() || repo.is_empty() {
            return;
        }
        store::comment_on_pull_request(&repo, number, &body);
        words.set(String::new());
    };
    let send2 = send.clone();
    view()
        .gap(sp(S2))
        .p(sp(S3))
        .rounded(px(R_LG))
        .border_1()
        .border_color(BORDER())
        .child(
            nsh_base::input()
                .multiline(true)
                .min_h(px(56.0))
                .text_size(tx(TEXT_SM))
                .text_color(FG())
                .caret_color(|| color(ACCENT()))
                .placeholder(t("Leave a comment"))
                .value(move || words.get())
                .on_change(move |v: &str| words.set(v.to_owned()))
                .on_submit(move |_: &str| send2()),
        )
        .child(view().flex_row().justify_end().child(btn(t("Comment"), Variant::Secondary, Sz::Sm, Some(Icon::MessageSquare), move || send())))
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
                    .px(if MOBILE { px(0.0) } else { px(32.0) })
                    .pt(px(16.0))
                    .pb(px(16.0))
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
    view().flex_1().min_h(px(0.0)).child(scroll().flex_1().child(view().px(if MOBILE { sp(S1) } else { px(32.0) }).pt(px(20.0)).pb(sp(S8)).gap(sp(S3)).child(dynamic(move || {
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
        CheckState::Skipped => (Icon::Minus, FG_FAINT(), t("Skipped")),
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
        .when(divider, |v| v.border_t(px(1.0)).border_color(BORDER()))
        .hover(|s| s.bg(highlight()))
        .child(icon(glyph, 15.0).text_color(tone))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .child(text(c.name.clone()).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).number_of_lines(1))
                .when(!c.workflow.is_empty(), |v| v.child(text(c.workflow.clone()).text_size(tx(TEXT_XS)).text_color(FG_MUTED()))),
        )
        .child(text(word).text_size(tx(TEXT_XS)).text_color(tone))
        .when(!c.url.is_empty(), |v| v.child(icon(Icon::ExternalLink, 12.0).text_color(FG_FAINT())))
}

#[cfg(test)]
mod tests {
    use super::by_repository;
    use crate::github::{Inbox, InboxItem};

    fn item(repo: &str, number: u64, updated: &str, review: bool) -> InboxItem {
        InboxItem { repo: repo.into(), number, title: String::new(), url: String::new(), draft: false, author: String::new(), updated_at: updated.into(), created_at: String::new(), state: "open".into(), labels: vec![], review_requested: review }
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
