//! Integrations and plugins, a page each. An integration is a service
//! Sophon is connected to: its card says how it stands and takes it the
//! steps to connected (what it asks to be told, its sign-in), and names
//! what it watches for. A plugin is what someone made of integrations
//! to hand on: its card says which it counts on and the automations it
//! brings. One more of either is installed at the foot of its page.

use crate::i18n::{t, tf};
use crate::ui::kit::text;
use nsh_core::{element::FontWeight, prelude::*};

use crate::ui::kit::*;
use crate::{
    daemon::connection::{Plugin, PluginKind},
    store,
};

pub(crate) fn plugins(compact: bool, kind: PluginKind) -> impl IntoElement {
    let of_plugins = kind == PluginKind::Plugin;
    // Asked for when the page opens, and again once the host is back:
    // a list asked of a host not there yet never came.
    effect(|| {
        if matches!(store::conn(), store::Conn::Connected { .. }) {
            store::load_plugins();
        }
    });
    let source = signal(String::new());
    let install = move || {
        // A line of settings someone handed on, or where to install from.
        store::add_or_import_plugin(&source.get_untracked());
        source.set(String::new());
    };
    scroll().flex_1().child(
        page_column(compact)
            .gap(sp(S6))
            .child(
                view()
                    .gap(sp(S2))
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap(sp(S2))
                            .when(compact, |v| v.child(icon_btn(Icon::ArrowLeft, Sz::Sm, t("Back"), store::go_home)))
                            .child(page_title(if of_plugins { t("Plugins") } else { t("Integrations") }).flex_1())
                            .child(icon_btn(Icon::RotateCw, Sz::Sm, t("Refresh"), store::load_plugins)),
                    )
                    .child(
                        text(if of_plugins {
                            t("Automations someone made to hand on, each ready to use. A plugin says which integrations it counts on, and may bring some with it")
                        } else {
                            t("The accounts and services Sophon is connected to. Automations start on what they watch for, and answer through them")
                        })
                            .text_size(tx(TEXT_BASE))
                            .text_color(FG_MUTED()),
                    ),
            )
            .child(dynamic(move || {
                let installed: Vec<Plugin> = store::plugins().into_iter().filter(|plugin| plugin.kind == kind).collect();
                Some(untracked(move || {
                    let mut list = view().gap(sp(S4));
                    if installed.is_empty() {
                        return list.child(
                            view()
                                .items_center()
                                .gap(sp(S2))
                                .py(sp(S8))
                                .child(icon(if of_plugins { Icon::Layers } else { Icon::Workflow }, 24.0).text_color(FG_FAINT()))
                                .child(text(if of_plugins { t("No plugins yet") } else { t("No integrations yet") }).text_size(tx(TEXT_BASE)).text_color(FG_MUTED())),
                        );
                    }
                    for plugin in installed {
                        list = list.child(plugin_card(plugin));
                    }
                    list
                }))
            }))
            // One more: from wherever git reaches, or a folder of the host.
            .child(
                card().p(sp(S4)).gap(sp(S3)).child(section_label(if of_plugins { t("Add a plugin") } else { t("Add an integration") })).child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(
                            field()
                                .flex_1()
                                .text_size(tx(TEXT_SM))
                                .value(move || source.get())
                                .on_change(move |v: &str| source.set(v.to_owned()))
                                .placeholder(if of_plugins { t("A git address, or a folder") } else { t("Settings someone sent you, a git address, or a folder") }),
                        )
                        .child(btn(t("Add"), Variant::Primary, Sz::Md, Some(Icon::Download), install).disabled(move || source.with(|s| s.trim().is_empty())))
                        // Or a file someone handed on: read and shown
                        // before any of it is installed.
                        .child(btn(t("Choose a file…"), Variant::Outline, Sz::Md, Some(Icon::File), store::choose_handed_file)),
                )
                .child(
                    text(if of_plugins {
                        t("A plugin runs its scripts on that computer, and so do the integrations it brings: add only what you trust")
                    } else {
                        t("One someone shared with you, its settings filled in. It runs its scripts on that computer: add only what you trust")
                    })
                    .text_size(tx(TEXT_SM))
                    .text_color(FG_MUTED()),
                )
                // Those that come with Sophon and were removed, to bring back.
                .child(dynamic(move || {
                    let away = store::plugins_away(kind);
                    if away.is_empty() {
                        return None;
                    }
                    Some(untracked(move || {
                        let mut row = view().flex_row().flex_wrap().items_center().gap(sp(S2)).child(small(t("Comes with Sophon:"), FG_MUTED()));
                        for (id, name) in away {
                            row = row.child(btn(name, Variant::Outline, Sz::Sm, Some(Icon::Plus), move || store::restore_plugin(&id)));
                        }
                        row
                    }))
                })),
            ),
    )
}

