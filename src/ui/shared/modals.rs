//! Dialogs: a centered card over a dimmed backdrop (Paseo's adaptive modal
//! on desktop), for renames and confirmations.

use crate::i18n::{t, tf};
use nsh_base::dialog;
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store::{self, Modal};

pub(crate) fn modal_layer() -> impl IntoElement {
    let open = computed(|| store::modal().is_some());
    when(open, || {
        dialog()
            .animation(ModalAnimation::Fade)
            .on_cancel(store::close_modal)
            .flex_1()
            .child(
                view()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.32))
                    .child(dynamic(|| {
                        let m = store::modal()?;
                        Some(untracked(move || {
                            if m == Modal::Palette {
                                view().self_stretch().items_center().pt(relative(0.12)).flex_1().child(crate::ui::desktop::shortcuts::palette())
                            } else {
                                body(m.clone())
                            }
                        }))
                    })),
            )
    })
}

fn sheet(title: &str) -> View {
    glass_strong(view())
        .w(px(420.0))
        .max_w(relative(0.92))
        .p(px(S6))
        .gap(px(S4))
        .rounded(px(22.0))
        .child(text(title.to_owned()).text_size(px(TEXT_LG)).font_weight(FontWeight(500)))
}

fn actions(primary: impl IntoElement) -> View {
    view()
        .flex_row()
        .justify_end()
        .gap(px(S2))
        .child(btn(t("Cancel"), Variant::Secondary, Sz::Md, None, store::close_modal))
        .child(primary)
}

fn body(m: Modal) -> View {
    match m {
        Modal::RenameWorkspace { id, name } => rename(t("Rename workspace"), name, move |v| {
            store::rename_workspace(&id, v);
        }),
        Modal::RenameAgent { id, name } => rename(t("Rename agent"), name, move |v| {
            store::rename_agent(&id, v);
        }),
        Modal::Commit => commit(),
        Modal::Palette => view(),
        Modal::NewSchedule => new_schedule(),
        Modal::AddProject => add_project(),
        Modal::SwitchBranch => switch_branch(),
        Modal::ImportSession => import_session(),
        Modal::Usage => usage(),
        Modal::AddHost => add_host(),
        Modal::PairDevice => pair_device(),
        Modal::Labels { workspace } => labels(workspace),
        Modal::ProjectSettings { name, .. } => project_settings(name),
        Modal::FileName { op, path, name } => rename(
            match op {
                "create-file" => t("New file"),
                "create-dir" => t("New folder"),
                _ => t("Rename"),
            },
            name,
            move |v| store::file_op(op, &path, v),
        ),
        Modal::RestartDaemon => sheet(t("Restart service"))
            .child(
                text(t("Running agents stop and resume when the service is back, if resume on startup is on. Terminals keep running"))
                    .text_size(px(TEXT_BASE))
                    .line_height(px(20.0))
                    .text_color(FG_MUTED),
            )
            .child(actions(btn(t("Restart"), Variant::Destructive, Sz::Md, Some(Icon::RotateCw), || {
                store::restart_daemon();
                store::close_modal();
            }))),
        Modal::ToolDetail { name, detail } => sheet(&name)
            .w(px(720.0))
            .child(
                scroll().max_h(px(480.0)).rounded(px(R_LG)).bg(SURFACE2).child(
                    text(if detail.is_empty() { t("No details").to_owned() } else { detail })
                        .p(px(S3))
                        .text_size(px(TEXT_CODE))
                        .line_height(px(18.0))
                        .font_family(|| MONO)
                        .selectable(true),
                ),
            )
            .child(actions(view())),
        Modal::RenameProject { id, name } => rename(t("Rename project"), name, move |v| store::rename_project(&id, v)),
        Modal::ArchiveWorkspace { id, name } => sheet(t("Archive workspace"))
            .child(
                text(format!(
                    "“{name}” and its agents move to the archive. A worktree's files are removed from disk; commit or push anything you want to keep"
                ))
                .text_size(px(TEXT_BASE))
                .line_height(px(20.0))
                .text_color(FG_MUTED),
            )
            .child(actions(btn(t("Archive"), Variant::Destructive, Sz::Md, None, move || {
                store::archive_workspace(&id);
                store::close_modal();
            }))),
    }
}

