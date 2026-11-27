//! [`Checkbox`], after shadcn/ui's `checkbox.tsx` (new-york-v4).
//!
//! Behavior is `nsh_base`'s `Checkbox` (presses, Space and Return, the
//! checkbox role); its root carries shadcn's `CheckboxPrimitive.Root`
//! classes and its indicator shadcn's `CheckboxPrimitive.Indicator`
//! classes. The classes are shadcn's, word for word, except:
//!
//! - `data-[state=checked]:` is `checked:`: NSH's variant of the checked
//!   state the root reports.
//! - `focus-visible:checked:border-primary` is added: in CSS
//!   `data-[state=checked]:border-primary` comes after
//!   `focus-visible:border-ring`, so a focused checked box keeps its primary
//!   border; NSH ranks `checked:` below the interaction states.
//! - `peer` is left out (NSH has no `peer-*`): a label beside a disabled
//!   checkbox dims itself.
//! - `flex items-center justify-center` is added: the root is a `<button>`
//!   in shadcn, which centers its content.
//! - `border` is `border border-input` already in shadcn.
//! - Radix's `indeterminate` state is not ported: shadcn draws it as an
//!   unchecked box holding the same check mark.

use nsh_core::prelude::*;

use super::field::FieldContext;
use super::state::Controllable;
use crate::cn::cn;
use crate::icon::{IconName, icon};

/// shadcn's root classes.
const ROOT: &str = "size-4 shrink-0 rounded-[4px] border border-input shadow-xs transition-shadow \
    outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 \
    disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive \
    aria-invalid:ring-destructive/20 checked:border-primary checked:bg-primary \
    checked:text-primary-foreground dark:bg-input/30 dark:aria-invalid:ring-destructive/40 \
    dark:checked:bg-primary focus-visible:checked:border-primary flex items-center justify-center";

/// shadcn's indicator classes.
const INDICATOR: &str = "grid place-content-center text-current transition-none";

/// shadcn's check icon class.
const CHECK: &str = "size-3.5";

/// shadcn's `has-[>[data-slot=field-content]]:[&>[role=checkbox]]:mt-px`
/// of a horizontal Field around it: it lines up with the content's first
/// line.
const IN_FIELD_WITH_CONTENT: &str = "mt-px";

/// The props of [`Checkbox`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CheckboxProps {
    /// Checked or not (controlled): the app keeps the value and sets the
    /// one `onCheckedChange` asks for. Without it the checkbox keeps its
    /// own, from `defaultChecked`.
    pub checked: Prop<Option<bool>>,
    /// Checked at first, while `checked` is not set.
    #[prop(default)]
    pub default_checked: Prop<bool>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The value fails the app's checks: a destructive border and ring
    /// (shadcn's `aria-invalid`).
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// What screen readers say: what the checkbox is for (shadcn's label
    /// is a separate `Label`).
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of a [`Checkbox`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum CheckboxEvent {
    /// It was pressed: the value it asks for (Radix's `onCheckedChange`).
    #[args(checked)]
    CheckedChange(bool),
}

/// Every class the root draws with, before the app's.
pub(crate) fn root_classes(in_field_with_content: bool) -> SharedString {
    cn([
        ROOT,
        if in_field_with_content {
            IN_FIELD_WITH_CONTENT
        } else {
            ""
        },
    ])
}

/// shadcn's Checkbox: `<Checkbox defaultChecked onCheckedChange={..} />`.
#[component]
pub fn Checkbox(props: CheckboxProps, cx: Scope<CheckboxEvent>) -> impl IntoElement {
    let checked = Controllable::new(props.checked, props.default_checked.get_untracked());
    let field = use_context::<FieldContext>();
    let toggle = move || {
        if props.disabled.get_untracked() {
            return;
        }
        let next = !checked.get_untracked();
        checked.set(next);
        cx.emit(CheckboxEvent::CheckedChange(next));
    };
    if let Some(field) = field {
        field.control(toggle, move || checked.get());
    }
    nsh_base::checkbox()
        .checked(move || checked.get())
        .disabled(props.disabled)
        .invalid(props.invalid)
        .accessibility_label(props.accessibility_label)
        .on_change(move |_: &bool| toggle())
        .app_class(move || Some(root_classes(field.is_some_and(|f| f.has_content()))))
        .indicator_class(Some(SharedString::from(INDICATOR)))
        .indicator(dynamic(move || {
            checked
                .get()
                .then(|| icon(IconName::Check).app_class(Some(SharedString::from(CHECK))))
        }))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Checkbox>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        for classes in [
            root_classes(false).to_string(),
            root_classes(true).to_string(),
        ] {
            let all = format!("{classes} {INDICATOR} {CHECK}");
            let unknown = crate::cn::unknown_classes(&all);
            assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
        }
    }
}
