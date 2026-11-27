//! [`Badge`], after shadcn/ui's `badge.tsx` (new-york-v4).
//!
//! The classes are shadcn's `badgeVariants`, word for word, except:
//!
//! - `[&>svg]:pointer-events-none [&>svg]:size-3` size an icon in the
//!   badge; NSH has no child selectors, so the `icon` prop's icon takes
//!   `size-3`.
//! - `transition-[color,box-shadow]` is `transition`: NSH has no
//!   arbitrary transition properties; `transition` animates colors and
//!   shadows (the focus ring), as shadcn's list does.
//! - The `[a&]:hover:…` classes style a badge that is a link (`asChild`
//!   with an `<a>`); a badge here is never one, so they are left out.
//! - `aria-invalid:` is the `invalid` prop's.

use nsh_core::prelude::*;

use super::cls;
use crate::cn::cn;
use crate::icon::{IconName, icon};

/// shadcn's base classes of every badge.
const BASE: &str = "inline-flex w-fit shrink-0 items-center justify-center gap-1 overflow-hidden \
    rounded-full border border-transparent px-2 py-0.5 text-xs font-medium whitespace-nowrap \
    transition focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50";

/// `aria-invalid:border-destructive aria-invalid:ring-destructive/20
/// dark:aria-invalid:ring-destructive/40`.
const INVALID: &str = "border-destructive ring-destructive/20 dark:ring-destructive/40";

/// shadcn's `[&>svg]:size-3`.
const ICON: &str = "pointer-events-none size-3";

/// How a badge looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum BadgeVariant {
    /// The primary fill.
    #[default]
    Default,
    /// A quieter fill.
    Secondary,
    /// An error, a destructive state.
    Destructive,
    /// A border, no fill.
    Outline,
    /// No border, no fill.
    Ghost,
    /// Looks like a link.
    Link,
}

impl BadgeVariant {
    #[cfg(test)]
    const ALL: [BadgeVariant; 6] = [
        BadgeVariant::Default,
        BadgeVariant::Secondary,
        BadgeVariant::Destructive,
        BadgeVariant::Outline,
        BadgeVariant::Ghost,
        BadgeVariant::Link,
    ];

    fn classes(self) -> &'static str {
        match self {
            BadgeVariant::Default => "bg-primary text-primary-foreground",
            BadgeVariant::Secondary => "bg-secondary text-secondary-foreground",
            BadgeVariant::Destructive => {
                "bg-destructive text-white focus-visible:ring-destructive/20 dark:bg-destructive/60 \
                 dark:focus-visible:ring-destructive/40"
            }
            BadgeVariant::Outline => "border-border text-foreground",
            BadgeVariant::Ghost => "",
            BadgeVariant::Link => "text-primary underline-offset-4",
        }
    }
}

fn badge_classes(variant: BadgeVariant, invalid: bool) -> SharedString {
    cn([BASE, variant.classes(), if invalid { INVALID } else { "" }])
}

/// The props of [`Badge`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct BadgeProps {
    /// The text of the badge.
    pub label: Prop<Option<SharedString>>,
    /// `default`, `secondary`, `destructive`, `outline`, `ghost` or `link`.
    #[prop(default)]
    pub variant: Prop<BadgeVariant>,
    /// A Lucide icon before the label, 12 points.
    pub icon: Prop<Option<IconName>>,
    /// Marks the badge as invalid (shadcn's `aria-invalid`): a destructive
    /// border.
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// Content after the label (a spinner, an icon of the app's sized
    /// `size-3`).
    pub children: Prop<Children>,
}

/// shadcn's Badge: `<Badge label="New" variant="secondary" />`.
#[component]
pub fn Badge(props: BadgeProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let label = props.label;
    view()
        .app_class(move || cls(badge_classes(props.variant.get(), props.invalid.get())))
        .child(dynamic(move || {
            props
                .icon
                .get()
                .map(|name| icon(name).app_class(cls(ICON)).into_element())
        }))
        .child(dynamic(move || {
            label.get().map(|label| text(label).into_element())
        }))
        .child(props.children)
}

impl BadgeElement {
    /// `.variant(BadgeVariant::Secondary)`.
    pub fn secondary(self) -> Self {
        self.variant(BadgeVariant::Secondary)
    }
    /// `.variant(BadgeVariant::Destructive)`.
    pub fn destructive(self) -> Self {
        self.variant(BadgeVariant::Destructive)
    }
    /// `.variant(BadgeVariant::Outline)`.
    pub fn outline(self) -> Self {
        self.variant(BadgeVariant::Outline)
    }
    /// `.variant(BadgeVariant::Ghost)`.
    pub fn ghost(self) -> Self {
        self.variant(BadgeVariant::Ghost)
    }
    /// `.variant(BadgeVariant::Link)`.
    pub fn link(self) -> Self {
        self.variant(BadgeVariant::Link)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Badge>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let all: Vec<SharedString> = BadgeVariant::ALL
            .iter()
            .flat_map(|&v| [badge_classes(v, false), badge_classes(v, true)])
            .collect();
        super::super::assert_known(all.iter().map(|s| s.as_ref()).chain([ICON]));
    }
}
