//! Sophon's controls on `nsh/base`'s unstyled ones, following Paseo's rules:
//! pill buttons in five variants (one accent CTA per surface), two sizes,
//! icon buttons as circles with a translucent highlight, fields on a muted
//! fill, segmented controls whose selected segment is a raised surface.

#![allow(dead_code)]

use crate::i18n::t;
use nsh_base::{ButtonElement, button, input};
use nsh_core::element::Input as InputElement;
use nsh_core::{
    element::{AdaptiveColor, FontWeight, View},
    prelude::*,
};

use super::{
    icon::{Icon, icon},
    tokens::*,
};

/// A text in the theme's foreground: NSH's texts are black unless told
/// otherwise and do not inherit a view's color, which hides them in dark
/// mode. A `.text_color(..)` after it still wins.
pub(crate) fn text(content: impl IntoReactive<SharedString>) -> nsh_core::element::Text {
    nsh_core::element::text(content).text_color(FG)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Variant {
    /// The one primary action on a surface, filled with the accent.
    Primary,
    /// The paired / default action.
    Secondary,
    /// A low-frequency action on a row.
    Outline,
    /// Chrome: back, toggles, "Load more".
    Ghost,
    /// Only inside a confirmation.
    Destructive,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Sz {
    Md,
    Sm,
}

impl Sz {
    fn height(self) -> f32 {
        match self {
            Sz::Md => CONTROL_MD,
            Sz::Sm => CONTROL_SM,
        }
    }
    fn text(self) -> f32 {
        match self {
            Sz::Md => TEXT_BASE,
            Sz::Sm => TEXT_SM,
        }
    }
}

fn fill(variant: Variant) -> Option<AdaptiveColor> {
    match variant {
        Variant::Primary => Some(ACCENT),
        Variant::Secondary => Some(SURFACE3),
        Variant::Destructive => Some(DESTRUCTIVE),
        Variant::Outline | Variant::Ghost => None,
    }
}

fn ink(variant: Variant) -> AdaptiveColor {
    match variant {
        Variant::Primary | Variant::Destructive => ACCENT_FG,
        _ => FG,
    }
}

/// A labelled pill button, optionally with a leading icon.
pub(crate) fn btn(
    label: impl IntoReactive<SharedString>,
    variant: Variant,
    size: Sz,
    lead: Option<Icon>,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let mut b = button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .h(px(size.height()))
        .px(px(if size == Sz::Md { 14.0 } else { 10.0 }))
        .rounded(px(R_FULL))
        .active(|s| s.opacity(0.7))
        .disabled_style(|s| s.opacity(0.45));
    b = match fill(variant) {
        Some(color) => b.bg(color).hover(|s| s.opacity(0.88)),
        None => b.hover(|s| s.bg(highlight())),
    };
    if variant == Variant::Outline {
        b = b.border_1().border_color(BORDER_STRONG);
    }
    if let Some(lead) = lead {
        b = b.child(icon(lead, if size == Sz::Md { 16.0 } else { 14.0 }).text_color(ink(variant)));
    }
    b.child(
        text(label)
            .text_size(px(size.text()))
            .font_weight(FontWeight(500))
            .text_color(ink(variant))
            .number_of_lines(1),
    )
}

/// An icon-only button: a circle with the translucent highlight on hover.
pub(crate) fn icon_btn(glyph: Icon, size: Sz, label: &'static str, on_press: impl Fn() + 'static) -> ButtonElement {
    let box_size = size.height();
    button()
        .on_press(on_press)
        .accessibility_label(label)
        .w(px(box_size))
        .h(px(box_size))
        .items_center()
        .justify_center()
        .rounded(px(R_FULL))
        .hover(|s| s.bg(highlight()))
        .active(|s| s.bg(highlight()).opacity(0.7))
        .disabled_style(|s| s.opacity(0.4))
        .child(icon(glyph, if size == Sz::Md { 18.0 } else { 16.0 }).text_color(FG_MUTED))
}

/// A single-line text field on the muted fill.
pub(crate) fn field() -> InputElement {
    input()
        .h(px(CONTROL_MD + 4.0))
        .px(px(S3))
        .rounded(px(R_LG))
        .bg(SURFACE2)
        .border_1()
        .border_color(BORDER)
        .text_size(px(TEXT_BASE))
        .text_color(FG)
        .caret_color(|| color(ACCENT))
        .selection_color(|| color(tint(ACCENT, 0.3)))
}

/// A segmented control: the selected segment is a raised surface, not an
/// inverted one.
pub(crate) fn segmented<T: Clone + PartialEq + 'static>(
    options: Vec<(T, SharedString)>,
    selected: impl Fn() -> T + 'static,
    on_select: impl Fn(T) + Clone + 'static,
) -> View {
    let selected = computed(selected);
    let values: Vec<T> = options.iter().map(|(v, _)| v.clone()).collect();
    // Where each option sits in the row (x, width), measured.
    let frames = signal(vec![(0.0f32, 0.0f32); options.len()]);
    let at = computed(move || {
        let s = selected.get();
        let i = values.iter().position(|v| *v == s).unwrap_or(0);
        frames.with(|f| f.get(i).copied().unwrap_or_default())
    });
    // One thumb under the options that slides to the picked one, springing
    // as iOS's segmented control does, rather than each option lighting up.
    let thumb = view()
        .absolute()
        .top(px(3.0))
        .bottom(px(3.0))
        .left(move || Length::from(px(at.get().0)))
        .w(move || Length::from(px(at.get().1)))
        .opacity(move || if at.get().1 > 0.0 { 1.0_f32 } else { 0.0 })
        .rounded(px(R_FULL))
        .bg(SURFACE0)
        .border_1()
        .border_color(BORDER);
    let mut row = view().flex_row().p(px(3.0)).gap(px(2.0)).rounded(px(R_FULL)).bg(SURFACE2).self_start().child(thumb);
    for (i, (value, label)) in options.into_iter().enumerate() {
        let is_on = {
            let value = value.clone();
            computed(move || selected.get() == value)
        };
        let pick = on_select.clone();
        row = row.child(view().on_layout(move |b: &Bounds<Points>| {
            let frame = (b.origin.x.0, b.size.width.0);
            if frames.with_untracked(|f| f.get(i) != Some(&frame)) {
                frames.update(|f| f[i] = frame);
            }
        }).child(
            button()
                .on_press(move || {
                    animate_layout(nsh_core::LayoutAnimation {
                        create: None,
                        update: Some(nsh_core::LayoutTransition::new(nsh_core::LayoutCurve::Spring { damping: 0.86 }, 320.0)),
                        delete: None,
                    });
                    pick(value.clone())
                })
                .h(px(CONTROL_SM - 2.0))
                .px(px(S3))
                .items_center()
                .justify_center()
                .rounded(px(R_FULL))
                .hover(|s| s.opacity(0.85))
                .child(
                    text(label)
                        .text_size(px(TEXT_SM))
                        .font_weight(FontWeight(500))
                        .text_color(move || if is_on.get() { FG } else { FG_MUTED })
                        .transition_colors(),
                ),
        ));
    }
    row
}

trait ShadowIf {
    fn shadow_sm_if(self, on: Memo<bool>) -> Self;
}

impl ShadowIf for ButtonElement {
    fn shadow_sm_if(self, on: Memo<bool>) -> Self {
        self.border_1().border_color(move || if on.get() { BORDER } else { AdaptiveColor::rgb(0, 0).opacity(0.0) })
    }
}

/// A selectable chip (provider, project, option lists).
pub(crate) fn chip(
    label: impl IntoReactive<SharedString>,
    lead: Option<Icon>,
    selected: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let selected = computed(selected);
    let mut b = button()
        .on_press(on_press)
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .h(px(CONTROL_MD))
        .px(px(S3))
        .rounded(px(R_FULL))
        .border_1()
        .border_color(move || if selected.get() { ACCENT } else { BORDER })
        .bg(move || if selected.get() { tint(ACCENT, 0.08) } else { SURFACE0 })
        .hover(|s| s.bg(highlight()))
        .active(|s| s.opacity(0.7))
        .disabled_style(|s| s.opacity(0.4));
    if let Some(lead) = lead {
        b = b.child(icon(lead, 15.0).text_color(move || if selected.get() { ACCENT } else { FG_MUTED }));
    }
    b.child(text(label).text_size(px(TEXT_BASE)).text_color(FG).number_of_lines(1))
}

/// The status of an agent or workspace, as one color family.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Status {
    Idle,
    Running,
    NeedsYou,
    Done,
    Failed,
}