fn rename(title: &'static str, current: String, apply: impl Fn(&str) + Clone + 'static) -> View {
    let value = signal(current);
    let save = {
        let apply = apply.clone();
        move || {
            let v = value.get_untracked();
            if !v.trim().is_empty() {
                apply(v.trim());
            }
            store::close_modal();
        }
    };
    let save2 = save.clone();
    sheet(title)
        .child(
            field()
                .value(move || value.get())
                .on_change(move |v: &str| value.set(v.to_owned()))
                .on_submit(move |_: &str| save2())
                .auto_focus(true),
        )
        .child(actions(btn(t("Save"), Variant::Primary, Sz::Md, None, save)))
}

fn commit() -> View {
    let message = signal(String::new());
    let go = move || {
        store::git_action(crate::daemon::connection::GitAction::Commit(message.get_untracked()));
        store::close_modal();
    };
    sheet(t("Commit all changes"))
        .child(
            field()
                .multiline(true)
                .h(px(96.0))
                .py(px(S2))
                .value(move || message.get())
                .on_change(move |v: &str| message.set(v.to_owned()))
                .placeholder(t("Commit message (leave empty to generate one)"))
                .auto_focus(true),
        )
        .child(actions(btn(t("Commit"), Variant::Primary, Sz::Md, Some(Icon::GitCommitHorizontal), go)))
}

fn new_schedule() -> View {
    let name = signal(String::new());
    let prompt = signal(String::new());
    let cron = signal("0 9 * * 1-5".to_owned());
    let path = signal(store::projects().into_iter().map(|p| p.root_path).find(|p| !p.is_empty()).unwrap_or_default());
    let worktree = signal(false);
    let presets: Vec<(String, SharedString)> = vec![
        ("every:3600000".into(), t("Hourly").into()),
        ("0 9 * * *".into(), t("Daily 9:00").into()),
        ("0 9 * * 1-5".into(), t("Weekdays 9:00").into()),
    ];
    let create = move || {
        let p = prompt.get_untracked();
        if p.trim().is_empty() || path.get_untracked().is_empty() {
            return;
        }
        store::create_schedule(name.get_untracked(), p, cron.get_untracked(), path.get_untracked(), worktree.get_untracked());
        store::close_modal();
    };
    let mut projects = view().flex_row().flex_wrap().gap(px(S2));
    for p in store::projects().into_iter().filter(|p| !p.root_path.is_empty()) {
        let (a, b) = (p.root_path.clone(), p.root_path.clone());
        projects = projects.child(chip(p.name.clone(), Some(Icon::Folder), move || path.get() == a, move || path.set(b.clone())));
    }
    sheet(t("New schedule"))
        .w(px(520.0))
        .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).placeholder(t("Name (optional)")))
        .child(
            field()
                .multiline(true)
                .h(px(96.0))
                .py(px(S2))
                .value(move || prompt.get())
                .on_change(move |v: &str| prompt.set(v.to_owned()))
                .placeholder(t("What should the agent do each time?")),
        )
        .child(section_label(t("Project")))
        .child(projects)
        .child(segmented(vec![(false, t("Local").into()), (true, t("Worktree").into())], move || worktree.get(), move |w| worktree.set(w)))
        .child(section_label(t("Cadence")))
        .child(segmented(presets, move || cron.get(), move |c| cron.set(c)))
        .child(
            field()
                .value(move || cron.get())
                .on_change(move |v: &str| cron.set(v.to_owned()))
                .font_family(|| MONO)
                .placeholder(t("Cron expression, e.g. 0 9 * * 1-5")),
        )
        .child(actions(
            btn(t("Create"), Variant::Primary, Sz::Md, Some(Icon::CalendarClock), create)
                .disabled(move || prompt.with(|p| p.trim().is_empty()) || path.with(String::is_empty)),
        ))
}