/// How an integration or a plugin stands, the first thing it lacks
/// before anything else: the words, and their tone.
fn standing(plugin: &Plugin) -> (&'static str, AdaptiveColor) {
    if !plugin.enabled {
        (t("Turned off"), FG_MUTED())
    } else if plugin.error.is_some() {
        (t("Cannot be read"), DANGER)
    } else if !plugin.needs().is_empty() {
        (t("Needs settings"), WARNING)
    } else if plugin.setup && plugin.account.is_none() {
        (t("Needs setting up"), WARNING)
    } else if plugin.kind == PluginKind::Plugin {
        // A plugin is as ready as what it counts on.
        if plugin.requires.iter().all(|need| need.ready) { (t("Ready"), SUCCESS) } else { (t("Needs an integration"), WARNING) }
    } else {
        // Set up is all an integration gets to be: what runs is the
        // automations', on their own page.
        (t("Connected"), SUCCESS)
    }
}

fn small(words: impl Into<SharedString>, tone: AdaptiveColor) -> nsh_core::element::Text {
    text(words.into()).text_size(tx(TEXT_SM)).text_color(tone)
}

/// An automation's own settings of its trigger, each a line of the
/// trigger's card: what it is on the leading side, its control trailing.
/// What is said goes to the automation's draft, kept when it is saved.
pub(crate) fn param_rows(plugin_id: &str, params: &[crate::daemon::connection::PluginParam]) -> View {
    let mut rows = view();
    for param in params {
        // Kept under whose it is: a trigger's and an automation's may be
        // called the same.
        let (id, name) = (plugin_id.to_owned(), param.of.key(&param.name));
        let given = param.value.clone();
        let read = {
            let name = name.clone();
            move || store::option_draft(&name).unwrap_or_else(|| given.clone())
        };
        let write = move |value: String| store::set_option_draft(&name, value);
        let words = view()
            .flex_1()
            .min_w(px(0.0))
            .gap(px(2.0))
            .child(text(param.label.clone()).text_size(tx(TEXT_BASE)))
            .when(!param.hint.is_empty(), |v| v.child(small(param.hint.clone(), FG_MUTED())));
        let line = view().px(sp(S4)).py(sp(S3)).border_t_1().border_color(BORDER());
        rows = rows.child(match param.kind.as_str() {
            "switch" => {
                let now = read.clone();
                line.flex_row().items_center().gap(sp(S4)).child(words).child(
                    switch(move || matches!(now().as_str(), "yes" | "true" | "1" | "on"), move |next| write(if next { "yes" } else { "no" }.to_owned())).accessibility_label(param.label.clone()),
                )
            }
            // Some of a list: how many are picked beside the list to pick
            // from, and under it each one picked, to take away.
            "multi" => {
                let (asked, optional) = (param.clone(), param.optional);
                let (counted, listed, opened) = (read.clone(), read.clone(), read.clone());
                let picked = |value: String| -> Vec<String> { value.split(',').map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned).collect() };
                line.gap(sp(S2))
                    .child(
                        view()
                            .flex_row()
                            .items_center()
                            .gap(sp(S3))
                            .child(words)
                            .child(
                                text(move || -> SharedString {
                                    match picked(counted()).len() {
                                        0 if optional => t("Any").into(),
                                        0 => t("None picked yet").into(),
                                        n => tf("{} picked", &[&n.to_string()]).into(),
                                    }
                                })
                                .text_size(tx(TEXT_SM))
                                .text_color(FG_MUTED()),
                            )
                            .child(btn(t("Choose…"), Variant::Outline, Sz::Sm, Some(Icon::ListFilter), move || {
                                // The list opens on what is picked now.
                                let mut asked = asked.clone();
                                asked.value = opened();
                                store::open_modal(store::Modal::PluginChoices { plugin: id.clone(), param: asked, draft: true })
                            })),
                    )
                    .child(dynamic(move || {
                        let all = picked(listed());
                        if all.is_empty() {
                            return None;
                        }
                        let write = write.clone();
                        Some(untracked(move || {
                            let mut chosen = view().flex_row().flex_wrap().items_center().gap(sp(S2));
                            for one in &all {
                                let rest: Vec<String> = all.iter().filter(|other| *other != one).cloned().collect();
                                let write = write.clone();
                                chosen = chosen.child(
                                    nsh_base::button()
                                        .on_press(move || write(rest.join(",")))
                                        .accessibility_label(t("Remove"))
                                        .flex_row()
                                        .items_center()
                                        .gap(px(6.0))
                                        .h(sp(CONTROL_SM))
                                        .px(sp(S3))
                                        .rounded(px(R_FULL))
                                        .bg(tint(FG(), 0.06))
                                        .hover(|style| style.bg(tint(FG(), 0.1)))
                                        .child(text(one.clone()).text_size(tx(TEXT_SM)).font_family(mono_font))
                                        .child(icon(Icon::X, 12.0).text_color(FG_MUTED())),
                                );
                            }
                            chosen
                        }))
                    }))
            }
            _ => {
                let value = signal(untracked(read));
                line.flex_row().items_center().gap(sp(S4)).child(words).child(
                    field()
                        .w(px(260.0))
                        .h(sp(CONTROL_SM + 6.0))
                        .text_size(tx(TEXT_SM))
                        .value(move || value.get())
                        .on_change(move |v: &str| {
                            value.set(v.to_owned());
                            write(v.trim().to_owned());
                        })
                        .placeholder(if param.optional { t("Optional") } else { t("Not filled in yet") }),
                )
            }
        });
    }
    rows
}