impl Status {
    pub(crate) fn of_agent(status: &str, needs_you: bool) -> Self {
        if needs_you {
            return Status::NeedsYou;
        }
        match status {
            "running" | "initializing" => Status::Running,
            "error" => Status::Failed,
            _ => Status::Idle,
        }
    }

    pub(crate) fn of_workspace(status: &str) -> Self {
        match status {
            "running" => Status::Running,
            "needs_input" | "attention" => Status::NeedsYou,
            "failed" => Status::Failed,
            "done" => Status::Done,
            _ => Status::Idle,
        }
    }

    pub(crate) fn dot(self) -> AdaptiveColor {
        match self {
            Status::Idle => DOT_IDLE,
            Status::Running => DOT_RUNNING,
            Status::NeedsYou => DOT_WARNING,
            Status::Done => DOT_SUCCESS,
            Status::Failed => DOT_DANGER,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Status::Idle => t("Idle"),
            Status::Running => t("Running"),
            Status::NeedsYou => t("Needs input"),
            Status::Done => t("Done"),
            Status::Failed => t("Failed"),
        }
    }
}

/// A 7 pt status dot.
pub(crate) fn status_dot(status: impl Fn() -> Status + 'static) -> View {
    view().w(px(7.0)).h(px(7.0)).rounded(px(R_FULL)).flex_shrink_0().bg(move || status().dot())
}

