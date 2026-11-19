//! Sophon's controls on `nsh/base`'s unstyled ones:
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
    /// nsh-ui's size: a phone keeps 32-point targets.
    fn ui(self) -> nsh_ui::size::Size {
        match self {
            Sz::Md => nsh_ui::size::Size::Medium,
            Sz::Sm if MOBILE => nsh_ui::size::Size::Medium,
            Sz::Sm => nsh_ui::size::Size::Small,
        }
    }
}

impl Variant {
    fn ui(self) -> nsh_ui::button::ButtonVariant {
        use nsh_ui::button::ButtonVariant as V;
        match self {
            Variant::Primary => V::Primary,
            Variant::Secondary => V::Secondary,
            Variant::Outline => V::Outline,
            Variant::Ghost => V::Ghost,
            Variant::Destructive => V::Danger,
        }
    }
}

/// A labelled button, optionally with a leading icon: nsh-ui's `Button`.
pub(crate) fn btn(
    label: impl IntoReactive<SharedString>,
    variant: Variant,
    size: Sz,
    lead: Option<Icon>,
    on_press: impl Fn() + 'static,
) -> ButtonElement {
    let (v, sz) = (variant.ui(), size.ui());
    let mut b = nsh_ui::button::Button::new().label(label).variant(v).size(sz).on_press(on_press);
    if let Some(lead) = lead {
        b = b.icon(icon(lead, sz.icon()).text_color(v.ink()));
    }
    b.build()
}

/// An icon-only button: nsh-ui's ghost `Button`, its glyph muted.
pub(crate) fn icon_btn(glyph: Icon, size: Sz, label: &'static str, on_press: impl Fn() + 'static) -> ButtonElement {
    let sz = size.ui();
    nsh_ui::button::Button::new()
        .ghost()
        .size(sz)
        .icon(icon(glyph, sz.icon()).text_color(FG_MUTED))
        .accessibility_label(label)
        .on_press(on_press)
        .build()
}

/// A single-line text field on the muted fill.
pub(crate) fn field() -> InputElement {
    input()
        .h(sp(CONTROL_MD + 4.0))
        .px(sp(S3))
        .rounded(px(R_LG))
        .bg(SURFACE2)
        .border_1()
        .border_color(BORDER)
        .text_size(tx(TEXT_BASE))
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
                .h(sp(CONTROL_SM - 2.0))
                .px(sp(S3))
                .items_center()
                .justify_center()
                .rounded(px(R_FULL))
                .hover(|s| s.opacity(0.85))
                .child(
                    text(label)
                        .text_size(tx(TEXT_SM))
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
        .h(sp(CONTROL_MD))
        .px(sp(S3))
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
    b.child(text(label).text_size(tx(TEXT_BASE)).text_color(FG).number_of_lines(1))
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
        .px(sp(S2))
        .rounded(px(R_FULL))
        .bg(tint(color, 0.12))
        .child(text(label).text_size(tx(TEXT_XS)).font_weight(FontWeight(500)).text_color(color))
}

/// A bordered card grouping related rows.
pub(crate) fn card() -> View {
    view().rounded(px(R_XL)).border_1().border_color(BORDER).bg(SURFACE0).overflow_hidden()
}

/// The label above a stack of rows or a form field.
pub(crate) fn section_label(label: impl IntoReactive<SharedString>) -> nsh_core::element::Text {
    text(label).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED)
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
        .child(text(label).text_size(tx(TEXT_XS)).text_color(FG_MUTED))
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

/// Loading: an eclipse passing, drawn rather than the icon's layers (no
/// tile, no box, on light pages and dark): the sun, a disc of the accent;
/// the moon, a disc of the page, sliding in from the left until it covers
/// the sun; at totality the corona flares round it and fades; the moon
/// slides on and the sun comes back. 2.4 s a pass, eased at both ends. All
/// natively, animated values bound to translation, scale and opacity: no
/// layout or engine work per frame.
pub(crate) fn eclipse(size: f32) -> View {
    let disc = size * 0.62;
    let travel = disc * 1.05;
    let moon_x = animated_value(-travel);
    let corona = animated_value(0.0);
    let flare = animated_value(0.9);
    let glide = nsh_core::element::Timing::new(1000.0, Easing::EaseInOut);
    Animation::sequence([
        Animation::timing(&moon_x, 0.0, glide.clone()),
        Animation::delay(250.0),
        Animation::timing(&moon_x, travel, glide),
        Animation::delay(150.0),
        Animation::timing(&moon_x, -travel, nsh_core::element::Timing::new(0.0, Easing::Linear)),
    ])
    .looped(None)
    .start();
    // The corona: dark until the moon nearly covers the sun, a flare at
    // totality (brightening and opening out), gone as the moon moves on.
    let quiet = nsh_core::element::Timing::new(0.0, Easing::Linear);
    Animation::sequence([
        Animation::delay(820.0),
        Animation::timing(&corona, 1.0, nsh_core::element::Timing::new(260.0, Easing::EaseOut)),
        Animation::timing(&corona, 0.0, nsh_core::element::Timing::new(420.0, Easing::EaseIn)),
        Animation::delay(900.0),
    ])
    .looped(None)
    .start();
    Animation::sequence([
        Animation::delay(820.0),
        Animation::timing(&flare, 1.12, nsh_core::element::Timing::new(680.0, Easing::EaseOut)),
        Animation::timing(&flare, 0.9, quiet),
        Animation::delay(900.0),
    ])
    .looped(None)
    .start();
    let centered = move |d: f32| view().absolute().top(px((size - d) / 2.0)).left(px((size - d) / 2.0)).w(px(d)).h(px(d)).rounded(px(R_FULL));
    view()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        // The moon only passes in front of the sun: clipped to it.
        .child(
            centered(size * 0.86)
                .when(size >= 32.0, |v| v.border_2())
                .when(size < 32.0, |v| v.border_1())
                .border_color(tint(ACCENT, 0.55))
                .bind_opacity(&corona)
                .bind_scale(&flare),
        )
        .child(
            centered(disc)
                .bg(ACCENT)
                .overflow_hidden()
                .child(view().w(px(disc)).h(px(disc)).rounded(px(R_FULL)).bg(SURFACE0).bind_translate_x(&moon_x)),
        )
}

/// How far the mark's tile stands off the page.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Lift {
    Flat,
    Small,
    Large,
}

