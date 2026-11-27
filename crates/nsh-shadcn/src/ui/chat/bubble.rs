//! [`BubbleGroup`], [`Bubble`], [`BubbleContent`] and [`BubbleReactions`],
//! after shadcn/ui's `bubble.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class:
//!
//! - A bubble's `variant` styles its content with
//!   `*:data-[slot=bubble-content]:…` (and `[&>[data-slot=bubble-content]:is(button,a):hover]:…`
//!   for a content that is a button or a link); NSH has no child
//!   selectors, so [`BubbleContent`] reads its bubble's variant and draws
//!   those classes itself (`hover:…` while it is `pressable`).
//! - Colors CSS computes, which NSH's classes cannot say, are computed
//!   from the theme with the same formulas, as typed styles: the `tinted`
//!   fill (`oklch(from var(--primary) 0.93 calc(c*0.4) h)`, `0.3` dark,
//!   hovered `0.88`/`0.35` at `c*0.5`) and the hover fills of `secondary`
//!   and `muted` (`color-mix(in oklch, var(--secondary|--muted), var(--foreground) 5%)`).
//! - `data-[align=end]:…`, `data-[variant=ghost]:…` and
//!   `group-data-[align=end]/…:…` are `data-align-end:…`,
//!   `data-variant-ghost:…` and `group-data-align-end/…:…`: NSH has the
//!   boolean `data-*` form only.
//! - shadcn's `asChild` content (a `<button>` or `<a>`) is the content's
//!   `pressable` prop: it takes presses, and its `[button,a]:…` classes
//!   apply.
//! - `has-[button]:p-0` on the reactions is their `has_button` prop.
//! - `wrap-break-word` is left out: NSH has no such class.

use nsh_core::prelude::*;

use super::message::MessageContext;
use crate::cn::cn;

/// How a bubble looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum BubbleVariant {
    /// The primary fill: the user's own messages.
    #[default]
    Default,
    /// The secondary fill.
    Secondary,
    /// The muted fill: the other side's messages.
    Muted,
    /// A soft tint of the primary color.
    Tinted,
    /// A border on the background.
    Outline,
    /// No frame and the full width: an assistant's text.
    Ghost,
    /// An error.
    Destructive,
}

/// Which side a bubble is on: shadcn's `align`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum BubbleAlign {
    /// The start.
    #[default]
    Start,
    /// The end.
    End,
}

/// Which edge of its bubble the reactions sit on: shadcn's `side`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum BubbleReactionsSide {
    /// Over the top edge.
    Top,
    /// Over the bottom edge.
    #[default]
    Bottom,
}

/// Which corner the reactions sit at: shadcn's `align`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum BubbleReactionsAlign {
    /// The left corner.
    Start,
    /// The right corner.
    #[default]
    End,
}

/// shadcn's `BubbleGroup` classes.
const GROUP: &str = "flex min-w-0 flex-col gap-2";

/// shadcn's `bubbleVariants` base classes.
const BUBBLE: &str = "group/bubble relative flex w-fit max-w-[80%] min-w-0 flex-col gap-1 \
    group-data-align-end/message:self-end data-align-end:self-end data-variant-ghost:max-w-full";

/// shadcn's `BubbleContent` classes (`wrap-break-word` left out).
const CONTENT: &str = "w-fit max-w-full min-w-0 overflow-hidden rounded-xl border border-transparent px-3 py-2 \
    text-sm leading-relaxed group-data-align-end/bubble:self-end";

/// shadcn's `[button]:…` and `[button,a]:…` classes of the content, which
/// apply to a pressable one.
const CONTENT_PRESSABLE: &str = "cursor-pointer text-left transition-colors outline-none focus-visible:border-ring \
    focus-visible:ring-3 focus-visible:ring-ring/50";

/// shadcn's `bubbleReactionsVariants` base classes.
const REACTIONS: &str = "absolute z-10 flex w-fit shrink-0 items-center justify-center gap-1 rounded-full \
    bg-muted px-1.5 py-0.5 text-sm ring-3 ring-card";

