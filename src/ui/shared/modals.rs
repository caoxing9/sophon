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
    // The wheel (macOS's monitor, `platform::picture_input`) zooms from
    // where the zoom is; it hears only while a picture is open.
    #[cfg(target_os = "macos")]
    {
        crate::platform::picture_input::set_open(true);
        cleanup(|| crate::platform::picture_input::set_open(false));
    }
    let (wheel_zoom, level_wheel) = (zoom_to.clone(), level.clone());
    let seen = std::cell::Cell::new(store::picture_zoom().0);
    effect(move || {
        let store::PictureZoom(n, factor) = store::picture_zoom();
        if seen.replace(n) != n {
            wheel_zoom(level_wheel.get() * factor);
        }
    });
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
                        .m(sp(S4))
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

/// Whether `m` is one of the modals that are no dialog: a picture over the
/// whole window, the palettes, a tool's details on the phone.
fn own_layer(m: &Modal) -> bool {
    matches!(m, Modal::Picture { .. } | Modal::Palette | Modal::ChangedFiles) || (MOBILE && matches!(m, Modal::ToolDetail { .. }))
}

pub(crate) fn modal_layer() -> impl IntoElement {
    let dialog_open = computed(|| store::modal().is_some_and(|m| !own_layer(&m)));
    let layer_open = computed(|| store::modal().is_some_and(|m| own_layer(&m)));
    view()
        .absolute()
        .inset(px(0.0))
        .pointer_events(nsh_core::PointerEvents::BoxNone)
        // The modals that are dialogs: nsh-shadcn's Dialog, as wide as
        // each one's content says.
        .child(
            nsh_shadcn::dialog()
                .open(move || Some(dialog_open.get()))
                .on_open_change(|open: &bool| {
                    if !*open {
                        store::close_modal();
                    }
                })
                .child(
                    nsh_shadcn::dialog_content()
                        .app_class(Some(SharedString::from("flex flex-col w-auto sm:max-w-none")))
                        .child(dynamic(|| {
                            let m = store::modal().filter(|m| !own_layer(m))?;
                            Some(untracked(move || body(m).into_element()))
                        })),
                ),
        )
        .child(when(layer_open, || {
            dialog()
                .animation(ModalAnimation::Fade)
                .on_cancel(store::close_modal)
                .flex_1()
                .child(
                    view()
                        .flex_1()
                        .child(
                            // The backdrop: a press outside closes it.
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
                            let m = store::modal().filter(own_layer)?;
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
                                                .top(sp(S4))
                                                .right(sp(S4))
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
                                } else if m == Modal::ChangedFiles {
                                    view().self_stretch().items_center().pt(relative(0.12)).flex_1().child(crate::ui::desktop::shortcuts::changed_files())
                                } else {
                                    // The phone's tool details: a sheet at the
                                    // foot, full width.
                                    view()
                                        .flex_1()
                                        .justify_end()
                                        .pointer_events(nsh_core::PointerEvents::BoxNone)
                                        .child(body(m.clone()))
                                }
                            }))
                        })),
                )
        }))
}

/// A dialog's body under its title: nsh-shadcn's DialogHeader and
/// DialogTitle, in a column as wide as the dialog is to be (the
/// DialogContent around it pads and frames it).
pub(crate) fn sheet(title: &str) -> View {
    view()
        .w(px(420.0))
        .max_w(relative(0.92))
        .gap(sp(S4))
        .child(nsh_shadcn::dialog_header().child(nsh_shadcn::dialog_title(title.to_owned())))
}

