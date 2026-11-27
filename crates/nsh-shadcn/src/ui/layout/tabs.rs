//! [`Tabs`], [`TabsList`], [`TabsTrigger`] and [`TabsContent`], after
//! shadcn/ui's `tabs.tsx` (new-york-v4), on `nsh_base`'s `Tabs` and `Tab`
//! (the list's keyboard, the tab role and its selected state).
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class:
//!
//! - `group-data-[orientation=…]/tabs:` and `group-data-[variant=…]/tabs-list:`
//!   read the root's orientation and the list's variant; the component
//!   knows both, so it picks the classes of the one it has.
//! - `data-[state=active]:` is `selected:` (the tab's selected state).
//! - `after:` draws the line variant's underline; NSH has no
//!   pseudo-elements, so the underline is a child of the trigger with the
//!   `after:` classes, shown by `group-selected:opacity-100`.
//! - In the line variant, `group-data-[variant=line]/tabs-list:data-[state=active]:bg-transparent`
//!   wins over `data-[state=active]:bg-background` in CSS by its
//!   specificity; here the line variant's classes come after the others,
//!   so they win by their place.
//! - `[&_svg]:…` sizes the `icon` prop's icon (`size-4`).
//! - `h-[calc(100%-1px)]` (NSH has no `calc()` in lengths) is `h-[29px]`
//!   in a row, the list's 36 points less its padding and the point; in a
//!   column, where the list fits its tabs, the tab's own height.
//! - `flex-1` is `flex-auto`: in a list that fits its tabs (`w-fit`) a
//!   browser resolves `flex-1`'s `0%` basis as the tab's content, so each
//!   tab keeps its width; NSH's `flex-1` would make them equal.
//! - `focus-visible:outline-1 focus-visible:outline-ring`: NSH draws an
//!   outline as CSS does.

use std::rc::Rc;

use nsh_core::geometry::Axis;
use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};

/// The direction a tab list runs: shadcn's (Radix's) `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum TabsOrientation {
    /// The list in a row over the content.
    #[default]
    Horizontal,
    /// The list in a column beside the content.
    Vertical,
}

/// How a tab list looks: shadcn's `variant` of `TabsList`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum TabsListVariant {
    /// A muted track, the selected tab raised on it.
    #[default]
    Default,
    /// No track; the selected tab underlined.
    Line,
}

/// What a tab list's parts find of their [`Tabs`].
#[derive(Clone)]
struct TabsContext {
    value: Memo<SharedString>,
    select: Rc<dyn Fn(SharedString)>,
    orientation: TabsOrientation,
}

/// What a [`TabsTrigger`] finds of its [`TabsList`].
#[derive(Clone, Copy)]
struct ListContext(TabsListVariant);

/// shadcn's `Tabs` classes.
pub(crate) fn tabs_classes(orientation: TabsOrientation) -> &'static str {
    match orientation {
        TabsOrientation::Horizontal => "flex gap-2 flex-col",
        TabsOrientation::Vertical => "flex gap-2",
    }
}

/// shadcn's `tabsListVariants`.
pub(crate) fn list_classes(variant: TabsListVariant, orientation: TabsOrientation) -> SharedString {
    cn([
        "inline-flex w-fit items-center justify-center rounded-lg p-[3px] text-muted-foreground",
        match orientation {
            TabsOrientation::Horizontal => "h-9",
            TabsOrientation::Vertical => "h-fit flex-col",
        },
        match variant {
            TabsListVariant::Default => "bg-muted",
            TabsListVariant::Line => "rounded-none gap-1 bg-transparent",
        },
    ])
}

/// shadcn's `TabsTrigger` classes.
pub(crate) fn trigger_classes(variant: TabsListVariant, orientation: TabsOrientation) -> SharedString {
    cn([
        "relative inline-flex flex-auto items-center justify-center gap-1.5 \
         rounded-md border border-transparent px-2 py-1 text-sm font-medium whitespace-nowrap \
         text-foreground/60 transition-all hover:text-foreground focus-visible:border-ring \
         focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 \
         focus-visible:outline-ring disabled:pointer-events-none disabled:opacity-50 \
         dark:text-muted-foreground dark:hover:text-foreground",
        match orientation {
            TabsOrientation::Horizontal => "h-[29px]",
            TabsOrientation::Vertical => "w-full justify-start",
        },
        "selected:bg-background selected:text-foreground dark:selected:border-input \
         dark:selected:bg-input/30 dark:selected:text-foreground",
        match variant {
            TabsListVariant::Default => "selected:shadow-sm",
            TabsListVariant::Line => {
                "selected:shadow-none bg-transparent selected:bg-transparent \
                 dark:selected:border-transparent dark:selected:bg-transparent"
            }
        },
        // `group` for the underline's `group-selected:`.
        "group",
    ])
}

/// The line variant's underline: shadcn's `after:` classes.
pub(crate) fn underline_classes(orientation: TabsOrientation) -> &'static str {
    match orientation {
        TabsOrientation::Horizontal => {
            "absolute bg-foreground opacity-0 transition-opacity inset-x-0 bottom-[-5px] h-0.5 \
             group-selected:opacity-100"
        }
        TabsOrientation::Vertical => {
            "absolute bg-foreground opacity-0 transition-opacity inset-y-0 -right-1 w-0.5 \
             group-selected:opacity-100"
        }
    }
}

