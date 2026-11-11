//! Dialogs: a centered card over a dimmed backdrop (an adaptive modal
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

/// A picture on the phone, as a photo viewer shows one: it fills the
/// screen, a pinch zooms it (dragging moves it while zoomed), two taps zoom
/// in and back out, one tap puts it away.
fn zooming_picture(uri: String) -> AnyElement {
    const DOUBLE_TAP: std::time::Duration = std::time::Duration::from_millis(260);
    let zoom = signal(1.0f32);
    let taps = std::rc::Rc::new(std::cell::Cell::new(0u32));
    scroll()
        .flex_1()
        .zoom_range(1.0, 5.0)
        .zoom(zoom)
        .center_content(true)
        .scrollbar(false)
        .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.92))
        .child(
            nsh_base::button()
                .on_press(move || {
                    taps.set(taps.get() + 1);
                    if taps.get() >= 2 {
                        taps.set(0);
                        zoom.set(if zoom.get_untracked() > 1.0 { 1.0 } else { 2.5 });
                        return;
                    }
                    let taps = taps.clone();
                    set_timeout(DOUBLE_TAP, move || {
                        if taps.get() == 1 {
                            taps.set(0);
                            store::close_modal();
                        }
                    });
                })
                .accessibility_label(t("Close"))
                .w(move || Length::from(px(window().width.0)))
                .h(move || Length::from(px(window().height.0)))
                .child(image(SharedString::from(uri)).flex_1().object_fit(nsh_core::ResizeMode::Contain)),
        )
        .into_element()
}

