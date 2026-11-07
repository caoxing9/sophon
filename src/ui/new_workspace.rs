//! The New workspace screen, as Paseo's: a hero title, the project and
//! isolation pickers, and the composer with its agent controls; one request
//! makes the workspace and starts its first agent.

use crate::i18n::{t, tf};
use nsh_core::{element::FontWeight, file_dialog::OpenDialog, prelude::*};
use super::kit::text;

use super::{
    composer::{self, Target},
    kit::*,
};
use crate::store;

/// The new workspace form: a narrow, centered column (Claude's new chat),
/// not the timeline's reading width.
const NEW_WORKSPACE_WIDTH: f32 = 600.0;

pub(crate) fn new_workspace(compact: bool) -> impl IntoElement {
    let form = computed(store::form);
    let project_label = computed(move || {
        let path = form.with(|f| f.path.clone());
        store::projects()
            .into_iter()
            .find(|p| p.root_path == path.trim_end_matches('/'))
            .map(|p| p.name)
            .unwrap_or_else(|| if path.is_empty() { t("Choose a project").into() } else { crate::daemon::model::folder_name(&path) })
    });
    let is_git = computed(move || {
        form.with(|f| store::projects().iter().any(|p| p.is_git() && p.root_path == f.path.trim_end_matches('/')))
    });

    view()
        .flex_1()
        .when(compact, |v| {
            v.child(view().flex_row().h(px(48.0)).px(px(S3)).items_center().child(icon_btn(
                Icon::ArrowLeft,
                Sz::Sm,
                t("Back"),
                store::go_home,
            )))
        })
        .child(
            view().flex_1().justify_center().px(px(S8)).pb(px(S8 * 2.0)).child(
                // One column for everything, as Paseo's: the title, the
                // pickers, the composer and what is under it share edges.
                view()
                    .w_full()
                    .max_w(px(NEW_WORKSPACE_WIDTH))
                    .self_center()
                    .gap(px(S3))
                    .child(
                        text(t("New workspace"))
                            .text_size(px(TEXT_XL))
                            .font_weight(FontWeight(450))
                            .px(px(S2))
                            .pb(px(S2)),
                    )
                    .child(
                        view()
                            .flex_row()
                            .flex_wrap()
                            .items_center()
                            .gap(px(S2))
                            .child(project_picker(project_label))
                            .child(when(move || form.with(|f| f.worktree), move || base_branch_picker(form)))
                            .child(when(move || form.with(|f| f.worktree), move || {
                                field()
                                    .w(px(200.0))
                                    .h(px(CONTROL_SM + 2.0))
                                    .rounded(px(R_FULL))
                                    .value(move || form.with(|f| f.branch_name.clone()))
                                    .on_change(|v: &str| store::edit_form(|f| f.branch_name = v.to_owned()))
                                    .placeholder(t("New branch (optional)"))
                                    .font_family(|| MONO)
                                    .text_size(px(TEXT_SM))
                            }))
                            .child(view().flex_1())
                            .child(when(is_git, move || {
                                segmented(
                                    vec![(false, t("Local").into()), (true, t("Worktree").into())],
                                    move || form.with(|f| f.worktree),
                                    |w| {
                                        store::edit_form(|f| f.worktree = w);
                                        if w {
                                            store::search_branches("");
                                        }
                                    },
                                )
                            })),
                    )
                    .child(composer::composer(Target::Form, t("What should the agent do?")))
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap(px(S3))
                            .pt(px(S1))
                            .child(btn(t("Import session"), Variant::Outline, Sz::Sm, Some(Icon::Upload), || {
                                store::search_sessions("");
                                store::open_modal(store::Modal::ImportSession)
                            }))
                            .child(view().flex_1().min_w(px(0.0)).child(composer::hint(move || {
                                form.with(|f| {
                                    f.error.clone().map(|e| (e, true)).or_else(|| {
                                        if f.busy {
                                            Some((t("Creating workspace…").into(), false))
                                        } else if f.path.is_empty() {
                                            None
                                        } else if f.worktree {
                                            Some((tf("A new worktree branched from {}", &[&f.path]), false))
                                        } else {
                                            Some((tf("Runs in {}", &[&f.path]), false))
                                        }
                                    })
                                })
                            }))),
                    ),
            ),
        )
}

fn project_picker(label: Memo<String>) -> impl IntoElement {
    dropdown(
        view()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .h(px(CONTROL_SM + 2.0))
            .px(px(S2))
            .rounded(px(R_FULL))
            .hover(|s| s.bg(highlight()))
            .child(icon(Icon::Folder, 15.0).text_color(FG_MUTED))
            .child(text(move || label.get()).text_size(px(TEXT_BASE)).number_of_lines(1))
            .child(icon(Icon::ChevronDown, 14.0).text_color(FG_FAINT)),
    )
    .child(dynamic(move || {
        let projects = store::projects();
        let current = store::form().path;
        Some(untracked(move || {
            let mut list = view().child(menu_heading(t("Projects")));
            for p in projects.iter().filter(|p| !p.root_path.is_empty()) {
                let path = p.root_path.clone();
                list = list.child(menu_row(
                    p.name.clone(),
                    Some(if p.is_git() { Icon::GitBranch } else { Icon::Folder }),
                    Some(p.root_path.clone().into()),
                    current.trim_end_matches('/') == p.root_path,
                    move || {
                        store::edit_form(|f| {
                            f.path = path.clone();
                            f.worktree = false;
                        })
                    },
                ));
            }
            list.child(menu_divider())
                .child(menu_row(t("Choose folder…"), Some(Icon::FolderOpen), None, false, || {
                    spawn(async move {
                        if let Ok(Some(paths)) = pick_paths(OpenDialog::folders().title(t("Choose a project folder"))).await
                            && let Some(path) = paths.into_iter().next()
                        {
                            store::edit_form(|f| {
                                f.path = path.to_string_lossy().into_owned();
                                f.worktree = false;
                            });
                        }
                    });
                }))
                .into_element()
        }))
    }))
}

fn base_branch_picker(form: Memo<store::Form>) -> impl IntoElement {
    dropdown(picker_trigger(Some(Icon::GitBranch), move || {
        form.with(|f| f.base_branch.clone().map(|b| tf("From {}", &[&b])).unwrap_or_else(|| t("From default branch").into()))
    }))
    .child(dynamic(move || {
        let branches = store::branches();
        Some(untracked(move || {
            let mut list = view()
                .child(menu_heading(t("Start from")))
                .child(menu_row(t("Default branch"), None, None, form.with_untracked(|f| f.base_branch.is_none()), || {
                    store::edit_form(|f| f.base_branch = None)
                }));
            for b in branches.clone() {
                let pick = b.clone();
                let on = form.with_untracked(|f| f.base_branch.as_deref() == Some(b.as_str()));
                list = list.child(menu_row(b, Some(Icon::GitBranch), None, on, move || {
                    store::edit_form(|f| f.base_branch = Some(pick.clone()))
                }));
            }
            menu_scroll(360.0, list).into_element()
        }))
    }))
}
