//! [`RadioGroup`] and [`RadioGroupItem`], after shadcn/ui's
//! `radio-group.tsx` (new-york-v4).
//!
//! Behavior is `nsh_base`'s `RadioGroup` and `Radio` (one stop of Tab,
//! the arrow keys move and check, the radio roles); the group keeps the
//! value as Radix's does (`value` / `defaultValue` / `onValueChange`) and
//! each item checks itself when it holds it. The classes are shadcn's,
//! word for word, except:
//!
//! - The indicator, `CircleIcon` with `absolute top-1/2 left-1/2 size-2
//!   -translate-x-1/2 -translate-y-1/2 fill-primary`, is that circle with
//!   those classes, filled with the text color (`text-primary` on the
//!   item): Lucide's circle stroked and filled.
//! - `aria-invalid:` is the item's `invalid` prop: its classes apply
//!   while it is set (`nsh_base`'s radio reports no invalid state).
//! - `mt-px` beside a [`super::FieldContent`] in a horizontal field is
//!   Field's `[&>[role=radio]]:mt-px`.

use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

use super::field::{FieldContext, FieldParent};
use super::state::Controllable;
use crate::cn::cn;

/// shadcn's RadioGroup classes.
const GROUP: &str = "grid gap-3";

/// shadcn's RadioGroupItem classes.
const ITEM: &str = "aspect-square size-4 shrink-0 rounded-full border border-input text-primary \
    shadow-xs transition-all outline-none focus-visible:border-ring focus-visible:ring-[3px] \
    focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 \
    aria-invalid:border-destructive aria-invalid:ring-destructive/20 dark:bg-input/30 \
    dark:aria-invalid:ring-destructive/40";

/// shadcn's indicator icon classes.
const DOT: &str = "absolute top-1/2 left-1/2 size-2 -translate-x-1/2 -translate-y-1/2";

/// Lucide's `circle`, filled (`fill-primary` over the stroke's
/// `currentColor`, which `text-primary` makes the primary color).
const DOT_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="currentColor" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/></svg>"#;

/// shadcn's `aria-invalid:` classes, while the item is invalid.
const INVALID: &str = "border-destructive ring-destructive/20 dark:ring-destructive/40";

/// Every class an item draws with, before the app's.
pub(crate) fn item_classes(invalid: bool, beside_content: bool) -> SharedString {
    cn([
        ITEM,
        if invalid { INVALID } else { "" },
        if beside_content { "mt-px" } else { "" },
    ])
}

/// What a [`RadioGroupItem`] finds of its group.
#[derive(Clone, Copy)]
struct GroupContext {
    value: Controllable<SharedString>,
    disabled: Prop<bool>,
    emit: Scope<RadioGroupEvent>,
}

/// The props of [`RadioGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct RadioGroupProps {
    /// The value of the checked item (controlled): the app keeps it and
    /// sets the one `onValueChange` asks for. Without it the group keeps
    /// its own, from `defaultValue`.
    pub value: Prop<Option<SharedString>>,
    /// The value checked at first, while `value` is not set.
    pub default_value: Prop<Option<SharedString>>,
    /// Every item takes no presses, at half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// What screen readers call the set.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// The items, and whatever lays them out (a row with a label each).
    pub children: Prop<Children>,
}

/// The events of a [`RadioGroup`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum RadioGroupEvent {
    /// An item was chosen: its value (Radix's `onValueChange`).
    #[args(value)]
    ValueChange(SharedString),
}

/// shadcn's RadioGroup: `<RadioGroup defaultValue="comfortable">`.
#[component]
pub fn RadioGroup(props: RadioGroupProps, cx: Scope<RadioGroupEvent>) -> impl IntoElement {
    let value = Controllable::new(
        props.value,
        props.default_value.get_untracked().unwrap_or_default(),
    );
    FieldParent::current().holds_choice_group();
    provide_context(GroupContext {
        value,
        disabled: props.disabled,
        emit: cx,
    });
    provide_context(FieldParent::Other);
    nsh_base::radio_group()
        .accessibility_label(props.accessibility_label)
        .app_class(Some(SharedString::from(GROUP)))
        .child(props.children)
}

/// The props of [`RadioGroupItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct RadioGroupItemProps {
    /// The value the group holds while this item is checked.
    #[prop(default)]
    pub value: Prop<SharedString>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The choice fails the app's checks: a destructive border and ring.
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// What screen readers say: the option (shadcn's label is a separate
    /// `Label`).
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// shadcn's RadioGroupItem: one option of a [`RadioGroup`].
#[component]
pub fn RadioGroupItem(props: RadioGroupItemProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let group = use_context::<GroupContext>();
    let checked = move || {
        group.is_some_and(|g| {
            let value = props.value.get();
            !value.is_empty() && g.value.get() == value
        })
    };
    let disabled = move || props.disabled.get() || group.is_some_and(|g| g.disabled.get());
    let choose = move || {
        let Some(group) = group else { return };
        if disabled() {
            return;
        }
        let value = props.value.get_untracked();
        if group.value.get_untracked() == value {
            return;
        }
        group.value.set(value.clone());
        group.emit.emit(RadioGroupEvent::ValueChange(value));
    };
    let field = use_context::<FieldContext>();
    if let Some(field) = field {
        field.control(choose, checked);
    }
    nsh_base::radio()
        .checked(checked)
        .disabled(disabled)
        .accessibility_label(props.accessibility_label)
        .on_change(move |_: &bool| choose())
        .app_class(move || {
            Some(item_classes(
                props.invalid.get(),
                field.is_some_and(|f| f.horizontal() && f.has_content()),
            ))
        })
        .child(when(checked, || {
            svg(ImageSrc::svg_markup(DOT_SVG))
                .accessibility_elements_hidden(true)
                .app_class(Some(SharedString::from(DOT)))
        }))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<RadioGroup>();
    registry.register::<RadioGroupItem>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let all = format!("{GROUP} {} {DOT}", item_classes(true, true));
        let unknown = crate::cn::unknown_classes(&all);
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
