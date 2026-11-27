//! [`Collapsible`], [`CollapsibleTrigger`] and [`CollapsibleContent`], after
//! shadcn/ui's `collapsible.tsx` (new-york-v4), on `nsh_base`'s
//! `Collapsible` (the content in the tree only while open).
//!
//! shadcn's parts have no classes of their own; the app's classes style
//! them. Two adaptations:
//!
//! - `CollapsibleTrigger asChild` around a `Button` is the trigger with a
//!   button's look: its `variant`, `size`, `icon` and `label` are
//!   `Button`'s (`button.tsx`'s classes); without a `variant` it has no
//!   look of its own. NSH gives a press to the innermost view that takes
//!   presses, so a trigger cannot wrap a button that presses itself.
//! - Radix hides the closed content (`hidden`); here it is out of the
//!   layout while closed, so a `gap` around it leaves no room.
//! - `nsh_base`'s `Collapsible` places the content in a view of its own:
//!   shadcn's `className` of `CollapsibleContent`, which lays the content
//!   out, is the `content_class` prop (the element's own classes style
//!   the region around it).

use std::rc::Rc;

use nsh_core::AccessibilityState;
use nsh_core::prelude::*;

use crate::icon::{IconName, icon};
use crate::ui::button::{ButtonSize, ButtonVariant, button_classes};

/// What a collapsible's parts find of it.
#[derive(Clone)]
struct CollapsibleContext {
    open: Memo<bool>,
    disabled: Prop<bool>,
    toggle: Rc<dyn Fn()>,
}

/// The props of [`Collapsible`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CollapsibleProps {
    /// Open, held by the app; without it the collapsible keeps its own,
    /// from `defaultOpen`.
    pub open: Prop<Option<bool>>,
    /// Open at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// The trigger takes no presses.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The [`CollapsibleTrigger`], the [`CollapsibleContent`] and what
    /// shows either way.
    pub children: Prop<Children>,
}

/// The events of [`Collapsible`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum CollapsibleEvent {
    /// The trigger was pressed: whether it is open now.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's Collapsible: content its trigger shows and hides.
#[component]
pub fn Collapsible(props: CollapsibleProps, cx: Scope<CollapsibleEvent>) -> impl IntoElement {
    let own = signal(props.default_open.get_untracked());
    let open = computed(move || props.open.get().unwrap_or_else(|| own.get()));
    provide_context(CollapsibleContext {
        open,
        disabled: props.disabled,
        toggle: Rc::new(move || {
            let next = !open.get_untracked();
            own.set(next);
            cx.emit(CollapsibleEvent::OpenChange(next));
        }),
    });
    view().child(props.children)
}

/// The props of [`CollapsibleTrigger`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CollapsibleTriggerProps {
    /// A button's look (`Button`'s `variant`); none: the app's classes.
    pub variant: Prop<Option<ButtonVariant>>,
    /// The button's size, with a `variant`.
    #[prop(default)]
    pub size: Prop<ButtonSize>,
    /// A Lucide icon before the label.
    pub icon: Prop<Option<IconName>>,
    /// Its text.
    pub label: Prop<Option<SharedString>>,
    /// What screen readers say, when there is no label (shadcn's
    /// `sr-only` text).
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Content after the label.
    pub children: Prop<Children>,
}

/// shadcn's CollapsibleTrigger: a press opens or closes its collapsible.
#[component]
pub fn CollapsibleTrigger(props: CollapsibleTriggerProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let ctx = use_context::<CollapsibleContext>();
    let open = ctx.as_ref().map(|c| c.open);
    let disabled = ctx.as_ref().map(|c| c.disabled);
    let toggle = ctx.map(|c| c.toggle);
    let label = props.label;
    let disabled = move || disabled.is_some_and(|d| d.get());
    view()
        .role(AccessibilityRole::Button)
        .focusable(true)
        .cursor_pointer()
        .disabled(disabled)
        .accessibility_state(move || AccessibilityState {
            expanded: Some(open.is_some_and(|o| o.get())),
            ..AccessibilityState::default()
        })
        .label(move || {
            props
                .accessibility_label
                .get()
                .or_else(|| label.get())
                .unwrap_or_default()
        })
        .on_press(move || {
            if let Some(toggle) = &toggle {
                toggle();
            }
        })
        .app_class(move || {
            props.variant.get().map(|variant| {
                button_classes(variant, props.size.get(), props.icon.get().is_some())
            })
        })
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                let size = match props.size.get() {
                    ButtonSize::Xs | ButtonSize::IconXs => "size-3",
                    _ => "size-4",
                };
                icon(name).app_class(Some(SharedString::from(size))).into_element()
            })
        }))
        .child(dynamic(move || label.get().map(|label| text(label).into_element())))
        .child(props.children)
}

/// The props of [`CollapsibleContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CollapsibleContentProps {
    /// The classes of the view the content is laid out in (shadcn's
    /// `className` of `CollapsibleContent`).
    pub content_class: Prop<Option<SharedString>>,
    /// What shows while open.
    pub children: Prop<Children>,
}

/// shadcn's CollapsibleContent: in the tree while its collapsible is open.
#[component]
pub fn CollapsibleContent(props: CollapsibleContentProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let open = use_context::<CollapsibleContext>().map(|c| c.open);
    let open = move || open.is_some_and(|o| o.get());
    nsh_base::collapsible()
        .open(open)
        .style_when(move || !open(), |s| s.hidden())
        .content(
            view()
                .app_class(props.content_class)
                .child(props.children),
        )
}

pub(crate) fn register(registry: &mut Registry) {
    registry
        .register::<Collapsible>()
        .register::<CollapsibleTrigger>()
        .register::<CollapsibleContent>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The trigger draws with a button's classes and its icon's size.
    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for variant in [ButtonVariant::Ghost, ButtonVariant::Outline, ButtonVariant::Default] {
            for size in [ButtonSize::Default, ButtonSize::Sm, ButtonSize::Icon, ButtonSize::IconSm] {
                let classes = format!("{} size-3 size-4", button_classes(variant, size, true));
                unknown.extend(crate::cn::unknown_classes(&classes));
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