pub(crate) fn actions(primary: impl IntoElement) -> View {
    view()
        .flex_row()
        .justify_end()
        .gap(sp(S2))
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
                    .text_size(tx(TEXT_BASE))
                    .line_height(px(20.0))
                    .text_color(FG_MUTED()),
            )
            .child(actions(btn(t("Remove"), Variant::Destructive, Sz::Md, Some(Icon::Trash2), move || {
                store::remove_host(&id);
                store::close_modal();
            }))),
        Modal::SpaceCap { home, name } => space_cap(home, name),
        Modal::DeviceFolder { id, name, folder } => device_folder(id, name, folder),
        Modal::Mcp { cwd } => mcp_panel(cwd),
        Modal::Commit => commit(),
        Modal::ReportIssue => report_issue(),
        Modal::Palette | Modal::ChangedFiles => view(),
        Modal::PluginChoices { plugin, param, draft } => plugin_choices(plugin, param, draft),
        Modal::RemovePlugin { id, name } => sheet(t("Remove integration"))
            .child(
                text(tf("“{}” goes, and its sign-in and settings with it. To only stop it for now, turn it off", &[&name]))
                    .text_size(tx(TEXT_BASE))
                    .line_height(px(20.0))
                    .text_color(FG_MUTED()),
            )
            .child(actions(btn(t("Remove"), Variant::Destructive, Sz::Md, Some(Icon::Trash2), move || {
                store::remove_plugin(&id);
                store::close_modal();
            }))),
        Modal::HandOn { id } => hand_on(id),
        Modal::TakeIn(file) => take_in(file),
        Modal::AddProject => add_project(),
        Modal::ChooseFolder => sheet(t("Choose a project folder")).w(px(520.0)).child(host_folders("Use “{}”", |path| {
            store::edit_form(|f| {
                f.path = path;
                f.worktree = false;
            });
            store::close_modal();
        })),
        Modal::SwitchBranch => switch_branch(),
        Modal::ImportSession => import_session(),
        Modal::Fonts { mono } => fonts(mono),
        Modal::AddProvider => crate::ui::shared::agents::add_provider(),
        Modal::RemoveAgent { id, label } => crate::ui::shared::agents::remove_agent(id, label),
        Modal::AddHost => add_host(),
        Modal::CreateSpace => create_space(),
        Modal::PairDevice => pair_device(),
        Modal::ApprovePairing { device, code, .. } => approve_pairing(device, code),
        Modal::PairNearby => pair_nearby(),
        Modal::RevokeDevices => sheet(t("Unpair every device"))
            .child(
                text(t("Every phone and computer paired with this Mac, by QR code or on the LAN, must pair again. Connected ones are cut off now"))
                    .text_size(tx(TEXT_BASE))
                    .line_height(px(20.0))
                    .text_color(FG_MUTED()),
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
        Modal::CloseElsewhere { session } => sheet(t("Close it there?"))
            .child(
                text(tf("Claude Code's background session {} stops: what it is doing there ends. Its conversation is kept, and goes on here", &[&crate::elsewhere::short(&session)]))
                    .text_size(tx(TEXT_BASE))
                    .line_height(px(20.0))
                    .text_color(FG_MUTED()),
            )
            .child(actions(btn(t("Close it"), Variant::Destructive, Sz::Md, None, move || {
                crate::elsewhere::close(&session);
                store::close_modal();
            }))),
        Modal::RestartDaemon => sheet(t("Restart service"))
            .child(
                text(t("Running agents stop and resume when the service is back, if resume on startup is on. Terminals keep running"))
                    .text_size(tx(TEXT_BASE))
                    .line_height(px(20.0))
                    .text_color(FG_MUTED()),
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
            .px(sp(S4))
            .pt(sp(S2))
            .pb_safe_area_plus(sp(S3))
            .gap(sp(S2))
            .rounded_t(px(22.0))
            .child(view().items_center().child(view().w(px(36.0)).h(px(5.0)).rounded(px(R_FULL)).bg(tint(FG(), 0.18))))
            .child(text(name).text_size(tx(TEXT_LG)).font_weight(FontWeight(600)).number_of_lines(1))
            .child(scroll().flex_shrink(1.0).child(crate::ui::shared::tool_detail::tool_body(&kind, &detail, error.as_deref()))),
        Modal::Update => update_dialog(),
        Modal::RenameProject { id, name } => rename(t("Rename project"), name, move |v| store::rename_project(&id, v)),
        Modal::ArchiveWorkspace { id, name } => sheet(t("Archive workspace"))
            .child(
                text(format!(
                    "“{name}” and its agents move to the archive. A worktree's files are removed from disk; commit or push anything you want to keep"
                ))
                .text_size(tx(TEXT_BASE))
                .line_height(px(20.0))
                .text_color(FG_MUTED()),
            )
            .child(actions(btn(t("Archive"), Variant::Destructive, Sz::Md, None, move || {
                store::archive_workspace(&id);
                store::close_modal();
            }))),
        // Spending a reset credit cannot be undone: said, and asked first.
        Modal::UseReset { provider, option } => {
            let name = if provider == "codex" { "Codex" } else { "Claude" };
            let mut body = String::new();
            if let Some(d) = option.description.as_deref().filter(|d| !d.is_empty()) {
                body.push_str(d);
                body.push_str("\n\n");
            }
            body.push_str(&crate::i18n::tf("This uses 1 of your {} {} reset credits ({}). It cannot be undone.", &[
                &option.count.to_string(),
                name,
                &option.label,
            ]));
            sheet(t("Use a reset credit?"))
                .child(text(body).text_size(tx(TEXT_BASE)).line_height(px(20.0)).text_color(FG_MUTED()))
                .child(actions(btn(t("Use credit"), Variant::Primary, Sz::Md, None, move || {
                    store::use_reset(&provider, option.credit_id.clone());
                    store::close_modal();
                })))
        }
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

/// A shared space's monthly cap: USD a month, or none. Past it, the
/// devices that reach the space through this Mac send nothing there until
/// the next month (`space_limits`).
fn space_cap(home: String, name: String) -> View {
    let (current, spend) = (crate::space_limits::caps().get(&home).copied(), crate::space_limits::spend(&home));
    let value = signal(current.map(|c| format!("{c}")).unwrap_or_default());
    let error = signal(None::<&'static str>);
    let apply = move |cap: Option<f64>| {
        crate::space_limits::set_cap(&home, cap);
        store::close_modal();
    };
    let save = {
        let apply = apply.clone();
        move || {
            let text = value.get_untracked();
            match text.trim().trim_start_matches('$').parse::<f64>() {
                Ok(c) if c.is_finite() && c > 0.0 => apply(Some(c)),
                _ => error.set(Some(t("A limit is an amount of dollars, more than 0"))),
            }
        }
    };
    let save2 = save.clone();
    let spent = spend.map(|s| tf("Used this month: ${}", &[&format!("{:.2}", s.spent)]));
    let mut primary = view().flex_row().justify_end().gap(sp(S2));
    if current.is_some() {
        let lift = apply.clone();
        primary = primary.child(btn(t("Remove limit"), Variant::Secondary, Sz::Md, None, move || lift(None)));
    }
    sheet(&tf("Monthly limit of “{}”", &[&name]))
        .child(
            text(t("What the conversations of this space may cost a calendar month, at API list prices. Past it, the devices it is shared with cannot send messages or start conversations there until the next month; reading stays, and this Mac is not limited."))
                .text_size(tx(TEXT_SM))
                .line_height(px(19.0))
                .text_color(FG_MUTED()),
        )
        .when_some(spent, |v, s| v.child(text(s).text_size(tx(TEXT_SM))))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .child(text("$").text_size(tx(TEXT_BASE)).text_color(FG_MUTED()))
                .child(
                    field()
                        .value(move || value.get())
                        .on_change(move |v: &str| {
                            value.set(v.to_owned());
                            error.set(None);
                        })
                        .on_submit(move |_: &str| save2())
                        .placeholder(t("e.g. 50"))
                        .auto_focus(true)
                        .flex_1(),
                )
                .child(text(t("a month")).text_size(tx(TEXT_SM)).text_color(FG_MUTED())),
        )
        .child(when(move || error.with(Option::is_some), move || text(move || error.get().unwrap_or_default()).text_size(tx(TEXT_SM)).text_color(DANGER)))
        .child(
            view()
                .flex_row()
                .justify_end()
                .gap(sp(S2))
                .child(btn(t("Cancel"), Variant::Secondary, Sz::Md, None, store::close_modal))
                .child(primary)
                .child(btn(t("Save"), Variant::Primary, Sz::Md, None, save)),
        )
}

/// Keeps a paired device to one folder of this Mac: picked in its folders;
/// or lets it use all of them again.
fn device_folder(id: String, name: String, folder: Option<String>) -> View {
    let (pick_id, all_id) = (id.clone(), id);
    let mut v = sheet(&tf("Folder of “{}”", &[&name]))
        .w(px(560.0))
        .child(
            text(t("The device sees and uses only this folder: its projects, conversations and files. Requests for anything outside it are refused. It keeps the device's view to the folder; an agent's own commands still run as you on this Mac."))
                .text_size(tx(TEXT_SM))
                .line_height(px(19.0))
                .text_color(FG_MUTED()),
        );
    if let Some(f) = &folder {
        v = v.child(text(tf("Now: {}", &[f])).text_size(tx(TEXT_SM)));
    }
    v = v.child(host_folders("Keep it to “{}”", move |path| {
        crate::lan::set_folder(&pick_id, Some(path));
        store::close_modal();
    }));
    if folder.is_some() {
        v = v.child(view().flex_row().justify_end().child(btn(t("Allow all folders"), Variant::Secondary, Sz::Md, None, move || {
            crate::lan::set_folder(&all_id, None);
            store::close_modal();
        })));
    }
    v
}

/// `/mcp`: Claude Code's MCP servers for the project, each with its state
/// and tools; Sign in on one that asks for it (its page opens in the
/// computer's browser, and the list is read again once it is done). A
/// server signed in to is there for the conversations started after.
fn mcp_panel(cwd: String) -> View {
    use crate::mcp::{State, servers_of};
    let key = crate::github::Op::McpStatus { cwd: cwd.clone() }.key();
    let (k1, k2) = (key.clone(), key);
    let answer = computed(move || store::github(&k1));
    let loading = computed(move || store::github_loading(&k2));
    let refresh_cwd = cwd.clone();
    sheet(t("MCP servers"))
        .w(px(560.0))
        .child(text(cwd.clone()).text_size(tx(TEXT_XS)).text_color(FG_FAINT()).number_of_lines(1))
        .child(view().child(dynamic(move || {
            let answer = answer.get();
            let busy = loading.get();
            // The sign-ins under way: read here, so a press shows at once.
            let signing_now = store::github_loading_with("mcp:signin:");
            let cwd = cwd.clone();
            Some(untracked(move || match answer {
                None => view().py(sp(S6)).items_center().gap(sp(S2)).child(spinner(20.0)).child(text(t("Asking Claude Code…")).text_size(tx(TEXT_SM)).text_color(FG_MUTED())).into_element(),
                Some(Err(e)) => text(e).text_size(tx(TEXT_SM)).text_color(DANGER).into_element(),
                Some(Ok(v)) => {
                    let list = servers_of(&v);
                    if list.is_empty() {
                        return text(t("No MCP servers: add one with claude mcp add, or a plugin")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).into_element();
                    }
                    let mut c = card();
                    for (i, s) in list.into_iter().enumerate() {
                        let (word, tone) = match s.state {
                            State::Connected => (t("Connected"), SUCCESS),
                            State::NeedsAuth => (t("Needs sign-in"), WARNING),
                            State::Failed => (t("Failed"), DANGER),
                            State::Pending => (t("Connecting…"), FG_MUTED()),
                            State::Disabled => (t("Disabled"), FG_FAINT()),
                        };
                        let detail = match (&s.error, s.tools) {
                            (Some(e), _) => e.clone(),
                            (None, Some(n)) => tf("{} tools", &[&n.to_string()]),
                            (None, None) => s.target.clone(),
                        };
                        let signing = signing_now.iter().any(|k| *k == format!("mcp:signin:{}", s.name));
                        let mut right = view().flex_row().items_center().gap(sp(S2)).child(text(word).text_size(tx(TEXT_XS)).text_color(tone));
                        if s.state == State::NeedsAuth {
                            right = if signing {
                                right.child(spinner(14.0)).child(text(t("Waiting for the browser…")).text_size(tx(TEXT_XS)).text_color(FG_MUTED()))
                            } else if MOBILE {
                                right.child(text(t("Sign in on the computer")).text_size(tx(TEXT_XS)).text_color(FG_FAINT()))
                            } else {
                                let (cwd, name) = (cwd.clone(), s.name.clone());
                                right.child(btn(t("Sign in"), Variant::Primary, Sz::Sm, None, move || store::mcp_sign_in(&cwd, &name)))
                            };
                        }
                        c = c.child(
                            view()
                                .flex_row()
                                .items_center()
                                .gap(sp(S3))
                                .px(sp(S4))
                                .py(sp(S3))
                                .when(i > 0, |v| v.border_t(px(1.0)).border_color(BORDER()))
                                .child(view().w(px(8.0)).h(px(8.0)).rounded(px(R_FULL)).bg(tone))
                                .child(
                                    view()
                                        .flex_1()
                                        .min_w(px(0.0))
                                        .child(text(s.label()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)).number_of_lines(1))
                                        .child(text(if s.scope.is_empty() { detail } else { format!("{} · {detail}", s.scope) }).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).number_of_lines(1)),
                                )
                                .child(right),
                        );
                    }
                    let _ = busy;
                    c.into_element()
                }
            }))
        })))
        .child(text(t("A server signed in to is there for conversations started after: start a new one to use it.")).text_size(tx(TEXT_SM)).line_height(px(19.0)).text_color(FG_MUTED()))
        .child(
            view()
                .flex_row()
                .justify_end()
                .gap(sp(S2))
                .child(btn(t("Refresh"), Variant::Secondary, Sz::Md, Some(Icon::RotateCw), move || {
                    store::github_fetch(crate::github::Op::McpStatus { cwd: refresh_cwd.clone() })
                }))
                .child(btn(t("New conversation"), Variant::Secondary, Sz::Md, Some(Icon::SquarePen), || {
                    store::close_modal();
                    store::open_tab(store::Tab::Draft);
                }))
                .child(btn(t("Done"), Variant::Primary, Sz::Md, None, store::close_modal)),
        )
}