impl BubbleVariant {
    /// The bubble's own classes of the variant.
    fn bubble_classes(self) -> &'static str {
        match self {
            BubbleVariant::Ghost => "border-none",
            _ => "",
        }
    }

    /// The `*:data-[slot=bubble-content]:…` classes of the variant, which
    /// its content draws.
    fn content_classes(self) -> &'static str {
        match self {
            BubbleVariant::Default => "bg-primary text-primary-foreground",
            BubbleVariant::Secondary => "bg-secondary text-secondary-foreground",
            BubbleVariant::Muted => "bg-muted",
            BubbleVariant::Tinted => "text-foreground",
            BubbleVariant::Outline => "border-border bg-background",
            BubbleVariant::Ghost => "rounded-none bg-transparent p-0",
            BubbleVariant::Destructive => "bg-destructive/10 text-destructive dark:bg-destructive/20",
        }
    }

    /// The `[&>[data-slot=bubble-content]:is(button,a):hover]:…` classes of
    /// the variant, which a pressable content draws.
    fn hover_classes(self) -> &'static str {
        match self {
            BubbleVariant::Default => "hover:bg-primary/80",
            BubbleVariant::Outline => "hover:bg-muted hover:text-foreground dark:hover:bg-input/30",
            BubbleVariant::Ghost => "hover:bg-muted hover:text-foreground dark:hover:bg-muted/50",
            BubbleVariant::Destructive => "hover:bg-destructive/20 dark:hover:bg-destructive/30",
            // Computed from the theme: `content_fill`.
            BubbleVariant::Secondary | BubbleVariant::Muted | BubbleVariant::Tinted => "",
        }
    }
}

/// Every class a content of `variant` draws with, before the app's.
fn content_classes(variant: Option<BubbleVariant>, pressable: bool) -> SharedString {
    let variant_classes = variant.map(BubbleVariant::content_classes).unwrap_or_default();
    let (pressable_classes, hover) = match (pressable, variant) {
        (true, Some(v)) => (CONTENT_PRESSABLE, v.hover_classes()),
        (true, None) => (CONTENT_PRESSABLE, ""),
        (false, _) => ("", ""),
    };
    cn([CONTENT, pressable_classes, variant_classes, hover])
}

/// The classes of the reactions on `side` at `align`, before the app's.
fn reactions_classes(side: BubbleReactionsSide, align: BubbleReactionsAlign, has_button: bool) -> SharedString {
    cn([
        REACTIONS,
        if has_button { "p-0" } else { "" },
        match side {
            BubbleReactionsSide::Top => "top-0 -translate-y-3/4",
            BubbleReactionsSide::Bottom => "bottom-0 translate-y-3/4",
        },
        match align {
            BubbleReactionsAlign::Start => "left-3",
            BubbleReactionsAlign::End => "right-3",
        },
    ])
}

/// A color in Oklab: lightness and the two opposing axes.
fn oklab(color: Hsla) -> (f32, f32, f32, f32) {
    let c = color.to_rgb();
    let lin = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = (lin(c.r), lin(c.g), lin(c.b));
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        c.a,
    )
}

/// The sRGB color of an Oklab one, clipped to sRGB.
fn from_oklab(l: f32, a: f32, b: f32, alpha: f32) -> Hsla {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let r = 4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_;
    let g = -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_;
    let bl = -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_;
    let gamma = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        if v <= 0.003_130_8 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        }
    };
    Rgba {
        r: gamma(r),
        g: gamma(g),
        b: gamma(bl),
        a: alpha,
    }
    .into()
}

/// CSS's `oklch(from <color> <lightness> calc(c * <chroma>) h)`.
fn relative_oklch(color: Hsla, lightness: f32, chroma: f32) -> Hsla {
    let (_, a, b, alpha) = oklab(color);
    from_oklab(lightness, a * chroma, b * chroma, alpha)
}

/// CSS's `color-mix(in oklch, <color>, <other> <part>)`; for the neutral
/// tokens Oklab and Oklch mix alike.
fn mix(color: Hsla, other: Hsla, part: f32) -> Hsla {
    color.mix_oklab(other, 1.0 - part)
}

