//! [`Button`], after shadcn/ui's `button.tsx` (new-york-v4).
//!
//! The classes are shadcn's `buttonVariants`, word for word, except where
//! NSH has no such class (each change is noted beside it):
//!
//! - The `[&_svg]:…` and `has-[>svg]:…` selectors size and pad an icon in
//!   the button; NSH has no descendant selectors, so the `icon` prop's
//!   icon takes the size, and the padding follows whether there is one.
//! - `aria-invalid:` is the `invalid` prop's (NSH has no such state).
//! - `border` is `border border-border`: shadcn's global
//!   `* { @apply border-border }` colors every border, and NSH has no
//!   global rules, so each component names the color.

use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};

/// shadcn's base classes of every button.
const BASE: &str = "inline-flex shrink-0 items-center justify-center gap-2 \
    rounded-md text-sm font-medium whitespace-nowrap transition-all outline-none \
    focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 \
    disabled:pointer-events-none disabled:opacity-50";

/// How a button looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ButtonVariant {
    /// The primary fill: the action a view leads with.
    #[default]
    Default,
    /// A destructive action.
    Destructive,
    /// A bordered button on the background.
    Outline,
    /// A quieter fill.
    Secondary,
    /// No fill until hovered.
    Ghost,
    /// Looks like a link.
    Link,
}

impl ButtonVariant {
    fn classes(self) -> &'static str {
        match self {
            ButtonVariant::Default => "bg-primary text-primary-foreground hover:bg-primary/90",
            ButtonVariant::Destructive => {
                "bg-destructive text-white hover:bg-destructive/90 \
                 focus-visible:ring-destructive/20 dark:bg-destructive/60 \
                 dark:focus-visible:ring-destructive/40"
            }
            ButtonVariant::Outline => {
                "border border-border bg-background shadow-xs hover:bg-accent hover:text-accent-foreground \
                 dark:border-input dark:bg-input/30 dark:hover:bg-input/50"
            }
            ButtonVariant::Secondary => {
                "bg-secondary text-secondary-foreground hover:bg-secondary/80"
            }
            ButtonVariant::Ghost => {
                "hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50"
            }
            ButtonVariant::Link => "text-primary underline-offset-4 hover:underline",
        }
    }
}

/// How big a button is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ButtonSize {
    /// 36 points tall.
    #[default]
    Default,
    /// 24 points tall, extra small text.
    Xs,
    /// 32 points tall.
    Sm,
    /// 40 points tall.
    Lg,
    /// A 36-point square, for an icon alone.
    Icon,
    /// A 24-point square.
    IconXs,
    /// A 32-point square.
    IconSm,
    /// A 40-point square.
    IconLg,
}

impl ButtonSize {
    /// shadcn's size classes; `has_icon` adds its `has-[>svg]:` padding.
    fn classes(self, has_icon: bool) -> &'static str {
        match (self, has_icon) {
            (ButtonSize::Default, false) => "h-9 px-4 py-2",
            (ButtonSize::Default, true) => "h-9 px-4 py-2 px-3",
            (ButtonSize::Xs, false) => "h-6 gap-1 rounded-md px-2 text-xs",
            (ButtonSize::Xs, true) => "h-6 gap-1 rounded-md px-2 text-xs px-1.5",
            (ButtonSize::Sm, false) => "h-8 gap-1.5 rounded-md px-3",
            (ButtonSize::Sm, true) => "h-8 gap-1.5 rounded-md px-3 px-2.5",
            (ButtonSize::Lg, false) => "h-10 rounded-md px-6",
            (ButtonSize::Lg, true) => "h-10 rounded-md px-6 px-4",
            (ButtonSize::Icon, _) => "size-9",
            (ButtonSize::IconXs, _) => "size-6 rounded-md",
            (ButtonSize::IconSm, _) => "size-8",
            (ButtonSize::IconLg, _) => "size-10",
        }
    }

    /// shadcn's `[&_svg:not([class*='size-'])]:size-*`: 12 points in the
    /// extra small sizes, 16 elsewhere.
    fn icon_class(self) -> &'static str {
        match self {
            ButtonSize::Xs | ButtonSize::IconXs => "size-3",
            _ => "size-4",
        }
    }
}

/// Every class a button of `variant` and `size` draws with, before the
/// app's.
pub(crate) fn button_classes(variant: ButtonVariant, size: ButtonSize, has_icon: bool) -> SharedString {
    cn([BASE, variant.classes(), size.classes(has_icon)])
}