/// The versions and system an issue is reported from: nothing of the
/// user's own (no names, addresses or paths).
fn issue_environment() -> Vec<(String, String)> {
    let mut env = vec![
        ("Sophon".to_owned(), crate::update::current_version()),
        ("System".to_owned(), format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)),
    ];
    if let store::Conn::Connected { version, .. } = store::conn() {
        env.push(("Service".to_owned(), version));
        env.push(("Computer".to_owned(), if store::on_this_mac() { "this Mac".to_owned() } else { "another computer".to_owned() }));
    }
    if let Some(a) = store::selected_agent() {
        env.push(("Agent".to_owned(), format!("{}{}", a.provider, a.model.map(|m| format!(" · {m}")).unwrap_or_default())));
    }
    env.push(("Language".to_owned(), if crate::i18n::is_zh() { "zh" } else { "en" }.to_owned()));
    env
}

/// Reporting an issue: what kind, a title, what happened (and for a bug
/// how, and what was expected), the versions. Submit files it through
/// Sophon's relay, no GitHub account needed (`feedback::file`); "Open in
/// GitHub" opens GitHub's page with it, to submit with one's own account.
fn report_issue() -> View {
    use crate::feedback::{Kind, Report};
    let kind = signal(Kind::Bug);
    let title = signal(String::new());
    let what = signal(String::new());
    let steps = signal(String::new());
    let expected = signal(String::new());
    let with_env = signal(true);
    let area = |value: Signal<String>, placeholder: &'static str, h: f32| {
        field()
            .multiline(true)
            .h(px(h))
            .py(sp(S2))
            .value(move || value.get())
            .on_change(move |v: &str| value.set(v.to_owned()))
            .placeholder(placeholder)
    };
    // Sending, then the issue's page or why it was not filed.
    let sending = signal(false);
    let filed = signal(None::<Result<String, String>>);
    let report = move || Report {
        title: title.get_untracked(),
        what: what.get_untracked(),
        steps: steps.get_untracked(),
        expected: expected.get_untracked(),
        environment: with_env.get_untracked().then(issue_environment),
    };
    let on_github = move || {
        let _ = open_url(crate::feedback::url(kind.get_untracked(), &report()));
        store::close_modal();
    };
    let submit = move || {
        if sending.get_untracked() {
            return;
        }
        sending.set(true);
        filed.set(None);
        let (k, r) = (kind.get_untracked(), report());
        store::spawn_detached(async move {
            #[cfg(not(target_arch = "wasm32"))]
            let result = crate::feedback::file(k, &r).await;
            #[cfg(target_arch = "wasm32")]
            let result: Result<String, String> = {
                let _ = (k, r);
                Err("NotConfigured".into())
            };
            sending.set(false);
            filed.set(Some(result));
        });
    };
    let empty = move || title.with(|s| s.trim().is_empty()) && what.with(|s| s.trim().is_empty());
    sheet(t("Report an issue"))
        .w(px(520.0))
        .child(segmented(
            vec![(Kind::Bug, t("Bug").into()), (Kind::Feature, t("Feature").into()), (Kind::Question, t("Question").into())],
            move || kind.get(),
            move |k| kind.set(k),
        ))
        .child(field().value(move || title.get()).on_change(move |v: &str| title.set(v.to_owned())).placeholder(t("Title, in a line")).auto_focus(true))
        .child(dynamic(move || {
            let k = kind.get();
            Some(untracked(move || match k {
                Kind::Bug => view()
                    .gap(sp(S3))
                    .child(area(what, t("What happened"), 84.0))
                    .child(area(steps, t("Steps to make it happen again"), 64.0))
                    .child(area(expected, t("What you expected"), 48.0))
                    .into_element(),
                Kind::Feature => view()
                    .gap(sp(S3))
                    .child(area(what, t("What it would do"), 84.0))
                    .child(area(steps, t("Why you want it"), 64.0))
                    .into_element(),
                Kind::Question => area(what, t("Your question"), 120.0).into_element(),
            }))
        }))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S3))
                .child(switch(move || with_env.get(), move |on| with_env.set(on)))
                .child(
                    text(t("Include Sophon's and the service's versions and the system (nothing of yours)"))
                        .text_size(tx(TEXT_SM))
                        .text_color(FG_MUTED())
                        .flex_1(),
                ),
        )
        .child(dynamic(move || {
            // Read outside `untracked`: the result must redraw this.
            let filed = filed.get();
            Some(untracked(move || match filed {
                // Filed: where it is, and done.
                Some(Ok(url)) => view()
                    .gap(sp(S3))
                    .child(text(t("Submitted. Thank you! It is a public issue on GitHub now")).text_size(tx(TEXT_SM)).text_color(SUCCESS))
                    .child(
                        view()
                            .flex_row()
                            .justify_end()
                            .gap(sp(S2))
                            .child(btn(t("View the issue"), Variant::Secondary, Sz::Md, Some(Icon::ExternalLink), move || {
                                let _ = open_url(url.clone());
                            }))
                            .child(btn(t("Done"), Variant::Primary, Sz::Md, None, store::close_modal)),
                    )
                    .into_element(),
                result => {
                    let why = match result.and_then(Result::err).as_deref() {
                        None => None,
                        Some("RateLimited") => Some(t("Too many reports from here just now: try again in a while, or open it in GitHub")),
                        Some("BadRequest") => Some(t("The report needs a title and what happened, and must not be too long")),
                        Some(_) => Some(t("It could not be sent now: try again, or open it in GitHub")),
                    };
                    let mut v = view().gap(sp(S3));
                    if let Some(why) = why {
                        v = v.child(text(why).text_size(tx(TEXT_SM)).text_color(DANGER));
                    }
                    v.child(text(t("Submitted as a public GitHub issue for Sophon's developers: no account needed")).text_size(tx(TEXT_SM)).text_color(FG_FAINT()))
                        .child(
                            view()
                                .flex_row()
                                .justify_end()
                                .gap(sp(S2))
                                .child(btn(t("Cancel"), Variant::Secondary, Sz::Md, None, store::close_modal))
                                .child(btn(t("Open in GitHub"), Variant::Secondary, Sz::Md, Some(Icon::ExternalLink), on_github).disabled(empty))
                                .child(
                                    btn(t("Submit"), Variant::Primary, Sz::Md, None, submit).disabled(move || empty() || sending.get()),
                                ),
                        )
                        .into_element()
                }
            }))
        }))
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
                .py(sp(S2))
                .value(move || message.get())
                .on_change(move |v: &str| message.set(v.to_owned()))
                .placeholder(t("Commit message (leave empty to generate one)"))
                .auto_focus(true),
        )
        .child(actions(btn(t("Commit"), Variant::Primary, Sz::Md, Some(Icon::GitCommitHorizontal), go)))
}