/// The fields of some of a plugin's parameters, each asked for as its
/// kind: a switch, some of a list, or words to type. For the plugin
/// itself a switch and a list keep themselves and what was typed is
/// kept when asked; `draft`: for the automation in the editor, where
/// all of it is kept with the automation when that is saved.
pub(crate) fn param_fields(plugin_id: &str, params: &[crate::daemon::connection::PluginParam], draft: bool) -> View {
    let mut form = view().gap(sp(S4));
    let mut typed = Vec::new();
    for param in params {
        let (id, name) = (plugin_id.to_owned(), if draft { param.of.key(&param.name) } else { param.name.clone() });
        let hint = (!param.hint.is_empty()).then(|| small(param.hint.clone(), FG_MUTED()));
        // What it is now: the automation's own word when it has said, else the plugin's.
        let given = param.value.clone();
        let read = {
            let name = name.clone();
            move || if draft { store::option_draft(&name).unwrap_or_else(|| given.clone()) } else { given.clone() }
        };
        let write = {
            let (id, name) = (id.clone(), name.clone());
            move |value: String| {
                if draft {
                    store::set_option_draft(&name, value);
                } else {
                    store::set_plugin_params(&id, vec![(name.clone(), value)]);
                }
            }
        };
        form = form.child(match param.kind.as_str() {
            // On or off, kept as soon as it is pressed.
            "switch" => {
                let now = read.clone();
                view()
                    .flex_row()
                    .items_center()
                    .gap(sp(S3))
                    .child(view().flex_1().gap(px(2.0)).child(text(param.label.clone()).text_size(tx(TEXT_SM))).when_some(hint, |v, hint| v.child(hint)))
                    .child(switch(move || matches!(now().as_str(), "yes" | "true" | "1" | "on"), move |next| write(if next { "yes" } else { "no" }.to_owned())).accessibility_label(param.label.clone()))
            }
            // Some of a list: those picked, each to take away, and the list to pick from.
            "multi" => {
                let asked = param.clone();
                let optional = param.optional;
                let chosen = dynamic(move || {
                    let value = read();
                    let (write, id, asked) = (write.clone(), id.clone(), asked.clone());
                    Some(untracked(move || {
                        let picked: Vec<String> = value.split(',').map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned).collect();
                        let mut chosen = view().flex_row().flex_wrap().items_center().gap(sp(S2));
                        for one in &picked {
                            let rest: Vec<String> = picked.iter().filter(|other| *other != one).cloned().collect();
                            let write = write.clone();
                            chosen = chosen.child(
                                nsh_base::button()
                                    .on_press(move || write(rest.join(",")))
                                    .accessibility_label(t("Remove"))
                                    .flex_row()
                                    .items_center()
                                    .gap(px(6.0))
                                    .h(sp(CONTROL_SM))
                                    .px(sp(S3))
                                    .rounded(px(R_FULL))
                                    .bg(tint(FG(), 0.06))
                                    .hover(|style| style.bg(tint(FG(), 0.1)))
                                    .child(text(one.clone()).text_size(tx(TEXT_SM)).font_family(mono_font))
                                    .child(icon(Icon::X, 12.0).text_color(FG_MUTED())),
                            );
                        }
                        if picked.is_empty() {
                            chosen = chosen.child(small(if optional { t("Any") } else { t("None picked yet") }, FG_MUTED()));
                        }
                        // The list opens on what is picked now.
                        let mut asked = asked.clone();
                        asked.value = value;
                        chosen.child(btn(t("Choose…"), Variant::Outline, Sz::Sm, Some(Icon::ListFilter), move || {
                            store::open_modal(store::Modal::PluginChoices { plugin: id.clone(), param: asked.clone(), draft })
                        }))
                    }))
                });
                view().gap(px(6.0)).child(text(param.label.clone()).text_size(tx(TEXT_SM))).when_some(hint, |v, hint| v.child(hint)).child(chosen)
            }
            _ => {
                let value = signal(if param.secret { String::new() } else { untracked(read) });
                let placeholder = match (param.secret, param.filled, param.optional) {
                    (true, true, _) => t("Filled in. Type a new one to replace it"),
                    (_, _, true) => t("Optional"),
                    _ => t("Not filled in yet"),
                };
                if !draft {
                    typed.push((param.name.clone(), value, param.secret));
                }
                view().gap(px(6.0)).child(text(param.label.clone()).text_size(tx(TEXT_SM))).when_some(hint, |v, hint| v.child(hint)).child(
                    field()
                        .h(sp(CONTROL_SM + 6.0))
                        .text_size(tx(TEXT_SM))
                        .value(move || value.get())
                        .on_change(move |v: &str| {
                            value.set(v.to_owned());
                            // The automation's own goes with it as typed.
                            if draft {
                                write(v.trim().to_owned());
                            }
                        })
                        .placeholder(placeholder),
                )
            }
        });
    }
    // What was typed for the plugin itself is kept when asked.
    if !typed.is_empty() {
        let id = plugin_id.to_owned();
        form = form.child(view().flex_row().child(btn(t("Save"), Variant::Outline, Sz::Sm, None, move || {
            // A secret left empty stays as it is; anything else is as typed.
            let values = typed
                .iter()
                .filter_map(|(name, value, secret)| {
                    let value = value.get_untracked().trim().to_owned();
                    (!secret || !value.is_empty()).then(|| (name.clone(), value))
                })
                .collect();
            store::set_plugin_params(&id, values);
        })));
    }
    form
}

