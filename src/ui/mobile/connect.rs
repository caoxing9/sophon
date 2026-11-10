//! With no computer paired, the welcome screen: the mark, a line on
//! what to do, and the ways to connect (scanning the computer's QR code
//! first, then a direct address or a pasted pairing link; the computers on
//! this network, to pair without a QR code), settings and the
//! version under them. Scanning opens the camera in a card with a
//! corner frame; a pairing code connects at once.

use crate::i18n::t;
use nsh_base::button;
use nsh_core::{element::FontWeight, prelude::*};

use crate::store;
use crate::ui::kit::text;
use crate::ui::kit::*;

#[derive(Clone, Copy, PartialEq)]
enum Step {
    Welcome,
    Scan,
}

pub(crate) fn connect() -> impl IntoElement {
    let step = signal(Step::Welcome);
    view().flex_1().child(dynamic(move || {
        let s = step.get();
        Some(untracked(move || match s {
            Step::Welcome => welcome(move || step.set(Step::Scan)).into_element(),
            Step::Scan => scan(move || step.set(Step::Welcome)).into_element(),
        }))
    }))
}

fn welcome(open_scan: impl Fn() + Clone + 'static) -> impl IntoElement {
    scroll().flex_1().child(
        view()
            .min_h(move || Length::from(px(window().height.0 - safe_area().top.0 - safe_area().bottom.0)))
            .px(px(S6))
            .pb(px(S6))
            .items_center()
            .child(
                view()
                    .flex_1()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .child(mark(96.0, Lift::Large))
                    .child(
                        view()
                            .items_center()
                            .gap(px(S2))
                            .mt(px(S4))
                            .mb(px(48.0))
                            .child(text(t("Welcome to Sophon")).text_size(px(TEXT_BASE)).font_weight(FontWeight(500)).text_center())
                            .child(text(t("Connect your computer to get started")).text_size(px(TEXT_BASE)).text_color(FG_MUTED).text_center()),
                    )
                    .child(
                        view()
                            .w_full()
                            .max_w(px(420.0))
                            .gap(px(S3))
                            .child(action(Icon::QrCode, t("Scan QR code"), true, open_scan))
                            .child(action(Icon::Link2, t("Direct connection"), false, || store::open_modal(store::Modal::AddHost)))
                            .child(action(Icon::ClipboardPaste, t("Paste pairing link"), false, || store::open_modal(store::Modal::AddHost))),
                    )
                    .child(view().w_full().max_w(px(420.0)).mt(px(S6)).child(crate::ui::shared::modals::nearby_list()))
                    .child(saved_hosts())
                    .child(view().mt(px(S6)).child(btn(t("Settings"), Variant::Ghost, Sz::Md, Some(Icon::Settings), store::open_settings))),
            )
            .child(
                text(format!("v{}", crate::update::current_version()))
                    .mt(px(S6))
                    .text_size(px(TEXT_SM))
                    .text_color(FG_MUTED)
                    .text_center(),
            ),
    )
}

/// One of the welcome screen's ways to connect: a full-width, 52 pt row.
fn action(glyph: Icon, label: &'static str, primary: bool, on_press: impl Fn() + 'static) -> impl IntoElement {
    let ink = if primary { ACCENT_FG } else { FG };
    button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(S3))
        .py(px(S4))
        .rounded(px(R_XL))
        .bg(if primary { ACCENT } else { SURFACE2 })
        .border_1()
        .border_color(if primary { ACCENT } else { BORDER })
        .active(|s| s.opacity(0.75))
        .child(icon(glyph, 18.0).text_color(ink))
        .child(text(label).text_size(px(TEXT_BASE)).font_weight(FontWeight(500)).text_color(ink))
}

/// Computers paired before, to connect again.
fn saved_hosts() -> impl IntoElement {
    view().w_full().max_w(px(420.0)).child(dynamic(|| {
        let hosts = store::hosts();
        if hosts.is_empty() {
            return None;
        }
        Some(untracked(move || {
            let mut c = card();
            for (i, h) in hosts.iter().enumerate() {
                let id = h.id.clone();
                c = c.child(
                    button()
                        .on_press(move || store::switch_host(Some(id.clone())))
                        .flex_row()
                        .items_center()
                        .gap(px(S3))
                        .h(px(52.0))
                        .px(px(S4))
                        .when(i > 0, |v| v.border_t_1().border_color(BORDER))
                        .active(|s| s.bg(highlight()))
                        .child(icon(Icon::Server, 18.0).text_color(FG_MUTED))
                        .child(text(h.label()).text_size(px(TEXT_BASE)).number_of_lines(1).flex_1())
                        .child(icon(Icon::ChevronRight, 16.0).text_color(FG_FAINT)),
                );
            }
            view().mt(px(S6)).gap(px(S2)).child(section_label(t("Paired before"))).child(c).into_element()
        }))
    }))
}

