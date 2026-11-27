//! Sophon's controls on `nsh/base`'s unstyled ones:
//! pill buttons in five variants (one accent CTA per surface), two sizes,
//! icon buttons as circles with a translucent highlight, fields on a muted
//! fill, segmented controls whose selected segment is a raised surface.

#![allow(dead_code)]

use crate::i18n::t;
use nsh_base::input;
use nsh_base::theme;
use nsh_shadcn::{ButtonElement as ShadcnButton, ButtonSize, ButtonVariant};
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
    nsh_core::element::text(content).text_color(FG())
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
    /// nsh-shadcn's size: a phone keeps the default 36-point targets.
    fn ui(self) -> ButtonSize {
        match self {
            Sz::Md => ButtonSize::Default,
            Sz::Sm if MOBILE => ButtonSize::Default,
            Sz::Sm => ButtonSize::Sm,
        }
    }

    /// The square of an icon-only button.
    fn square(self) -> ButtonSize {
        match self {
            Sz::Md => ButtonSize::Icon,
            Sz::Sm if MOBILE => ButtonSize::Icon,
            Sz::Sm => ButtonSize::IconSm,
        }
    }
}

impl Variant {
    fn ui(self) -> ButtonVariant {
        match self {
            Variant::Primary => ButtonVariant::Default,
            Variant::Secondary => ButtonVariant::Secondary,
            Variant::Outline => ButtonVariant::Outline,
            Variant::Ghost => ButtonVariant::Ghost,
            Variant::Destructive => ButtonVariant::Destructive,
        }
    }
}

/// A labelled button, optionally with a leading icon of Sophon's own:
/// nsh-shadcn's `Button`.
pub(crate) fn btn(
    label: impl IntoReactive<SharedString>,
    variant: Variant,
    size: Sz,
    lead: Option<Icon>,
    on_press: impl Fn() + 'static,
) -> ShadcnButton {
    let label = label.into_reactive();
    let mut b = nsh_shadcn::button("")
        .label(move || Some(label.get()))
        .variant(variant.ui())
        .size(size.ui())
        .on_press(on_press);
    if let Some(lead) = lead {
        b = b.leading(icon(lead, 16.0).text_color(move || current_color(variant)));
    }
    b
}

/// The color an icon of a button of `variant` draws in: its text's.
fn current_color(variant: Variant) -> AdaptiveColor {
    let t = theme();
    match variant {
        Variant::Primary => t.primary_foreground,
        // shadcn's `text-white`.
        Variant::Destructive => AdaptiveColor::rgb(0xffffff, 0xffffff),
        Variant::Secondary => t.secondary_foreground,
        Variant::Outline | Variant::Ghost => t.foreground,
    }
}

/// An icon-only button: nsh-shadcn's ghost `Button`, its glyph muted.
pub(crate) fn icon_btn(glyph: Icon, size: Sz, label: &'static str, on_press: impl Fn() + 'static) -> ShadcnButton {
    nsh_shadcn::button("")
        .label(None::<SharedString>)
        .ghost()
        .size(size.square())
        .leading(icon(glyph, 16.0).text_color(FG_MUTED()))
        .accessibility_label(Some(SharedString::from(label)))
        .on_press(on_press)
}

/// A single-line text field: nsh-shadcn's Input look on a field of
/// Sophon's own (its fonts, its events). Its width is the caller's: a
/// class wins over the field's own `w(..)`, so shadcn's `w-full` (a field
/// filling a form's column, which a column does anyway) is left out.
pub(crate) fn field() -> InputElement {
    thread_local! {
        static CLASSES: SharedString = SharedString::from(
            nsh_shadcn::input_classes().split_whitespace().filter(|c| *c != "w-full").collect::<Vec<_>>().join(" "),
        );
    }
    input().app_class(Some(CLASSES.with(Clone::clone)))
}

/// A segmented control: nsh-shadcn's Tabs list (a muted track, the
/// picked segment raised), its tabs Sophon's `options`.
pub(crate) fn segmented<T: Clone + PartialEq + 'static>(
    options: Vec<(T, SharedString)>,
    selected: impl Fn() -> T + 'static,
    on_select: impl Fn(T) + Clone + 'static,
) -> nsh_shadcn::TabsElement {
    let values: std::rc::Rc<Vec<T>> = std::rc::Rc::new(options.iter().map(|(v, _)| v.clone()).collect());
    let picked = {
        let values = values.clone();
        computed(move || {
            let s = selected();
            SharedString::from(values.iter().position(|v| *v == s).unwrap_or(0).to_string())
        })
    };
    let mut list = nsh_shadcn::tabs_list();
    for (i, (_, label)) in options.into_iter().enumerate() {
        list = list.child(nsh_shadcn::tabs_trigger(i.to_string(), label));
    }
    nsh_shadcn::tabs("0")
        .value(move || Some(picked.get()))
        .on_value_change(move |value: &SharedString| {
            if let Some(v) = value.parse::<usize>().ok().and_then(|i| values.get(i)) {
                on_select(v.clone());
            }
        })
        .child(list)
}

