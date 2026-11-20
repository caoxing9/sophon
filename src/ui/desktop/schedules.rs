//! Automations: what Sophon does on its own. Each is what starts it (a
//! time, or something an integration watches for) and what it does
//! then (a conversation with its instructions, or a command). One made
//! from a plugin keeps the trigger the plugin names, and does what the
//! plugin's automation does: how it answers, and the choices it offers.
//!
//! The page is a list beside what is open: an automation to read and
//! change, with its runs under it; or, to make one, a few to begin
//! from (the ones the installed plugins bring among them) and then the
//! same editor, empty or filled in.

use crate::cadence::{self, Picked};
use crate::i18n::{is_zh, t, tf};
use crate::ui::kit::text;
use crate::ui::shared::settings::{row, section};
use nsh_core::{element::FontWeight, prelude::*};

use crate::ui::kit::*;
use crate::{
    daemon::connection::{Plugin, PluginKind, Schedule, ScheduleRun},
    store,
};

/// What starts an automation's runs: the time alone, or something an
/// integration watches for (by its name: nobody reads the script behind it).
#[derive(Clone, PartialEq, Default)]
enum Starts {
    #[default]
    Time,
    Event {
        integration: String,
        trigger: String,
    },
}

/// How an automation on a time repeats, as the editor asks it.
#[derive(Clone, Copy, PartialEq)]
enum Repeat {
    Every,
    Daily,
    Weekly,
    Custom,
}

/// The kinds the ones to begin from are sorted by.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    All,
    Review,
    Upkeep,
    Plugins,
}

/// An automation to begin from: filled in, and all of it to change.
#[derive(Clone, PartialEq, Default)]
struct Template {
    title: String,
    about: String,
    prompt: String,
    starts: Starts,
    /// A cron expression or `every:<ms>`; empty leaves the editor's own.
    cadence: String,
    /// What starts it, in words, under its card.
    when: String,
    review: bool,
    /// The plugin's automation it is, for one a plugin brings: the
    /// plugin and which of its automations.
    made_of: Option<(String, String)>,
    /// What it says already for what its trigger and its plugin's
    /// automation ask, as the editor keeps each (`trigger/<name>`,
    /// `automation/<name>`): one that came set up, in a file.
    said: Vec<(String, String)>,
    /// And how many runs a day it was held to, if it was.
    limit: Option<u32>,
}

/// What the right side shows.
#[derive(Clone, PartialEq)]
enum Pane {
    /// The ones to begin from.
    Gallery,
    New(Template),
    Edit(String),
    /// One was just made: it is opened once the list has it, by not
    /// being among those known.
    Newest(Vec<String>),
}

/// The ones that come with Sophon, and each automation an installed
/// plugin brings.
fn templates(plugins: &[Plugin]) -> Vec<Template> {
    let zh = is_zh();
    let pick = |en: &str, zh_words: &str| if zh { zh_words.to_owned() } else { en.to_owned() };
    let timed = |title: (&str, &str), about: (&str, &str), prompt: (&str, &str), cron: &str, review: bool| Template {
        title: pick(title.0, title.1),
        about: pick(about.0, about.1),
        prompt: pick(prompt.0, prompt.1),
        starts: Starts::Time,
        cadence: cron.to_owned(),
        when: cadence::describe(cron),
        review,
        made_of: None,
        said: Vec::new(),
        limit: None,
    };
    let mut list = Vec::new();
    for plugin in plugins.iter().filter(|p| p.kind == PluginKind::Plugin && p.error.is_none() && p.enabled) {
        // What it counts on, said under each of its cards.
        let needs: Vec<&str> = plugin.requires.iter().map(|need| need.name.as_str()).collect();
        for automation in &plugin.automations {
            // The trigger that starts it is an integration's: its name
            // and how often it looks are read from there.
            let trigger = plugins.iter().find(|p| p.kind == PluginKind::Integration && p.id == automation.integration).and_then(|p| p.triggers.iter().find(|trigger| trigger.id == automation.trigger));
            let starts_on = match (trigger, automation.integration.is_empty()) {
                (Some(trigger), _) => trigger.label.clone(),
                (None, true) => t("On time").to_owned(),
                (None, false) => automation.integration.clone(),
            };
            list.push(Template {
                title: automation.name.clone(),
                about: automation.description.clone(),
                prompt: automation.prompt.clone(),
                starts: if automation.integration.is_empty() { Starts::Time } else { Starts::Event { integration: automation.integration.clone(), trigger: automation.trigger.clone() } },
                cadence: trigger.filter(|trigger| trigger.every_ms >= 60_000).map(|trigger| format!("every:{}", trigger.every_ms)).unwrap_or_default(),
                when: if needs.is_empty() { starts_on } else { format!("{starts_on} · {}", tf("Needs {}", &[&needs.join(if is_zh() { "、" } else { ", " })])) },
                review: automation.category == "review",
                made_of: Some((plugin.id.clone(), automation.id.clone())),
                said: Vec::new(),
                limit: None,
            });
        }
    }
    list.extend([
        timed(
            ("Find bugs in recent commits", "找出最近提交里的 bug"),
            ("Reads what was committed since the last working day and reports what would break", "读上一个工作日以来的提交，报告会出问题的地方"),
            (
                "Read the commits on the default branch since the last working day (`git log --since`), and look for bugs that would break things: wrong logic, data loss, security holes, missed edge cases. For each, say the file and line, why it is wrong, and how to fix it. Do not change any files. If nothing is wrong, say so in one line.",
                "读默认分支上自上一个工作日以来的提交（`git log --since`），找会出问题的 bug：逻辑错误、数据丢失、安全漏洞、漏掉的边界情况。每个问题写清文件和行号、为什么错、怎么改。不要修改任何文件。没有问题就用一句话说明。",
            ),
            "0 9 * * 1-5",
            true,
        ),
        timed(
            ("Add tests for risky changes", "给有风险的改动补测试"),
            ("Finds recent logic without tests and writes them on a branch of its own", "找出最近没有测试覆盖的逻辑，在单独的分支上补上"),
            (
                "Look at the changes of the last few days and find logic that matters and has no test. Write focused tests for the riskiest of it, run them, and leave them on a new branch with a summary of what each covers. Do not change the code under test.",
                "看最近几天的改动，找出重要但没有测试的逻辑。给风险最高的部分写针对性的测试并运行，放在一个新分支上，附上每个测试覆盖了什么。不要改被测试的代码。",
            ),
            "0 11 * * 1-5",
            true,
        ),
        timed(
            ("Audit dependencies", "依赖审计"),
            ("Checks for known vulnerabilities and long-outdated packages once a week", "每周检查已知漏洞和严重过期的依赖"),
            (
                "Audit this project's dependencies with the package manager's own audit command. List the known vulnerabilities by severity with the version that fixes each, and the packages more than a major version behind. Do not upgrade anything: only report.",
                "用包管理器自带的审计命令检查这个项目的依赖。按严重程度列出已知漏洞和修复它的版本，以及落后超过一个大版本的包。不要升级任何东西，只报告。",
            ),
            "30 9 * * 1",
            false,
        ),
        timed(
            ("Weekly changelog", "每周更新日志"),
            ("Writes up what changed this week, in words a user would read", "把这一周的改动写成用户看得懂的更新说明"),
            (
                "Read this week's merged changes (`git log --since='7 days ago'`) and write a changelog a user would read: what is new, what was fixed, what changed in behavior. Leave out refactors and chores. Do not change any files: answer with the text.",
                "读这一周合并的改动（`git log --since='7 days ago'`），写一份用户看得懂的更新说明：新增了什么、修了什么、行为有什么变化。重构和杂项不写。不要修改任何文件，直接回复文字。",
            ),
            "0 16 * * 5",
            false,
        ),
        timed(
            ("Repository health check", "仓库体检"),
            ("Stale branches, failing checks and what has sat unreviewed", "过期分支、失败的检查、长期没人看的 PR"),
            (
                "Check this repository's health: branches merged or untouched for a month, open pull requests waiting more than a week, and whether the default branch's latest checks pass (`gh`). Report each as a short list. Do not delete or change anything.",
                "检查这个仓库的状况：已合并或一个月没动的分支、等了超过一周的 PR、默认分支最近的检查是否通过（用 `gh`）。每项列一个简短清单。不要删除或修改任何东西。",
            ),
            "0 9 * * 1",
            false,
        ),
    ]);
    list
}