fn add_project() -> View {
    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        Open,
        Create,
        Clone,
    }
    let mode = signal(Mode::Open);
    let parent = signal(std::env::var("HOME").unwrap_or_default());
    let name = signal(String::new());
    let repo = signal(String::new());
    let pick_parent = move || {
        spawn(async move {
            let dialog = nsh_core::file_dialog::OpenDialog::folders().title(t("Choose where it goes"));
            if let Ok(Some(paths)) = pick_paths(dialog).await
                && let Some(p) = paths.into_iter().next()
            {
                parent.set(p.to_string_lossy().into_owned());
            }
        });
    };
    sheet(t("Add project"))
        .w(px(520.0))
        .child(segmented(
            vec![(Mode::Open, t("Existing folder").into()), (Mode::Create, t("New folder").into()), (Mode::Clone, t("Clone from GitHub").into())],
            move || mode.get(),
            move |m| mode.set(m),
        ))
        .child(dynamic(move || {
            let m = mode.get();
            Some(untracked(move || match m {
                Mode::Open => view()
                    .gap(px(S3))
                    .child(text(t("Pick a folder on this computer; Sophon lists it with your projects")).text_size(px(TEXT_SM)).text_color(FG_MUTED))
                    .child(actions(btn(t("Choose folder…"), Variant::Primary, Sz::Md, Some(Icon::FolderOpen), || {
                        spawn(async move {
                            let dialog = nsh_core::file_dialog::OpenDialog::folders().title(t("Add a project folder"));
                            if let Ok(Some(paths)) = pick_paths(dialog).await
                                && let Some(p) = paths.into_iter().next()
                            {
                                store::add_project(&p.to_string_lossy());
                                store::close_modal();
                            }
                        });
                    }))),
                Mode::Create => view()
                    .gap(px(S3))
                    .child(location_row(parent, pick_parent))
                    .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).placeholder(t("Folder name")))
                    .child(actions(
                        btn(t("Create"), Variant::Primary, Sz::Md, Some(Icon::FolderPlus), move || {
                            store::create_project_dir(&parent.get_untracked(), name.get_untracked().trim());
                            store::close_modal();
                        })
                        .disabled(move || name.with(|n| n.trim().is_empty())),
                    )),
                Mode::Clone => view()
                    .gap(px(S3))
                    .child(field().value(move || repo.get()).on_change(move |v: &str| repo.set(v.to_owned())).placeholder(t("owner/repository")))
                    .child(location_row(parent, pick_parent))
                    .child(actions(
                        btn(t("Clone"), Variant::Primary, Sz::Md, Some(Icon::GitBranch), move || {
                            let r = repo.get_untracked();
                            let folder = r.trim().rsplit('/').next().unwrap_or_default().trim_end_matches(".git").to_owned();
                            let target = format!("{}/{}", parent.get_untracked().trim_end_matches('/'), folder);
                            store::clone_project(r.trim(), &target);
                            store::close_modal();
                        })
                        .disabled(move || repo.with(|r| !r.contains('/'))),
                    )),
            }))
        }))
}

fn location_row(parent: Signal<String>, pick: impl Fn() + 'static) -> View {
    view()
        .flex_row()
        .items_center()
        .gap(px(S2))
        .child(icon(Icon::Folder, 15.0).text_color(FG_MUTED))
        .child(text(move || parent.get()).text_size(px(TEXT_SM)).font_family(|| MONO).text_color(FG_MUTED).number_of_lines(1).flex_1())
        .child(btn(t("Change…"), Variant::Outline, Sz::Sm, None, pick))
}

fn switch_branch() -> View {
    let query = signal(String::new());
    let list = computed(store::branches);
    sheet(t("Switch branch"))
        .child(
            field()
                .value(move || query.get())
                .on_change(move |v: &str| {
                    query.set(v.to_owned());
                    store::search_branches(v);
                })
                .placeholder(t("Search branches"))
                .auto_focus(true),
        )
        .child(scroll().max_h(px(320.0)).child(dynamic(move || {
            let branches = list.get();
            Some(untracked(move || {
                let mut col = view().gap(px(2.0));
                for b in branches.clone() {
                    let name = b.clone();
                    col = col.child(
                        nsh_base::button()
                            .on_press(move || {
                                store::git_action(crate::daemon::connection::GitAction::SwitchBranch(name.clone()));
                                store::close_modal();
                            })
                            .flex_row()
                            .items_center()
                            .gap(px(S2))
                            .h(px(CONTROL_MD))
                            .px(px(S2))
                            .rounded(px(R_LG))
                            .hover(|s| s.bg(highlight()))
                            .child(icon(Icon::GitBranch, 14.0).text_color(FG_MUTED))
                            .child(text(b).text_size(px(TEXT_BASE)).font_family(|| MONO).number_of_lines(1)),
                    );
                }
                col
            }))
        })))
        .child(actions(view()))
}

