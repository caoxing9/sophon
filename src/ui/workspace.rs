//! A workspace: its header (name, branch, diff), the agent tabs and a draft
//! tab, and the open tab's pane.

use crate::i18n::{t, tf};
use nsh_base::button;
use nsh_core::{element::FontWeight, prelude::*};
use super::kit::text;

use super::{
    composer::{self, Target},
    explorer, file_pane,
    kit::*,
    terminal, timeline,
};
use crate::store::{self, Tab};

#[derive(Clone, PartialEq)]
enum Pane {
    Draft,
    Agent,
    Diff(Option<String>),
    Terminal(String),
    File(String),
    Subagent(String, String),
    CommitDiff(String, String),
}

pub(crate) fn workspace(compact: bool) -> impl IntoElement {
    let pane = computed(|| match store::tab() {
        Tab::Draft => Pane::Draft,
        Tab::Agent(_) => Pane::Agent,
        Tab::Diff(p) => Pane::Diff(p),
        Tab::Terminal(id) => Pane::Terminal(id),
        Tab::File(p) => Pane::File(p),
        Tab::Subagent(parent, id) => Pane::Subagent(parent, id),
        Tab::CommitDiff(sha, path) => Pane::CommitDiff(sha, path),
    });
    let fits = computed(move || !compact && window().width >= px(900.0));
    let show_explorer = computed(move || fits.get() && store::explorer_open());
    view()
        .flex_1()
        .child(header(compact))
        .child(
            view()
                .flex_1()
                .flex_row()
                .child(
                    view().flex_1().min_w(px(0.0)).child(tabs()).child(dynamic(move || {
                        let pane = pane.get();
                        Some(untracked(move || match pane.clone() {
                            Pane::Draft => draft_pane().into_element(),
                            Pane::Agent => agent_pane().into_element(),
                            Pane::Diff(p) => explorer::diff_pane(p).into_element(),
                            Pane::Terminal(id) => terminal::terminal_pane(id).into_element(),
                            Pane::File(p) => file_pane::file_pane(p).into_element(),
                            Pane::Subagent(parent, id) => timeline::subagent_pane(parent, id).into_element(),
                            Pane::CommitDiff(sha, path) => explorer::commit_diff_pane(sha, path).into_element(),
                        }))
                    })),
                )
                .child(when(fits, move || {
                    // Mounted while it fits; the toggle slides it in and out
                    // (its width eases, the panel keeps its own).
                    view()
                        .flex_shrink_0()
                        .items_start()
                        .overflow_hidden()
                        .w(move || Length::from(px(if show_explorer.get() { explorer::EXPLORER_WIDTH + 10.0 } else { 0.0 })))
                        .opacity(move || if show_explorer.get() { 1.0_f32 } else { 0.0 })
                        .transition_opacity()
                        .child(explorer::explorer().flex_1())
                })),
        )
}

fn header(compact: bool) -> impl IntoElement {
    let ws = computed(store::current_workspace);
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .h(px(48.0))
        .px(px(S4))
        .when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home)))
        .child(
            text(move || ws.with(|w| w.as_ref().map(|w| w.name.clone()).unwrap_or_default()))
                .text_size(px(TEXT_BASE))
                .font_weight(FontWeight(500))
                .number_of_lines(1)
                .flex_shrink(1.0),
        )
        .child(dynamic(move || {
            let branch = ws.with(|w| w.as_ref().and_then(|w| w.branch.clone()))?;
            let worktree = ws.with(|w| w.as_ref().is_some_and(|w| w.is_worktree()));
            Some(untracked(move || {
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(5.0))
                    .h(px(22.0))
                    .px(px(S2))
                    .rounded(px(R_FULL))
                    .bg(SURFACE2)
                    .child(icon(if worktree { Icon::GitBranch } else { Icon::GitCommitHorizontal }, 13.0).text_color(FG_MUTED))
                    .child(text(branch.clone()).text_size(px(TEXT_SM)).text_color(FG_MUTED).number_of_lines(1))
            }))
        }))
        .child(
            text(move || {
                ws.with(|w| match w {
                    Some(w) if w.additions + w.deletions > 0 => format!("+{}  −{}", w.additions, w.deletions),
                    _ => String::new(),
                })
            })
            .text_size(px(TEXT_SM))
            .font_family(|| MONO)
            .text_color(FG_MUTED),
        )
        .child(view().flex_1())
        .child(scripts_menu())
        .child(super::git::git_controls())
        .when(!compact, |v| v.child(text(move || ws.with(|w| w.as_ref().map(|w| w.directory.clone()).unwrap_or_default()))
            .text_size(px(TEXT_SM))
            .text_color(FG_FAINT)
            .number_of_lines(1)
            .flex_shrink(1.0)))
        .when(!compact, |v| v.child(icon_btn(Icon::PanelRight, Sz::Sm, t("Toggle explorer"), store::toggle_explorer)))
}