/// The fill and the hovered fill of the content of a `variant` bubble
/// that classes cannot say (computed from the theme), if any.
fn content_fill(variant: BubbleVariant) -> Option<(AdaptiveColor, AdaptiveColor)> {
    let theme = nsh_base::theme();
    match variant {
        BubbleVariant::Tinted => {
            let p = theme.primary;
            Some((
                AdaptiveColor::new(relative_oklch(p.light, 0.93, 0.4), relative_oklch(p.dark, 0.3, 0.4)),
                AdaptiveColor::new(relative_oklch(p.light, 0.88, 0.5), relative_oklch(p.dark, 0.35, 0.5)),
            ))
        }
        BubbleVariant::Secondary | BubbleVariant::Muted => {
            let base = if variant == BubbleVariant::Secondary {
                theme.secondary
            } else {
                theme.muted
            };
            let fg = theme.foreground;
            Some((
                base,
                AdaptiveColor::new(mix(base.light, fg.light, 0.05), mix(base.dark, fg.dark, 0.05)),
            ))
        }
        _ => None,
    }
}

/// What a bubble tells its content.
#[derive(Clone, Copy)]
struct BubbleContext {
    variant: Prop<BubbleVariant>,
}

/// The props of [`BubbleGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BubbleGroupProps {
    /// The [`Bubble`]s, one under the other.
    pub children: Prop<Children>,
}

/// Bubbles sent one after the other, closer together.
#[component]
pub fn BubbleGroup(props: BubbleGroupProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(GROUP))).child(props.children)
}

/// The props of [`Bubble`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BubbleProps {
    /// `default`, `secondary`, `muted`, `tinted`, `outline`, `ghost` or
    /// `destructive`.
    #[prop(default)]
    pub variant: Prop<BubbleVariant>,
    /// `start` or `end`; in a [`super::Message`] aligned to the end, it
    /// goes to the end either way.
    #[prop(default)]
    pub align: Prop<BubbleAlign>,
    /// A [`BubbleContent`], and [`BubbleReactions`] over its edge.
    pub children: Prop<Children>,
}

/// A chat bubble: at most 80% of the width (a ghost bubble all of it), its
/// look from its `variant`.
#[component]
pub fn Bubble(props: BubbleProps) -> impl IntoElement {
    provide_context(BubbleContext {
        variant: props.variant,
    });
    if let Some(message) = use_context::<MessageContext>() {
        message
            .ghosts
            .count_while(move || props.variant.get() == BubbleVariant::Ghost);
    }
    view()
        .group_named("bubble")
        .app_class(move || Some(cn([BUBBLE, props.variant.get().bubble_classes()])))
        .project_data("align-end", move || props.align.get() == BubbleAlign::End)
        .project_data("variant-ghost", move || props.variant.get() == BubbleVariant::Ghost)
        .child(props.children)
}

/// The props of [`BubbleContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BubbleContentProps {
    /// The message's text.
    pub text: Prop<Option<SharedString>>,
    /// Takes presses, as shadcn's `asChild` content that is a `<button>`:
    /// a quick reply, a suggestion. Hovered, its fill deepens. Read when
    /// the content is made.
    #[prop(default)]
    pub pressable: Prop<bool>,
    /// What screen readers say for a pressable content, in place of its
    /// text.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Content after the text: rich text, markdown, a code block.
    pub children: Prop<Children>,
}

/// The events of a [`BubbleContent`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum BubbleContentEvent {
    /// A pressable content was pressed.
    Press,
}

/// What a bubble shows: its text on the variant's fill, in a rounded box.
#[component]
pub fn BubbleContent(props: BubbleContentProps, cx: Scope<BubbleContentEvent>) -> impl IntoElement {
    let variant = use_context::<BubbleContext>().map(|b| b.variant);
    let label = props.text;
    let pressable = props.pressable.get_untracked();
    let hovered = signal(false);
    let content = view()
        .app_class(move || Some(content_classes(variant.map(|v| v.get()), pressable)))
        .style_with(move |s| match variant.and_then(|v| content_fill(v.get())) {
            Some((_, hover)) if pressable && hovered.get() => s.bg(hover),
            Some((fill, _)) => s.bg(fill),
            None => s,
        })
        .child(dynamic(move || label.get().map(|t| text(t).into_element())))
        .child(props.children);
    if !pressable {
        return content;
    }
    content
        .role(AccessibilityRole::Button)
        .optional_label(move || props.accessibility_label.get().or_else(|| label.get()))
        .on_mouse_enter(move |_: &MouseEvent| hovered.set(true))
        .on_mouse_leave(move |_: &MouseEvent| hovered.set(false))
        .on_press(move || cx.emit(BubbleContentEvent::Press))
}