fn import_session() -> View {
    let query = signal(String::new());
    let list = computed(store::recent_sessions);
    sheet(t("Import a session"))
        .w(px(620.0))
        .child(text(t("Continue a Claude Code or Codex session you started outside Sophon")).text_size(px(TEXT_SM)).text_color(FG_MUTED))
        .child(
            field()
                .value(move || query.get())
                .on_change(move |v: &str| {
                    query.set(v.to_owned());
                    store::search_sessions(v);
                })
                .placeholder(t("Search sessions"))
                .auto_focus(true),
        )
        .child(scroll().max_h(px(380.0)).child(dynamic(move || {
            let sessions = list.get();
            Some(untracked(move || match sessions.clone() {
                None => text(t("Loading…")).text_color(FG_MUTED).into_element(),
                Some(s) if s.is_empty() => text(t("No sessions found")).text_size(px(TEXT_BASE)).text_color(FG_MUTED).into_element(),
                Some(s) => {
                    let mut col = view().gap(px(2.0));
                    for sess in s {
                        let pick = sess.clone();
                        col = col.child(
                            nsh_base::button()
                                .on_press(move || {
                                    store::import_session(&pick);
                                    store::close_modal();
                                })
                                .gap(px(2.0))
                                .px(px(S2))
                                .py(px(6.0))
                                .rounded(px(R_LG))
                                .hover(|st| st.bg(highlight()))
                                .child(text(sess.title.clone()).text_size(px(TEXT_BASE)).number_of_lines(1))
                                .child(
                                    text(format!(
                                        "{} · {} · {}",
                                        sess.provider_label,
                                        crate::daemon::model::folder_name(&sess.cwd),
                                        sess.last_activity.get(..10).unwrap_or_default()
                                    ))
                                    .text_size(px(TEXT_SM))
                                    .text_color(FG_MUTED),
                                ),
                        );
                    }
                    col.into_element()
                }
            }))
        })))
        .child(actions(view()))
}

fn usage() -> View {
    let data = computed(store::usage);
    sheet(t("Usage"))
        .w(px(560.0))
        .child(dynamic(move || {
            let d = data.get();
            Some(untracked(move || match d.clone() {
                None => text(t("Loading…")).text_color(FG_MUTED).into_element(),
                Some(list) if list.is_empty() => text(t("No usage reported by your providers")).text_size(px(TEXT_BASE)).text_color(FG_MUTED).into_element(),
                Some(list) => {
                    let mut col = view().gap(px(S4));
                    for p in list {
                        let mut c = card().p(px(S4)).gap(px(S3)).child(
                            view()
                                .flex_row()
                                .items_center()
                                .gap(px(S2))
                                .child(text(p.name.clone()).text_size(px(TEXT_BASE)).font_weight(FontWeight(500)))
                                .child(text(p.plan.clone().unwrap_or_default()).text_size(px(TEXT_SM)).text_color(FG_MUTED)),
                        );
                        if let Some(e) = p.error.clone() {
                            c = c.child(text(e).text_size(px(TEXT_SM)).text_color(DANGER));
                        }
                        for (label, pct, resets) in p.windows {
                            let used = pct.unwrap_or(0.0).clamp(0.0, 100.0);
                            let tone = if used >= 90.0 { DANGER } else if used >= 70.0 { WARNING } else { ACCENT };
                            c = c.child(
                                view()
                                    .gap(px(5.0))
                                    .child(
                                        view()
                                            .flex_row()
                                            .child(text(label).text_size(px(TEXT_SM)).flex_1())
                                            .child(text(format!("{used:.0}%")).text_size(px(TEXT_SM)).font_family(|| MONO).text_color(FG_MUTED)),
                                    )
                                    .child(
                                        view().h(px(6.0)).rounded(px(R_FULL)).bg(SURFACE3).child(
                                            view().h_full().w(relative(used as f32 / 100.0)).rounded(px(R_FULL)).bg(tone),
                                        ),
                                    )
                                    .when_some(resets, |v, r| {
                                        v.child(text(tf("Resets {}", &[&r.get(..16).unwrap_or(&r).replace('T', " ")])).text_size(px(TEXT_XS)).text_color(FG_FAINT))
                                    }),
                            );
                        }
                        col = col.child(c);
                    }
                    scroll().max_h(px(480.0)).child(col).into_element()
                }
            }))
        }))
        .child(actions(view()))
}