/// The tab strip: one tab per agent, then "+" for a new agent here.
fn tabs() -> impl IntoElement {
    let agents = computed(|| store::current_workspace().map(|w| store::workspace_agents(&w.id)).unwrap_or_default());
    view()
        .flex_row()
        .items_center()
        .gap(px(S1))
        .h(px(40.0))
        .px(px(S4))
        .child(each(
            move || agents.get(),
            |a| a.id.clone(),
            |a, _| {
                let id = a.get_untracked().id;
                agent_tab(id)
            },
        ))
        .child(each(
            store::terminals,
            |t| t.id.clone(),
            |t, _| terminal_tab(t.get_untracked()),
        ))
        .child(each(store::open_files, |p| p.clone(), |p, _| file_tab(p.get_untracked())))
        .child(dynamic(|| {
            let Tab::CommitDiff(sha, path) = store::tab() else { return None };
            Some(untracked(move || {
                tab_shell(true, || {})
                    .child(icon(Icon::GitCommitHorizontal, 14.0).text_color(FG_MUTED))
                    .child(text(format!("{} {}", sha.get(..7).unwrap_or(&sha), path.rsplit('/').next().unwrap_or(&path))).text_size(px(TEXT_SM)).number_of_lines(1))
                    .child(close_button(t("Close commit diff"), || {
                        if let Some(w) = store::current_workspace() {
                            store::open_workspace(&w.id);
                        }
                    }))
            }))
        }))
        .child(dynamic(|| {
            let Tab::Subagent(parent, id) = store::tab() else { return None };
            Some(untracked(move || {
                let title = store::subagent(&parent, &id).map(|s| s.title).unwrap_or_else(|| t("Sub-agent").into());
                let back = parent.clone();
                tab_shell(true, || {})
                    .child(icon(Icon::Bot, 14.0).text_color(FG_MUTED))
                    .child(text(title).text_size(px(TEXT_SM)).text_color(FG).number_of_lines(1))
                    .child(close_button(t("Close sub-agent"), move || store::open_tab(Tab::Agent(back.clone()))))
            }))
        }))
        .child(dynamic(|| {
            let Tab::Diff(path) = store::tab() else { return None };
            Some(untracked(move || {
                let label = path.as_deref().map(|p| p.rsplit('/').next().unwrap_or(p).to_owned()).unwrap_or_else(|| t("All changes").into());
                tab_shell(true, || {})
                    .child(icon(Icon::FileDiff, 14.0).text_color(FG_MUTED))
                    .child(text(label).text_size(px(TEXT_SM)).text_color(FG).number_of_lines(1))
                    .child(
                        button()
                            .on_press(|| {
                                if let Some(w) = store::current_workspace() {
                                    store::open_workspace(&w.id);
                                }
                            })
                            .accessibility_label(t("Close diff"))
                            .w(px(18.0))
                            .h(px(18.0))
                            .rounded(px(R_FULL))
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(highlight()))
                            .child(icon(Icon::X, 12.0).text_color(FG_MUTED)),
                    )
            }))
        }))
        .child(dynamic(|| {
            let draft = store::tab() == Tab::Draft;
            Some(untracked(move || {
                let plus = dropdown(
                    view()
                        .w(px(CONTROL_SM))
                        .h(px(CONTROL_SM))
                        .items_center()
                        .justify_center()
                        .rounded(px(R_FULL))
                        .hover(|s| s.bg(highlight()))
                        .label(t("New tab"))
                        .child(icon(Icon::Plus, 16.0).text_color(FG_MUTED)),
                )
                .child(menu_row(t("New agent"), Some(Icon::Bot), None, false, || store::open_tab(Tab::Draft)))
                .child(menu_row(t("New terminal"), Some(Icon::SquareTerminal), None, false, store::new_terminal));
                let mut row = view().flex_row().items_center().gap(px(S1)).ml(px(2.0));
                if draft {
                    row = row.child(
                        tab_shell(true, || store::open_tab(Tab::Draft))
                            .child(icon(Icon::SquarePen, 14.0).text_color(FG_MUTED))
                            .child(text(t("New agent")).text_size(px(TEXT_SM)).text_color(FG)),
                    );
                }
                row.child(plus).into_element()
            }))
        }))
}

