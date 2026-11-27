//! [`Combobox`], after shadcn/ui's `combobox.tsx` (new-york-v4: Base UI's
//! combobox in an `InputGroup`), on nsh-base's `Combobox`: typing in the
//! input filters the items (by title, ignoring case), the keys move the
//! highlight, a press on the input or on the trigger button opens and
//! closes the list, a sheet on phones.
//!
//! shadcn composes `Combobox`, `ComboboxInput` (an `InputGroup` with an
//! `InputGroupInput` and an `InputGroupAddon` holding the
//! `ComboboxTrigger` button), `ComboboxContent`, `ComboboxList`,
//! `ComboboxItem` and `ComboboxEmpty`; nsh-base's combobox takes its items
//! as data, so here they are one component: `items` are the items (an
//! item's `group` puts it under a `ComboboxLabel`), `placeholder` the
//! input's, `empty` the `ComboboxEmpty`'s text, `content_class` the
//! content's `className`; the app's `class` is `ComboboxInput`'s
//! `className` (on the input group, the root). `InputGroup`,
//! `InputGroupInput`, `InputGroupAddon` and `InputGroupButton` (the forms
//! family's) are drawn here with their classes, merged as shadcn's `cn`
//! merges them. Changes from the classes:
//!
//! - `has-[[data-slot=input-group-control]:focus-visible]:…` on the group
//!   is `focus-within:…` (a text input shows `focus-visible` whenever it
//!   has the focus); the other `has-[…]` rules are applied where they hold
//!   (`pr-2` on the input beside an `inline-end` addon, the addon's
//!   `mr-[-0.45rem]` around a button); `group-data-[disabled=true]/…` and
//!   the input's `aria-invalid:` / `file:` classes are left out (no
//!   `disabled`, `invalid` or file input here).
//! - NSH has no `order-*` (the addon comes after the input anyway),
//!   `scroll-py-*` (scroll padding for keyboard scrolling) or
//!   `selection:text-*` (only the highlight's color): left out.
//! - `transition-[color,box-shadow]` is `transition`;
//!   `rounded-[calc(var(--radius)-5px)]` is `rounded-[5px]` (the radius is
//!   10 points); `pointer-coarse:` sizes are left out.
//! - The content is at least as wide as the input group, nsh-base's
//!   popup at its anchor: Base UI's `w-(--anchor-width)
//!   min-w-[calc(var(--anchor-width)+--spacing(7))]` makes it the input's
//!   width (the group's less its addon) plus 28 points, about the group's,
//!   from the input's start. `max-w-(--available-width)` and the list's
//!   `max-h-[min(…)]` are nsh-base's room; `max-h-96` stays.
//! - An item's `data-highlighted:` is `focus:`, its `ItemIndicator` shows
//!   on `group-selected:`; the row adds `flex-col items-stretch` so the
//!   content nsh-base renders fills it, and the indicator's `right-2` from
//!   the row's edge is `-right-6` from that content (inside the row's
//!   `pr-8`). `ComboboxEmpty`'s `hidden … group-data-empty/…:flex` is
//!   `flex`: nsh-base shows it only while no item does. The animations are
//!   nsh-base's popup transitions (`menu_common.rs`).
//! - `multiple` with `ComboboxChips` is not ported: one value.

use nsh_base::{ComboboxEvent, SelectEntry, SelectItem};
use nsh_core::prelude::*;

use super::menu_common;
use crate::icon::{IconName, icon};

/// `InputGroup`, with `ComboboxInput`'s `w-auto`.
const GROUP: &str = "group/input-group relative flex w-auto items-center rounded-md border \
    border-input shadow-xs transition outline-none dark:bg-input/30 h-9 min-w-0 \
    focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/50";

/// `Input` merged with `InputGroupInput` (and the group's `pr-2` beside an
/// `inline-end` addon).
const INPUT: &str = "h-9 w-full min-w-0 flex-1 rounded-none border-0 bg-transparent px-3 py-1 \
    text-base shadow-none transition outline-none selection:bg-primary \
    placeholder:text-muted-foreground \
    disabled:pointer-events-none disabled:cursor-not-allowed disabled:opacity-50 md:text-sm \
    focus-visible:ring-0 dark:bg-transparent pr-2";

/// `InputGroupAddon` `align="inline-end"` holding a button.
const ADDON: &str = "flex h-auto cursor-text items-center justify-center gap-2 py-1.5 text-sm \
    font-medium text-muted-foreground select-none pr-3 mr-[-0.45rem]";

/// `InputGroupButton` (`ghost`, `icon-xs`): Button's base and ghost
/// classes, then the group button's.
const BUTTON: &str = "inline-flex shrink-0 items-center justify-center gap-2 rounded-md text-sm \
    font-medium whitespace-nowrap transition-all outline-none focus-visible:border-ring \
    focus-visible:ring-[3px] focus-visible:ring-ring/50 disabled:pointer-events-none \
    disabled:opacity-50 hover:bg-accent hover:text-accent-foreground dark:hover:bg-accent/50 \
    flex items-center gap-2 text-sm shadow-none size-6 rounded-[5px] p-0";

/// `ComboboxTrigger`'s chevron.
const CHEVRON: &str = "pointer-events-none size-4 text-muted-foreground";