/// The props of [`Tabs`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TabsProps {
    /// The selected tab's `value`, held by the app; without it the tabs
    /// keep their own, from `defaultValue`.
    pub value: Prop<Option<SharedString>>,
    /// The tab selected at first, while `value` is not set.
    #[prop(default)]
    pub default_value: Prop<SharedString>,
    /// `horizontal` (the list over the content) or `vertical` (beside it).
    #[prop(default)]
    pub orientation: Prop<TabsOrientation>,
    /// The [`TabsList`] and the [`TabsContent`]s.
    pub children: Prop<Children>,
}

/// The events of [`Tabs`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum TabsEvent {
    /// A tab was selected: its `value`.
    #[args(value)]
    ValueChange(SharedString),
}

/// shadcn's Tabs: a [`TabsList`] of [`TabsTrigger`]s and the
/// [`TabsContent`] of the selected one.
#[component]
pub fn Tabs(props: TabsProps, cx: Scope<TabsEvent>) -> impl IntoElement {
    let own = signal(props.default_value.get_untracked());
    let value = computed(move || props.value.get().unwrap_or_else(|| own.get()));
    let orientation = props.orientation.get_untracked();
    provide_context(TabsContext {
        value,
        select: Rc::new(move |next: SharedString| {
            own.set(next.clone());
            cx.emit(TabsEvent::ValueChange(next));
        }),
        orientation,
    });
    view()
        .app_class(Some(SharedString::from(tabs_classes(orientation))))
        .child(props.children)
}

/// The props of [`TabsList`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TabsListProps {
    /// `default` (a muted track) or `line` (an underline).
    #[prop(default)]
    pub variant: Prop<TabsListVariant>,
    /// What screen readers call the list.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// The [`TabsTrigger`]s.
    pub children: Prop<Children>,
}

/// shadcn's TabsList: the row (or column) of tabs; its arrow keys move
/// between them (`nsh_base::Tabs`).
#[component]
pub fn TabsList(props: TabsListProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let orientation = use_context::<TabsContext>().map_or(TabsOrientation::Horizontal, |c| c.orientation);
    let variant = props.variant.get_untracked();
    provide_context(ListContext(variant));
    nsh_base::tabs()
        .orientation(Some(match orientation {
            TabsOrientation::Horizontal => Axis::Horizontal,
            TabsOrientation::Vertical => Axis::Vertical,
        }))
        .accessibility_label(props.accessibility_label)
        .app_class(move || Some(list_classes(props.variant.get(), orientation)))
        .child(props.children)
}

/// The props of [`TabsTrigger`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TabsTriggerProps {
    /// The tab's value: the [`TabsContent`] of the same value shows while
    /// it is selected.
    #[prop(default)]
    pub value: Prop<SharedString>,
    /// Its text.
    pub label: Prop<Option<SharedString>>,
    /// A Lucide icon before the text.
    pub icon: Prop<Option<IconName>>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// What screen readers say, when there is no label.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Content after the label.
    pub children: Prop<Children>,
}

/// shadcn's TabsTrigger: a tab of the list; a press selects it.
#[component]
pub fn TabsTrigger(props: TabsTriggerProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let tabs = use_context::<TabsContext>();
    let orientation = tabs.as_ref().map_or(TabsOrientation::Horizontal, |c| c.orientation);
    let variant = use_context::<ListContext>().map_or(TabsListVariant::Default, |l| l.0);
    let selected = {
        let tabs = tabs.clone();
        computed(move || tabs.as_ref().is_some_and(|t| t.value.get() == props.value.get()))
    };
    let label = props.label;
    nsh_base::tab()
        .selected(move || selected.get())
        .disabled(props.disabled)
        .accessibility_label(move || props.accessibility_label.get().or_else(|| label.get()))
        .on_press(move || {
            if let Some(tabs) = &tabs {
                (tabs.select)(props.value.get_untracked());
            }
        })
        .app_class(Some(trigger_classes(variant, orientation)))
        .child(dynamic(move || props.icon.get().map(|name| icon(name).into_element())))
        .child(dynamic(move || label.get().map(|label| text(label).into_element())))
        .child(props.children)
        .child(match variant {
            TabsListVariant::Line => view()
                .pointer_events(PointerEvents::None)
                .app_class(Some(SharedString::from(underline_classes(orientation))))
                .into_element(),
            TabsListVariant::Default => view().hidden().into_element(),
        })
}

/// The props of [`TabsContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct TabsContentProps {
    /// The value of the tab it belongs to.
    #[prop(default)]
    pub value: Prop<SharedString>,
    /// The content.
    pub children: Prop<Children>,
}

/// shadcn's TabsContent: shown while its tab is selected.
#[component]
pub fn TabsContent(props: TabsContentProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let tabs = use_context::<TabsContext>();
    let shown = computed(move || tabs.as_ref().is_some_and(|t| t.value.get() == props.value.get()));
    view()
        .app_class(Some(SharedString::from("flex-1 outline-none")))
        .style_when(move || !shown.get(), |s| s.hidden())
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry
        .register::<Tabs>()
        .register::<TabsList>()
        .register::<TabsTrigger>()
        .register::<TabsContent>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = Vec::new();
        for orientation in [TabsOrientation::Horizontal, TabsOrientation::Vertical] {
            all.push(SharedString::from(tabs_classes(orientation)));
            all.push(SharedString::from(underline_classes(orientation)));
            for variant in [TabsListVariant::Default, TabsListVariant::Line] {
                all.push(list_classes(variant, orientation));
                all.push(trigger_classes(variant, orientation));
            }
        }
        all.push("flex-1 outline-none".into());
        let unknown: Vec<_> = all.iter().flat_map(|c| crate::cn::unknown_classes(c)).collect();
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