fn tab_shell(selected: bool, on_press: impl Fn() + 'static) -> nsh_base::ButtonElement {
    button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap(px(7.0))
        .h(px(30.0))
        .pl(px(10.0))
        .pr(px(12.0))
        .max_w(px(220.0))
        .rounded(px(R_FULL))
        .bg(if selected { tint(ACCENT, 0.10) } else { tint(SURFACE0, 0.0) })
        .hover(|s| s.bg(highlight()))
        .transition_colors()
}

fn close_button(label: &'static str, on_press: impl Fn() + 'static) -> impl IntoElement {
    button()
        .on_press(on_press)
        .accessibility_label(label)
        .w(px(18.0))
        .h(px(18.0))
        .rounded(px(R_FULL))
        .items_center()
        .justify_center()
        .hover(|s| s.bg(highlight()))
        .child(icon(Icon::X, 12.0).text_color(FG_MUTED))
}

fn file_tab(path: String) -> impl IntoElement {
    let selected = {
        let path = path.clone();
        computed(move || store::tab() == Tab::File(path.clone()))
    };
    let dirty = {
        let path = path.clone();
        computed(move || store::file(&path).is_some_and(|f| f.dirty()))
    };
    let label = path.rsplit('/').next().unwrap_or(&path).to_owned();
    view().child(dynamic(move || {
        let on = selected.get();
        let (open, close, label) = (path.clone(), path.clone(), label.clone());
        Some(untracked(move || {
            tab_shell(on, move || store::open_tab(Tab::File(open.clone())))
                .child(icon(Icon::FileText, 14.0).text_color(FG_MUTED))
                .child(text(label).text_size(px(TEXT_SM)).text_color(if on { FG } else { FG_MUTED }).number_of_lines(1))
                .child(when(dirty, || view().w(px(6.0)).h(px(6.0)).rounded(px(R_FULL)).bg(FG_MUTED)))
                .child(close_button(t("Close file"), move || store::close_file(&close)))
        }))
    }))
}

fn terminal_tab(t: crate::daemon::connection::TerminalInfo) -> impl IntoElement {
    let id = t.id.clone();
    let selected = {
        let id = id.clone();
        computed(move || store::tab() == Tab::Terminal(id.clone()))
    };
    let label = t.title.clone().unwrap_or_else(|| t.name.clone());
    view().child(dynamic(move || {
        let on = selected.get();
        let (open_id, kill_id, label) = (id.clone(), id.clone(), label.clone());
        Some(untracked(move || {
            tab_shell(on, move || store::open_tab(Tab::Terminal(open_id.clone())))
                .child(icon(Icon::SquareTerminal, 14.0).text_color(FG_MUTED))
                .child(text(label).text_size(px(TEXT_SM)).text_color(if on { FG } else { FG_MUTED }).number_of_lines(1))
                .child(close_button(crate::i18n::t("Close terminal"), move || store::kill_terminal(&kill_id)))
        }))
    }))
}