/// A small pill with a tinted fill.
pub(crate) fn badge(label: impl IntoReactive<SharedString>, color: AdaptiveColor) -> View {
    view()
        .flex_row()
        .items_center()
        .h(px(20.0))
        .px(px(S2))
        .rounded(px(R_FULL))
        .bg(tint(color, 0.12))
        .child(text(label).text_size(px(TEXT_XS)).font_weight(FontWeight(500)).text_color(color))
}

/// A bordered card grouping related rows.
pub(crate) fn card() -> View {
    view().rounded(px(R_XL)).border_1().border_color(BORDER).bg(SURFACE0).overflow_hidden()
}

/// The label above a stack of rows or a form field.
pub(crate) fn section_label(label: impl IntoReactive<SharedString>) -> nsh_core::element::Text {
    text(label).text_size(px(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED)
}

/// A one-point hairline.
pub(crate) fn hairline() -> View {
    view().h(px(1.0)).bg(BORDER).flex_shrink_0()
}

/// The keyboard key hint shown in menus and tooltips.
pub(crate) fn kbd(label: &'static str) -> View {
    view()
        .px(px(5.0))
        .h(px(18.0))
        .items_center()
        .justify_center()
        .rounded(px(R_SM))
        .border_1()
        .border_color(BORDER)
        .child(text(label).text_size(px(TEXT_XS)).text_color(FG_MUTED))
}

/// An on / off switch: 34 × 20, the accent when on.
pub(crate) fn switch(on: impl Fn() -> bool + 'static, on_change: impl Fn(bool) + 'static) -> ButtonElement {
    let on = computed(on);
    button()
        .on_press(move || on_change(!on.get_untracked()))
        .w(px(34.0))
        .h(px(20.0))
        .p(px(2.0))
        .rounded(px(R_FULL))
        .bg(move || if on.get() { ACCENT } else { SURFACE4 })
        .flex_row()
        .justify_start()
        .child(
            view()
                .w(px(16.0))
                .h(px(16.0))
                .rounded(px(R_FULL))
                .bg(ACCENT_FG)
                .shadow_sm()
                .ml(move || px(if on.get() { 14.0 } else { 0.0 })),
        )
}

/// A liquid-glass panel: translucent, blurring what is behind it, with a
/// bright rim and a soft drop shadow.
pub(crate) fn glass(v: View) -> View {
    v.bg(glass_fill())
        .backdrop_blur(px(GLASS_BLUR))
        .backdrop_saturate(1.7)
        .border_1()
        .border_color(glass_edge())
        .shadow_lg()
}

/// Apple's Liquid Glass (iOS 26 and later; earlier systems keep the
/// backdrop blur): what floats over content at the bottom of the phone's
/// screens, the composer and its buttons.
pub(crate) fn liquid_glass(v: View) -> View {
    v.glass_effect(nsh_core::GlassEffect::Regular)
        .bg(tint(SURFACE0, 0.35))
        .backdrop_blur(px(GLASS_BLUR))
        .backdrop_saturate(1.7)
        .border_1()
        .border_color(glass_edge())
        .shadow_lg()
}

/// The denser glass of menus, dialogs and the composer (Liquid Glass on
/// the phone).
pub(crate) fn glass_strong(v: View) -> View {
    if MOBILE {
        return liquid_glass(v);
    }
    v.bg(glass_fill_strong())
        .backdrop_blur(px(GLASS_BLUR))
        .backdrop_saturate(1.7)
        .border_1()
        .border_color(glass_edge())
        .shadow_xl()
}

/// The ambient light behind the glass: soft warm glows on the base surface.
pub(crate) fn ambient() -> View {
    let glow = |color: AdaptiveColor, top: Option<f32>, left: Option<f32>, size: f32| {
        let mut v = view()
            .absolute()
            .w(px(size))
            .h(px(size))
            .rounded(px(R_FULL))
            .bg_radial()
            .gradient_from(color)
            .gradient_to(tint(color, 0.0))
            // Transparent by the inscribed circle, so no edge shows.
            .gradient_to_position(nsh_core::geometry::Percentage(0.68));
        v = match top {
            Some(t) => v.top(px(t)),
            None => v.bottom(px(-size * 0.35)),
        };
        match left {
            Some(l) => v.left(px(l)),
            None => v.right(px(-size * 0.3)),
        }
    };
    view()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .overflow_hidden()
        .bg(SURFACE0)
        .child(glow(tint(ACCENT, 0.16), Some(-220.0), None, 760.0))
        .child(glow(tint(DOT_WARNING, 0.12), None, Some(-200.0), 700.0))
        .child(glow(tint(ACCENT, 0.07), Some(260.0), Some(420.0), 520.0))
}

/// The mark, loading: the eclipse alone (no sky, no tile), its corona
/// alive around a still moon. Two copies of the corona turn opposite ways
/// at different speeds (9 s and 13 s a turn), the first also breathing
/// (1.7 s): as their streamers cross, the light shifts unevenly, as a
/// corona's does, never in one steady beat. All natively, animated values
/// bound to rotation and opacity: no layout or engine work per frame.
pub(crate) fn eclipse(size: f32) -> View {
    let forward = animated_value(0.0);
    let backward = animated_value(0.0);
    let breath = animated_value(1.0);
    Animation::timing(&forward, 360.0, nsh_core::element::Timing::new(9000.0, Easing::Linear)).looped(None).start();
    Animation::timing(&backward, -360.0, nsh_core::element::Timing::new(13000.0, Easing::Linear)).looped(None).start();
    let half = nsh_core::element::Timing::new(850.0, Easing::EaseInOut);
    Animation::sequence([Animation::timing(&breath, 0.72, half.clone()), Animation::timing(&breath, 1.0, half)])
        .looped(None)
        .start();
    let layer = move |asset: ImageAsset| image(asset).absolute().top(px(0.0)).left(px(0.0)).w(px(size)).h(px(size));
    view()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        // Each appearance has its own corona (the night's pale glow, the
        // day's terracotta): the turning layers follow it.
        .child(dynamic(move || {
            let corona = if dark() { crate::LOADER_CORONA } else { crate::LOADER_CORONA_LIGHT };
            Some(untracked(move || {
                view()
                    .absolute()
                    .top(px(0.0))
                    .left(px(0.0))
                    .w(px(size))
                    .h(px(size))
                    .child(layer(corona).bind_rotate(&forward).bind_opacity(&breath))
                    .child(layer(corona).bind_rotate(&backward).opacity(0.45))
                    .into_element()
            }))
        }))
        .child(layer(crate::LOADER_MOON))
}

/// How far the mark's tile stands off the page.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Lift {
    Flat,
    Small,
    Large,
}