/// Some of the choices of a parameter: those its manifest gives and
/// those the integration or plugin lists when asked, to search and tick.
fn plugin_choices(plugin: String, param: crate::daemon::connection::PluginParam, draft: bool) -> View {
    if param.dynamic {
        store::load_plugin_choices(&plugin, &param);
    }
    let query = signal(String::new());
    let picked = signal(param.value.split(',').map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned).collect::<Vec<_>>());
    let (asked, name, listing, given) = (plugin.clone(), param.clone(), param.dynamic, param.options.clone());
    // The automation's own is kept under whose the parameter is.
    let (saved, saved_name) = (plugin.clone(), if draft { param.of.key(&param.name) } else { param.name.clone() });
    sheet(&param.label)
        .w(px(520.0))
        .max_h(relative(0.9))
        .child(field().value(move || query.get()).on_change(move |v: &str| query.set(v.to_owned())).placeholder(t("Search")).auto_focus(true))
        .child(scroll().h(px(360.0)).child(dynamic(move || {
            let listed = if listing { store::plugin_choices(&asked, &name) } else { Some(Ok(Vec::new())) };
            let needle = query.get().trim().to_lowercase();
            let given = given.clone();
            Some(untracked(move || {
                let mut col = view().gap(px(2.0));
                let more = match listed {
                    None => return col.child(loading()),
                    Some(Err(error)) => {
                        col = col.child(text(error).text_size(tx(TEXT_SM)).text_color(DANGER).px(sp(S2)).py(sp(S2)));
                        Vec::new()
                    }
                    Some(Ok(more)) => more,
                };
                // What is picked already and no longer listed stays to untick.
                let mut choices = given;
                for choice in more {
                    if !choices.iter().any(|known| known.value == choice.value) {
                        choices.push(choice);
                    }
                }
                for value in picked.get_untracked() {
                    if !choices.iter().any(|known| known.value == value) {
                        choices.insert(0, crate::daemon::connection::PluginChoice { label: value.clone(), value, hint: String::new() });
                    }
                }
                let shown: Vec<_> = choices
                    .into_iter()
                    .filter(|c| needle.is_empty() || c.value.to_lowercase().contains(&needle) || c.label.to_lowercase().contains(&needle))
                    .take(300)
                    .collect();
                if shown.is_empty() {
                    return col.child(text(t("Nothing matches")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).px(sp(S2)).py(sp(S2)));
                }
                for choice in shown {
                    let (value, ticked) = (choice.value.clone(), choice.value.clone());
                    col = col.child(
                        nsh_base::button()
                            .on_press(move || {
                                picked.update(|list| match list.iter().position(|v| *v == value) {
                                    Some(at) => {
                                        list.remove(at);
                                    }
                                    None => list.push(value.clone()),
                                })
                            })
                            .flex_row()
                            .items_center()
                            .gap(sp(S3))
                            .min_h(sp(CONTROL_MD))
                            .px(sp(S2))
                            .rounded(px(R_LG))
                            .hover(|s| s.bg(highlight()))
                            .child(check_mark(move || picked.with(|list| list.contains(&ticked))))
                            .child(text(choice.label.clone()).text_size(tx(TEXT_SM)).font_family(mono_font).number_of_lines(1).flex_1())
                            .when(!choice.hint.is_empty(), |b| b.child(text(choice.hint.clone()).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).number_of_lines(1))),
                    );
                }
                col
            }))
        })))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .child(text(move || tf("{} picked", &[&picked.with(Vec::len).to_string()])).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).flex_1())
                .child(btn(t("Cancel"), Variant::Secondary, Sz::Md, None, store::close_modal))
                .child(btn(t("Done"), Variant::Primary, Sz::Md, None, move || {
                    // The automation's own, or the plugin's for everything of it.
                    if draft {
                        store::set_option_draft(&saved_name, picked.get_untracked().join(","));
                    } else {
                        store::set_plugin_params(&saved, vec![(saved_name.clone(), picked.get_untracked().join(","))]);
                    }
                    store::close_modal();
                })),
        )
}

/// A line of a dialog to tick or not: what it is, and a word more.
fn ticked_row(on: impl Fn() -> bool + 'static, press: impl Fn() + 'static, label: impl Into<SharedString>, more: impl Into<SharedString>) -> nsh_base::ButtonElement {
    let (label, more): (SharedString, SharedString) = (label.into(), more.into());
    nsh_base::button()
        .on_press(press)
        .accessibility_label(label.clone())
        .flex_row()
        .items_start()
        .gap(sp(S3))
        .px(sp(S2))
        .py(sp(S2))
        .rounded(px(R_LG))
        .hover(|s| s.bg(highlight()))
        .child(view().pt(px(2.0)).child(check_mark(on)))
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(2.0))
                .child(text(label).text_size(tx(TEXT_SM)))
                .when(!more.is_empty(), |v| v.child(text(more).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))),
        )
}

/// An integration or a plugin as a file to hand on: what goes in it is
/// said, and chosen, before it is made. The file is saved on this device.
fn hand_on(id: String) -> View {
    let all = store::plugins();
    let Some(plugin) = all.iter().find(|p| p.id == id).cloned() else {
        return sheet(t("Save as a file")).child(actions(view()));
    };
    // What the file is about: itself, and each integration it counts on.
    let mut about = vec![plugin.clone()];
    about.extend(plugin.requires.iter().filter_map(|need| all.iter().find(|p| p.id == need.id).cloned()));
    // One that comes with Sophon is named and not packed; one a plugin
    // brought rides in that plugin's folder.
    let rides = |p: &crate::daemon::connection::Plugin| !p.from.is_empty() && about.iter().any(|other| other.id == p.from && !other.builtin);
    let packed: Vec<String> = about.iter().filter(|p| (!p.builtin && p.from.is_empty()) || rides(p)).map(|p| p.name.clone()).collect();
    let named: Vec<String> = about.iter().filter(|p| p.builtin).map(|p| p.name.clone()).collect();
    // What is set on this computer, by whose it is: the plain and the secret.
    let set = |secret: bool| -> Vec<String> {
        about.iter().flat_map(|p| p.params.iter().filter(move |param| param.changed && param.filled && param.secret == secret).map(move |param| format!("{} · {}", p.name, param.label))).collect()
    };
    let (plain, secret) = (set(false), set(true));
    let sep = if crate::i18n::is_zh() { "、" } else { ", " };
    // The automations made with it: one may go along, as it is set up.
    let made: Vec<(String, String)> = store::schedules().unwrap_or_default().into_iter().filter(|s| s.plugin == id || s.integration == id).map(|s| (s.id, s.name)).collect();
    let settings = signal(!plain.is_empty() || !secret.is_empty());
    let secrets = signal(!secret.is_empty());
    let automation = signal(None::<String>);
    let (any_set, any_packed) = (!plain.is_empty() || !secret.is_empty(), !packed.is_empty());
    // Whether the file would hold anything worth handing on.
    let worth = computed(move || any_packed || (any_set && settings.get()) || automation.with(Option::is_some));
    let mut body = sheet(&tf("Save “{}” as a file", &[&plugin.name])).w(px(520.0)).child(
        text(t("One file with all of it, for someone to add on their computer: nothing to host, and no address to give")).text_size(tx(TEXT_BASE)).line_height(px(20.0)).text_color(FG_MUTED()),
    );
    let mut lines = view().gap(px(2.0));
    if any_packed {
        lines = lines.child(view().px(sp(S2)).py(px(4.0)).child(text(tf("In the file: {}", &[&packed.join(sep)])).text_size(tx(TEXT_SM))));
    }
    if !named.is_empty() {
        lines = lines.child(view().px(sp(S2)).py(px(4.0)).child(text(tf("Named, not packed, since it comes with Sophon: {}", &[&named.join(sep)])).text_size(tx(TEXT_SM)).text_color(FG_MUTED())));
    }
    if any_set {
        let listed = plain.iter().chain(&secret).cloned().collect::<Vec<_>>().join(sep);
        lines = lines.child(ticked_row(move || settings.get(), move || settings.update(|on| *on = !*on), t("The settings filled in here"), listed));
    }
    if !secret.is_empty() {
        let listed = tf("Whoever has the file has these: {}", &[&secret.join(sep)]);
        lines = lines.child(ticked_row(move || settings.get() && secrets.get(), move || secrets.update(|on| *on = !*on), t("The secrets among them"), listed).disabled(move || !settings.get()));
    }
    for (schedule, name) in made {
        let (shown, pressed) = (schedule.clone(), schedule);
        lines = lines.child(ticked_row(
            move || automation.with(|picked| picked.as_deref() == Some(shown.as_str())),
            move || automation.update(|picked| *picked = if picked.as_deref() == Some(pressed.as_str()) { None } else { Some(pressed.clone()) }),
            tf("The automation “{}”, as it is set up", &[&name]),
            t("Its instructions and choices. Which project it runs in is theirs to pick"),
        ));
    }
    body = body
        .child(lines)
        // Why there is nothing to save, when there is not.
        .child(when(move || !worth.get(), || text(t("Nothing to put in the file yet: it comes with Sophon, and nothing is set or made with it here")).text_size(tx(TEXT_SM)).text_color(WARNING)))
        .child(text(t("A sign-in is never in the file: whoever adds it signs in themselves")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()));
    let (saved, copied) = (id.clone(), id);
    body.child(
        view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            // Its own settings alone, as a line to paste: quicker for one or two.
            .when(plugin.params.iter().any(|param| param.filled), |v| {
                v.child(btn(t("Copy settings as text"), Variant::Ghost, Sz::Md, Some(Icon::Copy), move || {
                    store::export_plugin(&copied);
                    store::close_modal();
                }))
            })
            .child(view().flex_1())
            .child(btn(t("Cancel"), Variant::Secondary, Sz::Md, None, store::close_modal))
            .child(
                btn(t("Save file…"), Variant::Primary, Sz::Md, Some(Icon::Download), move || {
                    let which = match (settings.get_untracked(), secrets.get_untracked()) {
                        (false, _) => "none",
                        (true, false) => "plain",
                        (true, true) => "all",
                    };
                    store::pack_plugin(&saved, which, automation.get_untracked());
                    store::close_modal();
                })
                .disabled(move || !worth.get()),
            ),
    )
}

