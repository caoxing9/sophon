//! [`ToggleGroup`] and [`ToggleGroupItem`], after shadcn/ui's
//! `toggle-group.tsx` (new-york-v4).
//!
//! Behavior is `nsh_base`'s `ToggleGroup` (one stop of Tab, the arrow
//! keys move between the items, the toolbar role) and `Toggle`; the group
//! keeps the value as Radix's does (`type` single or multiple, `value` /
//! `defaultValue` / `onValueChange`; a single group's value is a list of
//! one or none here). The items draw with shadcn's `toggleVariants` of the
//! group's variant and size (see [`super::Toggle`]), then the item's own
//! classes. The classes are shadcn's, word for word, except:
//!
//! - `gap-[--spacing(var(--gap))]` (a CSS variable NSH has no syntax for)
//!   is `gap-<spacing>`, the same length.
//! - `data-[spacing=0]:` and `data-[variant=outline]:` are the group's
//!   `spacing` and `variant` props'; `first:` and `last:` are NSH's.
//! - `data-[spacing=default]:data-[variant=outline]:shadow-xs` never
//!   applies in shadcn (`data-spacing` is a number) and is left out.

use nsh_core::prelude::*;

use super::state::Controllable;
use super::toggle::{ToggleSize, ToggleVariant, toggle_classes, toggle_content};
use crate::cn::cn;
use crate::icon::IconName;

/// shadcn's root classes.
const ROOT: &str = "group/toggle-group flex w-fit items-center rounded-md";
/// shadcn's item classes after `toggleVariants`.
const ITEM: &str = "w-auto min-w-0 shrink-0 px-3 focus:z-10 focus-visible:z-10";
/// shadcn's item classes while the spacing is 0: the items join.
const JOINED: &str = "rounded-none shadow-none first:rounded-l-md last:rounded-r-md";
/// ... and outlined.
const JOINED_OUTLINE: &str = "border-l-0 first:border-l";

/// Whether one item or any number can be on: Radix's `type`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ToggleGroupType {
    /// At most one item on (an alignment).
    #[default]
    Single,
    /// Any number on (bold, italic, underline).
    Multiple,
}

/// The classes of the group of `spacing`.
pub(crate) fn group_classes(spacing: u32) -> SharedString {
    let gap = format!("gap-{spacing}");
    cn([ROOT, gap.as_str()])
}

/// The classes of an item of a group of `variant`, `size` and `spacing`.
pub(crate) fn item_classes(variant: ToggleVariant, size: ToggleSize, spacing: u32) -> SharedString {
    let joined = spacing == 0;
    let base = toggle_classes(variant, size, false);
    cn([
        base.as_ref(),
        ITEM,
        if joined { JOINED } else { "" },
        if joined && variant == ToggleVariant::Outline {
            JOINED_OUTLINE
        } else {
            ""
        },
    ])
}

/// What an item finds of its group.
#[derive(Clone, Copy)]
struct GroupContext {
    kind: Prop<ToggleGroupType>,
    value: Controllable<Vec<SharedString>>,
    variant: Prop<ToggleVariant>,
    size: Prop<ToggleSize>,
    spacing: Prop<u32>,
    emit: Scope<ToggleGroupEvent>,
}

/// The props of [`ToggleGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ToggleGroupProps {
    /// `single` or `multiple`.
    #[prop(default)]
    pub r#type: Prop<ToggleGroupType>,
    /// The values of the items that are on (controlled): the app keeps
    /// them and sets the ones `onValueChange` asks for. Without it the
    /// group keeps its own, from `defaultValue`.
    pub value: Prop<Option<Vec<SharedString>>>,
    /// The values on at first, while `value` is not set.
    #[prop(default)]
    pub default_value: Prop<Vec<SharedString>>,
    /// The items' `variant`: `default` or `outline`.
    #[prop(default)]
    pub variant: Prop<ToggleVariant>,
    /// The items' `size`: `default`, `sm` or `lg`.
    #[prop(default)]
    pub size: Prop<ToggleSize>,
    /// The gap between the items, in Tailwind's spacing steps (4 points
    /// each); 0 joins them into one bar.
    #[prop(default)]
    pub spacing: Prop<u32>,
    /// Every item takes no presses, at half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The items.
    pub children: Prop<Children>,
}

/// The events of a [`ToggleGroup`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ToggleGroupEvent {
    /// An item was pressed: the values on now (Radix's `onValueChange`).
    #[args(value)]
    ValueChange(Vec<SharedString>),
}

/// shadcn's ToggleGroup: `<ToggleGroup type="multiple" variant="outline">`.
#[component]
pub fn ToggleGroup(props: ToggleGroupProps, cx: Scope<ToggleGroupEvent>) -> impl IntoElement {
    let value = Controllable::new(props.value, props.default_value.get_untracked());
    provide_context(GroupContext {
        kind: props.r#type,
        value,
        variant: props.variant,
        size: props.size,
        spacing: props.spacing,
        emit: cx,
    });
    nsh_base::toggle_group()
        .disabled(props.disabled)
        .app_class(move || Some(group_classes(props.spacing.get())))
        .child(props.children)
}

/// The props of [`ToggleGroupItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ToggleGroupItemProps {
    /// The value the group holds while this item is on.
    #[prop(default)]
    pub value: Prop<SharedString>,
    /// A Lucide icon before the label.
    pub icon: Prop<Option<IconName>>,
    /// The text on the item.
    pub label: Prop<Option<SharedString>>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// What screen readers say: what the item turns on (an icon's name).
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Content after the label.
    pub children: Prop<Children>,
}

/// shadcn's ToggleGroupItem: one toggle of a [`ToggleGroup`].
#[component]
pub fn ToggleGroupItem(props: ToggleGroupItemProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let group = use_context::<GroupContext>();
    let pressed = move || group.is_some_and(|g| g.value.get().contains(&props.value.get()));
    let [icon, label, children] = toggle_content(props.icon, props.label, props.children);
    nsh_base::toggle()
        .pressed(pressed)
        .disabled(props.disabled)
        .accessibility_label(props.accessibility_label)
        .on_change(move |on: &bool| {
            let Some(group) = group else { return };
            let item = props.value.get_untracked();
            let mut values = group.value.get_untracked();
            values.retain(|v| *v != item);
            if *on {
                match group.kind.get_untracked() {
                    ToggleGroupType::Single => values = vec![item],
                    ToggleGroupType::Multiple => values.push(item),
                }
            }
            group.value.set(values.clone());
            group.emit.emit(ToggleGroupEvent::ValueChange(values));
        })
        .app_class(move || {
            let (variant, size, spacing) = group
                .map_or((ToggleVariant::Default, ToggleSize::Default, 0), |g| {
                    (g.variant.get(), g.size.get(), g.spacing.get())
                });
            Some(item_classes(variant, size, spacing))
        })
        .child(icon)
        .child(label)
        .child(children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<ToggleGroup>();
    registry.register::<ToggleGroupItem>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for variant in [ToggleVariant::Default, ToggleVariant::Outline] {
            for size in [ToggleSize::Default, ToggleSize::Sm, ToggleSize::Lg] {
                for spacing in [0, 2] {
                    let all = format!(
                        "{} {}",
                        group_classes(spacing),
                        item_classes(variant, size, spacing)
                    );
                    unknown.extend(crate::cn::unknown_classes(&all));
                }
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