/// A step of a plugin's card: its number (a check once done), what the
/// step is, how it stands, and what there is to press.
fn step(number: &'static str, done: bool, title: &'static str, status: impl IntoElement, action: Option<nsh_shadcn::ButtonElement>) -> View {
    let mark = if done {
        view().w(px(20.0)).h(px(20.0)).rounded(px(R_FULL)).bg(tint(SUCCESS, 0.14)).items_center().justify_center().child(icon(Icon::Check, 12.0).text_color(SUCCESS))
    } else {
        view()
            .w(px(20.0))
            .h(px(20.0))
            .rounded(px(R_FULL))
            .bg(tint(FG(), 0.08))
            .items_center()
            .justify_center()
            .child(text(number).text_size(tx(TEXT_XS)).font_weight(FontWeight(500)).text_color(FG_MUTED()))
    };
    let mut row = view()
        .flex_row()
        .items_center()
        .gap(sp(S3))
        .min_h(sp(CONTROL_SM + 4.0))
        .child(mark)
        .child(text(title).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).w(px(72.0)))
        .child(view().flex_1().child(status));
    if let Some(action) = action {
        row = row.child(action);
    }
    row
}

/// What a card is marked with: its picture when it brings one, else its
/// first letter on the colour it goes by, else the sign of its kind.
fn mark(plugin: &Plugin) -> View {
    // A hairline round it: a colour near the card's own still reads as a tile.
    let tile = view().w(px(36.0)).h(px(36.0)).flex_shrink_0().rounded(px(R_LG)).border_1().border_color(tint(FG(), 0.1)).items_center().justify_center();
    if !plugin.icon.is_empty() {
        return tile.overflow_hidden().child(image(plugin.icon.clone()).w(px(36.0)).h(px(36.0)).object_fit(nsh_core::ResizeMode::Cover));
    }
    let colour = plugin.color.strip_prefix('#').and_then(|hex| u32::from_str_radix(hex, 16).ok());
    match (colour, plugin.name.chars().next()) {
        (Some(colour), Some(first)) => tile
            .bg(AdaptiveColor::rgb(colour, colour))
            .child(text(first.to_uppercase().to_string()).text_size(tx(TEXT_LG)).font_weight(FontWeight(600)).text_color(AdaptiveColor::rgb(0xffffff, 0xffffff))),
        _ => tile.bg(tint(ACCENT(), 0.1)).child(icon(if plugin.kind == PluginKind::Plugin { Icon::Layers } else { Icon::Zap }, 18.0).text_color(ACCENT())),
    }
}

