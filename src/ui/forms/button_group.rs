//! [`ButtonGroup`], [`ButtonGroupText`] and [`ButtonGroupSeparator`], after
//! shadcn/ui's `button-group.tsx` (new-york-v4).
//!
//! shadcn joins the group's children with selectors on the group
//! (`[&>*:not(:first-child)]:rounded-l-none`, …), which NSH does not have:
//! the group's children carry those classes themselves. In Rust,
//! [`ButtonGroupElement::item`] adds a child with them (`rounded-l-none
//! border-l-0 rounded-r-none` with `first:` / `last:` giving back the
//! ends, for a horizontal group; `t` / `b` for a vertical one); a child
//! added with `.child(..)` is placed as it is. `item` sets the child's app
//! classes: [`ButtonGroupElement::item_with`] adds the child's own. The
//! group's other selectors are the children's classes too:
//! `[&>*]:focus-visible:z-10`, `[&>input]:flex-1` (an [`super::Input`]),
//! and `has-[>[data-slot=button-group]]:gap-2` is the group's while a
//! group is inside it. The `has-[select…]` / `select-trigger` selectors
//! are for shadcn's Select, another family's.
//!
//! ButtonGroupSeparator is shadcn's `Separator` (`separator.tsx`'s classes,
//! drawn here) with ButtonGroupSeparator's classes; ButtonGroupText's
//! `border` is `border border-border` and its `[&_svg]:…` the `icon` prop's
//! `size-4`.

use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};
use crate::ui::button::{ButtonElement, ButtonVariant};

/// shadcn's base classes of a group.
const ROOT: &str = "flex w-fit items-stretch";
/// `has-[>[data-slot=button-group]]:gap-2`.
const NESTED: &str = "gap-2";

/// How a group runs: shadcn's `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ButtonGroupOrientation {
    /// A row.
    #[default]
    Horizontal,
    /// A column.
    Vertical,
}

/// The classes the group's selectors give a child: `bordered` children
/// get their first edge's border back at the start.
pub(crate) fn item_classes(orientation: ButtonGroupOrientation, bordered: bool) -> &'static str {
    match (orientation, bordered) {
        (ButtonGroupOrientation::Horizontal, true) => {
            "rounded-l-none border-l-0 rounded-r-none first:rounded-l-md first:border-l \
             last:rounded-r-md focus-visible:z-10"
        }
        (ButtonGroupOrientation::Horizontal, false) => {
            "rounded-l-none border-l-0 rounded-r-none first:rounded-l-md last:rounded-r-md \
             focus-visible:z-10"
        }
        (ButtonGroupOrientation::Vertical, true) => {
            "rounded-t-none border-t-0 rounded-b-none first:rounded-t-md first:border-t \
             last:rounded-b-md focus-visible:z-10"
        }
        (ButtonGroupOrientation::Vertical, false) => {
            "rounded-t-none border-t-0 rounded-b-none first:rounded-t-md last:rounded-b-md \
             focus-visible:z-10"
        }
    }
}

/// What a [`ButtonGroup`] inside another tells it.
#[derive(Clone, Copy)]
struct GroupContext {
    holds_group: Signal<bool>,
    orientation: Prop<ButtonGroupOrientation>,
}

/// The props of [`ButtonGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ButtonGroupProps {
    /// `horizontal` or `vertical`.
    #[prop(default)]
    pub orientation: Prop<ButtonGroupOrientation>,
    /// What screen readers call the group.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// The buttons, inputs, texts and separators.
    pub children: Prop<Children>,
}

/// Every class a group draws with, before the app's.
pub(crate) fn group_classes(
    orientation: ButtonGroupOrientation,
    holds_group: bool,
) -> SharedString {
    cn([
        ROOT,
        if orientation == ButtonGroupOrientation::Vertical {
            "flex-col"
        } else {
            ""
        },
        if holds_group { NESTED } else { "" },
    ])
}

/// shadcn's ButtonGroup: buttons joined into one bar.
#[component]
pub fn ButtonGroup(props: ButtonGroupProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    if let Some(outer) = use_context::<GroupContext>() {
        outer.holds_group.set(true);
    }
    let holds_group = signal(false);
    provide_context(GroupContext {
        holds_group,
        orientation: props.orientation,
    });
    view()
        .role(AccessibilityRole::Group)
        .optional_label(props.accessibility_label)
        .app_class(move || Some(group_classes(props.orientation.get(), holds_group.get())))
        .child(props.children)
}

/// An element a [`ButtonGroup`] joins with the others: what the group's
/// selectors would see of it.
pub trait ButtonGroupItem: Element {
    /// It draws a border, which the group takes off between items.
    fn bordered(&mut self) -> bool;
    /// Classes the group's selectors give this kind of element
    /// (`[&>input]:flex-1`).
    fn group_class(&self) -> &'static str {
        ""
    }
}