/// A file someone handed on, read: what it holds, shown before any of
/// it is installed. What it installs runs its scripts on that computer,
/// so nothing is until this is answered.
fn take_in(file: crate::daemon::connection::HandedFile) -> View {
    use crate::daemon::connection::PluginKind;
    let mut list = view().gap(sp(S3));
    let installs = file.packages.iter().any(|package| package.included);
    for package in &file.packages {
        let (words, tone) = match (package.included, package.here, package.builtin) {
            (true, true, _) => (t("Replaces the one here"), WARNING),
            (true, false, _) => (t("Will be installed"), FG_MUTED()),
            (false, _, true) => (t("Comes with Sophon"), FG_MUTED()),
            (false, true, false) => (t("Already here"), FG_MUTED()),
            (false, false, _) => (t("Not in the file and not here: add it yourself"), WARNING),
        };
        list = list.child(
            view()
                .flex_row()
                .items_start()
                .gap(sp(S3))
                .child(view().pt(px(2.0)).child(icon(if package.kind == PluginKind::Plugin { Icon::Layers } else { Icon::Zap }, 16.0).text_color(FG_MUTED())))
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap(px(2.0))
                        .child(view().flex_row().items_center().gap(sp(S2)).child(text(package.name.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500))).child(text(words).text_size(tx(TEXT_SM)).text_color(tone)))
                        .when(!package.description.is_empty(), |v| v.child(text(package.description.clone()).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).number_of_lines(2))),
                ),
        );
    }
    let sep = if crate::i18n::is_zh() { "、" } else { ", " };
    if !file.settings.is_empty() {
        let named: Vec<String> = file.settings.iter().map(|(whose, label, secret)| if *secret { tf("{} · {} (secret)", &[whose, label]) } else { format!("{whose} · {label}") }).collect();
        list = list.child(
            view()
                .flex_row()
                .items_start()
                .gap(sp(S3))
                .child(view().pt(px(2.0)).child(icon(Icon::Settings, 16.0).text_color(FG_MUTED())))
                .child(view().flex_1().min_w(px(0.0)).gap(px(2.0)).child(text(t("Fills in these settings")).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500))).child(text(named.join(sep)).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))),
        );
    }
    if let Some(automation) = &file.automation {
        list = list.child(
            view()
                .flex_row()
                .items_start()
                .gap(sp(S3))
                .child(view().pt(px(2.0)).child(icon(Icon::CalendarClock, 16.0).text_color(FG_MUTED())))
                .child(
                    view()
                        .flex_1()
                        .min_w(px(0.0))
                        .gap(px(2.0))
                        .child(text(tf("Opens the automation “{}”", &[automation])).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)))
                        .child(text(t("Filled in as it was set up. It is made once you pick its project and create it")).text_size(tx(TEXT_SM)).text_color(FG_MUTED())),
                ),
        );
    }
    let (dropped, taken) = (file.token.clone(), file.token.clone());
    sheet(&tf("Add “{}”", &[&file.name]))
        .w(px(520.0))
        .child(list)
        .when(installs, |v| v.child(text(t("What it installs runs its scripts on that computer: add only what you trust")).text_size(tx(TEXT_SM)).line_height(px(18.0)).text_color(WARNING)))
        .child(
            view()
                .flex_row()
                .justify_end()
                .gap(sp(S2))
                .child(btn(t("Cancel"), Variant::Secondary, Sz::Md, None, move || {
                    store::discard_handed(&dropped);
                    store::close_modal();
                }))
                .child(btn(t("Add"), Variant::Primary, Sz::Md, Some(Icon::Download), move || {
                    store::install_handed(&taken);
                    store::close_modal();
                })),
        )
}

fn add_project() -> View {
    #[derive(Clone, Copy, PartialEq)]
    enum Mode {
        Open,
        Create,
    }
    let mode = signal(Mode::Open);
    // Folders are the connected computer's: this Mac's through the system's
    // dialog, another's browsed here (the dialog would show this Mac's
    // folders, which that computer does not have).
    let local = store::on_this_mac();
    let parent = signal(if local { std::env::var("HOME").unwrap_or_default() } else { store::host_home() });
    // Another computer's folder for "where it goes" is being chosen.
    let choosing = signal(false);
    let name = signal(String::new());
    let pick_parent = move || {
        if !local {
            return choosing.set(true);
        }
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
            vec![(Mode::Open, t("Existing folder").into()), (Mode::Create, t("New folder").into())],
            move || mode.get(),
            move |m| mode.set(m),
        ))
        .child(dynamic(move || {
            let m = mode.get();
            let choosing_now = choosing.get();
            Some(untracked(move || match m {
                _ if choosing_now => host_folders("Use “{}”", move |path| {
                    parent.set(path);
                    choosing.set(false);
                }),
                Mode::Open if !local => view()
                    .gap(sp(S3))
                    .child(text(t("Pick a folder on the connected computer; Sophon lists it with your projects")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
                    .child(host_folders("Add “{}”", |path| {
                        store::add_project(&path);
                        store::close_modal();
                    })),
                Mode::Open => view()
                    .gap(sp(S3))
                    .child(text(t("Pick a folder on this computer; Sophon lists it with your projects")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
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
                    .gap(sp(S3))
                    .child(location_row(parent, pick_parent))
                    .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).placeholder(t("Folder name")))
                    .child(actions(
                        btn(t("Create"), Variant::Primary, Sz::Md, Some(Icon::FolderPlus), move || {
                            store::create_project_dir(&parent.get_untracked(), name.get_untracked().trim());
                            store::close_modal();
                        })
                        .disabled(move || name.with(|n| n.trim().is_empty())),
                    )),
            }))
        }))
}

/// The folders of the connected computer when it is not this Mac, browsed
/// from its home folder: a press goes into one, the arrow goes up, and the
/// button takes the folder shown (`confirm` names it: "Add “{}”").
fn host_folders(confirm: &'static str, on_pick: impl Fn(String) + 'static) -> View {
    store::browse(&store::host_home());
    let browse = computed(store::browse_state);
    let name = computed(move || browse.with(|b| b.cwd.trim_end_matches('/').rsplit('/').next().filter(|n| !n.is_empty()).unwrap_or("/").to_owned()));
    view()
        .gap(sp(S3))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .child(icon_btn(Icon::ArrowUp, Sz::Sm, t("Up"), move || {
                    let cwd = browse.with_untracked(|b| b.cwd.clone());
                    let parent = cwd.trim_end_matches('/').rsplit_once('/').map(|(p, _)| p).filter(|p| !p.is_empty()).unwrap_or("/").to_owned();
                    store::browse(&parent);
                }))
                .child(text(move || browse.with(|b| b.cwd.clone())).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(FG_MUTED()).number_of_lines(1).flex_1()),
        )
        .child(scroll().h(px(280.0)).rounded(px(R_LG)).border_1().border_color(tint(BORDER(), 0.6)).child(dynamic(move || {
            let b = browse.get();
            Some(untracked(move || {
                if let Some(e) = b.error.clone() {
                    return text(e).p(sp(S4)).text_size(tx(TEXT_SM)).text_color(DANGER).into_element();
                }
                if !b.loaded {
                    return view().pt(sp(S8)).items_center().child(loader(None)).into_element();
                }
                if b.folders.is_empty() {
                    return text(t("No folders here")).p(sp(S4)).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).into_element();
                }
                let mut rows = view().p(sp(S1));
                for path in b.folders.clone() {
                    let label = path.trim_end_matches('/').rsplit('/').next().unwrap_or(&path).to_owned();
                    rows = rows.child(
                        nsh_base::button()
                            .on_press(move || store::browse(&path))
                            .flex_row()
                            .items_center()
                            .gap(sp(S2))
                            .h(px(32.0))
                            .px(sp(S2))
                            .rounded(px(R_MD))
                            .hover(|s| s.bg(highlight()))
                            .child(icon(Icon::Folder, 15.0).text_color(FG_FAINT()))
                            .child(text(label).text_size(tx(TEXT_SM)).number_of_lines(1).flex_1())
                            .child(icon(Icon::ChevronRight, 14.0).text_color(FG_FAINT())),
                    );
                }
                rows.into_element()
            }))
        })))
        .child(actions(btn(
            move || SharedString::from(tf(confirm, &[&name.get()])),
            Variant::Primary,
            Sz::Md,
            Some(Icon::FolderPlus),
            move || {
                let cwd = browse.with_untracked(|b| b.cwd.clone());
                if cwd.len() > 1 {
                    on_pick(cwd);
                }
            },
        )))
}

