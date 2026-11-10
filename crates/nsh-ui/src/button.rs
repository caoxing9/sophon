//! A button: shadcn/ui's variants (Primary, Secondary, Outline, Ghost, Danger,
//! Link) with GPUI Component's sizes and its Success / Warning / Info,
//! on `nsh-base`'s `button()` (press, focus, accessibility).
//!
//! A label, an icon, or both; without a label it is a square icon button.
//! Fills turn 90% opaque under the pointer and 80% while pressed (shadcn's
//! `hover:bg-primary/90`); the clear variants take the theme's `accent`.

use std::rc::Rc;

use nsh_base::ButtonElement;

use nsh_core::{
    element::{AdaptiveColor, AnyElement, FontWeight, Reactive},
    prelude::*,
};

use crate::{
    size::Size,
    theme::{self, theme},
};

/// What a button looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    /// The one main action: filled with `primary`.
    Primary,
    /// A paired action: filled with `secondary`.
    Secondary,
    /// A bordered, quiet action (the default): `background` inside an
    /// `input` edge.
    #[default]
    Outline,
    /// Chrome and toolbars: no fill until the pointer is over it.
    Ghost,
    /// A destructive action: filled with `destructive`.
    Danger,
    /// Filled with the theme's `success`.
    Success,
    /// Filled with the theme's `warning`.
    Warning,
    /// Filled with the theme's `info`.
    Info,
    /// Text that reads as a link, underlined under the pointer.
    Link,
}

/// The colors of a variant: its fill, its edge, its text.
struct Look {
    fill: AdaptiveColor,
    edge: AdaptiveColor,
    ink: AdaptiveColor,
    /// Under the pointer, for the clear variants (the filled ones fade).
    hover_fill: Option<AdaptiveColor>,
    hover_ink: Option<AdaptiveColor>,
}

impl ButtonVariant {
    fn look(self) -> Look {
        let t = theme();
        let filled = |fill: AdaptiveColor, ink: AdaptiveColor| Look { fill, edge: fill, ink, hover_fill: None, hover_ink: None };
        match self {
            Self::Primary => filled(t.primary, t.primary_foreground),
            Self::Secondary => filled(t.secondary, t.secondary_foreground),
            Self::Danger => filled(theme::danger(), theme::on_strong()),
            Self::Success => filled(theme::success(), theme::on_strong()),
            Self::Warning => filled(theme::warning(), theme::on_strong()),
            Self::Info => filled(theme::info(), theme::on_strong()),
            Self::Outline => Look {
                fill: t.background,
                edge: t.input,
                ink: t.foreground,
                hover_fill: Some(t.accent),
                hover_ink: Some(t.accent_foreground),
            },
            Self::Ghost => Look {
                fill: theme::clear(),
                edge: theme::clear(),
                ink: t.foreground,
                hover_fill: Some(t.accent),
                hover_ink: Some(t.accent_foreground),
            },
            Self::Link => Look { fill: theme::clear(), edge: theme::clear(), ink: theme::link(), hover_fill: None, hover_ink: None },
        }
    }

    /// Its text color: an icon given to the button takes it to match.
    pub fn ink(self) -> AdaptiveColor {
        self.look().ink
    }

    fn filled(self) -> bool {
        !matches!(self, Self::Outline | Self::Ghost | Self::Link)
    }
}

/// A button, built and then placed as an element.
pub struct Button {
    label: Option<Reactive<SharedString>>,
    icon: Option<AnyElement>,
    variant: ButtonVariant,
    size: Size,
    disabled: Reactive<bool>,
    selected: Reactive<bool>,
    accessibility_label: Option<SharedString>,
    on_press: Option<Rc<dyn Fn()>>,
    full_width: bool,
}

impl Default for Button {
    fn default() -> Self {
        Self::new()
    }
}

impl Button {
    /// An outlined, medium button with nothing in it yet.
    pub fn new() -> Self {
        Button {
            label: None,
            icon: None,
            variant: ButtonVariant::default(),
            size: Size::default(),
            disabled: Reactive::Static(false),
            selected: Reactive::Static(false),
            accessibility_label: None,
            on_press: None,
            full_width: false,
        }
    }

    /// Its text.
    pub fn label(mut self, label: impl IntoReactive<SharedString>) -> Self {
        self.label = Some(label.into_reactive());
        self
    }