/// The props of [`Button`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ButtonProps {
    /// The text on the button, which screen readers say.
    pub label: Prop<Option<SharedString>>,
    /// `default`, `destructive`, `outline`, `secondary`, `ghost` or `link`.
    #[prop(default)]
    pub variant: Prop<ButtonVariant>,
    /// `default`, `xs`, `sm`, `lg`, `icon`, `icon-xs`, `icon-sm` or `icon-lg`.
    #[prop(default)]
    pub size: Prop<ButtonSize>,
    /// A Lucide icon before the label (shadcn puts a `lucide-react` icon
    /// first among the children); alone in the `icon*` sizes.
    pub icon: Prop<Option<IconName>>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// What screen readers say in place of the label (an icon button's name).
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Content before the label in place of a Lucide `icon`: an app's own
    /// icon (a brand's mark, a picture), which the app sizes (16 points
    /// as the icon is, 12 in the extra small sizes). The button takes the
    /// padding it takes with an icon.
    pub leading: Prop<Option<Slot>>,
    /// Content after the label: an icon, a badge.
    pub children: Prop<Children>,
}

/// The events of a [`Button`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ButtonEvent {
    /// It was pressed: a click, a tap, Return or Space.
    Press,
}

/// shadcn's Button: `<Button label="Save" variant="outline" onPress={save} />`.
#[component]
pub fn Button(props: ButtonProps, cx: Scope<ButtonEvent>) -> impl IntoElement {
    let label = props.label;
    view()
        .role(AccessibilityRole::Button)
        .disabled(move || props.disabled.get())
        .cursor_pointer()
        .app_class(move || {
            let has_icon = props.icon.get().is_some() || props.leading.get().is_some();
            Some(button_classes(props.variant.get(), props.size.get(), has_icon))
        })
        .label(move || {
            props
                .accessibility_label
                .get()
                .or_else(|| label.get())
                .unwrap_or_default()
        })
        .on_press(move || cx.emit(ButtonEvent::Press))
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(Some(SharedString::from(props.size.get().icon_class())))
                    .into_element()
            })
        }))
        .child(props.leading.place(|content| content))
        .child(dynamic(move || label.get().map(|label| text(label).into_element())))
        .child(props.children)
}

impl ButtonElement {
    /// `.variant(ButtonVariant::Destructive)`.
    pub fn destructive(self) -> Self {
        self.variant(ButtonVariant::Destructive)
    }
    /// `.variant(ButtonVariant::Outline)`.
    pub fn outline(self) -> Self {
        self.variant(ButtonVariant::Outline)
    }
    /// `.variant(ButtonVariant::Secondary)`.
    pub fn secondary(self) -> Self {
        self.variant(ButtonVariant::Secondary)
    }
    /// `.variant(ButtonVariant::Ghost)`.
    pub fn ghost(self) -> Self {
        self.variant(ButtonVariant::Ghost)
    }
    /// `.variant(ButtonVariant::Link)`.
    pub fn link(self) -> Self {
        self.variant(ButtonVariant::Link)
    }
    /// `.size(ButtonSize::Xs)`.
    pub fn xs(self) -> Self {
        self.size(ButtonSize::Xs)
    }
    /// `.size(ButtonSize::Sm)`.
    pub fn sm(self) -> Self {
        self.size(ButtonSize::Sm)
    }
    /// `.size(ButtonSize::Lg)`.
    pub fn lg(self) -> Self {
        self.size(ButtonSize::Lg)
    }
    /// `.size(ButtonSize::Icon)`.
    pub fn size_icon(self) -> Self {
        self.size(ButtonSize::Icon)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Button>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every class a button can draw with is one NSH knows: an unknown one
    /// would be dropped at run time with only a log line.
    #[test]
    fn every_class_is_known_to_nsh() {
        let variants = [
            ButtonVariant::Default,
            ButtonVariant::Destructive,
            ButtonVariant::Outline,
            ButtonVariant::Secondary,
            ButtonVariant::Ghost,
            ButtonVariant::Link,
        ];
        let sizes = [
            ButtonSize::Default,
            ButtonSize::Xs,
            ButtonSize::Sm,
            ButtonSize::Lg,
            ButtonSize::Icon,
            ButtonSize::IconXs,
            ButtonSize::IconSm,
            ButtonSize::IconLg,
        ];
        let mut unknown = Vec::new();
        for variant in variants {
            for (size, has_icon) in sizes.iter().flat_map(|&s| [(s, false), (s, true)]) {
                let classes = button_classes(variant, size, has_icon);
                let classes = format!("{classes} {}", size.icon_class());
                for class in crate::cn::unknown_classes(&classes) {
                    if !unknown.contains(&class) {
                        unknown.push(class);
                    }
                }
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
