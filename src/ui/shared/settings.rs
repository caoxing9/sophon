//! Settings sections: each a label over a card of rows,
//! controls in the rows' trailing slot. The desktop and the phone each
//! assemble their page from them.

use crate::i18n::{Lang, t};
use nsh_core::{
    element::{FontWeight, View},
    prelude::*,
};
use crate::ui::kit::text;

use crate::ui::kit::*;
use crate::store::{self, Conn, SendBehavior};

/// Theme and language.
pub(crate) fn appearance_card() -> View {
    card()
        .child(row(
            t("Theme"),
            t("Light, dark, or as the system is set"),
            segmented(
                vec![
                    (nsh_core::platform::Appearance::System, t("System").into()),
                    (nsh_core::platform::Appearance::Light, t("Light").into()),
                    (nsh_core::platform::Appearance::Dark, t("Dark").into()),
                ],
                store::appearance,
                store::set_appearance,
            ),
            true,
        ))
        .child(row(
            t("Language"),
            t("The language of Sophon's interface"),
            segmented(
                vec![(Lang::Auto, t("System").into()), (Lang::En, "English".into()), (Lang::Zh, "中文".into())],
                crate::i18n::lang,
                store::set_language,
            ),
            false,
        ))
}

/// What Return does while an agent runs.
pub(crate) fn sending_card() -> View {
    card().child(row(
        t("While an agent is running"),
        t("What Return does with your message"),
        segmented(
            vec![
                (SendBehavior::Steer, t("Steer").into()),
                (SendBehavior::Interrupt, t("Interrupt").into()),
                (SendBehavior::Queue, t("Queue").into()),
            ],
            || store::settings().send_behavior,
            |b| store::edit_settings(|s| s.send_behavior = b),
        ),
        true,
    ))
}

/// The settings page frame: a centered column under the title.
pub(crate) fn page(compact: bool, sections: View) -> impl IntoElement {
    scroll().flex_1().child(
        view()
            .w_full()
            .max_w(px(720.0))
            .self_center()
            .px(px(if compact { S4 } else { S6 }))
            .py(px(if compact { S4 } else { S8 }))
            .gap(px(if compact { S6 } else { S8 }))
            .child(
                view()
                    .flex_row()
                    .items_center()
                    .gap(px(S2))
                    .child(text(t("Settings")).text_size(px(TEXT_2XL)).font_weight(FontWeight(500))),
            )
            .child(sections.gap(px(if compact { S6 } else { S8 }))),
    )
}

pub(crate) fn section(title: &'static str, body: impl IntoElement) -> View {
    view().gap(px(S3)).child(section_label(title)).child(body)
}

/// A settings row: title and hint on the leading rail, the control trailing.
pub(crate) fn row(title: impl IntoReactive<SharedString>, hint: impl IntoReactive<SharedString>, control: impl IntoElement, first: bool) -> View {
    view()
        .flex_row()
        .items_center()
        .gap(px(S4))
        .px(px(S4))
        .py(px(S4))
        .when(!first, |v| v.border_t_1().border_color(BORDER))
        .child(
            view()
                .flex_1()
                .gap(px(2.0))
                .child(text(title).text_size(px(TEXT_BASE)).text_color(FG))
                .child(text(hint).text_size(px(TEXT_SM)).text_color(FG_MUTED)),
        )
        .child(control)
}

pub(crate) fn host_card() -> View {
    let conn = computed(store::conn);
    let field = move |f: fn(&Conn) -> String| move || conn.with(f);
    card()
        .child(row(
            field(|c| match c {
                Conn::Connected { hostname, .. } => hostname.clone(),
                Conn::Down(_) => t("Offline").into(),
                _ => t("Connecting…").into(),
            }),
            field(|c| match c {
                Conn::Connected { url, .. } => url.clone(),
                Conn::Down(reason) => reason.clone(),
                _ => String::new(),
            }),
            status_dot(move || match conn.get() {
                Conn::Connected { .. } => Status::Done,
                Conn::Down(_) => Status::Failed,
                _ => Status::Idle,
            }),
            true,
        ))
        .child(row(
            t("Service version"),
            t("The Sophon service this app talks to"),
            text(field(|c| match c {
                Conn::Connected { version, .. } if !version.is_empty() => version.clone(),
                _ => "—".into(),
            }))
            .text_size(px(TEXT_BASE))
            .font_family(|| MONO)
            .text_color(FG_MUTED),
            false,
        ))
}

fn flag(pointer: &'static str, default: bool) -> impl IntoElement {
    switch(
        move || store::daemon_config().and_then(|c| c.pointer(pointer).and_then(serde_json::Value::as_bool)).unwrap_or(default),
        move |on| store::set_daemon_config(pointer, on.into()),
    )
}

pub(crate) fn behavior_card() -> impl IntoElement {
    let prompt = signal(String::new());
    let loaded = computed(|| store::daemon_config().is_some());
    effect(move || {
        if let Some(p) = store::daemon_config().and_then(|c| c.get("appendSystemPrompt").and_then(|p| p.as_str().map(str::to_owned))) {
            untracked(|| prompt.set(p));
        }
    });
    view().child(dynamic(move || {
        if !loaded.get() {
            return Some(untracked(|| text(t("Loading…")).text_color(FG_MUTED).into_element()));
        }
        Some(untracked(move || {
            card()
                .child(row(t("Inject Sophon tools"), t("Agents can open terminals, start agents and more through MCP"), flag("/mcp/injectIntoAgents", true), true))
                .child(row(t("Archive after merge"), t("Archive a workspace once its branch is merged"), flag("/autoArchiveAfterMerge", false), false))
                .child(row(t("Free idle agents"), t("Stop idle agent runtimes to save memory; they resume on demand"), flag("/idleRuntimeCleanup", false), false))
                .child(row(t("Resume on startup"), t("Pick up running agents when the service restarts"), flag("/resumeOnStartup", false), false))
                .child(row(t("Usage reports"), t("Collect token and cost usage per provider"), flag("/usage/enabled", false), false))
                .child(row(t("Remote relay"), t("Reach this host from your phone through the encrypted relay"), flag("/relay/enabled", true), false))
                .child(
                    view()
                        .px(px(S4))
                        .py(px(S4))
                        .gap(px(S2))
                        .border_t_1()
                        .border_color(BORDER)
                        .child(text(t("Extra system prompt")).text_size(px(TEXT_BASE)))
                        .child(text(t("Appended to every agent's instructions")).text_size(px(TEXT_SM)).text_color(FG_MUTED))
                        .child(
                            field()
                                .multiline(true)
                                .h(px(88.0))
                                .py(px(S2))
                                .value(move || prompt.get())
                                .on_change(move |v: &str| prompt.set(v.to_owned()))
                                .placeholder(t("e.g. Answer in Chinese. Run the tests before you finish")),
                        )
                        .child(
                            view().flex_row().justify_end().child(btn(t("Save prompt"), Variant::Secondary, Sz::Sm, None, move || {
                                store::set_daemon_config("/appendSystemPrompt", prompt.get_untracked().into())
                            })),
                        ),
                )
                .child(row(
                    t("Restart service"),
                    t("Restarts the service. Sophon reconnects on its own"),
                    btn(t("Restart"), Variant::Outline, Sz::Sm, Some(Icon::RotateCw), || store::open_modal(store::Modal::RestartDaemon)),
                    false,
                ))
                .into_element()
        }))
    }))
}