    /// Its icon, before the label (alone, a square icon button).
    pub fn icon(mut self, icon: impl IntoElement) -> Self {
        self.icon = Some(icon.into_element());
        self
    }

    /// Its variant.
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// The main action: [`ButtonVariant::Primary`].
    pub fn primary(self) -> Self {
        self.variant(ButtonVariant::Primary)
    }

    /// [`ButtonVariant::Secondary`].
    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }

    /// [`ButtonVariant::Ghost`].
    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }

    /// [`ButtonVariant::Danger`].
    pub fn danger(self) -> Self {
        self.variant(ButtonVariant::Danger)
    }

    /// [`ButtonVariant::Link`].
    pub fn link(self) -> Self {
        self.variant(ButtonVariant::Link)
    }

    /// Its size.
    pub fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    /// [`Size::XSmall`].
    pub fn xsmall(self) -> Self {
        self.size(Size::XSmall)
    }

    /// [`Size::Small`].
    pub fn small(self) -> Self {
        self.size(Size::Small)
    }

    /// [`Size::Large`].
    pub fn large(self) -> Self {
        self.size(Size::Large)
    }

    /// Whether it takes presses (faded when not).
    pub fn disabled(mut self, disabled: impl IntoReactive<bool>) -> Self {
        self.disabled = disabled.into_reactive();
        self
    }

    /// Whether it shows as on (a toggle in a toolbar): the pressed fill.
    pub fn selected(mut self, selected: impl IntoReactive<bool>) -> Self {
        self.selected = selected.into_reactive();
        self
    }

    /// What assistive technology calls it: an icon button's name.
    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.accessibility_label = Some(label.into());
        self
    }

    /// What a press does.
    pub fn on_press(mut self, f: impl Fn() + 'static) -> Self {
        self.on_press = Some(Rc::new(f));
        self
    }

    /// As wide as its container.
    pub fn full_width(mut self) -> Self {
        self.full_width = true;
        self
    }

    /// The element, for what only `nsh-base`'s button has (`.tooltip`, a
    /// width).
    pub fn build(self) -> ButtonElement {
        let Button { label, icon, variant, size, disabled, selected, accessibility_label, on_press, full_width } = self;
        let look = variant.look();
        let icon_only = label.is_none();
        let radius = theme().radius;
        let (fill, ink) = (look.fill, look.ink);
        let selected_fill = look.hover_fill.unwrap_or(theme::pressed(fill));
        let mut b = nsh_base::button()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(size.gap()))
            .h(px(size.height()))
            .rounded(px(if icon_only { radius.0 - 2.0 } else { radius.0 }))
            .border_1()
            .border_color(look.edge)
            .group()
            .bg(move || if selected.get() { selected_fill } else { fill })
            .disabled(disabled)
            .disabled_style(|s| s.opacity(0.5));
        b = if icon_only { b.w(px(size.height())) } else { b.px(px(size.padding_x())) };
        if full_width {
            b = b.w_full();
        }
        b = match (variant.filled(), look.hover_fill) {
            (true, _) => b.hover(|s| s.opacity(0.9)).active(|s| s.opacity(0.8)),
            (false, Some(hover)) => b.hover(move |s| s.bg(hover)).active(move |s| s.bg(theme::pressed(hover))),
            (false, None) => b.active(|s| s.opacity(0.8)),
        };
        if let Some(f) = on_press {
            b = b.on_press(move || f());
        }
        if let Some(name) = accessibility_label {
            b = b.accessibility_label(name);
        }
        let hover_ink = look.hover_ink;
        let link = variant == ButtonVariant::Link;
        if let Some(icon) = icon {
            b = b.child(view().text_color(ink).group_hover(move |s| match hover_ink {
                Some(c) => s.text_color(c),
                None => s,
            }).child(icon));
        }
        if let Some(label) = label {
            b = b.child(
                text(move || label.get())
                    .text_size(px(size.text()))
                    .font_weight(FontWeight(500))
                    .text_color(ink)
                    .number_of_lines(1)
                    .group_hover(move |s| {
                        let s = match hover_ink {
                            Some(c) => s.text_color(c),
                            None => s,
                        };
                        if link { s.underline() } else { s }
                    }),
            );
        }
        b
    }
}

impl IntoElement for Button {
    fn into_element(self) -> AnyElement {
        self.build().into_element()
    }
}