/// The mark, still and sharp at any size (the icon's layers, 256 px): on
/// dark pages the icon's tile, its night sky rounded and lifted; on light
/// pages the eclipse alone, no tile, its terracotta corona straight on
/// the page.
pub(crate) fn mark(size: f32, lift: Lift) -> View {
    let layer = move |asset: ImageAsset| image(asset).absolute().top(px(0.0)).left(px(0.0)).w(px(size)).h(px(size));
    view().w(px(size)).h(px(size)).flex_shrink_0().child(dynamic(move || {
        let night = dark();
        Some(untracked(move || {
            if !night {
                // No tile to fill: the eclipse is drawn larger, its glow
                // spilling past the mark's box, so the moon reads at the
                // tile's weight.
                let big = size * 1.35;
                let off = (size - big) / 2.0;
                let wide = move |asset: ImageAsset| image(asset).absolute().top(px(off)).left(px(off)).w(px(big)).h(px(big));
                return view()
                    .w(px(size))
                    .h(px(size))
                    .child(wide(crate::LOADER_CORONA_LIGHT))
                    .child(wide(crate::LOADER_MOON))
                    .into_element();
            }
            let tile = view()
                .w(px(size))
                .h(px(size))
                .rounded(px(size * 0.24))
                .overflow_hidden()
                .child(layer(crate::LOADER_SKY))
                .child(layer(crate::LOADER_CORONA))
                .child(layer(crate::LOADER_MOON));
            // The shadow sits on a frame of its own: the tile clips.
            let frame = view().w(px(size)).h(px(size)).rounded(px(size * 0.24));
            match lift {
                Lift::Flat => frame,
                Lift::Small => frame.shadow_sm(),
                Lift::Large => frame.shadow_lg(),
            }
            .child(tile)
            .into_element()
        }))
    }))
}

/// Loading: the mark with its corona running, under an optional line.
pub(crate) fn loader(label: Option<&'static str>) -> View {
    view()
        .items_center()
        .gap(px(S3))
        .child(eclipse(44.0))
        .when_some(label, |v, l| v.child(text(crate::i18n::t(l)).text_size(px(TEXT_SM)).text_color(FG_MUTED)))
}

/// The inline form of [`loader`], for work in progress in a list (an agent
/// thinking): a small running mark beside a line.
pub(crate) fn working(label: impl IntoReactive<SharedString>) -> View {
    view()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .child(eclipse(24.0))
        .child(text(label).text_size(px(TEXT_SM)).text_color(FG_MUTED))
}