fn project_settings(name: String) -> View {
    let cfg = computed(store::project_config);
    sheet(&format!("{name} · paseo.json"))
        .w(px(680.0))
        .child(text(t("Setup and teardown scripts, services and other project settings, saved in the project")).text_size(px(TEXT_SM)).text_color(FG_MUTED))
        .child(dynamic(move || {
            let loaded = cfg.with(|c| c.text.is_some());
            Some(untracked(move || {
                if !loaded {
                    return text(t("Loading…")).text_color(FG_MUTED).into_element();
                }
                view()
                    .h(px(360.0))
                    .rounded(px(R_LG))
                    .border_1()
                    .border_color(BORDER)
                    .overflow_hidden()
                    .child(
                        nsh_base::editor()
                            .value(move || cfg.with(|c| c.text.clone().map(SharedString::from)))
                            .language("json")
                            .on_change(|v: &SharedString| store::edit_project_config(v))
                            .text_size(px(TEXT_CODE))
                            .flex_1(),
                    )
                    .into_element()
            }))
        }))
        .child(dynamic(move || {
            let e = cfg.with(|c| c.error.clone())?;
            Some(untracked(move || text(e.clone()).text_size(px(TEXT_SM)).text_color(DANGER)))
        }))
        .child(actions(btn(t("Save"), Variant::Primary, Sz::Md, None, store::save_project_config)))
}

fn labels(workspace: String) -> View {
    let ws = {
        let id = workspace.clone();
        computed(move || store::workspace(&id))
    };
    let catalog = computed(store::labels);
    let name = signal(String::new());
    let color = signal("violet".to_owned());
    let ws_id = workspace.clone();
    let mut colors = view().flex_row().gap(px(6.0));
    for c in LABEL_COLORS {
        colors = colors.child(
            nsh_base::button()
                .on_press(move || color.set(c.to_owned()))
                .w(px(20.0))
                .h(px(20.0))
                .rounded(px(R_FULL))
                .bg(label_tone(c))
                .border_2()
                .border_color(move || if color.get() == c { FG } else { label_tone(c) }),
        );
    }
    sheet(t("Labels"))
        .child(dynamic(move || {
            let cat = catalog.get();
            let assigned = ws.with(|w| w.as_ref().map(|w| w.labels.clone()).unwrap_or_default());
            let ws_id = workspace.clone();
            Some(untracked(move || {
                if cat.is_empty() {
                    return text(t("No labels yet; create one below")).text_size(px(TEXT_SM)).text_color(FG_MUTED).into_element();
                }
                let mut list = view().flex_row().flex_wrap().gap(px(S2));
                for (n, c) in cat.clone() {
                    let on = assigned.contains(&n);
                    let (id, n2, c2) = (ws_id.clone(), n.clone(), c.clone());
                    list = list.child(chip(
                        n.clone(),
                        None,
                        move || on,
                        move || store::set_label(&id, &n2, &c2, !on),
                    )
                    .child(view().w(px(8.0)).h(px(8.0)).rounded(px(R_FULL)).bg(label_tone(&c))));
                }
                list.into_element()
            }))
        }))
        .child(section_label(t("New label")))
        .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).placeholder(t("Label name")))
        .child(colors)
        .child(actions(
            btn(t("Add label"), Variant::Primary, Sz::Md, Some(Icon::Plus), move || {
                let n = name.get_untracked();
                if !n.trim().is_empty() {
                    store::set_label(&ws_id, n.trim(), &color.get_untracked(), true);
                    name.set(String::new());
                }
            })
            .disabled(move || name.with(|n| n.trim().is_empty())),
        ))
}