/// A picture on the desktop: a trackpad pinch (Control + wheel with a
/// mouse) zooms it, a drag moves it, and the bar under it zooms by steps,
/// turns it a quarter, mirrors it and puts it back as it was. A press
/// puts it away.
fn desktop_picture(uri: String) -> AnyElement {
    use nsh_core::element::{GestureEvent, Interactive, Spring};
    const STEP: f32 = 1.25;
    const LIMITS: (f32, f32) = (0.25, 8.0);
    let [x, y, scale, angle] = [0.0, 0.0, 1.0, 0.0].map(animated_value);
    // Where the bar's steps start from: the scale, and the quarter turns.
    let level = std::rc::Rc::new(std::cell::Cell::new(1.0f32));
    let turns = std::rc::Rc::new(std::cell::Cell::new(0.0f32));
    let mirrored = signal(false);
    let zoom_to = {
        let level = level.clone();
        move |to: f32| {
            let to = to.clamp(LIMITS.0, LIMITS.1);
            level.set(to);
            let _ = scale.spring(to, Spring::default());
            if to <= 1.0 {
                let _ = x.spring(0.0, Spring::default());
                let _ = y.spring(0.0, Spring::default());
            }
        }
    };
    let white = AdaptiveColor::rgb(0xffffff, 0xffffff);
    let tool = move |glyph: Icon, label: &'static str, on_press: Box<dyn Fn()>| {
        nsh_base::button()
            .on_press(move || on_press())
            .accessibility_label(label)
            .w(px(32.0))
            .h(px(32.0))
            .rounded(px(R_FULL))
            .items_center()
            .justify_center()
            .hover(|s| s.bg(AdaptiveColor::rgb(0xffffff, 0xffffff).opacity(0.16)))
            .child(icon(glyph, 16.0).text_color(white))
    };
    let (zoom_in, zoom_out, reset_zoom, pinched) = (zoom_to.clone(), zoom_to.clone(), zoom_to.clone(), zoom_to);
    let (level_in, level_out, level_pinch) = (level.clone(), level.clone(), level.clone());
    let (turns_turn, turns_reset) = (turns.clone(), turns);
    view()
        .flex_1()
        .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.55))
        .child(
            view()
                .on_press(|_: &PressEvent| store::close_modal())
                .flex_1()
                .overflow_hidden()
                .pan_values(Some(&x), Some(&y))
                .pinch_value(&scale)
                .on_pinch_end(move |e: &GestureEvent| pinched(level_pinch.get() * e.value))
                .child(
                    view()
                        .flex_1()
                        .m(px(S4))
                        .bind_translate_x(&x)
                        .bind_translate_y(&y)
                        .bind_scale(&scale)
                        .bind_rotate(&angle)
                        .child(
                            image(SharedString::from(uri))
                                .flex_1()
                                .object_fit(nsh_core::ResizeMode::Contain)
                                .scale_x(move || if mirrored.get() { -1.0_f32 } else { 1.0 }),
                        ),
                ),
        )
        .child(
            view().absolute().bottom(px(20.0)).left(px(0.0)).right(px(0.0)).items_center().pointer_events(nsh_core::PointerEvents::BoxNone).child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(2.0))
                    .p(px(4.0))
                    .rounded(px(R_FULL))
                    .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.6))
                    .child(tool(Icon::Minus, t("Zoom out"), Box::new(move || zoom_out(level_out.get() / STEP))))
                    .child(tool(Icon::Plus, t("Zoom in"), Box::new(move || zoom_in(level_in.get() * STEP))))
                    .child(tool(
                        Icon::RotateCw,
                        t("Rotate"),
                        Box::new(move || {
                            turns_turn.set(turns_turn.get() + 1.0);
                            let _ = angle.spring(turns_turn.get() * 90.0, Spring::default());
                        }),
                    ))
                    .child(tool(Icon::FlipHorizontal, t("Flip"), Box::new(move || mirrored.update(|m| *m = !*m))))
                    .child(tool(
                        Icon::Maximize2,
                        t("Actual fit"),
                        Box::new(move || {
                            reset_zoom(1.0);
                            turns_reset.set(0.0);
                            let _ = angle.spring(0.0, Spring::default());
                            mirrored.set(false);
                        }),
                    )),
            ),
        )
        .into_element()
}

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
                    .child(
                        // The backdrop: a press outside the card closes it.
                        nsh_base::button()
                            .on_press(store::close_modal)
                            .absolute()
                            .top(px(0.0))
                            .bottom(px(0.0))
                            .left(px(0.0))
                            .right(px(0.0))
                            .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.32)),
                    )
                    .child(dynamic(|| {
                        let m = store::modal()?;
                        Some(untracked(move || {
                            if let Modal::Picture { uri, .. } = &m {
                                // A picture takes the whole window: itself,
                                // and a small cross to put it away (a press
                                // anywhere does too).
                                view()
                                    .flex_1()
                                    .self_stretch()
                                    .child(if MOBILE {
                                        zooming_picture(uri.clone())
                                    } else {
                                        desktop_picture(uri.clone())
                                    })
                                    .child(
                                        nsh_base::button()
                                            .on_press(store::close_modal)
                                            .accessibility_label(t("Close"))
                                            .absolute()
                                            .top(px(S4))
                                            .right(px(S4))
                                            .w(px(30.0))
                                            .h(px(30.0))
                                            .rounded(px(R_FULL))
                                            .items_center()
                                            .justify_center()
                                            .bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.45))
                                            .hover(|s| s.bg(AdaptiveColor::rgb(0x000000, 0x000000).opacity(0.7)))
                                            .child(icon(Icon::X, 16.0).text_color(AdaptiveColor::rgb(0xffffff, 0xffffff))),
                                    )
                            } else if m == Modal::Palette {
                                view().self_stretch().items_center().pt(relative(0.12)).flex_1().child(crate::ui::desktop::shortcuts::palette())
                            } else if MOBILE && matches!(m, Modal::ToolDetail { .. }) {
                                // The phone: a sheet at the foot, full width.
                                view()
                                    .flex_1()
                                    .justify_end()
                                    .pointer_events(nsh_core::PointerEvents::BoxNone)
                                    .child(body(m.clone()))
                            } else {
                                view()
                                    .flex_1()
                                    .items_center()
                                    .justify_center()
                                    .pointer_events(nsh_core::PointerEvents::BoxNone)
                                    .child(body(m.clone()))
                            }
                        }))
                    })),
            )
    })
}