#[derive(Clone, PartialEq)]
enum ScanState {
    Looking,
    Pairing,
    Failed(String),
    Denied,
}

fn scan(back: impl Fn() + Clone + 'static) -> impl IntoElement {
    let state = signal(ScanState::Looking);
    let on_event = move |e: &NativeEvent| match &*e.name {
        "scan" => {
            let value = e.data.get("value").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
            if crate::daemon::relay::PairingLink::parse(&value).is_some() {
                state.set(ScanState::Pairing);
                store::add_host(t("My computer"), &value, "");
            } else {
                state.set(ScanState::Failed(t("That isn't a Sophon pairing code").into()));
            }
        }
        "denied" => state.set(ScanState::Denied),
        "unavailable" => state.set(ScanState::Failed(t("This device has no camera").into())),
        _ => {}
    };
    view()
        .flex_1()
        .child(
            view()
                .flex_row()
                .items_center()
                .h(px(56.0))
                .px(px(S2))
                .child(icon_btn(Icon::ChevronLeft, Sz::Md, t("Back"), back))
                .child(text(t("Scan QR code")).text_size(px(TEXT_LG)).font_weight(FontWeight(600))),
        )
        .child(
            view().flex_1().px(px(S6)).pb(px(S6)).child(dynamic(move || {
                let denied = state.with(|s| *s == ScanState::Denied);
                Some(untracked(move || {
                    if denied {
                        return permission_card().into_element();
                    }
                    view()
                        .flex_1()
                        .rounded(px(R_XL))
                        .overflow_hidden()
                        .bg(SURFACE2)
                        .child(native(crate::platform::QR_SCANNER).flex_1().on_native_event(on_event))
                        .child(
                            view()
                                .absolute()
                                .top(px(0.0))
                                .bottom(px(0.0))
                                .left(px(0.0))
                                .right(px(0.0))
                                .items_center()
                                .justify_center()
                                .pointer_events(|| nsh_core::PointerEvents::BoxNone)
                                .child(frame())
                                .child(
                                    text(move || match state.get() {
                                        ScanState::Pairing => t("Pairing…").to_owned(),
                                        ScanState::Failed(e) => e,
                                        _ => t("Point the camera at the QR code in Sophon on your computer").to_owned(),
                                    })
                                    .mt(px(S6))
                                    .px(px(S6))
                                    .text_size(px(TEXT_BASE))
                                    .text_color(rgb(0xffffff))
                                    .text_center(),
                                ),
                        )
                        .into_element()
                }))
            })),
        )
}

/// The scan frame: four accent corners around a 260 pt square.
fn frame() -> impl IntoElement {
    let corner = |top: bool, left: bool| {
        let mut v = view().absolute().w(px(36.0)).h(px(36.0)).border_color(ACCENT);
        v = if top { v.top(px(0.0)).border_t(px(4.0)) } else { v.bottom(px(0.0)).border_b(px(4.0)) };
        v = if left { v.left(px(0.0)).border_l(px(4.0)) } else { v.right(px(0.0)).border_r(px(4.0)) };
        match (top, left) {
            (true, true) => v.rounded_tl(px(12.0)),
            (true, false) => v.rounded_tr(px(12.0)),
            (false, true) => v.rounded_bl(px(12.0)),
            (false, false) => v.rounded_br(px(12.0)),
        }
    };
    view()
        .w(px(260.0))
        .h(px(260.0))
        .child(corner(true, true))
        .child(corner(true, false))
        .child(corner(false, true))
        .child(corner(false, false))
}

fn permission_card() -> impl IntoElement {
    view()
        .mt(px(S6))
        .p(px(S6))
        .rounded(px(R_XL))
        .bg(SURFACE2)
        .gap(px(S4))
        .child(text(t("Camera permission")).text_size(px(TEXT_BASE)).font_weight(FontWeight(600)))
        .child(
            text(t("Allow camera access to scan the pairing QR code from your computer."))
                .text_size(px(TEXT_BASE))
                .line_height(px(20.0))
                .text_color(FG_MUTED),
        )
        .child(view().items_start().child(btn(t("Open Settings"), Variant::Primary, Sz::Md, None, || {
            let _ = open_url("app-settings:");
        })))
}
