//! [`NativeSelect`], after shadcn/ui's `native-select.tsx` (new-york-v4).
//!
//! shadcn styles a browser's `<select>`, whose list is the system's menu.
//! NSH has no such element; `nsh_base`'s `Select` is the behavior (the
//! keyboard, typeahead, the combobox role; on macOS its list opens with
//! the chosen row over the trigger, as the system's pop-up buttons do).
//! Its trigger draws with shadcn's `select` classes and chevron, its root
//! with the wrapper's. The classes are shadcn's, word for word, except:
//!
//! - The wrapper's `has-[select:disabled]:opacity-50` is the `disabled`
//!   prop's `opacity-50`; `data-[size=sm]:` the `size` prop's.
//! - `aria-invalid:` is the `invalid` prop's: its classes apply while it is
//!   set.
//! - `appearance-none`, `selection:text-primary-foreground` and
//!   `placeholder:` (no placeholder or selection on a select) are left out;
//!   `transition-[color,box-shadow]` is `transition`.
//! - `focus-visible:` is `group-focus-within/native-select:`: `nsh_base`
//!   wraps the trigger in its own node, which takes the focus, so the
//!   trigger shows the focus of the wrapper's group (after a press too, not
//!   only after a key).
//! - A `<select>` is as wide as its longest option: the trigger lays every
//!   title out, hidden (`h-0 invisible`), under the chosen one.
//! - The chevron is inside the trigger rather than the wrapper:
//!   `right-3.5` from the wrapper's edge is `right-[13px]` inside the
//!   trigger's border.
//! - The list (the system's menu in a browser) draws as shadcn's Select
//!   content: `rounded-md border bg-popover p-1 shadow-md`, its options as
//!   SelectItem (`rounded-sm py-1.5 pl-2 pr-8 text-sm focus:bg-accent`),
//!   its groups' titles as SelectLabel (`px-2 py-1.5 text-xs
//!   text-muted-foreground`). NativeSelectOption's `bg-[Canvas]
//!   text-[CanvasText]` (CSS system colors, unknown to NSH) are the
//!   popover's colors.

use nsh_base::SelectItem;
use nsh_core::prelude::*;

use super::state::Controllable;
use crate::cn::cn;
use crate::icon::{IconName, icon};

/// shadcn's wrapper classes.
const WRAPPER: &str = "group/native-select relative w-fit";
/// shadcn's select classes.
const SELECT: &str = "h-9 w-full min-w-0 rounded-md border border-input bg-transparent px-3 py-2 \
    pr-9 text-sm shadow-xs transition outline-none disabled:pointer-events-none \
    disabled:cursor-not-allowed dark:bg-input/30 dark:hover:bg-input/50 \
    group-focus-within/native-select:border-ring group-focus-within/native-select:ring-[3px] \
    group-focus-within/native-select:ring-ring/50";
/// The trigger's layout (a `<select>` lays its text out itself).
const TRIGGER_LAYOUT: &str = "relative flex flex-row items-center";
/// shadcn's `aria-invalid:` classes, while invalid.
const INVALID: &str = "border-destructive ring-destructive/20 dark:ring-destructive/40 \
    group-focus-within/native-select:border-destructive";
/// shadcn's icon classes, `right-3.5` inside the trigger's border.
const CHEVRON: &str = "pointer-events-none absolute top-1/2 right-[13px] size-4 -translate-y-1/2 \
    text-muted-foreground opacity-50 select-none";
/// The hidden titles that give the trigger its width.
const SIZER: &str = "h-0 overflow-hidden invisible";
/// The list, its options and its groups' titles.
const POPUP: &str = "min-w-[8rem] rounded-md border border-border bg-popover p-1 \
    text-popover-foreground shadow-md";
const OPTION: &str = "relative flex w-full items-center gap-2 rounded-sm py-1.5 pr-8 pl-2 text-sm \
    select-none focus:bg-accent focus:text-accent-foreground disabled:pointer-events-none \
    disabled:opacity-50";