pub(crate) fn sheet(title: &str) -> View {
    glass_strong(view())
        .w(px(420.0))
        .max_w(relative(0.92))
        .p(px(S6))
        .gap(px(S4))
        .rounded(px(22.0))
        .child(text(title.to_owned()).text_size(px(TEXT_LG)).font_weight(FontWeight(500)))
}

pub(crate) fn actions(primary: impl IntoElement) -> View {
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
        Modal::RenameHost { id, name } => rename(t("Rename host"), name, move |v| {
            store::rename_host(&id, v);
        }),
        // Drawn by the layer itself, over the whole window.
        Modal::Picture { .. } => view(),
        Modal::RenameMySpace { name } => rename(t("Rename space"), name, store::rename_my_space),
        Modal::RemoveSpace { id, name, folder } => sheet(t("Remove space"))
            .child(
                text(tf("“{}” leaves Sophon and its service stops. Its projects and conversations stay in {}, to delete by hand or to bring back", &[&name, &folder]))
                    .text_size(px(TEXT_BASE))
                    .line_height(px(20.0))
                    .text_color(FG_MUTED),
            )
            .child(actions(btn(t("Remove"), Variant::Destructive, Sz::Md, Some(Icon::Trash2), move || {
                store::remove_host(&id);
                store::close_modal();
            }))),
        Modal::Commit => commit(),
        Modal::Palette => view(),
        Modal::NewSchedule => schedule_form(None),
        Modal::EditSchedule(s) => schedule_form(Some(s)),
        Modal::AddProject => add_project(),
        Modal::SwitchBranch => switch_branch(),
        Modal::ImportSession => import_session(),
        Modal::AddAgentAccount => crate::ui::shared::agents::add_account(),
        Modal::AddAgentKey => crate::ui::shared::agents::add_api_key(),
        Modal::RemoveAgent { id, label } => crate::ui::shared::agents::remove_agent(id, label),
        Modal::AddHost => add_host(),
        Modal::CreateSpace => create_space(),
        Modal::PairDevice => pair_device(),
        Modal::ApprovePairing { device, code, .. } => approve_pairing(device, code),
        Modal::PairNearby => pair_nearby(),
        Modal::RevokeDevices => sheet(t("Unpair every device"))
            .child(
                text(t("Every phone and computer paired with this Mac, by QR code or on the LAN, must pair again. Connected ones are cut off now"))
                    .text_size(px(TEXT_BASE))
                    .line_height(px(20.0))
                    .text_color(FG_MUTED),
            )
            .child(actions(btn(t("Unpair all"), Variant::Destructive, Sz::Md, Some(Icon::Trash2), || {
                crate::lan::revoke_all();
                store::close_modal();
            }))),
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
        // The phone's tool sheet: tight margins, the detail taking the room;
        // a press outside or the handle's swipe-free close puts it away.
        Modal::ToolDetail { name, kind, detail, error } => glass_strong(view())
            .w_full()
            .max_h(relative(0.8))
            .px(px(S4))
            .pt(px(S2))
            .pb_safe_area_plus(px(S3))
            .gap(px(S2))
            .rounded_t(px(22.0))
            .child(view().items_center().child(view().w(px(36.0)).h(px(5.0)).rounded(px(R_FULL)).bg(tint(FG, 0.18))))
            .child(text(name).text_size(px(TEXT_LG)).font_weight(FontWeight(600)).number_of_lines(1))
            .child(scroll().flex_shrink(1.0).child(crate::ui::shared::tool_detail::tool_body(&kind, &detail, error.as_deref()))),
        Modal::Update => update_dialog(),
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

/// How a new schedule repeats, as the form asks it.
#[derive(Clone, Copy, PartialEq)]
enum Repeat {
    Every,
    Daily,
    Weekly,
    Custom,
}

/// The form of a schedule: a new one, or `existing` to change.
fn schedule_form(existing: Option<crate::daemon::connection::Schedule>) -> View {
    use crate::cadence::{self, Picked};
    let editing = existing.as_ref().map(|s| s.id.clone());
    // One that messages an agent it already has: no folder to pick.
    let no_folder = existing.as_ref().is_some_and(|s| s.cwd.is_empty());
    // What an existing one repeats on, back in the form's choices.
    let was = existing.as_ref().map(|s| cadence::picked(&s.expression));
    let first_project = || store::projects().into_iter().map(|p| p.root_path).find(|p| !p.is_empty()).unwrap_or_default();
    let name = signal(existing.as_ref().map(|s| s.given_name.clone()).unwrap_or_default());
    let prompt = signal(existing.as_ref().map(|s| s.prompt.clone()).unwrap_or_default());
    let path = signal(existing.as_ref().map(|s| s.cwd.clone()).filter(|c| !c.is_empty()).unwrap_or_else(first_project));
    let worktree = signal(existing.as_ref().is_some_and(|s| s.worktree));
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
    let is_edit = editing.is_some();
    let create = move || {
        let p = prompt.get_untracked();
        let Some(cron) = expression.get_untracked() else { return };
        if p.trim().is_empty() || (!no_folder && path.get_untracked().is_empty()) {
            return;
        }
        let path = if no_folder { String::new() } else { path.get_untracked() };
        let (name, worktree) = (name.get_untracked(), worktree.get_untracked());
        match &editing {
            Some(id) => store::update_schedule(id, name, p, cron, path, worktree),
            None => store::create_schedule(name, p, cron, path, worktree),
        }
        store::close_modal();
    };
    let mut projects = view().flex_row().flex_wrap().gap(px(S2));
    for p in store::projects().into_iter().filter(|p| !p.root_path.is_empty()) {
        let (a, b) = (p.root_path.clone(), p.root_path.clone());
        projects = projects.child(chip(p.name.clone(), Some(Icon::Folder), move || path.get() == a, move || path.set(b.clone())));
    }
    // A few characters beside a segmented control: as tall and as round as
    // it, so the row reads as one line of controls.
    let short = |width: f32| field().w(px(width)).h(px(CONTROL_SM + 4.0)).rounded(px(R_FULL)).text_size(px(TEXT_SM)).text_center();
    let lead = |label: &'static str| text(label).text_size(px(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED);
    let time_row = move || {
        view()
            .flex_row()
            .items_center()
            .gap(px(S2))
            .child(lead(t("At")))
            .child(short(76.0).value(move || time.get()).on_change(move |v: &str| time.set(v.to_owned())).placeholder("9:00"))
    };
    // What was picked, said back: the check that it was understood.
    let summary = move || -> SharedString {
        match expression.get() {
            Some(cron) if repeat.get() == Repeat::Custom && cadence::describe(&cron).starts_with("cron ") => t("Runs on this cron expression").into(),
            Some(cron) => format!("{}{}", t("Runs: "), cadence::describe(&cron)).into(),
            None => match repeat.get() {
                Repeat::Custom => t("Five fields: minute, hour, day, month, weekday").into(),
                Repeat::Every => t("Type a number, like 45").into(),
                Repeat::Weekly if days.with(Vec::is_empty) => t("Pick at least one day").into(),
                _ => t("Type a time, like 9:30 or 18:00").into(),
            },
        }
    };
    sheet(if is_edit { t("Edit schedule") } else { t("New schedule") })
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
        .when(!no_folder, |v| {
            v.child(section_label(t("Project")))
                .child(projects)
                .child(segmented(vec![(false, t("Local").into()), (true, t("Worktree").into())], move || worktree.get(), move |w| worktree.set(w)))
        })
        .child(section_label(t("Repeats")))
        .child(segmented(
            vec![
                (Repeat::Every, t("Every so often").into()),
                (Repeat::Daily, t("Every day").into()),
                (Repeat::Weekly, t("Every week").into()),
                (Repeat::Custom, t("Custom").into()),
            ],
            move || repeat.get(),
            move |r| repeat.set(r),
        ))
        .child(dynamic(move || {
            let r = repeat.get();
            Some(untracked(move || match r {
                Repeat::Every => view()
                    .flex_row()
                    .items_center()
                    .gap(px(S2))
                    .child(lead(t("Every")))
                    .child(short(56.0).value(move || count.get()).on_change(move |v: &str| count.set(v.to_owned())).placeholder("1"))
                    .child(segmented(
                        vec![(1u32, t("Minutes").into()), (60, t("Hours").into()), (1440, t("Days").into())],
                        move || unit.get(),
                        move |u| unit.set(u),
                    )),
                Repeat::Daily => time_row(),
                Repeat::Weekly => {
                    let zh = crate::i18n::is_zh();
                    let mut row = view().flex_row().flex_wrap().gap(px(S2));
                    // Monday first.
                    for d in [1u8, 2, 3, 4, 5, 6, 0] {
                        row = row.child(chip(
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
                    view().gap(px(S3)).child(row).child(time_row())
                }
                Repeat::Custom => view().child(
                    field()
                        .value(move || custom.get())
                        .on_change(move |v: &str| custom.set(v.to_owned()))
                        .font_family(|| MONO)
                        .placeholder(t("Cron expression, e.g. 0 9 * * 1-5")),
                ),
            }))
        }))
        .child(text(summary).text_size(px(TEXT_SM)).text_color(FG_MUTED))
        .child(actions(
            btn(if is_edit { t("Save") } else { t("Create") }, Variant::Primary, Sz::Md, Some(Icon::CalendarClock), create)
                .disabled(move || prompt.with(|p| p.trim().is_empty()) || (!no_folder && path.with(String::is_empty)) || expression.with(Option::is_none)),
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
        store::spawn_detached(async move {
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
                        store::spawn_detached(async move {
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

/// Names a new space: its own projects and conversations on this Mac.
fn create_space() -> View {
    let name = signal(String::new());
    let create = move || {
        let n = name.get_untracked();
        if !n.trim().is_empty() {
            store::create_space(&n);
            store::close_modal();
        }
    };
    sheet(t("New space"))
        .child(
            text(t("A space has its own projects, conversations and schedules on this Mac, for another person or another kind of work. Its agents can still open any folder here"))
                .text_size(px(TEXT_SM))
                .line_height(px(18.0))
                .text_color(FG_MUTED),
        )
        .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).on_submit(move |_: &str| create()).placeholder(t("Name, e.g. Lin")).auto_focus(true))
        .child(actions(
            btn(t("Create"), Variant::Primary, Sz::Md, Some(Icon::FolderPlus), create).disabled(move || name.with(|n| n.trim().is_empty())),
        ))
}

fn add_host() -> View {
    let name = signal(String::new());
    let url = signal(String::new());
    let password = signal(String::new());
    sheet(t("Add host"))
        .w(px(520.0))
        .child(nearby_list())
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

/// The six digits of a pairing session, large, to compare.
fn pairing_code(code: &str) -> View {
    view()
        .self_center()
        .px(px(S6))
        .py(px(S3))
        .rounded(px(R_LG))
        .bg(SURFACE2)
        .child(text(code.to_owned()).text_size(px(32.0)).font_weight(FontWeight(500)).font_family(|| MONO))
}

/// This Mac: a phone nearby asks to pair, into one of its spaces.
fn approve_pairing(device: String, code: String) -> View {
    let space = signal(None::<String>);
    sheet(t("Pairing request"))
        .child(
            text(tf("“{}” on this network asks to connect to this Mac. Allowed, it can use your agents here, from anywhere", &[&device]))
                .text_size(px(TEXT_BASE))
                .line_height(px(20.0))
                .text_color(FG_MUTED),
        )
        .child(pairing_code(&code))
        .child(text(t("Allow only if the device shows the same code")).text_size(px(TEXT_SM)).text_color(FG_MUTED).self_center())
        .child(space_choice(space))
        .child(
            view()
                .flex_row()
                .justify_end()
                .gap(px(S2))
                .child(btn(t("Deny"), Variant::Secondary, Sz::Md, None, || crate::lan::answer_pairing(false, None)))
                .child(btn(t("Allow"), Variant::Primary, Sz::Md, Some(Icon::Check), move || {
                    crate::lan::answer_pairing(true, space.get_untracked())
                })),
        )
}

/// Which space on this Mac a device joins, when there is more than one.
fn space_choice(space: Signal<Option<String>>) -> AnyElement {
    let spaces = store::spaces();
    if spaces.is_empty() {
        return view().into_element();
    }
    let mut options: Vec<(Option<String>, SharedString)> = vec![(None, t("My space").into())];
    options.extend(spaces.into_iter().map(|h| (Some(h.id.clone()), h.label().into())));
    view()
        .gap(px(S2))
        .child(text(t("Joins the space")).text_size(px(TEXT_SM)).text_color(FG_MUTED))
        .child(segmented(options, move || space.get(), move |v| space.set(v)))
        .into_element()
}

/// A phone (or another Mac): pairing with a computer nearby.
fn pair_nearby() -> View {
    let p = computed(crate::lan::pairing);
    sheet(t("Pair with a computer")).child(dynamic(move || {
        let st = p.get();
        Some(untracked(move || {
            let mut v = view().gap(px(S4));
            if let Some(e) = &st.error {
                v = v.child(text(e.clone()).text_size(px(TEXT_BASE)).text_color(DANGER));
            } else if let Some(code) = &st.code {
                v = v
                    .child(
                        text(tf("Allow it on “{}”. The code there must be the same as this one", &[&st.name]))
                            .text_size(px(TEXT_BASE))
                            .line_height(px(20.0))
                            .text_color(FG_MUTED),
                    )
                    .child(pairing_code(code));
            } else {
                v = v.child(text(tf("Connecting to “{}”…", &[&st.name])).text_size(px(TEXT_BASE)).text_color(FG_MUTED));
            }
            v.child(view().flex_row().justify_end().child(btn(
                if st.error.is_some() { t("Close") } else { t("Cancel") },
                Variant::Secondary,
                Sz::Md,
                None,
                store::close_modal,
            )))
            .into_element()
        }))
    }))
}

/// The computers on this network: one paired already connects at once,
/// another asks to pair. Searches when shown.
pub(crate) fn nearby_list() -> View {
    crate::lan::scan();
    view().gap(px(S2)).child(dynamic(|| {
        let list = crate::lan::nearby();
        let scanning = crate::lan::scanning();
        let scanned = crate::lan::scanned();
        Some(untracked(move || {
            let head = view()
                .flex_row()
                .items_center()
                .gap(px(S2))
                .child(text(t("On this network")).text_size(px(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED).flex_1())
                .child(if scanning {
                    text(t("Searching…")).text_size(px(TEXT_SM)).text_color(FG_FAINT).into_element()
                } else {
                    icon_btn(Icon::RotateCw, Sz::Sm, t("Search again"), crate::lan::scan).into_element()
                });
            let mut c = card();
            if list.is_empty() {
                let note = if scanned && !scanning { t("No computer running Sophon found on this network") } else { t("Searching…") };
                c = c.child(view().px(px(S4)).py(px(S3)).child(text(note).text_size(px(TEXT_SM)).text_color(FG_MUTED)));
            }
            for (i, n) in list.into_iter().enumerate() {
                let paired = n.host.is_some();
                let name = n.name.clone();
                c = c.child(
                    nsh_base::button()
                        .on_press(move || crate::lan::connect(n.clone()))
                        .flex_row()
                        .items_center()
                        .gap(px(S3))
                        .h(px(52.0))
                        .px(px(S4))
                        .when(i > 0, |v| v.border_t_1().border_color(BORDER))
                        .active(|s| s.bg(highlight()))
                        .hover(|s| s.bg(highlight()))
                        .child(icon(Icon::Monitor, 18.0).text_color(FG_MUTED))
                        .child(text(name).text_size(px(TEXT_BASE)).number_of_lines(1).flex_1())
                        .child(text(if paired { t("Paired") } else { t("Pair") }).text_size(px(TEXT_SM)).text_color(if paired { FG_MUTED } else { ACCENT })),
                );
            }
            view().gap(px(S2)).child(head).child(c).into_element()
        }))
    }))
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
                let (sophon_link, daemon_link) = (link.clone(), st.url.clone());
                let mut buttons = view().flex_row().gap(px(S2)).child(btn(t("Copy link"), Variant::Secondary, Sz::Md, Some(Icon::Copy), move || {
                    set_clipboard(sophon_link.clone());
                    store::notify(t("Pairing link copied").into());
                }));
                if let Some(pl) = daemon_link.filter(|_| st.relay_enabled) {
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

/// What changed since the running version, release by release, and a
/// restart into the new one (already downloaded in the background).
fn update_dialog() -> View {
    use crate::update::{self, Phase};
    let st = update::state();
    let version = st.version.clone().unwrap_or_default();
    let mut changes = st.changes.clone();
    if changes.is_empty() {
        changes.push((version.clone(), st.notes.clone().unwrap_or_default()));
    }
    let mut list = view().px(px(S6)).py(px(S4)).gap(px(S4));
    for (i, (v, notes)) in changes.into_iter().enumerate() {
        if i > 0 {
            list = list.child(view().h(px(1.0)).bg(BORDER));
        }
        let notes = if notes.trim().is_empty() { t("No notes for this release").to_owned() } else { notes };
        list = list.child(
            view()
                .gap(px(S2))
                .child(text(format!("v{v}")).text_size(px(TEXT_LG)).font_weight(FontWeight(600)))
                .child(
                    nsh_base::text_view()
                        .markdown(move || Some(SharedString::from(notes.clone())))
                        .selectable(true)
                        .gap_2()
                        .text_size(px(TEXT_BASE))
                        .line_height(px(22.0))
                        .text_color(FG),
                ),
        );
    }
    let state = computed(update::state);
    let primary = dynamic(move || {
        let (phase, can_install) = state.with(|s| (s.phase.clone(), s.can_install));
        Some(untracked(move || {
            let (label, icon_) = match phase {
                _ if !can_install => (t("Download").to_owned(), Icon::ExternalLink),
                Phase::Installing => (t("Installing…").to_owned(), Icon::RotateCw),
                Phase::Failed(_) => (t("Retry").to_owned(), Icon::RotateCw),
                _ => (t("Restart to update").to_owned(), Icon::RotateCw),
            };
            btn(label, Variant::Primary, Sz::Md, Some(icon_), update::install).into_element()
        }))
    });
    let failed = computed(move || state.with(|s| match &s.phase {
        Phase::Failed(e) => Some(e.clone()),
        _ => None,
    }));
    glass_strong(view())
        .w(px(620.0))
        .max_w(relative(0.92))
        .max_h(relative(0.86))
        .rounded(px(22.0))
        .overflow_hidden()
        .child(
            view()
                .flex_row()
                .items_baseline()
                .gap(px(S3))
                .px(px(S6))
                .pt(px(S6))
                .pb(px(S4))
                .border_b_1()
                .border_color(BORDER)
                .child(text(tf("Update to {}", &[&format!("v{version}")])).text_size(px(TEXT_LG)).font_weight(FontWeight(600)))
                .child(text(tf("Changes since {}", &[&format!("v{}", st.current)])).text_size(px(TEXT_SM)).text_color(FG_MUTED).flex_1().number_of_lines(1)),
        )
        .child(scroll().flex_shrink(1.0).child(list))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_end()
                .gap(px(S2))
                .px(px(S6))
                .py(px(S4))
                .border_t_1()
                .border_color(BORDER)
                .child(dynamic(move || {
                    let e = failed.get()?;
                    Some(untracked(move || text(e.clone()).text_size(px(TEXT_SM)).text_color(DANGER).flex_1().number_of_lines(2).into_element()))
                }))
                .child(btn(t("Later"), Variant::Ghost, Sz::Md, None, store::close_modal))
                .child(primary),
        )
}