impl ButtonGroupItem for ButtonElement {
    fn bordered(&mut self) -> bool {
        self.__element().__props_mut().variant.get_untracked() == ButtonVariant::Outline
    }
}

impl ButtonGroupItem for super::InputElement {
    fn bordered(&mut self) -> bool {
        true
    }
    fn group_class(&self) -> &'static str {
        "flex-1"
    }
}

impl ButtonGroupItem for ButtonGroupElement {
    fn bordered(&mut self) -> bool {
        false
    }
}

impl ButtonGroupItem for ButtonGroupTextElement {
    fn bordered(&mut self) -> bool {
        true
    }
}

impl ButtonGroupItem for ButtonGroupSeparatorElement {
    fn bordered(&mut self) -> bool {
        false
    }
}

impl ButtonGroupElement {
    /// Adds `item` joined to the others: its app classes are the ones the
    /// group's selectors give it (an item's own classes:
    /// [`Self::item_with`]). The group's `orientation` is set first.
    pub fn item<E: ButtonGroupItem>(self, item: E) -> Self {
        self.item_with(item, "")
    }

    /// Adds `item` joined to the others, with `classes` of its own after
    /// the group's.
    pub fn item_with<E: ButtonGroupItem>(mut self, mut item: E, classes: &str) -> Self {
        let orientation = self.0.__props_mut().orientation.get_untracked();
        let bordered = item.bordered();
        let all = cn([
            item_classes(orientation, bordered),
            item.group_class(),
            classes,
        ]);
        self.child(item.app_class(Some(all)))
    }

    /// `.orientation(ButtonGroupOrientation::Vertical)`.
    pub fn vertical(self) -> Self {
        self.orientation(ButtonGroupOrientation::Vertical)
    }
}

/// shadcn's ButtonGroupText classes.
const TEXT: &str = "flex items-center gap-2 rounded-md border border-border bg-muted px-4 text-sm \
    font-medium shadow-xs";

/// The props of [`ButtonGroupText`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ButtonGroupTextProps {
    /// A Lucide icon before the text.
    pub icon: Prop<Option<IconName>>,
    /// The text.
    pub label: Prop<Option<SharedString>>,
    /// Content after the text.
    pub children: Prop<Children>,
}

/// shadcn's ButtonGroupText: a text (a label, a unit) joined to the
/// group's buttons.
#[component]
pub fn ButtonGroupText(props: ButtonGroupTextProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .app_class(Some(SharedString::from(TEXT)))
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(Some(SharedString::from("size-4")))
                    .into_element()
            })
        }))
        .child(dynamic(move || {
            props.label.get().map(|label| text(label).into_element())
        }))
        .child(props.children)
}

/// `separator.tsx`'s classes, then ButtonGroupSeparator's, by orientation.
const SEPARATOR_VERTICAL: &str =
    "shrink-0 bg-border h-full w-px relative m-0! self-stretch bg-input h-auto";
const SEPARATOR_HORIZONTAL: &str =
    "shrink-0 bg-border h-px w-full relative m-0! self-stretch bg-input";

/// The props of [`ButtonGroupSeparator`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ButtonGroupSeparatorProps {}

/// shadcn's ButtonGroupSeparator: a line between two buttons (that have
/// no border of their own).
#[component]
pub fn ButtonGroupSeparator(
    _props: ButtonGroupSeparatorProps,
    _cx: Scope<NoEvent>,
) -> impl IntoElement {
    // shadcn's default `orientation="vertical"` is the line across a
    // horizontal group: the group's other way.
    let vertical_group = use_context::<GroupContext>()
        .is_some_and(|g| g.orientation.get_untracked() == ButtonGroupOrientation::Vertical);
    view()
        .accessibility_elements_hidden(true)
        .app_class(Some(SharedString::from(if vertical_group {
            SEPARATOR_HORIZONTAL
        } else {
            SEPARATOR_VERTICAL
        })))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<ButtonGroup>();
    registry.register::<ButtonGroupText>();
    registry.register::<ButtonGroupSeparator>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for orientation in [
            ButtonGroupOrientation::Horizontal,
            ButtonGroupOrientation::Vertical,
        ] {
            for bordered in [false, true] {
                for nested in [false, true] {
                    let all = format!(
                        "{} {} flex-1",
                        group_classes(orientation, nested),
                        item_classes(orientation, bordered)
                    );
                    unknown.extend(crate::cn::unknown_classes(&all));
                }
            }
        }
        let all = format!("{TEXT} size-4 {SEPARATOR_VERTICAL} {SEPARATOR_HORIZONTAL}");
        unknown.extend(crate::cn::unknown_classes(&all));
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