/// The mark, still and sharp at any size (the icon's layers, 256 px): the
/// icon's tile, its night sky rounded, on light pages as on dark ones (the
/// Dock's icon is the same tile).
pub(crate) fn mark(size: f32, lift: Lift) -> View {
    let layer = move |asset: ImageAsset| image(asset).absolute().top(px(0.0)).left(px(0.0)).w(px(size)).h(px(size));
    let tile = view()
        .w(px(size))
        .h(px(size))
        .rounded(px(size * 0.24))
        .overflow_hidden()
        .child(layer(crate::LOADER_SKY))
        .child(layer(crate::LOADER_CORONA))
        .child(layer(crate::LOADER_MOON));
    // The shadow sits on a frame of its own: the tile clips.
    let frame = view().w(px(size)).h(px(size)).flex_shrink_0().rounded(px(size * 0.24));
    match lift {
        Lift::Flat => frame,
        Lift::Small => frame.shadow_sm(),
        Lift::Large => frame.shadow_lg(),
    }
    .child(tile)
}

/// Loading: the mark with its corona running, under an optional line.
pub(crate) fn loader(label: Option<&'static str>) -> View {
    view()
        .items_center()
        .gap(sp(S3))
        .child(eclipse(44.0))
        .when_some(label, |v, l| v.child(text(crate::i18n::t(l)).text_size(tx(TEXT_SM)).text_color(FG_MUTED)))
}

/// [`loader`] in the middle of the room a list will take once it is here:
/// what a sheet or a page shows while it waits for its rows.
/// The mark alone, in the middle of a block `height` tall: what a part of
/// a page shows while its data comes (a card's worth, not the page's).
pub(crate) fn loading_block(height: f32) -> View {
    view().w_full().min_h(px(height)).items_center().justify_center().child(eclipse(36.0))
}

pub(crate) fn loading() -> View {
    view().w_full().min_h(px(180.0)).items_center().justify_center().child(loader(Some("Loading…")))
}

/// The inline form of [`loader`], for work in progress in a list (an agent
/// thinking): a small running mark beside a line.
pub(crate) fn working(label: impl IntoReactive<SharedString>) -> View {
    view()
        .flex_row()
        .items_center()
        .gap(px(6.0))
        .child(eclipse(24.0))
        .child(text(label).text_size(tx(TEXT_SM)).text_color(FG_MUTED))
}