/// The props of [`BubbleReactions`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BubbleReactionsProps {
    /// `top` or `bottom`: the edge of the bubble it sits over.
    #[prop(default)]
    pub side: Prop<BubbleReactionsSide>,
    /// `start` (left) or `end` (right): the corner.
    #[prop(default)]
    pub align: Prop<BubbleReactionsAlign>,
    /// It holds a button (shadcn's `has-[button]:p-0`): no padding.
    #[prop(default)]
    pub has_button: Prop<bool>,
    /// What screen readers say: "Reactions: thumbs up".
    pub accessibility_label: Prop<Option<SharedString>>,
    /// The reactions: emoji, a count, a button.
    pub children: Prop<Children>,
}

/// Reactions in a pill over the edge of their bubble.
#[component]
pub fn BubbleReactions(props: BubbleReactionsProps) -> impl IntoElement {
    view()
        .app_class(move || {
            Some(reactions_classes(
                props.side.get(),
                props.align.get(),
                props.has_button.get(),
            ))
        })
        .optional_label(move || props.accessibility_label.get())
        .role(move || {
            if props.accessibility_label.get().is_some() {
                AccessibilityRole::Image
            } else {
                AccessibilityRole::None
            }
        })
        .child(props.children)
}

impl BubbleElement {
    /// `.variant(BubbleVariant::Secondary)`.
    pub fn secondary(self) -> Self {
        self.variant(BubbleVariant::Secondary)
    }
    /// `.variant(BubbleVariant::Muted)`.
    pub fn muted(self) -> Self {
        self.variant(BubbleVariant::Muted)
    }
    /// `.variant(BubbleVariant::Tinted)`.
    pub fn tinted(self) -> Self {
        self.variant(BubbleVariant::Tinted)
    }
    /// `.variant(BubbleVariant::Outline)`.
    pub fn outline(self) -> Self {
        self.variant(BubbleVariant::Outline)
    }
    /// `.variant(BubbleVariant::Ghost)`.
    pub fn ghost(self) -> Self {
        self.variant(BubbleVariant::Ghost)
    }
    /// `.variant(BubbleVariant::Destructive)`.
    pub fn destructive(self) -> Self {
        self.variant(BubbleVariant::Destructive)
    }
    /// `.align(BubbleAlign::End)`.
    pub fn end(self) -> Self {
        self.align(BubbleAlign::End)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<BubbleGroup>();
    registry.register::<Bubble>();
    registry.register::<BubbleContent>();
    registry.register::<BubbleReactions>();
}

#[cfg(test)]
mod tests {
    use super::*;

    const VARIANTS: [BubbleVariant; 7] = [
        BubbleVariant::Default,
        BubbleVariant::Secondary,
        BubbleVariant::Muted,
        BubbleVariant::Tinted,
        BubbleVariant::Outline,
        BubbleVariant::Ghost,
        BubbleVariant::Destructive,
    ];

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = vec![cn([GROUP]), cn([BUBBLE]), content_classes(None, false), content_classes(None, true)];
        for variant in VARIANTS {
            all.push(cn([BUBBLE, variant.bubble_classes()]));
            for pressable in [false, true] {
                all.push(content_classes(Some(variant), pressable));
            }
        }
        for side in [BubbleReactionsSide::Top, BubbleReactionsSide::Bottom] {
            for align in [BubbleReactionsAlign::Start, BubbleReactionsAlign::End] {
                for has_button in [false, true] {
                    all.push(reactions_classes(side, align, has_button));
                }
            }
        }
        let mut unknown = Vec::new();
        for classes in all {
            for class in crate::cn::unknown_classes(&classes) {
                if !unknown.contains(&class) {
                    unknown.push(class);
                }
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }

    /// The tint of a neutral primary is the gray at the lightness CSS
    /// gives it.
    #[test]
    fn tinted_fill_follows_css() {
        let black = Hsla::from(Rgba { r: 0.0, g: 0.0, b: 0.0, a: 1.0 });
        let tint = relative_oklch(black, 0.93, 0.4).to_rgb();
        // oklch(0.93 0 0) is #e8e8e8.
        assert!((tint.r - 0.910).abs() < 0.01, "{}", tint.r);
        assert!((tint.r - tint.b).abs() < 0.001);
    }
}