fn location_row(parent: Signal<String>, pick: impl Fn() + 'static) -> View {
    view()
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .child(icon(Icon::Folder, 15.0).text_color(FG_MUTED()))
        .child(text(move || parent.get()).text_size(tx(TEXT_SM)).font_family(mono_font).text_color(FG_MUTED()).number_of_lines(1).flex_1())
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
                            .gap(sp(S2))
                            .h(sp(CONTROL_MD))
                            .px(sp(S2))
                            .rounded(px(R_LG))
                            .hover(|s| s.bg(highlight()))
                            .child(icon(Icon::GitBranch, 14.0).text_color(FG_MUTED()))
                            .child(text(b).text_size(tx(TEXT_BASE)).font_family(mono_font).number_of_lines(1)),
                    );
                }
                col
            }))
        })))
        .child(actions(view()))
}

/// A checkbox's mark: filled with the accent when on.
fn check_mark(on: impl Fn() -> bool + 'static) -> View {
    let on = computed(on);
    view()
        .w(px(16.0))
        .h(px(16.0))
        .flex_shrink_0()
        .rounded(px(5.0))
        .border_1()
        .border_color(move || if on.get() { ACCENT().into() } else { tint(FG(), 0.28) })
        .bg(move || if on.get() { ACCENT().into() } else { tint(FG(), 0.0) })
        .items_center()
        .justify_center()
        .child(when(move || on.get(), || icon(Icon::Check, 11.0).text_color(ACCENT_FG())))
}

/// The sessions started outside Sophon, to pick from: by agent (all,
/// Claude, Codex), then by folder; ticked one by one, a folder at a time,
/// or all that show, and imported together.
/// Bringing in projects from Claude Code's and Codex's own history, as
/// T3 Code does: the folders their sessions ran in, grouped by
/// repository, each with how many conversations Sophon does not have yet
/// and when it was last worked in. The new projects worked in lately are
/// picked to begin with; the rest a review away. Importing a project
/// brings in all its conversations.
fn import_session() -> View {
    use crate::daemon::connection::ImportProject;
    use std::collections::HashSet;
    store::scan_import_projects();
    let scan = computed(store::import_projects);
    let found = computed(move || scan.with(|s| match s {
        Some(Ok(list)) => list.clone(),
        _ => Vec::new(),
    }));
    // None: what is picked to begin with.
    let picked: Signal<Option<HashSet<String>>> = signal(None);
    let reviewing = signal(false);
    let selected = computed(move || {
        picked.get().unwrap_or_else(|| {
            let known: Vec<String> = store::projects().into_iter().map(|p| p.root_path).collect();
            found.with(|list| store::import_defaults(list, &known, crate::ui::mobile::time::now()))
        })
    });
    let set = move |paths: Vec<String>, on: bool| {
        let mut next = selected.get_untracked();
        for path in paths {
            if on {
                next.insert(path);
            } else {
                next.remove(&path);
            }
        }
        picked.set(Some(next));
    };
    let chosen = computed(move || found.with(|list| list.iter().filter(|p| selected.with(|s| s.contains(&p.path))).cloned().collect::<Vec<ImportProject>>()));
    let import = move || {
        let sessions: Vec<_> = chosen.get_untracked().into_iter().flat_map(|p| p.sessions).collect();
        if !sessions.is_empty() {
            store::import_sessions(sessions);
            store::close_modal();
        }
    };

    sheet(t("Import projects"))
        .w(px(640.0))
        .child(hint_text(t("The folders your Claude Code and Codex conversations ran in, as projects, with their conversations")))
        .child(dynamic(move || {
            let state = scan.get();
            Some(untracked(move || match state {
                None => view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .py(sp(S2))
                    .child(working(t("Looking for projects in your Claude Code and Codex history…")))
                    .into_element(),
                Some(Err(error)) => view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S2))
                    .child(text(tf("Could not look for projects: {}", &[&error])).flex_1().text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
                    .child(btn(t("Try again"), Variant::Ghost, Sz::Sm, None, store::scan_import_projects))
                    .into_element(),
                Some(Ok(list)) if list.is_empty() => text(t("Nothing to import: every conversation of Claude Code and Codex here is in Sophon"))
                    .text_size(tx(TEXT_SM))
                    .text_color(FG_MUTED())
                    .into_element(),
                Some(Ok(list)) => import_list(list, selected, reviewing, set).into_element(),
            }))
        }))
        .child(actions(
            btn(
                move || -> SharedString {
                    match chosen.with(Vec::len) {
                        0 => t("Import").into(),
                        n => tf("Import {} projects", &[&n.to_string()]).into(),
                    }
                },
                Variant::Primary,
                Sz::Md,
                Some(Icon::Download),
                import,
            )
            .disabled(move || chosen.with(Vec::is_empty) || store::importing()),
        ))
}

fn hint_text(s: &'static str) -> nsh_core::element::Text {
    text(s).text_size(tx(TEXT_SM)).text_color(FG_MUTED())
}

/// The projects found: one line to import those picked, the whole list a
/// review away.
fn import_list(
    list: Vec<crate::daemon::connection::ImportProject>,
    selected: Memo<std::collections::HashSet<String>>,
    reviewing: Signal<bool>,
    set: impl Fn(Vec<String>, bool) + Copy + 'static,
) -> View {
    let all: Vec<String> = list.iter().map(|p| p.path.clone()).collect();
    let total = all.len();
    let count = move || selected.with(|s| s.len());
    let all_on = all.clone();
    let summary = view()
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .py(sp(S1))
        .child(
            nsh_shadcn::checkbox()
                .checked(move || Some(count() > 0))
                .on_checked_change(move |on: &bool| set(all_on.clone(), *on)),
        )
        .child(
            view()
                .flex_1()
                .min_w(px(0.0))
                .gap(px(2.0))
                .child(text(move || match count() {
                    0 => t("Import projects and their conversations").to_owned(),
                    n => tf("Import {} projects and their conversations", &[&n.to_string()]),
                })
                .text_size(tx(TEXT_SM))
                .font_weight(FontWeight(500)))
                .child(text(t("From your Claude Code and Codex history")).text_size(tx(TEXT_XS)).text_color(FG_MUTED())),
        )
        .child(btn(
            move || -> SharedString { if reviewing.get() { t("Hide").into() } else { tf("Review all {}", &[&total.to_string()]).into() } },
            // A control, not a line of text: outlined, with the chevron a
            // list that opens has.
            Variant::Outline,
            Sz::Sm,
            Some(Icon::ChevronDown),
            move || reviewing.update(|r| *r = !*r),
        ));
    let all_select = all.clone();
    let review = view().gap(px(2.0)).child(
        view()
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .px(sp(S2))
            .child(text(move || tf("{} of {} selected", &[&count().to_string(), &total.to_string()])).flex_1().text_size(tx(TEXT_XS)).text_color(FG_MUTED()))
            .child(btn(t("Select all"), Variant::Ghost, Sz::Sm, None, move || set(all_select.clone(), true)).disabled(move || count() == total))
            .child(btn(t("Select none"), Variant::Ghost, Sz::Sm, None, move || set(all.clone(), false)).disabled(move || count() == 0)),
    );
    let (groups, other) = store::import_groups(&list);
    let mut rows = view().gap(px(2.0)).pr(sp(S2));
    for (label, of) in groups {
        let members: Vec<crate::daemon::connection::ImportProject> = of.iter().map(|i| list[*i].clone()).collect();
        if members.len() == 1 {
            rows = rows.child(import_row(members[0].clone(), Some(label), false, selected, set));
            continue;
        }
        rows = rows.child(import_group(label, members, true, selected, set));
    }
    if !other.is_empty() {
        let members = other.iter().map(|i| list[*i].clone()).collect();
        rows = rows.child(import_group(t("Other folders").to_owned(), members, false, selected, set));
    }
    view()
        .gap(sp(S2))
        .child(summary)
        .child(dynamic(move || {
            let open = reviewing.get();
            let (review, rows) = (review.clone(), rows.clone());
            open.then(|| untracked(move || view().gap(sp(S1)).child(review).child(scroll().max_h(px(360.0)).child(rows)).into_element()))
        }))
}

