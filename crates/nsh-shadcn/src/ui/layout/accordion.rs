//! [`Accordion`], [`AccordionItem`], [`AccordionTrigger`] and
//! [`AccordionContent`], after shadcn/ui's `accordion.tsx` (new-york-v4), on
//! `nsh_base`'s `Accordion` and its parts (the open items, the trigger's
//! button and expanded state, the panel's region and reveal).
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class:
//!
//! - `border-b` is `border-b border-border` (NSH has no global border color).
//! - `[&[data-state=open]>svg]:rotate-180` is the chevron's
//!   `group-aria-expanded:rotate-180`, the trigger a `group`.
//! - `data-[state=closed]:animate-accordion-up` /
//!   `data-[state=open]:animate-accordion-down` (keyframes NSH does not
//!   have) is the panel's `reveal`: its clipped height springs open and
//!   shut.
//! - Radix's `type="single" | "multiple"` is the `multiple` prop.
//! - shadcn passes `AccordionContent`'s `className` to the inner `div`;
//!   here that is the `content_class` prop (the element's own classes
//!   style the panel).

use std::rc::Rc;

use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};

/// shadcn's `AccordionItem` classes.
pub(crate) const ITEM: &str = "border-b border-border last:border-b-0";
/// shadcn's `AccordionPrimitive.Header` classes.
pub(crate) const HEADER: &str = "flex";
/// shadcn's `AccordionTrigger` classes (and the `group` its chevron reads).
pub(crate) const TRIGGER: &str = "flex flex-1 items-start justify-between gap-4 rounded-md py-4 \
    text-left text-sm font-medium transition-all outline-none hover:underline \
    focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 \
    disabled:pointer-events-none disabled:opacity-50 group";
/// shadcn's chevron classes, turned while open.
pub(crate) const CHEVRON: &str = "pointer-events-none size-4 shrink-0 translate-y-0.5 \
    text-muted-foreground transition-transform duration-200 group-aria-expanded:rotate-180";
/// shadcn's `AccordionContent` classes: the panel's.
pub(crate) const PANEL: &str = "overflow-hidden text-sm";
/// shadcn's inner `div` of `AccordionContent`.
pub(crate) const INNER: &str = "pt-0 pb-4";

/// The props of [`Accordion`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AccordionProps {
    /// The open items' values, held by the app; without it the accordion
    /// keeps its own, from `defaultValue`.
    pub value: Prop<Option<Vec<SharedString>>>,
    /// The open items at first, while `value` is not set.
    #[prop(default)]
    pub default_value: Prop<Vec<SharedString>>,
    /// Several items open at once (Radix's `type="multiple"`); without
    /// it, opening one closes the other (`type="single"`).
    #[prop(default)]
    pub multiple: Prop<bool>,
    /// In a single accordion, the open item closes when its trigger is
    /// pressed again (Radix's `collapsible`); without it one item stays
    /// open.
    #[prop(default)]
    pub collapsible: Prop<bool>,
    /// No item opens or closes.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The [`AccordionItem`]s.
    pub children: Prop<Children>,
}

/// The events of [`Accordion`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum AccordionEvent {
    /// The open items changed: their values.
    #[args(value)]
    ValueChange(Vec<SharedString>),
}

/// shadcn's Accordion: stacked sections that open and close.
#[component]
pub fn Accordion(props: AccordionProps, cx: Scope<AccordionEvent>) -> impl IntoElement {
    let own = signal(props.default_value.get_untracked());
    let value = computed(move || props.value.get().unwrap_or_else(|| own.get()));
    let change: Rc<dyn Fn(Vec<SharedString>)> = Rc::new(move |next: Vec<SharedString>| {
        // A single accordion that is not collapsible keeps its open item.
        if next.is_empty() && !props.multiple.get_untracked() && !props.collapsible.get_untracked() {
            return;
        }
        own.set(next.clone());
        cx.emit(AccordionEvent::ValueChange(next));
    });
    nsh_base::accordion()
        .value(move || value.get())
        .multiple(props.multiple)
        .disabled(props.disabled)
        .on_change(move |next: &Vec<SharedString>| change(next.clone()))
        .child(props.children)
}

/// The props of [`AccordionItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AccordionItemProps {
    /// What identifies the item in the accordion's value.
    #[prop(default)]
    pub value: Prop<SharedString>,
    /// Its trigger takes no presses.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// Its [`AccordionTrigger`] and [`AccordionContent`].
    pub children: Prop<Children>,
}

/// shadcn's AccordionItem: a section, a line under it.
#[component]
pub fn AccordionItem(props: AccordionItemProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::accordion_item()
        .value(props.value)
        .disabled(props.disabled)
        .app_class(Some(SharedString::from(ITEM)))
        .child(props.children)
}

/// The props of [`AccordionTrigger`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AccordionTriggerProps {
    /// The section's title.
    pub label: Prop<Option<SharedString>>,
    /// Content after the title, before the chevron.
    pub children: Prop<Children>,
}

/// shadcn's AccordionTrigger: the section's heading, a chevron that turns
/// as it opens.
#[component]
pub fn AccordionTrigger(props: AccordionTriggerProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let label = props.label;
    nsh_base::accordion_header()
        .app_class(Some(SharedString::from(HEADER)))
        .child(
            nsh_base::accordion_trigger()
                .accessibility_label(move || label.get())
                .app_class(Some(SharedString::from(TRIGGER)))
                .child(dynamic(move || label.get().map(|label| text(label).into_element())))
                .child(props.children)
                .child(icon(IconName::ChevronDown).app_class(Some(SharedString::from(CHEVRON)))),
        )
}

/// The props of [`AccordionContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AccordionContentProps {
    /// The classes of the content's `div` (shadcn's `className` of
    /// `AccordionContent`), after its `pt-0 pb-4`.
    pub content_class: Prop<Option<SharedString>>,
    /// The content.
    pub children: Prop<Children>,
}

/// shadcn's AccordionContent: shown while its item is open.
#[component]
pub fn AccordionContent(props: AccordionContentProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let class = props.content_class;
    nsh_base::accordion_panel()
        .reveal(true)
        .app_class(Some(SharedString::from(PANEL)))
        .child(
            view()
                .app_class(move || Some(cn([INNER, class.get().as_deref().unwrap_or("")])))
                .child(props.children),
        )
}

pub(crate) fn register(registry: &mut Registry) {
    registry
        .register::<Accordion>()
        .register::<AccordionItem>()
        .register::<AccordionTrigger>()
        .register::<AccordionContent>();
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_class_is_known_to_nsh() {
        let unknown: Vec<_> = [super::ITEM, super::HEADER, super::TRIGGER, super::CHEVRON, super::PANEL, super::INNER]
            .iter()
            .flat_map(|c| crate::cn::unknown_classes(c))
            .collect();
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
