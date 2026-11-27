//! [`Select`], after shadcn/ui's `select.tsx` (new-york-v4), on nsh-base's
//! `Select`: the list opens at the trigger with the chosen item's row over
//! it on macOS (Radix's default `position="item-aligned"`; on the side of
//! the trigger elsewhere, and while nothing is chosen), the keys, typeahead,
//! a sheet on phones.
//!
//! shadcn composes `Select`, `SelectTrigger`, `SelectValue`,
//! `SelectContent`, `SelectGroup`, `SelectLabel` and `SelectItem`; nsh-base's
//! select takes its items as data (a value, a title, a `group`), so here
//! they are one component: `items` are the `SelectItem`s, an item's `group`
//! the `SelectGroup` with its `SelectLabel`, `placeholder` the
//! `SelectValue`'s, `size` the trigger's, `content_class` the content's
//! `className` (the app's own `class` goes on the root, around the trigger:
//! a width, `w-[180px]`, which the trigger fills). The classes are shadcn's,
//! with these changes:
//!
//! - `transition-[color,box-shadow]` (an arbitrary property list NSH does
//!   not parse) is `transition`, which animates colors and shadows too.
//! - `w-fit` is the root's; `data-[size=…]:h-*` is the `size` prop's,
//!   `data-[placeholder]:text-muted-foreground` applies while nothing is
//!   chosen, `aria-invalid:` is the `invalid` prop's.
//! - `[&_svg]:…` size and color the chevron and the check themselves;
//!   `*:data-[slot=select-value]:line-clamp-1 …` is on the value's text.
//! - `focus-visible:` on the trigger is not drawn: nsh-base gives the
//!   trigger's keyboard focus to a node around the view a Rust component
//!   fills the trigger with.
//! - An item's `data-[highlighted]` is `focus:` (nsh-base projects it on
//!   the highlighted row), its `ItemIndicator` shows on `group-selected:`.
//!   The row's content is a view of its own (nsh-base's `renderItem`, in
//!   a holder nsh-base places in the row), which holds the indicator: the
//!   row adds `flex-col items-stretch` so the holder fills the row inside
//!   its `pr-8`, the content's own view is the row `flex items-center
//!   gap-2`, and the indicator's `right-2` from the row's edge is
//!   `-right-6` from the content's (an absolute view is placed in its
//!   parent).
//! - The content's `p-1` viewport is the list's (`list_class`);
//!   `SelectScrollUpButton` / `SelectScrollDownButton` are not drawn: the
//!   list scrolls natively. The animations are nsh-base's popup
//!   transitions (`menu_common.rs`), without a slide: item-aligned, Radix
//!   gives the content no side to slide from.
//! - `SelectSeparator` is not a part: nsh-base's select lists items and
//!   their sections' headers only.

use nsh_base::{SelectEntry, SelectEvent, SelectItem, SelectValue};
use nsh_core::prelude::*;

use super::menu_common;
use crate::cn::cn;
use crate::icon::{IconName, icon};

const ROOT: &str = "w-fit";

const TRIGGER: &str = "flex items-center justify-between gap-2 rounded-md border border-input \
    bg-transparent px-3 py-2 text-sm whitespace-nowrap shadow-xs transition \
    outline-none focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 \
    disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30 dark:hover:bg-input/50";

const INVALID: &str = "border-destructive ring-destructive/20 dark:ring-destructive/40";
const PLACEHOLDER: &str = "text-muted-foreground";
const VALUE: &str = "line-clamp-1 flex items-center gap-2";
const CHEVRON: &str = "size-4 opacity-50 text-muted-foreground";

const CONTENT: &str = "relative z-50 min-w-[8rem] rounded-md border border-border bg-popover \
    text-popover-foreground shadow-md";

const VIEWPORT: &str = "p-1";

const LABEL: &str = "px-2 py-1.5 text-xs text-muted-foreground";

const ITEM: &str = "group relative flex w-full cursor-default items-center gap-2 rounded-sm py-1.5 \
    pr-8 pl-2 text-sm outline-hidden select-none focus:bg-accent focus:text-accent-foreground \
    disabled:pointer-events-none disabled:opacity-50 flex-col items-stretch";

const INDICATOR: &str = "absolute -right-6 flex size-3.5 items-center justify-center opacity-0 \
    group-selected:opacity-100";
const CHECK: &str = "size-4 text-muted-foreground";

/// How tall the trigger is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SelectSize {
    /// 36 points.
    #[default]
    Default,
    /// 32 points.
    Sm,
}