/// A selectable chip (provider, project, option lists).
pub(crate) fn chip(
    label: impl IntoReactive<SharedString>,
    lead: Option<Icon>,
    selected: impl Fn() -> bool + 'static,
    on_press: impl Fn() + 'static,
) -> nsh_shadcn::ToggleElement {
    let label = label.into_reactive();
    let mut t = nsh_shadcn::toggle()
        .variant(nsh_shadcn::ToggleVariant::Outline)
        .pressed(move || Some(selected()))
        .label(move || Some(label.get()))
        .on_pressed_change(move |_: &bool| on_press());
    if let Some(lead) = lead {
        t = t.child(icon(lead, 16.0));
    }
    t
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

    /// What a conversation asks of the user, as the sidebar marks it: an
    /// approval or a question (needs you), a failure not yet seen, its
    /// work, a finished turn not yet seen (done); nothing once seen.
    pub(crate) fn of_agent_row(agent: &crate::daemon::Agent) -> Self {
        let unseen = agent.requires_attention;
        if !agent.pending.is_empty() || (unseen && agent.attention_reason.as_deref() == Some("needs_input")) {
            Status::NeedsYou
        } else if unseen && (agent.status == "error" || agent.attention_reason.as_deref() == Some("error")) {
            Status::Failed
        } else if matches!(agent.status.as_str(), "running" | "initializing") {
            Status::Running
        } else if unseen {
            Status::Done
        } else {
            Status::Idle
        }
    }

    /// The one of several conversations' marks a workspace shows: what
    /// most wants the user (needs you, then a failure, work, a finished
    /// turn).
    pub(crate) fn most_urgent(marks: impl IntoIterator<Item = Self>) -> Self {
        let rank = |s: &Status| match s {
            Status::NeedsYou => 4,
            Status::Failed => 3,
            Status::Running => 2,
            Status::Done => 1,
            Status::Idle => 0,
        };
        marks.into_iter().max_by_key(rank).unwrap_or(Status::Idle)
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
pub(crate) fn badge(label: impl IntoReactive<SharedString>, color: AdaptiveColor) -> nsh_shadcn::BadgeElement {
    // shadcn's variants for Sophon's tones: an error is destructive, a
    // quiet word secondary, a warning outlined, the rest the primary.
    use nsh_shadcn::BadgeVariant as V;
    let variant = if color == DANGER {
        V::Destructive
    } else if color == FG_MUTED() || color == FG_FAINT() {
        V::Secondary
    } else if color == WARNING {
        V::Outline
    } else {
        V::Default
    };
    let label = label.into_reactive();
    nsh_shadcn::badge("").label(move || Some(label.get())).variant(variant)
}

/// A tooltip's bubble of words: shadcn's TooltipContent (nsh-shadcn's
/// classes), the foreground's fill and the background's text.
pub(crate) fn tooltip_text(label: impl IntoReactive<SharedString>) -> View {
    view()
        .app_class(Some(SharedString::from("w-fit rounded-md bg-foreground px-3 py-1.5")))
        .child(nsh_core::element::text(label).app_class(Some(SharedString::from("text-xs text-background"))))
}

/// A floating surface of richer content (a picture, a panel): shadcn's
/// PopoverContent surface, its padding the caller's.
pub(crate) fn popover_surface() -> View {
    view().app_class(Some(SharedString::from("rounded-md border border-border bg-popover text-popover-foreground shadow-md")))
}

/// A bordered card grouping related rows: shadcn's Card surface (its
/// fill, border, radius and shadow), without its padding and gaps, which
/// the rows have.
pub(crate) fn card() -> View {
    view().app_class(Some(SharedString::from("bg-card text-card-foreground rounded-xl border border-border shadow-sm overflow-hidden")))
}

/// The label above a stack of rows or a form field.
pub(crate) fn section_label(label: impl IntoReactive<SharedString>) -> nsh_core::element::Text {
    text(label).text_size(tx(TEXT_SM)).font_weight(FontWeight(500)).text_color(FG_MUTED())
}

/// A one-point hairline.
pub(crate) fn hairline() -> View {
    view().h(px(1.0)).bg(BORDER()).flex_shrink_0()
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
        .border_color(BORDER())
        .child(text(label).text_size(tx(TEXT_XS)).text_color(FG_MUTED()))
}

/// An on/off switch: nsh-shadcn's `Switch`, controlled by `on`.
pub(crate) fn switch(on: impl Fn() -> bool + 'static, on_change: impl Fn(bool) + 'static) -> nsh_shadcn::SwitchElement {
    nsh_shadcn::switch().checked(move || Some(on())).on_checked_change(move |checked: &bool| on_change(*checked))
}

/// A liquid-glass panel: translucent, blurring what is behind it, with a
/// bright rim; flat, no drop shadow.
pub(crate) fn glass(v: View) -> View {
    v.bg(glass_fill())
        .backdrop_blur(px(GLASS_BLUR))
        .backdrop_saturate(1.7)
        .border_1()
        .border_color(glass_edge())
}

/// Apple's Liquid Glass (iOS 26 and later; earlier systems keep the
/// backdrop blur): what floats over content at the bottom of the phone's
/// screens, the composer and its buttons.
pub(crate) fn liquid_glass(v: View) -> View {
    v.glass_effect(nsh_core::GlassEffect::Regular)
        .bg(tint(SURFACE0(), 0.35))
        .backdrop_blur(px(GLASS_BLUR))
        .backdrop_saturate(1.7)
        .border_1()
        .border_color(glass_edge())
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
}

/// The base behind the glass: the plain page surface, a solid color with
/// no glows, so a chosen background reads as chosen.
pub(crate) fn ambient() -> View {
    view()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .bg(SURFACE0())
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
                .border_color(tint(ACCENT(), 0.55))
                .bind_opacity(&corona)
                .bind_scale(&flare),
        )
        .child(
            centered(disc)
                .bg(ACCENT())
                .overflow_hidden()
                .child(view().w(px(disc)).h(px(disc)).rounded(px(R_FULL)).bg(SURFACE0()).bind_translate_x(&moon_x)),
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

/// Working: Claude Code's own mark for it, the six it turns through (a
/// dot opening into a flower and closing again), set one on another and
/// each shown in its turn. Natively, as [`eclipse`]: an animated opacity
/// for each, no layout or engine work per frame.
/// A hand raised and waving now and then: a conversation waits for the
/// user (an approval, a question).
pub(crate) fn raised_hand(size: f32) -> View {
    let turn = animated_value(0.0);
    let step = |to: f32, ms: f32| Animation::timing(&turn, to, nsh_core::element::Timing::new(ms, Easing::EaseInOut));
    Animation::sequence([
        step(-14.0, 120.0),
        step(12.0, 160.0),
        step(-10.0, 140.0),
        step(0.0, 120.0),
        Animation::delay(1600.0),
    ])
    .looped(None)
    .start();
    view()
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        // It turns at the wrist.
        .transform_origin(nsh_core::TransformOrigin { x: relative(0.5).into(), y: relative(1.0).into(), z: px(0.0) })
        .bind_rotate(&turn)
        .child(icon(Icon::Hand, size).text_color(DOT_WARNING))
}

pub(crate) fn spark(size: f32) -> View {
    // The third has a sign after it that keeps it a drawn mark where a
    // system would make an emoji of it.
    const MARKS: [&str; 6] = ["·", "✢", "✳\u{fe0e}", "✶", "✻", "✽"];
    /// How long each stays, as Claude Code's.
    const STEP: f32 = 120.0;
    // Out and back: whose turn each of the ten steps of a pass is.
    let turns: Vec<usize> = (0..MARKS.len()).chain((1..MARKS.len() - 1).rev()).collect();
    let mut mark = view().w(px(size)).h(px(size)).flex_shrink_0();
    for (index, glyph) in MARKS.into_iter().enumerate() {
        let shown = animated_value(if index == 0 { 1.0 } else { 0.0 });
        Animation::sequence(turns.iter().flat_map(|turn| {
            [
                Animation::timing(&shown, if *turn == index { 1.0 } else { 0.0 }, nsh_core::element::Timing::new(0.0, Easing::Linear)),
                Animation::delay(STEP),
            ]
        }))
        .looped(None)
        .start();
        mark = mark.child(
            view()
                .absolute()
                .top(px(0.0))
                .left(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .items_center()
                .justify_center()
                .bind_opacity(&shown)
                // One font for all six, so none sits higher or wider than
                // the last: Menlo has them all, where a font someone picked
                // for code has some and borrows the rest.
                .child(text(glyph).text_size(px(size)).line_height(px(size)).font_family(MONO).text_color(ACCENT())),
        );
    }
    mark
}

/// Loading: the mark with its corona running, under an optional line.
pub(crate) fn loader(label: Option<&'static str>) -> View {
    view()
        .items_center()
        .gap(sp(S3))
        .child(eclipse(44.0))
        .when_some(label, |v, l| v.child(text(crate::i18n::t(l)).text_size(tx(TEXT_SM)).text_color(FG_MUTED())))
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
        .child(text(label).text_size(tx(TEXT_SM)).text_color(FG_MUTED()))
}
