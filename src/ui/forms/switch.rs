//! [`Switch`], after shadcn/ui's `switch.tsx` (new-york-v4).
//!
//! Behavior is `nsh_base`'s `Switch`: its root carries shadcn's
//! `SwitchPrimitive.Root` classes, its thumb `SwitchPrimitive.Thumb`'s; its
//! track (a part shadcn does not have, the root being the track) only lays
//! the thumb out (`flex flex-1 items-center self-stretch`). The classes
//! are shadcn's, word for word, except:
//!
//! - `data-[state=checked]:` is `checked:`, and `data-[state=unchecked]:`
//!   the plain class; in dark, `dark:checked:bg-primary` (root) keeps the
//!   checked fill over `dark:bg-input/80`, which CSS's exclusive states do.
//! - `data-[size=…]:` / `group-data-[size=…]/switch:` are the `size` prop's.
//! - `translate-x-[calc(100%-2px)]` (unknown to NSH) is the length it comes
//!   to: 14 points (`translate-x-3.5`) at the default size, 10
//!   (`translate-x-2.5`) at `sm`.
//! - `peer` is left out (NSH has no `peer-*`).

use nsh_core::prelude::*;

use super::field::FieldContext;
use super::state::Controllable;
use crate::cn::cn;

/// shadcn's root classes.
const ROOT: &str = "group/switch inline-flex shrink-0 items-center rounded-full border \
    border-transparent shadow-xs transition-all outline-none focus-visible:border-ring \
    focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:cursor-not-allowed \
    disabled:opacity-50 checked:bg-primary bg-input dark:bg-input/80 dark:checked:bg-primary";

/// The track part: the root's content box.
const TRACK: &str = "flex flex-1 items-center self-stretch";

/// shadcn's thumb classes.
const THUMB: &str = "pointer-events-none block rounded-full bg-background ring-0 \
    transition-transform checked:translate-x-[calc(100%-2px)] translate-x-0 \
    dark:checked:bg-primary-foreground dark:bg-foreground";

/// How big a switch is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SwitchSize {
    /// 32 × 18.4 points.
    #[default]
    Default,
    /// 24 × 14 points.
    Sm,
}

impl SwitchSize {
    fn root(self) -> &'static str {
        match self {
            SwitchSize::Default => "h-[1.15rem] w-8",
            SwitchSize::Sm => "h-3.5 w-6",
        }
    }

    fn thumb(self) -> &'static str {
        match self {
            SwitchSize::Default => "size-4 checked:translate-x-3.5",
            SwitchSize::Sm => "size-3 checked:translate-x-2.5",
        }
    }
}

/// Every class the root, then the thumb, draw with.
pub(crate) fn switch_classes(size: SwitchSize) -> (SharedString, SharedString) {
    // shadcn's `translate-x-[calc(100%-2px)]` NSH cannot parse.
    let thumb = THUMB.replace(" checked:translate-x-[calc(100%-2px)]", "");
    (cn([ROOT, size.root()]), cn([&thumb, size.thumb()]))
}

/// The props of [`Switch`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SwitchProps {
    /// On or off (controlled): the app keeps the value and sets the one
    /// `onCheckedChange` asks for. Without it the switch keeps its own,
    /// from `defaultChecked`.
    pub checked: Prop<Option<bool>>,
    /// On at first, while `checked` is not set.
    #[prop(default)]
    pub default_checked: Prop<bool>,
    /// `default` or `sm`.
    #[prop(default)]
    pub size: Prop<SwitchSize>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// What screen readers say: what the switch turns on.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of a [`Switch`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum SwitchEvent {
    /// It was pressed: the value it asks for (Radix's `onCheckedChange`).
    #[args(checked)]
    CheckedChange(bool),
}

/// shadcn's Switch: `<Switch onCheckedChange={..} />`.
#[component]
pub fn Switch(props: SwitchProps, cx: Scope<SwitchEvent>) -> impl IntoElement {
    let checked = Controllable::new(props.checked, props.default_checked.get_untracked());
    let toggle = move || {
        if props.disabled.get_untracked() {
            return;
        }
        let next = !checked.get_untracked();
        checked.set(next);
        cx.emit(SwitchEvent::CheckedChange(next));
    };
    if let Some(field) = use_context::<FieldContext>() {
        field.control(toggle, move || checked.get());
    }
    nsh_base::switch()
        .checked(move || checked.get())
        .disabled(props.disabled)
        .accessibility_label(props.accessibility_label)
        .on_change(move |_: &bool| toggle())
        .app_class(move || Some(switch_classes(props.size.get()).0))
        .track_class(Some(SharedString::from(TRACK)))
        .thumb_class(move || Some(switch_classes(props.size.get()).1))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Switch>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        for size in [SwitchSize::Default, SwitchSize::Sm] {
            let (root, thumb) = switch_classes(size);
            let all = format!("{root} {TRACK} {thumb}");
            let unknown = crate::cn::unknown_classes(&all);
            assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
        }
    }
}