/// The trigger's classes.
fn trigger_classes(size: SelectSize, placeholder: bool, invalid: bool) -> SharedString {
    cn([
        TRIGGER,
        match size {
            SelectSize::Sm => "h-8",
            _ => "h-9",
        },
        if placeholder { PLACEHOLDER } else { "" },
        if invalid { INVALID } else { "" },
    ])
}

/// The props of [`Select`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SelectProps {
    /// The items (`SelectItem`s): a value and a title each; adjacent items
    /// of one `group` show under its label (`SelectGroup`, `SelectLabel`).
    #[prop(default)]
    pub items: Prop<Vec<SelectItem>>,
    /// The chosen item's value (controlled; `null` for none): the app sets
    /// it in `onValueChange`. Left out, the select keeps its own.
    #[prop(default)]
    pub value: Prop<SelectValue>,
    /// The value at first, while `value` is not set.
    pub default_value: Prop<Option<SharedString>>,
    /// What the trigger says while nothing is chosen.
    pub placeholder: Prop<Option<SharedString>>,
    /// The trigger's height: `default` or `sm`.
    #[prop(default)]
    pub size: Prop<SelectSize>,
    /// It does not open; half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The value is rejected (`aria-invalid`): a destructive border.
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// `SelectContent`'s `className`.
    pub content_class: Prop<Option<SharedString>>,
    /// What screen readers call it.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of a [`Select`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum SelectEvents {
    /// The user chose an item: its value. Set `value` to follow it.
    #[args(value)]
    ValueChange(Option<SharedString>),
    /// The list opened or closed.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's Select: one value from a list its trigger opens.
#[component]
pub fn Select(props: SelectProps, cx: Scope<SelectEvents>) -> impl IntoElement {
    let own = signal(props.default_value.get_untracked());
    let value = computed(move || match props.value.get() {
        SelectValue::Controlled(value) => value,
        _ => own.get(),
    });
    let items = props.items;
    let title = computed(move || {
        let value = value.get()?;
        items.with(|items| {
            items
                .iter()
                .find(|item| item.value == value)
                .map(|item| item.title.clone())
        })
    });
    let content_class = props.content_class;
    view().app_class(Some(SharedString::from(ROOT))).child(
        nsh_base::select()
            .items(items)
            .value(move || SelectValue::Controlled(value.get()))
            .placeholder(props.placeholder)
            .disabled(props.disabled)
            .accessibility_label(props.accessibility_label)
            .popup_class(move || {
                Some(menu_common::content_classes(CONTENT, menu_common::motion(None), content_class.get()))
            })
            .popup_starting_class(menu_common::PRESENCE)
            .popup_ending_class(menu_common::PRESENCE)
            .list_class(VIEWPORT)
            .item_class(ITEM)
            .group_class(LABEL)
            .render_item(|entry: &SelectEntry| {
                view()
                    .flex_1()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(text(entry.title.clone()).app_class(Some(SharedString::from(VALUE))))
                    .child(
                        view()
                            .app_class(Some(SharedString::from(INDICATOR)))
                            .child(icon(IconName::Check).app_class(Some(SharedString::from(CHECK)))),
                    )
            })
            .on_change(move |next: &Option<SharedString>| {
                if props.value.get_untracked() == SelectValue::Uncontrolled {
                    own.set(next.clone());
                }
                cx.emit(SelectEvents::ValueChange(next.clone()));
            })
            .on_open_change(move |e: &SelectEvent| {
                if let SelectEvent::OpenChange(open, _) = e {
                    cx.emit(SelectEvents::OpenChange(*open));
                }
            })
            .trigger(
                view()
                    .disabled(props.disabled)
                    .app_class(move || {
                        Some(trigger_classes(
                            props.size.get(),
                            title.get().is_none(),
                            props.invalid.get(),
                        ))
                    })
                    .child(
                        text(move || {
                            title
                                .get()
                                .or_else(|| props.placeholder.get())
                                .unwrap_or_default()
                        })
                        .app_class(Some(SharedString::from(VALUE))),
                    )
                    .child(icon(IconName::ChevronDown).app_class(Some(SharedString::from(CHEVRON)))),
            ),
    )
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Select>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = vec![
            ROOT, VALUE, CHEVRON, CONTENT, VIEWPORT, LABEL, ITEM, INDICATOR, CHECK,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        for size in [SelectSize::Default, SelectSize::Sm] {
            for placeholder in [false, true] {
                for invalid in [false, true] {
                    all.push(trigger_classes(size, placeholder, invalid).to_string());
                }
            }
        }
        menu_common::assert_known(all);
    }
}