/// A repository's folders (or the folders that are none): its line, which
/// picks them all, and theirs under it while it is open.
fn import_group(
    label: String,
    members: Vec<crate::daemon::connection::ImportProject>,
    open: bool,
    selected: Memo<std::collections::HashSet<String>>,
    set: impl Fn(Vec<String>, bool) + Copy + 'static,
) -> View {
    let paths: Vec<String> = members.iter().map(|p| p.path.clone()).collect();
    let n = members.len();
    let shown = signal(open);
    let (inside, toggle) = (paths.clone(), paths.clone());
    let all_on = computed(move || selected.with(|s| inside.iter().all(|p| s.contains(p))));
    let mut nested = view().gap(px(2.0));
    for p in members {
        nested = nested.child(import_row(p, None, true, selected, set));
    }
    view()
        .gap(px(2.0))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .px(sp(S2))
                .py(px(6.0))
                .rounded(px(R_MD))
                .hover(|st| st.bg(highlight()))
                .child(nsh_shadcn::checkbox().checked(move || Some(all_on.get())).on_checked_change(move |on: &bool| set(toggle.clone(), *on)))
                .child(
                    nsh_base::button()
                        .on_press(move || shown.update(|s| *s = !*s))
                        .flex_1()
                        .min_w(px(0.0))
                        .flex_row()
                        .items_center()
                        .gap(sp(S1))
                        .child(icon(Icon::ChevronRight, 13.0).text_color(FG_MUTED()).rotate(move || if shown.get() { 90.0 } else { 0.0 }))
                        .child(text(label).flex_1().min_w(px(0.0)).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).number_of_lines(1))
                        .child(
                            text(if n == 1 { t("1 folder").to_owned() } else { tf("{} folders", &[&n.to_string()]) })
                                .text_size(tx(TEXT_XS))
                                .text_color(FG_MUTED()),
                        ),
                ),
        )
        .child(dynamic(move || {
            let nested = nested.clone();
            shown.get().then(|| untracked(move || nested.into_element()))
        }))
}

/// One folder: its name (a repository's, on its own line) and path, the
/// agents of its conversations, how many and when last.
fn import_row(
    p: crate::daemon::connection::ImportProject,
    label: Option<String>,
    nested: bool,
    selected: Memo<std::collections::HashSet<String>>,
    set: impl Fn(Vec<String>, bool) + Copy + 'static,
) -> nsh_base::ButtonElement {
    let path = p.path.clone();
    let (is_on, flip) = (path.clone(), path.clone());
    let on = computed(move || selected.with(|s| s.contains(&is_on)));
    let mut meta = view().flex_row().items_center().gap(sp(S2)).flex_shrink_0();
    for source in &p.sources {
        meta = meta.child(crate::ui::shared::model_picker::mark(source, 13.0));
    }
    meta = meta
        .child(
            text(match p.sessions.len() {
                1 => t("1 conversation").to_owned(),
                n => tf("{} conversations", &[&n.to_string()]),
            })
            .text_size(tx(TEXT_XS))
            .text_color(FG_MUTED()),
        )
        .child(text(crate::ui::mobile::time::ago_long(&p.last_activity)).text_size(tx(TEXT_XS)).text_color(FG_FAINT()));
    let mut names = view().flex_1().min_w(px(0.0)).flex_row().items_baseline().gap(sp(S2));
    if let Some(label) = label {
        names = names.child(text(label).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).number_of_lines(1).flex_shrink_0().max_w(px(220.0)));
    }
    names = names.child(text(path).flex_1().min_w(px(0.0)).font_family(mono_font).text_size(tx(TEXT_XS)).text_color(FG_MUTED()).number_of_lines(1));
    nsh_base::button()
        .on_press(move || set(vec![flip.clone()], !on.get_untracked()))
        .flex_row()
        .items_center()
        .gap(sp(S2))
        .px(sp(S2))
        .py(px(6.0))
        .when(nested, |v| v.pl(px(32.0)))
        .rounded(px(R_MD))
        .hover(|st| st.bg(highlight()))
        .child(nsh_shadcn::checkbox().checked(move || Some(on.get())).on_checked_change({
            let path = p.path.clone();
            move |checked: &bool| set(vec![path.clone()], *checked)
        }))
        .child(names)
        .child(meta)
}

/// The fonts installed on this device, to pick the interface's or the
/// code font from: each name in its own letters, a search over them, and
/// the default first. A name that is not listed can still be typed.
fn fonts(mono: bool) -> View {
    let query = signal(String::new());
    let current = if mono { store::fonts_untracked().1 } else { store::fonts_untracked().0 };
    // For code, the fixed-width ones; all of them for the interface.
    let all: Vec<String> = crate::platform::fonts::families().into_iter().filter(|f| !mono || f.fixed).map(|f| f.name).collect();
    let pick = move |name: &str| {
        store::set_font(mono, name);
        store::close_modal();
    };
    let row = move |label: String, family: Option<String>, chosen: bool, value: String| {
        let mut name = text(label).flex_1().min_w(px(0.0)).text_size(tx(TEXT_BASE)).number_of_lines(1);
        if let Some(family) = family {
            name = name.font_family(SharedString::from(family));
        }
        nsh_base::button()
            .on_press(move || pick(&value))
            .flex_row()
            .items_center()
            .gap(sp(S2))
            .px(sp(S3))
            .h(px(34.0))
            .rounded(px(R_LG))
            .hover(|st| st.bg(highlight()))
            .child(name)
            .child(view().w(px(16.0)).child(when(move || chosen, || icon(Icon::Check, 14.0).text_color(ACCENT()))))
    };
    sheet(if mono { t("Code font") } else { t("Interface font") })
        .w(px(460.0))
        .child(
            field()
                .value(move || query.get())
                .on_change(move |v: &str| query.set(v.to_owned()))
                .placeholder(t("Search fonts"))
                .auto_focus(true),
        )
        .child(scroll().h(px(360.0)).child(dynamic(move || {
            let needle = query.get().trim().to_lowercase();
            let (all, current) = (all.clone(), current.clone());
            Some(untracked(move || {
                let mut col = view().gap(px(1.0));
                if needle.is_empty() {
                    let default = if mono { tf("Default ({})", &["Menlo"]) } else { t("Default (the system's font)").to_owned() };
                    col = col.child(row(default, None, current.is_none(), String::new()));
                }
                let shown: Vec<&String> = all.iter().filter(|n| needle.is_empty() || n.to_lowercase().contains(&needle)).collect();
                for name in &shown {
                    col = col.child(row((*name).clone(), Some((*name).clone()), current.as_deref() == Some(name.as_str()), (*name).clone()));
                }
                // Not in the list (or no list on this system): as typed.
                let typed = query.get_untracked().trim().to_owned();
                if !typed.is_empty() && !all.iter().any(|n| n.to_lowercase() == needle) {
                    col = col.child(row(tf("Use “{}”", &[&typed]), None, false, typed.clone()));
                }
                col
            }))
        })))
        .child(actions(view()))
}