fn plugin_card(plugin: Plugin) -> View {
    let (tone_words, tone) = standing(&plugin);
    let (updated, removed) = (plugin.id.clone(), plugin.id.clone());
    let mut head = view().gap(sp(S2)).p(sp(S4)).child(
        view()
            .flex_row()
            .items_center()
            .gap(sp(S3))
            .child(mark(&plugin))
            .child(
                view().flex_1().gap(px(2.0)).child(
                    view()
                        .flex_row()
                        .items_center()
                        .gap(sp(S2))
                        .child(text(plugin.name.clone()).text_size(tx(TEXT_BASE)).font_weight(FontWeight(500)))
                        .when(!plugin.version.is_empty(), |v| v.child(small(format!("v{}", plugin.version), FG_MUTED())))
                        .child(badge(tone_words, tone)),
                ),
            )
            .when(plugin.git, |v| v.child(icon_btn(Icon::RotateCw, Sz::Sm, t("Update"), move || store::update_plugin(&updated))))
            // All of it as a file, to hand on to someone who should have
            // the same. One a plugin brought goes in that plugin's file.
            .when(plugin.error.is_none() && plugin.from.is_empty(), |v| {
                let handed = plugin.id.clone();
                v.child(icon_btn(Icon::Upload, Sz::Sm, t("Save as a file to share"), move || store::open_modal(store::Modal::HandOn { id: handed.clone() })))
            })
            // On or off: off, it stays as it is and nothing of it runs.
            .child({
                let (on, switched) = (plugin.enabled, plugin.id.clone());
                switch(move || on, move |next| store::enable_plugin(&switched, next)).accessibility_label(t("Turned on"))
            })
            // Gone, with what it kept: asked first. One a plugin brought
            // goes when the plugin does.
            .when(plugin.from.is_empty(), |v| {
                let name = plugin.name.clone();
                let label = if plugin.kind == PluginKind::Plugin { t("Remove plugin") } else { t("Remove integration") };
                v.child(icon_btn(Icon::Trash2, Sz::Sm, label, move || store::open_modal(store::Modal::RemovePlugin { id: removed.clone(), name: name.clone() })))
            }),
    );
    if !plugin.description.is_empty() {
        head = head.child(small(plugin.description.clone(), FG_MUTED()));
    }
    if let Some(error) = plugin.error.clone() {
        return card().child(head.child(small(error, DANGER)));
    }
    // Whose it came with, for an integration a plugin brought.
    if !plugin.from.is_empty() {
        let bringer = store::plugins().into_iter().find(|other| other.id == plugin.from).map(|other| other.name).unwrap_or_else(|| plugin.from.clone());
        head = head.child(small(tf("Came with the plugin “{}”: it stays while that does", &[&bringer]), FG_MUTED()));
    }
    // Off: what it is, and no more until it is on again.
    if !plugin.enabled {
        let resting = if plugin.kind == PluginKind::Plugin {
            t("Automations made from it do nothing, until it is turned on again")
        } else {
            t("Its triggers start nothing and nothing can send through it, until it is turned on again")
        };
        return card().child(head.child(small(resting, FG_MUTED())));
    }
    // Whose it works as, when it has no sign-in of its own to say so.
    if !plugin.setup
        && let Some(account) = plugin.account.clone()
    {
        head = head.child(small(account, FG_MUTED()));
    }
    // The integrations a plugin counts on, and how each stands: each a
    // way to where it is set up.
    for need in &plugin.requires {
        let (words, tone) = if need.ready {
            (tf("Uses {}: ready", &[&need.name]), FG_MUTED())
        } else if need.installed {
            (tf("Uses {}: set that integration up first", &[&need.name]), WARNING)
        } else {
            (tf("Uses {}: not installed, so that part is skipped", &[&need.name]), WARNING)
        };
        head = head.child(
            nsh_base::button()
                .on_press(store::open_plugins)
                .accessibility_label(t("Open Integrations"))
                .self_start()
                .flex_row()
                .items_center()
                .gap(px(4.0))
                .rounded(px(R_LG))
                .hover(|style| style.bg(highlight()))
                .child(small(words, tone))
                .child(icon(Icon::ChevronRight, 12.0).text_color(FG_FAINT())),
        );
    }
    let mut steps = view().gap(sp(S3)).p(sp(S4)).border_t_1().border_color(BORDER());
    let needs = plugin.needs();
    let configured = needs.is_empty();

    // 1. What it asks to be told, once for everything that uses it.
    // Filled in (by whoever shared it, or here), it is a line.
    let own = plugin.params.clone();
    if !own.is_empty() {
        let pressed = plugin.id.clone();
        let open = !configured || store::plugin_open(&plugin.id);
        let status = small(if configured { t("Filled in").to_owned() } else { format!("{}{}", t("To fill in: "), needs.join(", ")) }, if configured { FG_MUTED() } else { WARNING });
        steps = steps.child(step(
            "1",
            configured,
            t("Settings"),
            status,
            configured.then(|| btn(if open { t("Close") } else { t("Change") }, Variant::Ghost, Sz::Sm, None, move || store::toggle_plugin_open(&pressed, !store::plugin_open(&pressed)))),
        ));
        if open {
            steps = steps.child(param_fields(&plugin.id, &own, false).pl(px(20.0 + S3)));
        }
    }

    // 2. Its setup: a sign-in in the host's browser.
    let signed = !plugin.setup || plugin.account.is_some();
    if plugin.setup {
        let (shown, pressed, held) = (plugin.id.clone(), plugin.id.clone(), plugin.id.clone());
        let account = plugin.account.clone();
        let label: SharedString = match (&plugin.account, plugin.setup_label.is_empty()) {
            (Some(_), _) => t("Again").into(),
            (None, true) => t("Set up").into(),
            (None, false) => plugin.setup_label.clone().into(),
        };
        let status = dynamic(move || {
            let (busy, failed) = (store::plugin_busy(&shown), store::plugin_failed(&shown));
            let account = account.clone();
            Some(untracked(move || {
                if busy {
                    small(t("Finish it in the browser that opened on that computer…"), FG_MUTED())
                } else if let Some(error) = failed {
                    small(error, DANGER)
                } else if let Some(account) = account {
                    small(account, FG_MUTED())
                } else {
                    small(t("Not set up yet"), WARNING)
                }
            }))
        });
        let variant = if signed || !configured { Variant::Ghost } else { Variant::Primary };
        steps = steps.child(step(
            "2",
            signed,
            t("Sign-in"),
            status,
            Some(btn(label, variant, Sz::Sm, None, move || store::setup_plugin(&pressed)).disabled(move || !configured || store::plugin_busy(&held))),
        ));
    }

    // The skills a plugin brings: what the agents of its automations
    // know how to do, by name and what each is for.
    if !plugin.skills.is_empty() {
        let mut list = view().gap(px(4.0)).child(small(t("Skills it brings: "), FG_MUTED()));
        for (name, about) in &plugin.skills {
            list = list.child(
                view()
                    .flex_row()
                    .items_start()
                    .gap(sp(S2))
                    .child(icon(Icon::Sparkles, 13.0).text_color(FG_FAINT()))
                    .child(view().flex_1().min_w(px(0.0)).gap(px(2.0)).child(text(name.clone()).text_size(tx(TEXT_SM)).font_family(mono_font)).when(!about.is_empty(), |v| v.child(small(about.clone(), FG_MUTED()).number_of_lines(2)))),
            );
        }
        steps = steps.child(list);
    }
    // The MCP servers it brings them: what those agents can call.
    if !plugin.mcp.is_empty() {
        steps = steps.child(
            view()
                .flex_row()
                .items_start()
                .gap(sp(S2))
                .child(view().pt(px(2.0)).child(icon(Icon::Server, 13.0).text_color(FG_FAINT())))
                .child(view().flex_1().min_w(px(0.0)).child(small(format!("{}{}", t("MCP servers it brings: "), plugin.mcp.join(", ")), FG_MUTED()))),
        );
    }
    // What it watches for is started on from the automations page; so are
    // the automations a plugin brings.
    let offered: Vec<String> = if plugin.kind == PluginKind::Plugin { plugin.automations.iter().map(|automation| automation.name.clone()).collect() } else { plugin.triggers.iter().map(|trigger| trigger.label.clone()).collect() };
    if !offered.is_empty() {
        let lead = if plugin.kind == PluginKind::Plugin { t("Automations it brings: ") } else { t("Triggers it brings: ") };
        steps = steps.child(
            view()
                .flex_row()
                .items_center()
                .gap(sp(S3))
                .child(view().flex_1().child(small(format!("{lead}{}", offered.join(", ")), FG_MUTED())))
                .child(btn(t("New automation"), Variant::Outline, Sz::Sm, Some(Icon::Plus), store::open_schedules)),
        );
    }
    card().child(head).child(steps)
}
