//! [`Toggle`], after shadcn/ui's `toggle.tsx` (new-york-v4).
//!
//! Behavior is `nsh_base`'s `Toggle` (presses, Space and Return, a toggle
//! button for screen readers). The classes are shadcn's `toggleVariants`,
//! word for word, except:
//!
//! - `data-[state=on]:` is `aria-pressed:`, NSH's variant of a toggle
//!   button that is on.
//! - `[&_svg]:…` (the icon's size) is the `icon` prop's `size-4`.
//! - `transition-[color,box-shadow]` (unknown to NSH) is `transition`.
//! - `aria-invalid:` is the `invalid` prop's: its classes apply while it
//!   is set (`nsh_base`'s toggle reports no invalid state).
//! - `border` is `border border-input` already in shadcn.

use nsh_core::prelude::*;

use super::state::Controllable;
use crate::cn::cn;
use crate::icon::{IconName, icon};

/// shadcn's base classes of every toggle.
const BASE: &str = "inline-flex items-center justify-center gap-2 rounded-md text-sm font-medium \
    whitespace-nowrap transition outline-none hover:bg-muted hover:text-muted-foreground \
    focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 \
    disabled:pointer-events-none disabled:opacity-50 aria-pressed:bg-accent \
    aria-pressed:text-accent-foreground";

/// shadcn's `aria-invalid:` classes, while invalid.
const INVALID: &str = "border-destructive ring-destructive/20 dark:ring-destructive/40";

/// shadcn's icon size (`[&_svg:not([class*='size-'])]:size-4`).
pub(crate) const ICON: &str = "size-4";

/// How a toggle looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ToggleVariant {
    /// No fill or border until hovered or on.
    #[default]
    Default,
    /// A bordered toggle.
    Outline,
}

impl ToggleVariant {
    fn classes(self) -> &'static str {
        match self {
            ToggleVariant::Default => "bg-transparent",
            ToggleVariant::Outline => {
                "border border-input bg-transparent shadow-xs hover:bg-accent hover:text-accent-foreground"
            }
        }
    }
}

/// How big a toggle is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ToggleSize {
    /// 36 points tall.
    #[default]
    Default,
    /// 32 points tall.
    Sm,
    /// 40 points tall.
    Lg,
}

impl ToggleSize {
    fn classes(self) -> &'static str {
        match self {
            ToggleSize::Default => "h-9 min-w-9 px-2",
            ToggleSize::Sm => "h-8 min-w-8 px-1.5",
            ToggleSize::Lg => "h-10 min-w-10 px-2.5",
        }
    }
}

/// shadcn's `toggleVariants({ variant, size })`, and the invalid classes.
pub(crate) fn toggle_classes(
    variant: ToggleVariant,
    size: ToggleSize,
    invalid: bool,
) -> SharedString {
    cn([
        BASE,
        variant.classes(),
        size.classes(),
        if invalid { INVALID } else { "" },
    ])
}

/// The props of [`Toggle`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ToggleProps {
    /// On or off (controlled): the app keeps the value and sets the one
    /// `onPressedChange` asks for. Without it the toggle keeps its own,
    /// from `defaultPressed`.
    pub pressed: Prop<Option<bool>>,
    /// On at first, while `pressed` is not set.
    #[prop(default)]
    pub default_pressed: Prop<bool>,
    /// `default` or `outline`.
    #[prop(default)]
    pub variant: Prop<ToggleVariant>,
    /// `default`, `sm` or `lg`.
    #[prop(default)]
    pub size: Prop<ToggleSize>,
    /// A Lucide icon before the label.
    pub icon: Prop<Option<IconName>>,
    /// The text on the toggle.
    pub label: Prop<Option<SharedString>>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// A destructive border and ring (shadcn's `aria-invalid`).
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// What screen readers say: what the toggle turns on (an icon toggle's
    /// name).
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Content after the label (an icon of the app's own).
    pub children: Prop<Children>,
}

/// The events of a [`Toggle`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ToggleEvent {
    /// It was pressed: the value it asks for (Radix's `onPressedChange`).
    #[args(pressed)]
    PressedChange(bool),
}

/// The icon, the label and the children of a toggle.
pub(crate) fn toggle_content(
    icon_name: Prop<Option<IconName>>,
    label: Prop<Option<SharedString>>,
    children: Prop<Children>,
) -> [AnyElement; 3] {
    [
        dynamic(move || {
            icon_name.get().map(|name| {
                icon(name)
                    .app_class(Some(SharedString::from(ICON)))
                    .into_element()
            })
        }),
        dynamic(move || label.get().map(|label| text(label).into_element())),
        children.into_element(),
    ]
}

/// shadcn's Toggle: `<Toggle aria-label="Toggle italic"><Italic /></Toggle>`.
#[component]
pub fn Toggle(props: ToggleProps, cx: Scope<ToggleEvent>) -> impl IntoElement {
    let pressed = Controllable::new(props.pressed, props.default_pressed.get_untracked());
    let [icon, label, children] = toggle_content(props.icon, props.label, props.children);
    nsh_base::toggle()
        .pressed(move || pressed.get())
        .disabled(props.disabled)
        .accessibility_label(props.accessibility_label)
        .on_change(move |next: &bool| {
            pressed.set(*next);
            cx.emit(ToggleEvent::PressedChange(*next));
        })
        .app_class(move || {
            Some(toggle_classes(
                props.variant.get(),
                props.size.get(),
                props.invalid.get(),
            ))
        })
        .child(icon)
        .child(label)
        .child(children)
}

impl ToggleElement {
    /// `.variant(ToggleVariant::Outline)`.
    pub fn outline(self) -> Self {
        self.variant(ToggleVariant::Outline)
    }
    /// `.size(ToggleSize::Sm)`.
    pub fn sm(self) -> Self {
        self.size(ToggleSize::Sm)
    }
    /// `.size(ToggleSize::Lg)`.
    pub fn lg(self) -> Self {
        self.size(ToggleSize::Lg)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Toggle>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for variant in [ToggleVariant::Default, ToggleVariant::Outline] {
            for size in [ToggleSize::Default, ToggleSize::Sm, ToggleSize::Lg] {
                for invalid in [false, true] {
                    let all = format!("{} {ICON}", toggle_classes(variant, size, invalid));
                    unknown.extend(crate::cn::unknown_classes(&all));
                }
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