pub(crate) fn schedules(compact: bool) -> impl IntoElement {
    let pane = signal(Pane::Gallery);
    let filter = signal(String::new());
    // A narrow window shows the list, or what is open: not both.
    let opened = signal(false);
    // The integrations name the triggers: asked for when the page opens,
    // and again once the host is back (a list asked of a host not there
    // yet never came).
    effect(|| {
        if matches!(store::conn(), store::Conn::Connected { .. }) {
            store::load_plugins();
        }
    });
    // One that came with a file just added: opened as it was set up, for
    // its project to be picked.
    effect(move || {
        if let Some(handed) = store::handed_automation() {
            store::take_handed_automation();
            let key = |under: &str, said: &[(String, String)]| said.iter().map(|(name, value)| (format!("{under}/{name}"), value.clone())).collect::<Vec<_>>();
            pane.set(Pane::New(Template {
                title: handed.name.clone(),
                about: String::new(),
                prompt: handed.prompt.clone(),
                starts: if handed.integration.is_empty() { Starts::Time } else { Starts::Event { integration: handed.integration.clone(), trigger: handed.trigger.clone() } },
                cadence: handed.expression.clone(),
                when: String::new(),
                review: false,
                made_of: (!handed.plugin.is_empty()).then(|| (handed.plugin.clone(), handed.template.clone())),
                said: [key("trigger", &handed.trigger_options), key("automation", &handed.options)].concat(),
                limit: handed.daily_limit,
            }));
            opened.set(true);
        }
    });
    // The one just made, opened once the list has it.
    effect(move || {
        if let Pane::Newest(known) = pane.get()
            && let Some(list) = store::schedules()
            && let Some(made) = list.iter().find(|s| !known.contains(&s.id))
        {
            pane.set(Pane::Edit(made.id.clone()));
        }
    });
    let detail = move || {
        view().flex_1().min_w(px(0.0)).child(dynamic(move || {
            let now = pane.get();
            Some(untracked(move || match now {
                Pane::Gallery => gallery(pane, opened, compact).into_element(),
                Pane::Newest(_) => view().flex_1().items_center().justify_center().child(loading()).into_element(),
                Pane::New(template) => editor(None, template, pane, opened, compact).into_element(),
                Pane::Edit(id) => match store::schedules().and_then(|list| list.into_iter().find(|s| s.id == id)) {
                    Some(existing) => editor(Some(existing), Template::default(), pane, opened, compact).into_element(),
                    None => gallery(pane, opened, compact).into_element(),
                },
            }))
        }))
    };
    if compact {
        view().flex_1().child(when(move || !opened.get(), move || list_column(pane, filter, opened, true))).child(when(move || opened.get(), detail))
    } else {
        view().flex_1().flex_row().child(list_column(pane, filter, opened, false)).child(detail())
    }
}

/// The automations there are, to pick one; and a + for another.
fn list_column(pane: Signal<Pane>, filter: Signal<String>, opened: Signal<bool>, compact: bool) -> View {
    view()
        .when(compact, |v| v.flex_1())
        .when(!compact, |v| v.w(px(300.0)).flex_shrink_0().border_r_1().border_color(BORDER))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .px(sp(S4))
                .pt(sp(S6))
                .pb(sp(S3))
                .when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home)))
                .child(text(t("Automations")).text_size(tx(TEXT_LG)).font_weight(FontWeight(500)).flex_1())
                .child(icon_btn(Icon::Plus, Sz::Sm, t("New automation"), move || {
                    pane.set(Pane::Gallery);
                    opened.set(true);
                })),
        )
        .child(
            view().px(sp(S3)).pb(sp(S2)).child(
                field()
                    .h(sp(CONTROL_SM + 4.0))
                    .text_size(tx(TEXT_SM))
                    .value(move || filter.get())
                    .on_change(move |v: &str| filter.set(v.to_owned()))
                    .placeholder(t("Filter automations")),
            ),
        )
        .child(scroll().flex_1().child(dynamic(move || {
            let items = store::schedules();
            // Read here, so the rows are drawn again once the plugins are
            // known: a row names its trigger in the plugin's words.
            let plugins = store::plugins();
            let needle = filter.get().trim().to_lowercase();
            Some(untracked(move || {
                let mut list = view().px(sp(S2)).pb(sp(S4)).gap(px(2.0));
                let Some(items) = items else {
                    return list.child(loading());
                };
                let shown: Vec<_> = items.into_iter().filter(|s| needle.is_empty() || s.name.to_lowercase().contains(&needle) || s.prompt.to_lowercase().contains(&needle)).collect();
                if shown.is_empty() {
                    return list.child(text(t("No automations yet")).text_size(tx(TEXT_SM)).text_color(FG_MUTED).px(sp(S2)).py(sp(S4)));
                }
                for s in shown {
                    list = list.child(list_row(s, &plugins, pane, opened));
                }
                list
            }))
        })))
}

/// What starts `s`, in a few words.
fn starts_words(s: &Schedule, plugins: &[Plugin]) -> String {
    if !s.integration.is_empty() {
        let label = plugins
            .iter()
            .find(|p| p.id == s.integration)
            .and_then(|p| p.triggers.iter().find(|trigger| s.trigger.is_empty() || trigger.id == s.trigger))
            .map(|trigger| trigger.label.clone());
        label.unwrap_or_else(|| s.integration.clone())
    } else if !s.probe.is_empty() {
        t("When a script finds something").to_owned()
    } else if s.timed {
        s.cadence.clone()
    } else {
        t("When told").to_owned()
    }
}

/// The project a folder is, by its name; else the folder's own.
fn project_name(path: &str) -> String {
    store::projects().into_iter().find(|p| p.root_path == path).map(|p| p.name).unwrap_or_else(|| crate::daemon::model::folder_name(path))
}

/// What a conversation of `s` runs on: its model, else its provider by
/// name; in full, the provider, the model and the access mode.
fn runs_on_words(s: &Schedule, full: bool) -> String {
    let mut provider = s.provider.clone();
    if let Some(first) = provider.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    let model = crate::ui::shared::composer::model_name(&s.model);
    if !full {
        return if model.is_empty() { provider } else { model };
    }
    [provider, model, s.mode.clone()].into_iter().filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" · ")
}

