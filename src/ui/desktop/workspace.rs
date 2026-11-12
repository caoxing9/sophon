//! A workspace: its header (name, branch, diff), the agent tabs and a draft
//! tab, and the open tab's pane.

use crate::i18n::{t, tf};
use nsh_base::button;
use nsh_core::{element::FontWeight, prelude::*};
use crate::ui::kit::text;

use super::{explorer, file_pane, terminal};
use crate::ui::{
    kit::*,
    shared::{agent, timeline},
};
use crate::store::{self, Tab};

#[derive(Clone, PartialEq)]
enum Pane {
    Draft,
    Agent,
    Diff(Option<String>),
    File(String),
    Subagent(String, String),
    CommitDiff(String, String),
}

pub(crate) fn workspace(compact: bool) -> impl IntoElement {
    let pane = computed(|| match store::tab() {
        Tab::Draft => Pane::Draft,
        Tab::Agent(_) => Pane::Agent,
        Tab::Diff(p) => Pane::Diff(p),
        Tab::File(p) => Pane::File(p),
        Tab::Subagent(parent, id) => Pane::Subagent(parent, id),
        Tab::CommitDiff(sha, path) => Pane::CommitDiff(sha, path),
    });
    let fits = computed(move || !compact && window().width >= px(900.0));
    let show_explorer = computed(move || fits.get() && store::explorer_open());
    let bar = computed(nsh_core::element::desktop::title_bar);
    // The workspace's column (header, tabs, pane) beside the explorer, which
    // reaches up into the title bar; the terminals under both, the window's
    // whole width, as wide output wants.
    let top = view()
        .flex_1()
        .min_h(px(0.0))
        .flex_row()
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .child(header(compact, show_explorer))
                .child(view().flex_1().child(tabs()).child(dynamic(move || {
                    let pane = pane.get();
                    Some(untracked(move || match pane.clone() {
                        Pane::Draft => agent::draft_pane().into_element(),
                        Pane::Agent => agent::agent_pane(false).into_element(),
                        Pane::Diff(p) => explorer::diff_pane(p).into_element(),
                        Pane::File(p) => file_pane::file_pane(p).into_element(),
                        Pane::Subagent(parent, id) => timeline::subagent_pane(parent, id).into_element(),
                        Pane::CommitDiff(sha, path) => explorer::commit_diff_pane(sha, path).into_element(),
                    }))
                }))),
        )
        .child(when(fits, move || {
            // Mounted while it fits; the toggle slides it in and out
            // (its width eases, the panel keeps its own).
            view()
                .flex_shrink_0()
                .flex_row()
                .items_stretch()
                .overflow_hidden()
                .mt(move || Length::from(px(-bar.get().top.0)))
                .w(move || Length::from(px(if show_explorer.get() { store::explorer_width() + 10.0 } else { 0.0 })))
                .opacity(move || if show_explorer.get() { 1.0_f32 } else { 0.0 })
                .transition_opacity()
                // Its left edge drags its width.
                .child(explorer::resize_handle())
                .child(explorer::explorer().flex_1())
        }));
    view().flex_1().child(top).child(when(store::terminal_panel_open, terminal::terminal_panel))
}

fn header(compact: bool, explorer_shown: Memo<bool>) -> impl IntoElement {
    let ws = computed(store::current_workspace);
    let bar = computed(nsh_core::element::desktop::title_bar);
    // Beside the window's buttons, in the title bar's height, rather than a
    // row of its own under it: up over the strip that keeps the bar's room,
    // clear of the traffic lights and the sidebar toggle when the sidebar
    // is folded away. It drags the window, as the strip did.
    let inline = !compact && !MOBILE;
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .h(move || Length::from(px(if inline { bar.get().top.0.max(40.0) } else { 48.0 })))
        .mt(move || Length::from(px(if inline { -bar.get().top.0 } else { 0.0 })))
        .pl(move || {
            Length::from(px(if inline && store::sidebar_hidden() && bar.get().top.0 > 0.0 { bar.get().left.0 + 40.0 } else { S4 }))
        })
        .pr(px(S3))
        .window_drag(true)
        .when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home)))
        .child(
            text(move || ws.with(|w| w.as_ref().map(|w| w.name.clone()).unwrap_or_default()))
                .text_size(px(TEXT_BASE))
                .font_weight(FontWeight(500))
                .number_of_lines(1)
                .flex_shrink(1.0),
        )
        .child(view().flex_1())
        .child(scripts_menu())
        .child(super::git::git_controls())
        // The terminal panel, as VS Code's ⌃` and ⌘J.
        .when(!compact, |v| v.child(icon_btn(Icon::PanelBottom, Sz::Sm, t("Toggle terminal  ⌃`"), store::toggle_terminal_panel)))
        // The explorer has its own toggle while it shows.
        .when(!compact, |v| {
            v.child(when(move || !explorer_shown.get(), || icon_btn(Icon::PanelRight, Sz::Sm, t("Toggle explorer"), store::toggle_explorer)))
        })
}