fn add_host() -> View {
    let name = signal(String::new());
    let url = signal(String::new());
    let password = signal(String::new());
    sheet(t("Add host"))
        .w(px(520.0))
        .child(
            text(t("Paste a pairing link (Settings → Pair a device on the other machine) to connect from anywhere through the encrypted relay, or a LAN / Tailscale address, or a tunnel URL"))
                .text_size(px(TEXT_SM))
                .line_height(px(18.0))
                .text_color(FG_MUTED),
        )
        .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).placeholder(t("Name, e.g. Studio Mac")))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(px(S2))
                .child(
                    field()
                        .flex_1()
                        .value(move || url.get())
                        .on_change(move |v: &str| url.set(v.to_owned()))
                        .placeholder(t("Address (192.168.1.20:6767) or a pairing link"))
                        .font_family(|| MONO),
                )
                .child(btn(t("Paste"), Variant::Secondary, Sz::Md, Some(Icon::Copy), move || {
                    spawn(async move {
                        let text = clipboard_string().await;
                        if !text.trim().is_empty() {
                            url.set(text.trim().to_owned());
                        }
                    });
                })),
        )
        .child(
            field()
                .value(move || password.get())
                .on_change(move |v: &str| password.set(v.to_owned()))
                .masked(true)
                .placeholder(t("Service password")),
        )
        .child(
            text(t("On that machine, let the service listen beyond localhost (daemon.listen, e.g. 0.0.0.0:6767) and set a password first"))
                .text_size(px(TEXT_XS))
                .text_color(FG_FAINT),
        )
        .child(actions(
            btn(t("Connect"), Variant::Primary, Sz::Md, Some(Icon::Server), move || {
                store::add_host(&name.get_untracked(), &url.get_untracked(), &password.get_untracked());
                store::close_modal();
            })
            .disabled(move || url.with(|u| u.trim().is_empty())),
        ))
}

fn pair_device() -> View {
    let p = computed(store::pairing);
    sheet(t("Pair a device"))
        .w(px(480.0))
        .child(
            text(t("Scan with your iPhone's camera and open it in Sophon. On the same Wi-Fi it connects directly; anywhere else through the encrypted relay. Both are end to end encrypted"))
                .text_size(px(TEXT_SM))
                .line_height(px(18.0))
                .text_color(FG_MUTED),
        )
        .child(dynamic(move || {
            let st = p.get();
            Some(untracked(move || {
                if !st.loaded {
                    return text(t("Loading…")).text_color(FG_MUTED).into_element();
                }
                if let Some(e) = st.error.clone() {
                    return text(e).text_size(px(TEXT_SM)).text_color(DANGER).into_element();
                }
                let Some(link) = st.sophon_url.clone() else {
                    return text(t("Neither the LAN bridge nor the relay is available: turn on Remote relay in Settings"))
                        .text_size(px(TEXT_SM))
                        .text_color(WARNING)
                        .into_element();
                };
                let mut v = view().gap(px(S3)).items_center();
                if let Some(svg_text) = st.qr_svg.clone() {
                    v = v.child(
                        view().p(px(S2)).rounded(px(R_XL)).bg(AdaptiveColor::rgb(0xffffff, 0xffffff)).child(
                            svg(nsh_core::element::ImageSrc::svg_markup(&svg_text)).w(px(240.0)).h(px(240.0)),
                        ),
                    );
                }
                let mut ways = view().flex_row().flex_wrap().gap(px(S2)).justify_center();
                if let Some(addr) = store::bridge_addr() {
                    ways = ways.child(badge(tf("Same Wi-Fi · {}", &[&addr]), SUCCESS));
                }
                if st.relay_enabled {
                    ways = ways.child(badge(t("Anywhere · relay"), ACCENT));
                }
                let (sophon_link, paseo_link) = (link.clone(), st.url.clone());
                let mut buttons = view().flex_row().gap(px(S2)).child(btn(t("Copy link"), Variant::Secondary, Sz::Md, Some(Icon::Copy), move || {
                    set_clipboard(sophon_link.clone());
                    store::notify(t("Pairing link copied").into());
                }));
                if let Some(pl) = paseo_link.filter(|_| st.relay_enabled) {
                    buttons = buttons.child(btn(t("Copy relay link"), Variant::Ghost, Sz::Md, None, move || {
                        set_clipboard(pl.clone());
                        store::notify(t("Relay pairing link copied").into());
                    }));
                }
                v.child(ways).child(buttons).into_element()
            }))
        }))
        .child(actions(view()))
}