/// One of the list: what starts it, its name, and where it works and on
/// what. Its switch is its own, beside the row and not a part of it.
fn list_row(s: Schedule, plugins: &[Plugin], pane: Signal<Pane>, opened: Signal<bool>) -> impl IntoElement {
    let (picked, shown, flipped) = (s.id.clone(), s.id.clone(), s.id.clone());
    let event = !s.integration.is_empty() || !s.probe.is_empty() || !s.timed;
    let active = s.status == "active";
    let place = if s.cwd.is_empty() { String::new() } else { project_name(&s.cwd) };
    let with = if !s.command.is_empty() { t("Command").to_owned() } else { runs_on_words(&s, false) };
    let muted = |words: String| text(words).text_size(tx(TEXT_XS)).text_color(FG_MUTED).number_of_lines(1);
    view()
        .rounded(px(R_LG))
        .bg(move || if pane.with(|now| *now == Pane::Edit(shown.clone())) { highlight() } else { tint(FG, 0.0) })
        .hover(|style| style.bg(highlight()))
        .child(
            nsh_base::button()
                .on_press(move || {
                    pane.set(Pane::Edit(picked.clone()));
                    opened.set(true);
                })
                .accessibility_label(s.name.clone())
                .px(sp(S3))
                .py(sp(S3))
                .gap(px(5.0))
                .child(
                    // Clear of the switch, which lies over the row's corner.
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .pr(px(44.0))
                        .child(icon(if s.probe_error.is_some() { Icon::TriangleAlert } else if event { Icon::Zap } else { Icon::CalendarClock }, 13.0).text_color(if s.probe_error.is_some() { DANGER } else { FG_MUTED }))
                        .child(muted(starts_words(&s, plugins)).flex_1().min_w(px(0.0))),
                )
                .child(text(s.name.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(600)).number_of_lines(1))
                .child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .when(!place.is_empty(), |v| v.child(icon(Icon::Folder, 12.0).text_color(FG_FAINT)))
                        .child(muted(place).flex_1().min_w(px(0.0)))
                        .child(muted(with).flex_shrink_0()),
                ),
        )
        .child(
            view().absolute().top(sp(S3)).right(sp(S3)).child(
                switch(move || active, move |on| store::schedule_op(&flipped, if on { "resume" } else { "pause" })).accessibility_label(tf("Turn “{}” on or off", &[&s.name])),
            ),
        )
}

/// A few automations to begin from, or an empty one.
fn gallery(pane: Signal<Pane>, opened: Signal<bool>, compact: bool) -> impl IntoElement {
    let kind = signal(Kind::All);
    scroll().flex_1().child(
        view()
            .w_full()
            .max_w(px(860.0))
            .self_center()
            .px(sp(S6))
            .py(sp(S8))
            .gap(sp(S4))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), move || opened.set(false))))
                    .child(text(t("New automation")).text_size(tx(TEXT_2XL)).font_weight(FontWeight(500))),
            )
            .child(text(t("Begin from one of these, or from nothing. All of it can be changed")).text_size(tx(TEXT_BASE)).text_color(FG_MUTED))
            .child(view().flex_row().child(segmented(
                vec![(Kind::All, t("All").into()), (Kind::Review, t("Code review").into()), (Kind::Upkeep, t("Upkeep").into()), (Kind::Plugins, t("From plugins").into())],
                move || kind.get(),
                move |next| kind.set(next),
            )))
            .child(dynamic(move || {
                let (which, plugins) = (kind.get(), store::plugins());
                Some(untracked(move || {
                    let mut grid = view().flex_row().flex_wrap().gap(sp(S4));
                    if which == Kind::All {
                        grid = grid.child(template_card(
                            Icon::Plus,
                            t("Start from scratch").to_owned(),
                            t("Write your own instructions and pick what starts it").to_owned(),
                            String::new(),
                            move || {
                                pane.set(Pane::New(Template::default()));
                                opened.set(true);
                            },
                        ));
                    }
                    let fitting = templates(&plugins).into_iter().filter(|template| match which {
                        Kind::All => true,
                        Kind::Review => template.review,
                        Kind::Upkeep => !template.review,
                        Kind::Plugins => template.made_of.is_some(),
                    });
                    let mut any = which == Kind::All;
                    for template in fitting {
                        any = true;
                        let chosen = template.clone();
                        grid = grid.child(template_card(
                            if template.made_of.is_some() { Icon::Zap } else if template.review { Icon::GitPullRequest } else { Icon::Wrench },
                            template.title.clone(),
                            template.about.clone(),
                            template.when.clone(),
                            move || {
                                pane.set(Pane::New(chosen.clone()));
                                opened.set(true);
                            },
                        ));
                    }
                    if !any {
                        // Only the plugins' can be none: there are none installed.
                        return view().py(sp(S6)).gap(sp(S3)).items_start().child(text(t("No plugin brings one yet")).text_size(tx(TEXT_BASE)).text_color(FG_MUTED)).child(btn(
                            t("Open Plugins"),
                            Variant::Outline,
                            Sz::Sm,
                            Some(Icon::Layers),
                            store::open_extensions,
                        ));
                    }
                    // Where the plugins that bring these are kept, a press
                    // from the ones they bring.
                    if which == Kind::Plugins {
                        return view().gap(sp(S4)).child(grid).child(view().flex_row().child(btn(t("Open Plugins"), Variant::Ghost, Sz::Sm, Some(Icon::Layers), store::open_extensions)));
                    }
                    grid
                }))
            })),
    )
}

fn template_card(glyph: Icon, title: String, about: String, when: String, on_press: impl Fn() + 'static) -> impl IntoElement {
    let timed = !when.is_empty();
    // By the time, or by something that happens.
    let mark = if matches!(glyph, Icon::Zap) { Icon::Zap } else { Icon::CalendarClock };
    nsh_base::button()
        .on_press(on_press)
        .accessibility_label(title.clone())
        .flex_1()
        .min_w(px(280.0))
        .max_w(px(420.0))
        .min_h(px(116.0))
        .p(sp(S4))
        .gap(sp(S3))
        .rounded(px(R_XL))
        .border_1()
        .border_color(BORDER)
        .bg(SURFACE0)
        .hover(|style| style.bg(highlight()))
        .child(
            view()
                .flex_row()
                .gap(sp(S3))
                .child(glyph_tile(glyph))
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap(px(4.0))
                        .child(text(title).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)))
                        .child(text(about).text_size(tx(TEXT_SM)).text_color(FG_MUTED).number_of_lines(3)),
                ),
        )
        .child(view().flex_1())
        .when(timed, |card| card.child(view().flex_row().items_center().gap(px(6.0)).child(icon(mark, 12.0).text_color(FG_FAINT)).child(text(when).text_size(tx(TEXT_XS)).text_color(FG_MUTED).number_of_lines(1))))
}