fn project_settings(name: String) -> View {
    let cfg = computed(store::project_config);
    sheet(&format!("{name} · sophon.json"))
        .w(px(680.0))
        .child(text(t("Setup and teardown scripts, services and other project settings, saved in the project")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
        .child(dynamic(move || {
            let loaded = cfg.with(|c| c.text.is_some());
            Some(untracked(move || {
                if !loaded {
                    return loading().into_element();
                }
                view()
                    .h(px(360.0))
                    .rounded(px(R_LG))
                    .border_1()
                    .border_color(BORDER())
                    .overflow_hidden()
                    .child(
                        nsh_base::editor()
                            .value(move || cfg.with(|c| c.text.clone().map(SharedString::from)))
                            .language("json")
                            .on_change(|v: &SharedString| store::edit_project_config(v))
                            .text_size(tx(TEXT_CODE))
                            .flex_1(),
                    )
                    .into_element()
            }))
        }))
        .child(dynamic(move || {
            let e = cfg.with(|c| c.error.clone())?;
            Some(untracked(move || text(e.clone()).text_size(tx(TEXT_SM)).text_color(DANGER)))
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
                .border_color(move || if color.get() == c { FG() } else { label_tone(c) }),
        );
    }
    sheet(t("Labels"))
        .child(dynamic(move || {
            let cat = catalog.get();
            let assigned = ws.with(|w| w.as_ref().map(|w| w.labels.clone()).unwrap_or_default());
            let ws_id = workspace.clone();
            Some(untracked(move || {
                if cat.is_empty() {
                    return text(t("No labels yet; create one below")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).into_element();
                }
                let mut list = view().flex_row().flex_wrap().gap(sp(S2));
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
                .text_size(tx(TEXT_SM))
                .line_height(px(18.0))
                .text_color(FG_MUTED()),
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
                .text_size(tx(TEXT_SM))
                .line_height(px(18.0))
                .text_color(FG_MUTED()),
        )
        .child(field().value(move || name.get()).on_change(move |v: &str| name.set(v.to_owned())).placeholder(t("Name, e.g. Studio Mac")))
        .child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .child(
                    field()
                        .flex_1()
                        .value(move || url.get())
                        .on_change(move |v: &str| url.set(v.to_owned()))
                        .placeholder(t("Address (192.168.1.20:6767) or a pairing link"))
                        .font_family(mono_font),
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
                .text_size(tx(TEXT_XS))
                .text_color(FG_FAINT()),
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
        .px(sp(S6))
        .py(sp(S3))
        .rounded(px(R_LG))
        .bg(SURFACE2())
        .child(text(code.to_owned()).text_size(px(32.0)).font_weight(FontWeight(500)).font_family(mono_font))
}

/// This Mac: a phone nearby asks to pair, into one of its spaces.
fn approve_pairing(device: String, code: String) -> View {
    let space = signal(None::<String>);
    sheet(t("Pairing request"))
        .child(
            text(tf("“{}” on this network asks to connect to this Mac. Allowed, it can use your agents here, from anywhere", &[&device]))
                .text_size(tx(TEXT_BASE))
                .line_height(px(20.0))
                .text_color(FG_MUTED()),
        )
        .child(pairing_code(&code))
        .child(text(t("Allow only if the device shows the same code")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).self_center())
        .child(space_choice(space))
        .child(
            view()
                .flex_row()
                .justify_end()
                .gap(sp(S2))
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
        .gap(sp(S2))
        .child(text(t("Joins the space")).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
        .child(segmented(options, move || space.get(), move |v| space.set(v)))
        .into_element()
}

/// A phone (or another Mac): pairing with a computer nearby.
fn pair_nearby() -> View {
    let p = computed(crate::lan::pairing);
    sheet(t("Pair with a computer")).child(dynamic(move || {
        let st = p.get();
        Some(untracked(move || {
            let mut v = view().gap(sp(S4));
            if let Some(e) = &st.error {
                v = v.child(text(e.clone()).text_size(tx(TEXT_BASE)).text_color(DANGER));
            } else if let Some(code) = &st.code {
                v = v
                    .child(
                        text(tf("Allow it on “{}”. The code there must be the same as this one", &[&st.name]))
                            .text_size(tx(TEXT_BASE))
                            .line_height(px(20.0))
                            .text_color(FG_MUTED()),
                    )
                    .child(pairing_code(code));
            } else {
                v = v.child(text(tf("Connecting to “{}”…", &[&st.name])).text_size(tx(TEXT_BASE)).text_color(FG_MUTED()));
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
    // Android finds no computers yet (Bonjour is Apple's): no empty list.
    if cfg!(target_os = "android") {
        return view();
    }
    crate::lan::scan();
    view().gap(sp(S2)).child(dynamic(|| {
        let list = crate::lan::nearby();
        let scanning = crate::lan::scanning();
        let scanned = crate::lan::scanned();
        Some(untracked(move || {
            let head = view()
                .flex_row()
                .items_center()
                .gap(sp(S2))
                .child(text(t("On this network")).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED()).flex_1())
                .child(if scanning {
                    text(t("Searching…")).text_size(tx(TEXT_SM)).text_color(FG_FAINT()).into_element()
                } else {
                    icon_btn(Icon::RotateCw, Sz::Sm, t("Search again"), crate::lan::scan).into_element()
                });
            let mut c = card();
            if list.is_empty() {
                let note = if scanned && !scanning { t("No computer running Sophon found on this network") } else { t("Searching…") };
                c = c.child(view().px(sp(S4)).py(sp(S3)).child(text(note).text_size(tx(TEXT_SM)).text_color(FG_MUTED())));
            }
            for (i, n) in list.into_iter().enumerate() {
                let paired = n.host.is_some();
                let name = n.name.clone();
                c = c.child(
                    nsh_base::button()
                        .on_press(move || crate::lan::connect(n.clone()))
                        .flex_row()
                        .items_center()
                        .gap(sp(S3))
                        .h(px(52.0))
                        .px(sp(S4))
                        .when(i > 0, |v| v.border_t_1().border_color(BORDER()))
                        .active(|s| s.bg(highlight()))
                        .hover(|s| s.bg(highlight()))
                        .child(icon(Icon::Monitor, 18.0).text_color(FG_MUTED()))
                        .child(text(name).text_size(tx(TEXT_BASE)).number_of_lines(1).flex_1())
                        .child(text(if paired { t("Paired") } else { t("Pair") }).text_size(tx(TEXT_SM)).text_color(if paired { FG_MUTED() } else { ACCENT() })),
                );
            }
            view().gap(sp(S2)).child(head).child(c).into_element()
        }))
    }))
}

fn pair_device() -> View {
    let p = computed(store::pairing);
    sheet(t("Pair a device"))
        .w(px(480.0))
        .child(
            text(t("Scan with your phone's camera and open it in Sophon. On the same Wi-Fi it connects directly; anywhere else through the encrypted relay. Both are end to end encrypted"))
                .text_size(tx(TEXT_SM))
                .line_height(px(18.0))
                .text_color(FG_MUTED()),
        )
        .child(dynamic(move || {
            let st = p.get();
            Some(untracked(move || {
                if !st.loaded {
                    return loading().into_element();
                }
                if let Some(e) = st.error.clone() {
                    return text(e).text_size(tx(TEXT_SM)).text_color(DANGER).into_element();
                }
                let Some(link) = st.sophon_url.clone() else {
                    return text(t("Neither the LAN bridge nor the relay is available: turn on Remote relay in Settings"))
                        .text_size(tx(TEXT_SM))
                        .text_color(WARNING)
                        .into_element();
                };
                let mut v = view().gap(sp(S3)).items_center();
                if let Some(svg_text) = st.qr_svg.clone() {
                    v = v.child(
                        view().p(sp(S2)).rounded(px(R_XL)).bg(AdaptiveColor::rgb(0xffffff, 0xffffff)).child(
                            svg(nsh_core::element::ImageSrc::svg_markup(&svg_text)).w(px(240.0)).h(px(240.0)),
                        ),
                    );
                }
                let mut ways = view().flex_row().flex_wrap().gap(sp(S2)).justify_center();
                if let Some(addr) = store::bridge_addr() {
                    ways = ways.child(badge(tf("Same Wi-Fi · {}", &[&addr]), SUCCESS));
                }
                if st.relay_enabled {
                    ways = ways.child(badge(t("Anywhere · relay"), ACCENT()));
                }
                let (sophon_link, daemon_link) = (link.clone(), st.url.clone());
                let mut buttons = view().flex_row().gap(sp(S2)).child(btn(t("Copy link"), Variant::Secondary, Sz::Md, Some(Icon::Copy), move || {
                    let _ = set_clipboard(sophon_link.clone());
                    store::notify(t("Pairing link copied").into());
                }));
                if let Some(pl) = daemon_link.filter(|_| st.relay_enabled) {
                    buttons = buttons.child(btn(t("Copy relay link"), Variant::Ghost, Sz::Md, None, move || {
                        let _ = set_clipboard(pl.clone());
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
    let mut list = view().px(sp(S6)).py(sp(S4)).gap(sp(S4));
    for (i, (v, notes)) in changes.into_iter().enumerate() {
        if i > 0 {
            list = list.child(view().h(px(1.0)).bg(BORDER()));
        }
        let notes = if notes.trim().is_empty() { t("No notes for this release").to_owned() } else { notes };
        list = list.child(
            view()
                .gap(sp(S2))
                .child(text(format!("v{v}")).text_size(tx(TEXT_LG)).font_weight(FontWeight(600)))
                .child(
                    nsh_base::text_view()
                        // A link opens in the browser (or the app it belongs to).
                        .links(nsh_base::TextViewLinks::Open)
.markdown(move || Some(SharedString::from(notes.clone())))
                        .selectable(true)
                        .gap_2()
                        .text_size(tx(TEXT_BASE))
                        .line_height(px(22.0))
                        .text_color(FG()),
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
    // Framed by the dialog around it.
    view()
        .w(px(620.0))
        .max_w(relative(0.92))
        .max_h(relative(0.86))
        .overflow_hidden()
        .child(
            view()
                .flex_row()
                .items_baseline()
                .gap(sp(S3))
                .px(sp(S6))
                .pt(sp(S6))
                .pb(sp(S4))
                .border_b_1()
                .border_color(BORDER())
                .child(text(tf("Update to {}", &[&format!("v{version}")])).text_size(tx(TEXT_LG)).font_weight(FontWeight(600)))
                .child(text(tf("Changes since {}", &[&format!("v{}", st.current)])).text_size(tx(TEXT_SM)).text_color(FG_MUTED()).flex_1().number_of_lines(1)),
        )
        .child(scroll().flex_shrink(1.0).child(list))
        .child(
            view()
                .flex_row()
                .items_center()
                .justify_end()
                .gap(sp(S2))
                .px(sp(S6))
                .py(sp(S4))
                .border_t_1()
                .border_color(BORDER())
                .child(dynamic(move || {
                    let e = failed.get()?;
                    Some(untracked(move || text(e.clone()).text_size(tx(TEXT_SM)).text_color(DANGER).flex_1().number_of_lines(2).into_element()))
                }))
                .child(btn(t("Later"), Variant::Ghost, Sz::Md, None, store::close_modal))
                .child(primary),
        )
}