const GROUP: &str = "px-2 py-1.5 text-xs text-muted-foreground";

/// How big a native select is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum NativeSelectSize {
    /// 36 points tall.
    #[default]
    Default,
    /// 32 points tall.
    Sm,
}

/// Every class the trigger draws with, before the app's.
pub(crate) fn trigger_classes(size: NativeSelectSize, invalid: bool) -> SharedString {
    cn([
        SELECT,
        if size == NativeSelectSize::Sm {
            "h-8 py-1"
        } else {
            ""
        },
        if invalid { INVALID } else { "" },
        TRIGGER_LAYOUT,
    ])
}

/// The props of [`NativeSelect`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct NativeSelectProps {
    /// The options: a value and a title each (shadcn's
    /// `NativeSelectOption`), a group each optionally (its
    /// `NativeSelectOptGroup`).
    #[prop(default)]
    pub options: Prop<Vec<SelectItem>>,
    /// The chosen option's value (controlled): the app keeps it and sets
    /// the one `onChange` reports. Without it the select keeps its own,
    /// from `defaultValue`, else the first option, as a `<select>` does.
    pub value: Prop<Option<SharedString>>,
    /// The value chosen at first, while `value` is not set.
    pub default_value: Prop<Option<SharedString>>,
    /// `default` or `sm`.
    #[prop(default)]
    pub size: Prop<NativeSelectSize>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// A destructive border and ring (shadcn's `aria-invalid`).
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// What screen readers call the select.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of a [`NativeSelect`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum NativeSelectEvent {
    /// An option was chosen: its value.
    #[args(value)]
    Change(SharedString),
}

/// shadcn's NativeSelect: `<NativeSelect><NativeSelectOption …/></NativeSelect>`.
#[component]
pub fn NativeSelect(props: NativeSelectProps, cx: Scope<NativeSelectEvent>) -> impl IntoElement {
    let options = props.options;
    let first = options
        .get_untracked()
        .first()
        .map(|o| o.value.clone())
        .unwrap_or_default();
    let value = Controllable::new(
        props.value,
        props.default_value.get_untracked().unwrap_or(first),
    );
    let shown = move || {
        let value = value.get();
        options.with(|options| {
            options
                .iter()
                .find(|o| o.value == value)
                .map(|o| o.title.clone())
                .unwrap_or_default()
        })
    };
    let sizer = dynamic(move || {
        options
            .get()
            .into_iter()
            .map(|o| {
                text(o.title)
                    .app_class(Some(SharedString::from(SIZER)))
                    .into_element()
            })
            .collect::<Vec<_>>()
    });
    let trigger = view()
        .app_class(move || Some(trigger_classes(props.size.get(), props.invalid.get())))
        .child(
            view()
                .flex_col()
                .child(text(shown).number_of_lines(1))
                .child(sizer),
        )
        .child(icon(IconName::ChevronDown).app_class(Some(SharedString::from(CHEVRON))));
    nsh_base::select()
        .items(options)
        .value(move || Some(value.get()))
        .disabled(props.disabled)
        .accessibility_label(props.accessibility_label)
        .on_change(move |next: &Option<SharedString>| {
            if let Some(next) = next {
                value.set(next.clone());
                cx.emit(NativeSelectEvent::Change(next.clone()));
            }
        })
        .app_class(move || {
            Some(cn([
                WRAPPER,
                if props.disabled.get() {
                    "opacity-50"
                } else {
                    ""
                },
            ]))
        })
        .popup_class(Some(SharedString::from(POPUP)))
        .item_class(Some(SharedString::from(OPTION)))
        .group_class(Some(SharedString::from(GROUP)))
        .trigger(trigger)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<NativeSelect>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for size in [NativeSelectSize::Default, NativeSelectSize::Sm] {
            for invalid in [false, true] {
                let all = format!(
                    "{} {WRAPPER} opacity-50 {CHEVRON} {SIZER} {POPUP} {OPTION} {GROUP}",
                    trigger_classes(size, invalid)
                );
                unknown.extend(crate::cn::unknown_classes(&all));
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