/// An automation to read and change, or a new one to fill in: what
/// starts it, what it does, where, and (of one there is) its runs.
fn editor(existing: Option<Schedule>, template: Template, pane: Signal<Pane>, opened: Signal<bool>, compact: bool) -> impl IntoElement {
    let editing = existing.as_ref().map(|s| s.id.clone());
    let is_edit = editing.is_some();
    store::reset_trial();
    // What this one says for what its trigger and its plugin's automation
    // ask: the last automation's are not this one's.
    store::reset_option_drafts(match &existing {
        Some(s) => {
            let kept = |under: &'static str, said: &[(String, String)]| said.iter().map(move |(name, value)| (format!("{under}/{name}"), value.clone())).collect::<Vec<_>>();
            [kept("trigger", &s.trigger_options), kept("automation", &s.options)].concat()
        }
        // A new one says what its template does: nothing, unless it came set up.
        None => template.said.clone(),
    });
    // The plugin's automation it was made from, if it was: that stays
    // what it is for as long as the automation is.
    let made_of: Option<(String, String)> = match &existing {
        Some(s) => (!s.plugin.is_empty()).then(|| (s.plugin.clone(), s.template.clone())),
        None => template.made_of.clone(),
    };
    // One that messages an agent it already has: no folder to pick.
    let no_folder = existing.as_ref().is_some_and(|s| s.cwd.is_empty());
    let expression_was = existing.as_ref().map(|s| s.expression.clone()).filter(|e| s_timed(&existing) && !e.is_empty()).or_else(|| (!template.cadence.is_empty()).then(|| template.cadence.clone()));
    let was = expression_was.as_deref().map(cadence::picked);
    let first_project = || store::projects().into_iter().map(|p| p.root_path).find(|p| !p.is_empty()).unwrap_or_default();
    let name = signal(existing.as_ref().map(|s| s.given_name.clone()).unwrap_or_else(|| template.title.clone()));
    let prompt = signal(existing.as_ref().map(|s| s.prompt.clone()).unwrap_or_else(|| template.prompt.clone()));
    let path = signal(existing.as_ref().map(|s| s.cwd.clone()).filter(|c| !c.is_empty()).unwrap_or_else(first_project));
    let worktree = signal(existing.as_ref().is_some_and(|s| s.worktree));
    let starts = signal(match &existing {
        Some(s) if !s.integration.is_empty() => Starts::Event { integration: s.integration.clone(), trigger: s.trigger.clone() },
        Some(_) => Starts::Time,
        None => template.starts.clone(),
    });
    // Scripts of its own are not the editor's to write; one made elsewhere
    // (the service takes them) keeps what it has through an edit here.
    let probe = signal(existing.as_ref().map(|s| s.probe.clone()).unwrap_or_default());
    let reply = signal(existing.as_ref().map(|s| s.reply.clone()).unwrap_or_default());
    // Started by something that happens, not by the time alone.
    let probing = computed(move || starts.with(|by| *by != Starts::Time) || probe.with(|p| !p.is_empty()));
    // What a run does: a conversation with the instructions, or a command
    // of the user's own, which no agent has a part in.
    let commanding = signal(existing.as_ref().is_some_and(|s| !s.command.is_empty()));
    // What one there is runs on, picked again: until then it stays as it
    // was made.
    let repicking = signal(false);
    let made_with = existing.as_ref().map(|s| (s.provider.clone(), s.model.clone(), s.mode.clone()));
    let command = signal(existing.as_ref().map(|s| s.command.clone()).unwrap_or_default());
    let limit = signal(existing.as_ref().and_then(|s| s.daily_limit).or(template.limit).map(|n| n.to_string()).unwrap_or_default());
    let repeat = signal(match &was {
        Some(Picked::Every { .. }) => Repeat::Every,
        Some(Picked::Weekly(..)) => Repeat::Weekly,
        Some(Picked::Custom(_)) => Repeat::Custom,
        Some(Picked::Daily(..)) | None => Repeat::Daily,
    });
    // Every `count` of a unit: 1 a minute, 60 an hour, 1440 a day.
    let (count, unit) = match &was {
        Some(Picked::Every { count, unit }) => (count.to_string(), *unit),
        _ => ("1".to_owned(), 60),
    };
    let (count, unit) = (signal(count), signal(unit));
    let minutes = computed(move || count.with(|c| c.trim().parse::<u32>().ok()).filter(|n| (1..=999).contains(n)).map(|n| n * unit.get()));
    let time = signal(match &was {
        Some(Picked::Daily(h, m) | Picked::Weekly(_, (h, m))) => format!("{h}:{m:02}"),
        _ => "9:00".to_owned(),
    });
    // Monday to Friday; 0 is Sunday.
    let days = signal(match &was {
        Some(Picked::Weekly(days, _)) => days.clone(),
        _ => vec![1u8, 2, 3, 4, 5],
    });
    let custom = signal(match was {
        Some(Picked::Custom(c)) => c,
        _ => "0 9 * * 1-5".to_owned(),
    });
    // The cron expression of what is picked, when it is whole.
    let expression = computed(move || match repeat.get() {
        Repeat::Every => minutes.get().map(cadence::every),
        Repeat::Daily => time.with(|t| cadence::parse_time(t)).map(cadence::daily),
        Repeat::Weekly => time.with(|t| cadence::parse_time(t)).and_then(|t| days.with(|d| cadence::weekly(d, t))),
        Repeat::Custom => custom.with(|c| (c.split_whitespace().count() == 5).then(|| c.trim().to_owned())),
    });
    // Whether there is enough to keep.
    let whole = computed(move || {
        let does = if commanding.get() { command.with(|c| !c.trim().is_empty()) } else { prompt.with(|p| !p.trim().is_empty()) };
        does && (no_folder || path.with(|p| !p.is_empty())) && expression.with(Option::is_some)
    });
    let saved = editing.clone();
    let made = made_of.clone();
    let save = move || {
        if !whole.get_untracked() {
            return;
        }
        let Some(cron) = expression.get_untracked() else { return };
        // The one or the other: a command leaves no instructions behind it.
        let by_command = commanding.get_untracked();
        let by = starts.get_untracked();
        // An integration's trigger: its probe is the integration's own.
        let (integration, trigger) = match &by {
            Starts::Event { integration, trigger } => (integration.clone(), trigger.clone()),
            _ => (String::new(), String::new()),
        };
        let (plugin, template) = made.clone().unwrap_or_default();
        // What it says for what each asks: as it stands in the editor,
        // said or left as the manifest gives it.
        let installed = store::plugins();
        let stands = |params: &[crate::daemon::connection::PluginParam]| -> Vec<(String, String)> {
            params.iter().map(|param| (param.name.clone(), store::option_draft(&param.of.key(&param.name)).unwrap_or_else(|| param.value.clone()))).collect()
        };
        let trigger_options = installed
            .iter()
            .find(|p| p.kind == PluginKind::Integration && p.id == integration)
            .and_then(|p| p.triggers.iter().find(|found| trigger.is_empty() || found.id == trigger))
            .map(|found| stands(&found.params))
            .unwrap_or_default();
        let options = installed
            .iter()
            .find(|p| p.kind == PluginKind::Plugin && p.id == plugin)
            .and_then(|p| p.automations.iter().find(|found| template.is_empty() || found.id == template))
            .map(|found| stands(&found.params))
            .unwrap_or_default();
        let scripted = integration.is_empty() && plugin.is_empty();
        let extras = store::ScheduleExtras {
            options,
            trigger_options,
            cwd: if no_folder { String::new() } else { path.get_untracked() },
            worktree: !by_command && worktree.get_untracked(),
            probe: if scripted { probe.get_untracked() } else { String::new() },
            reply: if scripted { reply.get_untracked() } else { String::new() },
            integration,
            trigger,
            plugin,
            template,
            repicked: repicking.get_untracked(),
            command: if by_command { command.get_untracked() } else { String::new() },
            daily_limit: limit.with_untracked(|l| l.trim().parse::<u32>().ok()).filter(|n| *n > 0),
        };
        let instructions = if by_command { String::new() } else { prompt.get_untracked() };
        match &saved {
            Some(id) => {
                store::update_schedule(id, name.get_untracked(), instructions, cron, extras);
                store::notify(t("Saved").to_owned());
            }
            None => {
                let known = store::schedules().unwrap_or_default().into_iter().map(|s| s.id).collect();
                store::create_schedule(name.get_untracked(), instructions, cron, extras);
                pane.set(Pane::Newest(known));
            }
        }
    };

    // ------------------------------------------------------------ the head
    // Its name as the page's title, and beside it what is done with it.
    let mut head = view().flex_row().items_center().gap(sp(S2)).when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), move || opened.set(false)))).child(
        field()
            .flex_1()
            .h(px(44.0))
            .px(px(0.0))
            .bg(tint(FG, 0.0))
            .border_color(tint(FG, 0.0))
            .text_size(tx(TEXT_2XL))
            .font_weight(FontWeight(600))
            .value(move || name.get())
            .on_change(move |v: &str| name.set(v.to_owned()))
            .placeholder(t("Untitled automation")),
    );
    if let Some(id) = editing.clone() {
        head = head
            .child(btn(t("Run now"), Variant::Outline, Sz::Md, Some(Icon::Play), move || store::schedule_op(&id, "run-once")))
            .child(btn(t("Save"), Variant::Primary, Sz::Md, None, save).disabled(move || !whole.get()));
    } else {
        head = head
            .child(btn(t("Cancel"), Variant::Secondary, Sz::Md, None, move || {
                pane.set(Pane::Gallery);
            }))
            .child(btn(t("Create"), Variant::Primary, Sz::Md, None, save).disabled(move || !whole.get()));
    }
    // Under the name, on one line: whether it is on, where it works, and
    // the rest of what is done with it.
    let bar_rule = || view().w(px(1.0)).h(px(16.0)).bg(BORDER);
    let now = |id: &str| store::schedules().and_then(|list| list.into_iter().find(|s| s.id == id));
    let mut bar = view().flex_row().items_center().gap(sp(S3)).min_h(sp(CONTROL_SM));
    if let Some(id) = editing.clone() {
        let (switched, flipped, said) = (id.clone(), id.clone(), id.clone());
        bar = bar
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .child(switch(move || now(&switched).is_some_and(|s| s.status == "active"), move |on| store::schedule_op(&flipped, if on { "resume" } else { "pause" })).accessibility_label(t("Active")))
                    .child(text(move || -> SharedString { if now(&said).is_some_and(|s| s.status == "active") { t("Active").into() } else { t("Paused").into() } }).text_size(tx(TEXT_SM)).text_color(FG_MUTED)),
            )
            .child(bar_rule());
    }
    if !no_folder {
        bar = bar.child(
            dropdown(picker_trigger(Some(Icon::Folder), move || -> SharedString { path.with(|p| if p.is_empty() { t("Project").to_owned() } else { project_name(p) }).into() }))
                .align(nsh_core::overlay::Align::Start)
                .child(dynamic(move || {
                    let projects: Vec<_> = store::projects().into_iter().filter(|p| !p.root_path.is_empty()).collect();
                    let picked = path.get();
                    Some(untracked(move || {
                        let mut list = view().child(menu_heading(t("Project")));
                        for p in projects {
                            let (on, to) = (p.root_path == picked, p.root_path.clone());
                            list = list.child(menu_row(p.name.clone(), Some(Icon::Folder), None, on, move || path.set(to.clone())));
                        }
                        menu_scroll(320.0, list)
                    }))
                })),
        );
    }
    if let Some(id) = editing.clone() {
        let (said, removed) = (id.clone(), id.clone());
        bar = bar
            .child(bar_rule())
            .child(
                dropdown(view().w(sp(CONTROL_SM)).h(sp(CONTROL_SM)).items_center().justify_center().rounded(px(R_FULL)).hover(|s| s.bg(highlight())).child(icon(Icon::Ellipsis, 16.0).text_color(FG_MUTED)))
                    .align(nsh_core::overlay::Align::Start)
                    .child(menu_row_danger(t("Delete automation"), Icon::Trash2, move || {
                        store::schedule_op(&removed, "delete");
                        pane.set(Pane::Gallery);
                        opened.set(false);
                    })),
            )
            .child(view().flex_1())
            // When it runs next and ran last: read as the list is, so a
            // pause shows without opening it again.
            .child(text(move || -> SharedString {
                match now(&said) {
                    Some(s) if s.status == "active" => format!("{} {} · {} {}", t("next"), when_label(&s.next_run), t("last"), when_label(&s.last_run)).into(),
                    Some(s) => format!("{} {}", t("last"), when_label(&s.last_run)).into(),
                    None => SharedString::default(),
                }
            })
            .text_size(tx(TEXT_SM))
            .text_color(FG_FAINT)
            .number_of_lines(1));
    }
    let failing = editing.clone().map(|id| {
        dynamic(move || {
            let error = now(&id)?.probe_error?;
            Some(untracked(move || text(format!("{}{error}", t("Probe failed: "))).text_size(tx(TEXT_SM)).text_color(DANGER)))
        })
    });
    // One there is has two pages: what it is set to, and what it did.
    let history = signal(false);

    // -------------------------------------------------------- what starts it
    // One card: the trigger picked on its first line (the others a press
    // away), under it what the trigger asks, then how often it looks.
    let choosing = signal(!is_edit && template.title.is_empty());
    // Made from a plugin, what starts it is the plugin's to say.
    let fixed = made_of.is_some();
    let line = || view().px(sp(S4)).py(sp(S3)).border_t_1().border_color(BORDER);
    let triggers = dynamic(move || {
        let (installed, open) = (store::plugins(), choosing.get());
        // Read here when closed: the line shown is the one picked.
        let current = if open { None } else { Some(starts.get()) };
        Some(untracked(move || {
            // What there is to start on: its name, a line about it, and what
            // picking it fills in.
            let mut options: Vec<(Starts, Icon, String, String, std::rc::Rc<dyn Fn()>)> =
                vec![(Starts::Time, Icon::CalendarClock, t("On time").to_owned(), t("At the times you set below").to_owned(), std::rc::Rc::new(|| {}))];
            // Of those turned on: one turned off starts nothing.
            for plugin in installed.iter().filter(|p| p.kind == PluginKind::Integration && p.error.is_none() && p.enabled) {
                for trigger in &plugin.triggers {
                    let every = trigger.every_ms;
                    // Named by its plugin when its own words do not say whose it is.
                    let title = if trigger.label == plugin.name { trigger.label.clone() } else { format!("{} · {}", plugin.name, trigger.label) };
                    options.push((
                        Starts::Event { integration: plugin.id.clone(), trigger: trigger.id.clone() },
                        Icon::Zap,
                        title,
                        if trigger.description.is_empty() { plugin.description.clone() } else { trigger.description.clone() },
                        std::rc::Rc::new(move || {
                            // How often it suggests looking.
                            if every >= 60_000 {
                                repeat.set(Repeat::Every);
                                unit.set(1);
                                count.set((every / 60_000).min(999).to_string());
                            }
                        }),
                    ));
                }
            }
            let words = |title: String, about: String| {
                view()
                    .flex_1()
                    .min_w(px(0.0))
                    .gap(px(2.0))
                    .child(text(title).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)))
                    .when(!about.is_empty(), |v| v.child(text(about).text_size(tx(TEXT_SM)).text_color(FG_MUTED).number_of_lines(2)))
            };
            if let Some(current) = current {
                // One on an integration no longer installed still says so.
                let (glyph, title, about) = options
                    .iter()
                    .find(|option| option.0 == current)
                    .map(|option| (option.1, option.2.clone(), option.3.clone()))
                    .unwrap_or_else(|| match &current {
                        Starts::Event { integration, .. } => (Icon::TriangleAlert, integration.clone(), t("This integration is not installed").to_owned()),
                        _ => (Icon::CalendarClock, t("On time").to_owned(), String::new()),
                    });
                return view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S3))
                    .px(sp(S4))
                    .py(sp(S4))
                    .child(glyph_tile(glyph))
                    .child(words(title, about))
                    .when(!fixed, |v| v.child(btn(t("Change"), Variant::Outline, Sz::Sm, None, move || choosing.set(true))));
            }
            let mut list = view().p(sp(S2)).gap(px(2.0));
            for (this, glyph, title, about, picked) in options {
                let (marked, set) = (this.clone(), this);
                list = list.child(
                    nsh_base::button()
                        .on_press(move || {
                            starts.set(set.clone());
                            picked();
                            choosing.set(false);
                        })
                        .accessibility_label(title.clone())
                        .flex_row()
                        .items_center()
                        .gap(sp(S3))
                        .px(sp(S2))
                        .py(sp(S2))
                        .rounded(px(R_LG))
                        .hover(|style| style.bg(highlight()))
                        .child(glyph_tile(glyph))
                        .child(words(title, about))
                        .child(view().w(px(20.0)).child(dynamic(move || starts.with(|by| *by == marked).then(|| untracked(|| icon(Icon::Check, 16.0).text_color(ACCENT)))))),
                );
            }
            list
        }))
    });
    // What it counts on, when that is not ready yet: the integration
    // behind the trigger picked, then those its plugin names.
    let made = made_of.clone();
    let unready = dynamic(move || {
        let installed = store::plugins();
        let standing = |behind: &Plugin| -> Option<String> {
            let ready = behind.needs().is_empty() && (!behind.setup || behind.account.is_some());
            if !behind.enabled {
                Some(tf("“{}” is turned off under Integrations: this starts nothing until it is on again", &[&behind.name]))
            } else if !ready {
                Some(tf("Set “{}” up under Integrations first, or this never starts", &[&behind.name]))
            } else {
                None
            }
        };
        let mut words = match starts.get() {
            Starts::Event { integration, .. } => installed.iter().find(|p| p.kind == PluginKind::Integration && p.id == integration).and_then(&standing),
            _ => None,
        };
        if words.is_none()
            && let Some(plugin) = made.as_ref().and_then(|(id, _)| installed.iter().find(|p| p.kind == PluginKind::Plugin && &p.id == id))
        {
            words = if !plugin.enabled {
                Some(tf("“{}” is turned off under Plugins: this does nothing until it is on again", &[&plugin.name]))
            } else {
                plugin.requires.iter().find(|need| !need.ready).map(|need| tf("“{}” needs {}: set that up under Integrations first", &[&plugin.name, &need.name]))
            };
        }
        let words = words?;
        Some(untracked(move || {
            line()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .bg(tint(WARNING, 0.08))
                .child(icon(Icon::TriangleAlert, 14.0).text_color(WARNING))
                .child(text(words).text_size(tx(TEXT_SM)).flex_1())
        }))
    });
    // What each automation says for itself: what its trigger asks, then
    // what the plugin's automation it was made from asks.
    let made = made_of.clone();
    let options = dynamic(move || {
        let installed = store::plugins();
        let mut rows = view();
        let mut any = false;
        if let Starts::Event { integration, trigger } = starts.get()
            && let Some(behind) = installed.iter().find(|p| p.kind == PluginKind::Integration && p.id == integration)
            && let Some(found) = behind.triggers.iter().find(|found| trigger.is_empty() || found.id == trigger)
            && !found.params.is_empty()
        {
            let (id, own) = (behind.id.clone(), found.params.clone());
            rows = rows.child(untracked(move || super::plugins::param_rows(&id, &own)));
            any = true;
        }
        if let Some((plugin, template)) = &made
            && let Some(behind) = installed.iter().find(|p| p.kind == PluginKind::Plugin && &p.id == plugin)
            && let Some(found) = behind.automations.iter().find(|found| template.is_empty() || &found.id == template)
            && !found.params.is_empty()
        {
            let (id, own) = (behind.id.clone(), found.params.clone());
            rows = rows.child(untracked(move || super::plugins::param_rows(&id, &own)));
            any = true;
        }
        any.then_some(rows)
    });
    // When: the times it runs at, or how often it looks.
    // A few characters beside a segmented control: as tall and as round as
    // it, so the row reads as one line of controls.
    let short = |width: f32| field().w(px(width)).h(sp(CONTROL_SM + 4.0)).rounded(px(R_FULL)).text_size(tx(TEXT_SM)).text_center();
    let lead = |label: &'static str| text(label).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED);
    // What was picked, said back: the check that it was understood.
    let summary = move || -> SharedString {
        match expression.get() {
            Some(cron) if repeat.get() == Repeat::Custom && cadence::describe(&cron).starts_with("cron ") => t("Runs on this cron expression").into(),
            // Beside "Looks" it needs no word of its own.
            Some(cron) if probing.get() => cadence::describe(&cron).into(),
            Some(cron) => format!("{}{}", t("Runs: "), cadence::describe(&cron)).into(),
            None => match repeat.get() {
                Repeat::Custom => t("Five fields: minute, hour, day, month, weekday").into(),
                Repeat::Every => t("Type a number, like 45").into(),
                Repeat::Weekly if days.with(Vec::is_empty) => t("Pick at least one day").into(),
                _ => t("Type a time, like 9:30 or 18:00").into(),
            },
        }
    };
    let picking = move || {
        let time_row = move || view().flex_row().items_center().gap(sp(S2)).child(lead(t("At"))).child(short(76.0).value(move || time.get()).on_change(move |v: &str| time.set(v.to_owned())).placeholder("9:00"));
        view()
            .gap(sp(S3))
            .child(view().flex_row().child(segmented(
                vec![(Repeat::Every, t("Every so often").into()), (Repeat::Daily, t("Every day").into()), (Repeat::Weekly, t("Every week").into()), (Repeat::Custom, t("Custom").into())],
                move || repeat.get(),
                move |r| repeat.set(r),
            )))
            .child(dynamic(move || {
                let r = repeat.get();
                Some(untracked(move || match r {
                    Repeat::Every => view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(lead(t("Every")))
                        .child(short(56.0).value(move || count.get()).on_change(move |v: &str| count.set(v.to_owned())).placeholder("1"))
                        .child(segmented(vec![(1u32, t("Minutes").into()), (60, t("Hours").into()), (1440, t("Days").into())], move || unit.get(), move |u| unit.set(u))),
                    Repeat::Daily => time_row(),
                    Repeat::Weekly => {
                        let zh = is_zh();
                        let mut week = view().flex_row().flex_wrap().gap(sp(S2));
                        // Monday first.
                        for d in [1u8, 2, 3, 4, 5, 6, 0] {
                            week = week.child(chip(
                                cadence::day_name(d, zh),
                                None,
                                move || days.with(|l| l.contains(&d)),
                                move || {
                                    days.update(|l| match l.iter().position(|x| *x == d) {
                                        Some(i) => {
                                            l.remove(i);
                                        }
                                        None => l.push(d),
                                    })
                                },
                            ));
                        }
                        view().gap(sp(S3)).child(week).child(time_row())
                    }
                    Repeat::Custom => view().child(field().value(move || custom.get()).on_change(move |v: &str| custom.set(v.to_owned())).font_family(mono_font).placeholder(t("Cron expression, e.g. 0 9 * * 1-5"))),
                }))
            }))
    };
    // The times are what a timed one is about, and there to set. How often
    // an event's trigger looks is a detail: one line, opened to change.
    let pacing = signal(false);
    let times = line()
        .gap(sp(S3))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S3))
                .min_h(sp(CONTROL_SM))
                .child(text(move || if probing.get() { t("Looks") } else { t("Repeats") }).text_size(tx(TEXT_BASE)).flex_1())
                .child(text(summary).text_size(tx(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1))
                .child(when(move || probing.get() && !pacing.get(), move || btn(t("Change"), Variant::Outline, Sz::Sm, None, move || pacing.set(true))))
                .child(when(move || probing.get() && pacing.get(), move || btn(t("Done"), Variant::Outline, Sz::Sm, None, move || pacing.set(false)))),
        )
        .child(when(move || !probing.get() || pacing.get(), picking));

    // ------------------------------------------------------------ what it does
    // One card, as a composer is: what a run does at its head, the words
    // for it, and at its foot what it runs on. A new one runs on the
    // composer's own choice, changed here as there; one there is says what
    // it was made with.
    let runs_on = existing.as_ref().map(|s| if s.provider.is_empty() { s.target.clone() } else { runs_on_words(s, true) });
    // A field with no frame of its own: the card around it is its frame.
    // Nothing here is checked for spelling: commands and instructions
    // are full of names no dictionary has.
    let bare = || field().multiline(true).auto_correct(false).bg(tint(FG, 0.0)).border_color(tint(FG, 0.0)).rounded(px(0.0)).px(sp(S4)).py(sp(S3));
    let does = card()
        .child(view().flex_row().px(sp(S4)).py(sp(S3)).child(segmented(vec![(false, t("Start a conversation").into()), (true, t("Run a command").into())], move || commanding.get(), move |on| commanding.set(on))))
        .child(dynamic(move || {
            let on = commanding.get();
            let (runs_on, made_with) = (runs_on.clone(), made_with.clone());
            Some(untracked(move || {
                let body = view().border_t_1().border_color(BORDER);
                let foot = || view().flex_row().items_center().gap(sp(S2)).min_h(sp(CONTROL_MD)).px(sp(S4)).py(sp(S2)).border_t_1().border_color(BORDER);
                let said = |words: String| text(words).text_size(tx(TEXT_SM)).text_color(FG_MUTED);
                if on {
                    // How the last try of it went, under the command: on its
                    // way, or whether it ended well and the end of what it wrote.
                    let tried = dynamic(move || {
                        let trial = store::trial();
                        match trial {
                            store::Trial::None => None,
                            store::Trial::Running => Some(untracked(move || foot().child(icon(Icon::LoaderCircle, 14.0).text_color(FG_FAINT)).child(said(t("Trying it on that computer…").to_owned())))),
                            store::Trial::Done { ok, output, took_ms } => Some(untracked(move || {
                                let (tone, label) = if ok { (SUCCESS, t("Succeeded")) } else { (DANGER, t("Failed")) };
                                let wrote = if output.trim().is_empty() { t("It printed nothing").to_owned() } else { output };
                                view()
                                    .gap(sp(S2))
                                    .px(sp(S4))
                                    .py(sp(S3))
                                    .border_t_1()
                                    .border_color(BORDER)
                                    .child(view().flex_row().items_center().gap(sp(S2)).child(badge(label, tone)).child(said(format!("{:.1}s", took_ms as f64 / 1000.0))).child(said(t("A try: nothing of it is kept").to_owned())))
                                    .child(text(wrote).text_size(tx(TEXT_SM)).line_height(px(18.0)).font_family(mono_font).text_color(if ok { FG } else { DANGER }).number_of_lines(14))
                            })),
                        }
                    });
                    return body
                        .child(
                            bare()
                                .h(px(132.0))
                                .font_family(mono_font)
                                .text_size(tx(TEXT_SM))
                                .value(move || command.get())
                                .on_change(move |v: &str| {
                                    command.set(v.to_owned());
                                    // What the last try said was of other words.
                                    store::reset_trial();
                                })
                                .placeholder(t("A shell command, run in the project's folder")),
                        )
                        // What it can count on, said where it is written.
                        .child(
                            view()
                                .px(sp(S4))
                                .pb(sp(S3))
                                .child(said(t("Run by sh in the project's folder, ten minutes at most. $SOPHON_PROJECT_DIR is that folder; when a trigger starts it, what the event carries is in $SOPHON_VAR_<NAME> and its id in $SOPHON_EVENT_ID").to_owned())),
                        )
                        .child(
                            foot()
                                .child(icon(Icon::SquareTerminal, 14.0).text_color(FG_FAINT))
                                .child(said(t("No conversation and no model: it does the same each time, and what it prints is kept with the run").to_owned()).flex_1())
                                .child(
                                    btn(t("Try it"), Variant::Outline, Sz::Sm, Some(Icon::Play), move || store::try_command(&command.get_untracked(), &path.get_untracked()))
                                        .disabled(move || command.with(|c| c.trim().is_empty()) || store::trial() == store::Trial::Running),
                                ),
                        )
                        .child(tried);
                }
                let with = dynamic(move || {
                    let (picking, runs_on, made_with) = (repicking.get(), runs_on.clone(), made_with.clone());
                    Some(untracked(move || match runs_on.filter(|_| !picking) {
                        // Made already: what it runs on stays as it was made,
                        // until it is asked to be picked again.
                        Some(target) => foot()
                            .child(icon(Icon::Cpu, 14.0).text_color(FG_FAINT))
                            .child(said(target))
                            .when_some(made_with, |v, (provider, model, mode)| {
                                v.child(btn(t("Change"), Variant::Ghost, Sz::Sm, None, move || {
                                    // The pickers are the composer's: they begin
                                    // on what this one runs on now.
                                    if !provider.is_empty() {
                                        store::set_control(false, store::Control::Provider(provider.clone()));
                                    }
                                    if !model.is_empty() {
                                        store::set_control(false, store::Control::Model(model.clone()));
                                    }
                                    if !mode.is_empty() {
                                        store::set_control(false, store::Control::Mode(mode.clone()));
                                    }
                                    repicking.set(true);
                                }))
                            }),
                        None => foot().px(sp(S2)).child(crate::ui::shared::composer::model_picker(false)).child(crate::ui::shared::model_picker::access_mode(false)),
                    }))
                });
                // Written as it reads: Markdown edited in place, with no
                // other view of it to switch to.
                let words = crate::ui::shared::markdown_editor::markdown_editor(prompt, t("Tell the agent what to do each time this runs…"), 180.0, 460.0);
                body.child(words).child(with)
            }))
        }));

    // --------------------------------------------------------------- where
    // The project is picked under the name, at the head.
    let session = card()
        .child(row(
            t("Runs a day"),
            t("The most it may start in a day, whatever starts it. Empty: no limit"),
            short(72.0).value(move || limit.get()).on_change(move |v: &str| limit.set(v.to_owned())).placeholder("∞"),
            true,
        ))
        .child(dynamic(move || {
            // A command runs in the folder itself: a worktree is a conversation's.
            if commanding.get() || no_folder {
                return None;
            }
            Some(untracked(move || {
                row(
                    t("Working copy"),
                    t("The project's own folder, or a new worktree for each run"),
                    segmented(vec![(false, t("This folder").into()), (true, t("New worktree").into())], move || worktree.get(), move |w| worktree.set(w)),
                    false,
                )
            }))
        }));

    // ---------------------------------------------------------------- its runs
    let runs = editing.clone().map(|id| {
        if store::schedule_runs(&id).is_none() {
            store::toggle_schedule_runs(&id);
        }
        dynamic(move || {
            let runs = store::schedule_runs(&id)?;
            Some(untracked(move || runs_list(runs)))
        })
    });

    // The head stays; what is under it scrolls.
    let column = || view().w_full().max_w(px(820.0)).self_center().px(sp(S6));
    let mut top = column().pt(sp(S8)).gap(sp(S3)).child(head).child(bar).when_some(failing, |v, failing| v.child(failing));
    if is_edit {
        top = top.child(
            view()
                .flex_row()
                .gap(px(20.0))
                .pt(sp(S2))
                .child(tab_button(t("Settings"), move || !history.get(), move || history.set(false)))
                .child(tab_button(t("Run history"), move || history.get(), move || history.set(true))),
        );
    }
    let settings = column()
        .py(sp(S6))
        .gap(sp(S6))
        .child(section(t("Trigger"), card().child(triggers).child(unready).child(options).child(times)))
        .child(section(if is_edit { t("Does") } else { t("Instructions") }, does))
        .child(section(t("Session"), session));
    let ran = column().py(sp(S6)).when_some(runs, |v, runs| v.child(runs));
    // Both are there and one has the room: what was typed in the settings
    // is still there on coming back from the runs.
    view()
        .flex_1()
        .child(view().when(is_edit, |v| v.border_b_1().border_color(BORDER)).child(top))
        .child(
            view()
                .flex_1()
                .child(view().h(px(0.0)).flex_grow(move || -> f32 { if history.get() { 0.0 } else { 1.0 } }).overflow_hidden().child(scroll().flex_1().child(settings)))
                .child(view().h(px(0.0)).flex_grow(move || -> f32 { if history.get() { 1.0 } else { 0.0 } }).overflow_hidden().child(scroll().flex_1().child(ran))),
        )
}