/// The tab strip, while there is something beside the conversation to
/// switch to: files, a diff, a sub-agent. The conversations themselves are
/// rows of the sidebar, not tabs; the one last open has a tab here to come
/// back to.
fn tabs() -> impl IntoElement {
    let shown = computed(|| !store::open_files().is_empty() || matches!(store::tab(), Tab::File(_) | Tab::Diff(_) | Tab::CommitDiff(..) | Tab::Subagent(..)));
    view().child(when(move || shown.get(), tab_strip))
}

fn tab_strip() -> impl IntoElement {
    // The conversation last open here: the tab that leads back to it.
    let last = signal(None::<String>);
    effect(move || {
        if let Tab::Agent(id) = store::tab() {
            last.set(Some(id));
        } else if let Tab::Subagent(parent, _) = store::tab() {
            last.set(Some(parent));
        }
    });
    let agents = computed(move || {
        let known = last.get().and_then(|id| store::agent(&id));
        // None open yet in this window: the workspace's newest.
        known.or_else(|| store::current_workspace().and_then(|w| store::workspace_agents(&w.id).pop())).into_iter().collect::<Vec<_>>()
    });
    view()
        .flex_row()
        .items_center()
        .gap(px(S1))
        .h(px(40.0))
        .pl(px(S4))
        .pr(px(S3))
        .border_b_1()
        .border_color(tint(BORDER, 0.6))
        .child(each(
            move || agents.get(),
            |a| a.id.clone(),
            |a, _| {
                let id = a.get_untracked().id;
                agent_tab(id)
            },
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
                // "+" starts a new conversation here, straight away.
                let plus = button()
                    .on_press(|| store::open_tab(Tab::Draft))
                    .accessibility_label(t("New chat"))
                    .w(px(CONTROL_SM))
                    .h(px(CONTROL_SM))
                    .items_center()
                    .justify_center()
                    .rounded(px(R_FULL))
                    .hover(|s| s.bg(highlight()))
                    .child(icon(Icon::Plus, 16.0).text_color(FG_MUTED));
                let mut row = view().flex_row().items_center().gap(px(S1)).ml(px(2.0));
                if draft {
                    row = row.child(
                        tab_shell(true, || store::open_tab(Tab::Draft))
                            .child(icon(Icon::SquarePen, 14.0).text_color(FG_MUTED))
                            .child(text(t("New chat")).text_size(px(TEXT_SM)).text_color(FG)),
                    );
                }
                row.child(plus).into_element()
            }))
        }))
        .child(view().flex_1().min_w(px(S4)))
        // The open agent's state and actions, at the strip's end rather
        // than on a bar of their own.
        .child(when(|| matches!(store::tab(), Tab::Agent(_)), agent::agent_tools))
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
        .min_w(px(0.0))
        .flex_shrink(1.0)
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
                // A long name ends in an ellipsis inside the tab.
                .child(text(label).text_size(px(TEXT_SM)).text_color(if on { FG } else { FG_MUTED }).number_of_lines(1).flex_shrink(1.0).min_w(px(0.0)))
                .child(when(dirty, || view().w(px(6.0)).h(px(6.0)).rounded(px(R_FULL)).bg(FG_MUTED)))
                .child(close_button(t("Close file"), move || store::close_file(&close)))
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
                        .flex_shrink(1.0)
                        .min_w(px(0.0)),
                )
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
