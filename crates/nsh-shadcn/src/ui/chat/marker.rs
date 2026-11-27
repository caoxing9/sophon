//! [`Marker`], [`MarkerIcon`] and [`MarkerContent`], after shadcn/ui's
//! `marker.tsx` (new-york-v4): a line in a conversation that is not a
//! message ("Switched to a new branch", "Conversation compacted", "Oliver
//! is typing...").
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class:
//!
//! - The `separator` variant draws its two rules with `before:` and
//!   `after:`; NSH has no pseudo-elements, so they are two views of the
//!   marker, before and after its content, with the same classes
//!   (`mr-1 h-px min-w-0 flex-1 bg-border`, `ml-1 …`).
//! - `[&_svg:not([class*='size-'])]:size-4` is the size of the
//!   [`MarkerIcon`]'s `icon`.
//! - shadcn's `asChild` marker (a `<button>` or an `<a>`) is the marker's
//!   `pressable` and `link` props; `[a]:…` classes apply to a `link`.
//!   `*:[a]:…` (links inside the content) are left to the app's spans.
//! - `group-data-[variant=separator]/marker:…` is
//!   `group-data-variant-separator/marker:…` (NSH has the boolean `data-*`
//!   form only); `wrap-break-word` is left out (NSH has no such class).
//! - The `shimmer` utility shadcn's examples put on the content (an
//!   animated gradient clipped to the text) has no NSH equivalent: NSH
//!   clips no background to text.

use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};

/// How a marker looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum MarkerVariant {
    /// A line of muted text.
    #[default]
    Default,
    /// Centered between two rules.
    Separator,
    /// Over a rule.
    Border,
}

/// shadcn's `markerVariants` base classes.
const MARKER: &str = "group/marker relative flex min-h-4 w-full items-center gap-2 text-left text-sm \
    text-muted-foreground";

/// shadcn's `[a]:…` classes, of a marker that is a link.
const LINK: &str = "underline underline-offset-3 hover:text-foreground";

/// The rule before the content (shadcn's `before:…`).
const RULE_BEFORE: &str = "mr-1 h-px min-w-0 flex-1 bg-border";

/// The rule after the content (shadcn's `after:…`).
const RULE_AFTER: &str = "ml-1 h-px min-w-0 flex-1 bg-border";

/// shadcn's `MarkerIcon` classes.
const ICON: &str = "size-4 shrink-0";

/// shadcn's `MarkerContent` classes (`wrap-break-word` left out).
const CONTENT: &str = "min-w-0 group-data-variant-separator/marker:flex-none \
    group-data-variant-separator/marker:text-center";

impl MarkerVariant {
    fn classes(self) -> &'static str {
        match self {
            MarkerVariant::Default | MarkerVariant::Separator => "",
            MarkerVariant::Border => "border-b border-border pb-2",
        }
    }
}

/// Every class a marker of `variant` draws with, before the app's.
fn marker_classes(variant: MarkerVariant, link: bool) -> SharedString {
    cn([MARKER, variant.classes(), if link { LINK } else { "" }])
}

/// The props of [`Marker`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MarkerProps {
    /// `default`, `separator` or `border`.
    #[prop(default)]
    pub variant: Prop<MarkerVariant>,
    /// Takes presses, as shadcn's `asChild` marker that is a `<button>`.
    /// Read when the marker is made.
    #[prop(default)]
    pub pressable: Prop<bool>,
    /// Takes presses and looks like a link, as shadcn's `asChild` marker
    /// that is an `<a>`. Read when the marker is made.
    #[prop(default)]
    pub link: Prop<bool>,
    /// What screen readers say for a pressable marker, in place of its
    /// texts.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// A [`MarkerIcon`] and a [`MarkerContent`].
    pub children: Prop<Children>,
}

/// The events of a [`Marker`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum MarkerEvent {
    /// A pressable or link marker was pressed.
    Press,
}

/// A line of the conversation that is not a message: an event, a status,
/// a divider.
#[component]
pub fn Marker(props: MarkerProps, cx: Scope<MarkerEvent>) -> impl IntoElement {
    let link = props.link.get_untracked();
    let pressable = link || props.pressable.get_untracked();
    let separator = move || props.variant.get() == MarkerVariant::Separator;
    let marker = view()
        .group_named("marker")
        .app_class(move || Some(marker_classes(props.variant.get(), link)))
        .project_data("variant-separator", separator)
        .child(when(separator, || view().app_class(Some(SharedString::from(RULE_BEFORE)))))
        .child(props.children)
        .child(when(separator, || view().app_class(Some(SharedString::from(RULE_AFTER)))));
    if !pressable {
        return marker;
    }
    marker
        .role(if link {
            AccessibilityRole::Link
        } else {
            AccessibilityRole::Button
        })
        .optional_label(move || props.accessibility_label.get())
        .cursor_pointer()
        .on_press(move || cx.emit(MarkerEvent::Press))
}

/// The props of [`MarkerIcon`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MarkerIconProps {
    /// A Lucide icon, 16 points.
    pub icon: Prop<Option<IconName>>,
    /// Another 16-point glyph in place of `icon`: a spinner.
    pub children: Prop<Children>,
}

/// The marker's icon, hidden from screen readers.
#[component]
pub fn MarkerIcon(props: MarkerIconProps) -> impl IntoElement {
    view()
        .app_class(Some(SharedString::from(ICON)))
        .accessibility_elements_hidden(true)
        .child(dynamic(move || {
            props
                .icon
                .get()
                .map(|name| icon(name).app_class(Some(SharedString::from("size-4"))).into_element())
        }))
        .child(props.children)
}

/// The props of [`MarkerContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MarkerContentProps {
    /// The text.
    pub text: Prop<Option<SharedString>>,
    /// Content after the text: a text with spans ("**Oliver** is
    /// typing...").
    pub children: Prop<Children>,
}

/// What the marker says; centered between the rules of a separator.
#[component]
pub fn MarkerContent(props: MarkerContentProps) -> impl IntoElement {
    view()
        .app_class(Some(SharedString::from(CONTENT)))
        .child(dynamic(move || props.text.get().map(|t| text(t).into_element())))
        .child(props.children)
}

impl MarkerElement {
    /// `.variant(MarkerVariant::Separator)`.
    pub fn separator(self) -> Self {
        self.variant(MarkerVariant::Separator)
    }
    /// `.variant(MarkerVariant::Border)`.
    pub fn border(self) -> Self {
        self.variant(MarkerVariant::Border)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Marker>();
    registry.register::<MarkerIcon>();
    registry.register::<MarkerContent>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = vec![cn([RULE_BEFORE, RULE_AFTER, ICON, CONTENT])];
        for variant in [MarkerVariant::Default, MarkerVariant::Separator, MarkerVariant::Border] {
            for link in [false, true] {
                all.push(marker_classes(variant, link));
            }
        }
        for classes in all {
            let unknown = crate::cn::unknown_classes(&classes);
            assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
        }
    }
}