/// The mark of a trigger or a template: its glyph on a soft square.
fn glyph_tile(glyph: Icon) -> View {
    view().w(px(34.0)).h(px(34.0)).rounded(px(R_LG)).bg(tint(FG, 0.06)).items_center().justify_center().flex_shrink_0().child(icon(glyph, 16.0).text_color(FG_MUTED))
}

/// One of an automation's two pages, underlined while it is the one shown.
fn tab_button(label: &'static str, on: impl Fn() -> bool + Copy + 'static, on_press: impl Fn() + 'static) -> impl IntoElement {
    nsh_base::button()
        .on_press(on_press)
        .accessibility_label(label)
        .h(px(36.0))
        .justify_end()
        .gap(px(8.0))
        .child(text(label).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)).text_color(move || if on() { FG } else { FG_MUTED }))
        .child(view().h(px(2.0)).rounded(px(1.0)).bg(move || if on() { FG } else { tint(FG, 0.0) }))
}

/// Whether the one there is runs at times of its own (its cadence is
/// then what the editor begins with).
fn s_timed(existing: &Option<Schedule>) -> bool {
    existing.as_ref().is_some_and(|s| s.timed)
}

/// A time the service gave (UTC) on this computer's clock: `11-11 09:00`,
/// with the seconds for a run.
fn when_label(at: &Option<String>) -> String {
    at.as_deref().map(|t| local_time(t, false)).unwrap_or_else(|| "—".into())
}