const CONTENT: &str = "group/combobox-content relative max-h-96 overflow-hidden rounded-md \
    bg-popover text-popover-foreground shadow-md ring-1 ring-foreground/10";

/// The content's `duration-100`, over the popup transition's 150 ms.
const DURATION: &str = "duration-100";

const LIST: &str = "p-1";

const ITEM: &str = "group relative flex w-full cursor-default items-center gap-2 rounded-sm py-1.5 \
    pr-8 pl-2 text-sm outline-hidden select-none focus:bg-accent focus:text-accent-foreground \
    disabled:pointer-events-none disabled:opacity-50 flex-col items-stretch";

/// The content of a row nsh-base renders.
const ITEM_CONTENT: &str = "flex items-center gap-2";

const INDICATOR: &str = "pointer-events-none absolute -right-6 flex size-4 items-center \
    justify-center opacity-0 group-selected:opacity-100";
const CHECK: &str = "pointer-events-none size-4";

const LABEL: &str = "px-2 py-1.5 text-xs text-muted-foreground";

const EMPTY: &str = "flex w-full justify-center py-2 text-center text-sm text-muted-foreground";

/// The props of [`Combobox`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ComboboxProps {
    /// The items: a value and a title each (an item's `group` puts it
    /// under a label).
    #[prop(default)]
    pub items: Prop<Vec<SelectItem>>,
    /// The chosen item's value (controlled; `null` for none): the app sets
    /// it in `onValueChange`. Left out, the combobox keeps its own.
    pub value: Prop<Option<Option<SharedString>>>,
    /// The value at first, while `value` is not set.
    pub default_value: Prop<Option<SharedString>>,
    /// What the input shows while it is empty.
    pub placeholder: Prop<Option<SharedString>>,
    /// What the list says while no item matches (`ComboboxEmpty`).
    pub empty: Prop<Option<SharedString>>,
    /// It does not open; half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// `ComboboxContent`'s `className`.
    pub content_class: Prop<Option<SharedString>>,
    /// What screen readers call it.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of a [`Combobox`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ComboboxEvents {
    /// The user chose an item: its value. Set `value` to follow it.
    #[args(value)]
    ValueChange(Option<SharedString>),
    /// The list opened or closed.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's Combobox: an input whose typing filters a list to choose from.
#[component]
pub fn Combobox(props: ComboboxProps, cx: Scope<ComboboxEvents>) -> impl IntoElement {
    let own = signal(props.default_value.get_untracked());
    let value = computed(move || match props.value.get() {
        Some(value) => value,
        None => own.get(),
    });
    let content_class = props.content_class;
    let empty = props.empty;
    nsh_base::combobox()
        .app_class(Some(SharedString::from(GROUP)))
        .items(props.items)
        .value(move || Some(value.get().into_iter().collect::<Vec<_>>()))
        .placeholder(props.placeholder)
        .disabled(props.disabled)
        .accessibility_label(props.accessibility_label)
        .offset(px(6.0))
        .input_class(INPUT)
        .popup_class(move || {
            Some(menu_common::content_classes(
                CONTENT,
                &crate::cn::cn([menu_common::motion(Some(Placement::Bottom)), DURATION]),
                content_class.get(),
            ))
        })
        .popup_starting_class(menu_common::starting(Some(Placement::Bottom)))
        .popup_ending_class(menu_common::PRESENCE)
        .list_class(LIST)
        .item_class(ITEM)
        .group_class(LABEL)
        .render_item(|entry: &SelectEntry| {
            view()
                .app_class(Some(SharedString::from(ITEM_CONTENT)))
                .child(text(entry.title.clone()))
                .child(
                    view()
                        .app_class(Some(SharedString::from(INDICATOR)))
                        .child(icon(IconName::Check).app_class(Some(SharedString::from(CHECK)))),
                )
        })
        .empty(dynamic(move || {
            empty
                .get()
                .map(|words| text(words).app_class(Some(SharedString::from(EMPTY))).into_element())
        }))
        .trigger_button(
            view().app_class(Some(SharedString::from(ADDON))).child(
                view()
                    .app_class(Some(SharedString::from(BUTTON)))
                    .child(icon(IconName::ChevronDown).app_class(Some(SharedString::from(CHEVRON)))),
            ),
        )
        .on_change(move |next: &Vec<SharedString>| {
            let next = next.first().cloned();
            if props.value.get_untracked().is_none() {
                own.set(next.clone());
            }
            cx.emit(ComboboxEvents::ValueChange(next));
        })
        .on_open_change(move |e: &ComboboxEvent| {
            if let ComboboxEvent::OpenChange(open, _) = e {
                cx.emit(ComboboxEvents::OpenChange(*open));
            }
        })
}

/// Every class string the combobox draws with.
#[cfg(test)]
fn all_classes() -> Vec<String> {
    [
        GROUP, INPUT, ADDON, BUTTON, CHEVRON, CONTENT, DURATION, LIST, ITEM, ITEM_CONTENT, INDICATOR, CHECK,
        LABEL, EMPTY,
    ]
    .iter()
    .map(|c| crate::cn::cn([*c]).to_string())
    .collect()
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Combobox>();
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_class_is_known_to_nsh() {
        super::menu_common::assert_known(super::all_classes());
    }
}