fn agent_tab(id: String) -> impl IntoElement {
    let agent = {
        let id = id.clone();
        computed(move || store::agent(&id))
    };
    let selected = {
        let id = id.clone();
        computed(move || store::tab() == Tab::Agent(id.clone()))
    };
    let open_id = id.clone();
    view().child(dynamic(move || {
        let on = selected.get();
        let open_id = open_id.clone();
        Some(untracked(move || {
            tab_shell(on, move || store::open_tab(Tab::Agent(open_id.clone())))
                .child(status_dot(move || {
                    agent.with(|a| {
                        a.as_ref()
                            .map(|a| Status::of_agent(&a.status, a.requires_attention || !a.pending.is_empty()))
                            .unwrap_or(Status::Idle)
                    })
                }))
                .child(
                    text(move || agent.with(|a| a.as_ref().map(|a| a.title.clone()).unwrap_or_default()))
                        .text_size(px(TEXT_SM))
                        .text_color(if on { FG } else { FG_MUTED })
                        .number_of_lines(1)
                        .flex_shrink(1.0),
                )
        }))
    }))
}

fn agent_pane() -> impl IntoElement {
    let agent = computed(store::selected_agent);
    // The composer floats over the conversation, which scrolls under the
    // glass.
    view()
        .flex_1()
        .child(agent_bar(agent))
        .child(timeline::find_bar())
        .child(timeline::timeline())
        .child(
            view()
                .absolute()
                .left(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .px(px(S6))
                .pb(px(S4))
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
            .child(dynamic(move || {
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
            })),
        )
}

/// A new agent in this workspace.
fn draft_pane() -> impl IntoElement {
    let form = computed(store::form);
    view()
        .flex_1()
        .justify_center()
        .px(px(S6))
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
}

/// The outline: the conversation's prompts, to jump to.
fn dropup_or_down_outline() -> impl IntoElement {
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

/// The workspace's scripts and services: run state, health, start / stop.
pub(crate) fn scripts_menu() -> impl IntoElement {
    let scripts = computed(|| store::current_workspace().map(|w| w.scripts).unwrap_or_default());
    view().child(dynamic(move || {
        let list = scripts.get();
        if list.is_empty() {
            return None;
        }
        let running = list.iter().filter(|s| s.running).count();
        Some(untracked(move || {
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
                    .child(icon(Icon::Play, 13.0).text_color(if running > 0 { SUCCESS } else { FG_MUTED }))
                    .child(text(format!("{}{}", t("Scripts"), if running > 0 { format!(" · {running}") } else { String::new() })).text_size(px(TEXT_SM)).font_weight(FontWeight(500))),
            )
            .align(nsh_core::overlay::Align::End)
            .child(dynamic(move || {
                let list = store::current_workspace().map(|w| w.scripts).unwrap_or_default();
                Some(untracked(move || {
                    let mut m = view();
                    for sc in list.clone() {
                        let (name, start) = (sc.name.clone(), !sc.running);
                        let state = match (sc.running, sc.healthy) {
                            (true, Some(false)) => t("Running · unhealthy"),
                            (true, _) => t("Running"),
                            (false, _) => t("Stopped"),
                        };
                        m = m.child(menu_row(
                            sc.name.clone(),
                            Some(if sc.running { Icon::Square } else { Icon::Play }),
                            Some(format!("{state} · {}", if sc.running { t("click to stop") } else { t("click to start") }).into()),
                            false,
                            move || store::run_script(&name, start),
                        ));
                        if let (true, Some(url)) = (sc.running, sc.url.clone()) {
                            m = m.child(menu_row(tf("Open {}", &[&url]), Some(Icon::ExternalLink), None, false, move || {
                                let _ = open_url(url.clone());
                            }));
                        }
                    }
                    m.into_element()
                }))
            }))
            .into_element()
        }))
    }))
}