fn local_time(at: &str, seconds: bool) -> String {
    let Some(utc) = crate::usage::parse::rfc3339(at) else { return at.get(..16).unwrap_or(at).replace('T', " ") };
    let local = utc + crate::usage::local_offset(utc);
    let (_, month, day) = crate::usage::civil(local.div_euclid(86_400));
    let of_day = local.rem_euclid(86_400);
    let (h, m, s) = (of_day / 3600, of_day % 3600 / 60, of_day % 60);
    if seconds { format!("{month:02}-{day:02} {h:02}:{m:02}:{s:02}") } else { format!("{month:02}-{day:02} {h:02}:{m:02}") }
}

/// How long a run took: `42s`, `3m 05s`.
fn took(run: &ScheduleRun) -> Option<String> {
    let parse = crate::usage::parse::rfc3339;
    let secs = (parse(run.ended.as_deref()?)? - parse(&run.started)?).max(0);
    Some(if secs < 60 { format!("{secs}s") } else { format!("{}m {:02}s", secs / 60, secs % 60) })
}

/// An automation's runs, newest first. One that started a conversation
/// is all of it to press: the conversation opens.
fn runs_list(runs: Vec<ScheduleRun>) -> View {
    if runs.is_empty() {
        return view().h(px(180.0)).items_center().justify_center().rounded(px(R_LG)).border_1().border_color(BORDER).child(text(t("This automation has not run yet")).text_size(tx(TEXT_SM)).text_color(FG_MUTED));
    }
    let mut list = view().gap(sp(S3));
    // The last fifty: more is the conversations' own history.
    for run in runs.into_iter().take(50) {
        let mut entry = view().gap(px(4.0));
        // The service says `completed`; the one before it said `succeeded`.
        let (tone, label) = match run.status.as_str() {
            "succeeded" | "completed" => (SUCCESS, t("Succeeded")),
            "failed" => (DANGER, t("Failed")),
            "canceled" => (FG_MUTED, t("Canceled")),
            _ => (WARNING, t("Running")),
        };
        let mut line = view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .min_h(sp(CONTROL_SM))
            .child(text(local_time(&run.started, true)).text_size(tx(TEXT_SM)).text_color(FG).font_family(mono_font))
            .child(badge(label, tone));
        if let Some(took) = took(&run) {
            line = line.child(text(took).text_size(tx(TEXT_SM)).text_color(FG_MUTED));
        }
        // What caused it, in the room that is left: a long one is cut short.
        line = line.child(
            text(run.event.clone().unwrap_or_default())
                .text_size(tx(TEXT_SM))
                .text_color(FG_MUTED)
                .font_family(mono_font)
                .number_of_lines(1)
                .flex_1()
                .min_w(px(0.0)),
        );
        // That there is a conversation behind it to open.
        if run.agent_id.is_some() {
            line = line.child(icon(Icon::ChevronRight, 14.0).text_color(FG_FAINT));
        }
        entry = entry.child(line);
        // Why it failed, on a line of its own.
        if let Some(error) = run.error.clone().filter(|error| !error.is_empty()) {
            entry = entry.child(text(crate::i18n::tr(&error)).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(DANGER).number_of_lines(3));
        }
        // What it said to whoever caused it.
        for (action, said) in run.replies.clone() {
            entry = entry.child(
                text(format!("{} {said}", if action == "react" { "☺" } else { "↩" }))
                    .text_size(tx(TEXT_SM))
                    .line_height(px(18.0))
                    .text_color(FG)
                    .number_of_lines(2),
            );
        }
        // What the agent answered: usually all there is to read of a run.
        if let Some(output) = run.output.clone() {
            entry = entry.child(
                text(output).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(FG_MUTED).number_of_lines(4),
            );
        }
        list = list.child(match run.agent_id.clone() {
            Some(agent) => {
                let workspace = run.workspace_id.clone();
                nsh_base::button()
                    .on_press(move || store::open_run_agent(&agent, workspace.as_deref()))
                    .accessibility_label(t("Open conversation"))
                    .px(sp(S4))
                    .py(sp(S3))
                    .rounded(px(R_XL))
                    .border_1()
                    .border_color(BORDER)
                    .bg(SURFACE0)
                    .hover(|style| style.bg(highlight()))
                    .child(entry)
                    .into_element()
            }
            // One that ran a command, or never got as far as an agent: nothing to open.
            None => card().px(sp(S4)).py(sp(S3)).child(entry).into_element(),
        });
    }
    list
}
