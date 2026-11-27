//! [`Alert`] and its parts, after shadcn/ui's `alert.tsx` (new-york-v4).
//!
//! The classes are shadcn's `alertVariants` and parts, word for word,
//! except:
//!
//! - `border` is `border border-border` (shadcn's global
//!   `* { @apply border-border }`).
//! - `has-[>svg]:grid-cols-[calc(var(--spacing)*4)_1fr] has-[>svg]:gap-x-3`
//!   and `[&>svg]:size-4 [&>svg]:translate-y-0.5 [&>svg]:text-current`
//!   style an icon in the alert; NSH has no `:has()` or child selectors, so
//!   the `icon` prop places the icon, which takes those classes, and the
//!   grid takes `grid-cols-[1rem_1fr] gap-x-3` (NSH has no `calc()`;
//!   `--spacing` × 4 is 1rem).
//! - The destructive variant's `*:data-[slot=alert-description]:text-destructive/90`
//!   colors the description; the alert tells its [`AlertDescription`] its
//!   variant (a context), which takes `text-destructive/90`.
//! - `AlertDescription`'s `[&_p]:leading-relaxed` styles the paragraphs
//!   in it: a paragraph the app puts in it (a child) takes
//!   `leading-relaxed` itself. Its `description` is its own text, as
//!   shadcn's text written straight in it, at its line height.

use nsh_core::prelude::*;

use super::cls;
use crate::cn::cn;
use crate::icon::{IconName, icon};

const BASE: &str = "relative grid w-full grid-cols-[0_1fr] items-start gap-y-0.5 rounded-lg border \
    border-border px-4 py-3 text-sm";
/// `has-[>svg]:grid-cols-[calc(var(--spacing)*4)_1fr] has-[>svg]:gap-x-3`.
const WITH_ICON: &str = "grid-cols-[1rem_1fr] gap-x-3";
/// `[&>svg]:size-4 [&>svg]:translate-y-0.5 [&>svg]:text-current`.
const ICON: &str = "size-4 translate-y-0.5 text-current";
const TITLE: &str = "col-start-2 line-clamp-1 min-h-4 font-medium tracking-tight";
const DESCRIPTION: &str =
    "col-start-2 grid justify-items-start gap-1 text-sm text-muted-foreground";
/// `[&_p]:leading-relaxed`: what a paragraph the app puts in a description
/// takes.
#[cfg(test)]
const PARAGRAPH: &str = "leading-relaxed";
/// `*:data-[slot=alert-description]:text-destructive/90`.
const DESTRUCTIVE_DESCRIPTION: &str = "text-destructive/90";

/// How an alert looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum AlertVariant {
    /// The card's colors.
    #[default]
    Default,
    /// An error: destructive text.
    Destructive,
}

impl AlertVariant {
    fn classes(self) -> &'static str {
        match self {
            AlertVariant::Default => "bg-card text-card-foreground",
            AlertVariant::Destructive => "bg-card text-destructive",
        }
    }
}

fn alert_classes(variant: AlertVariant, has_icon: bool) -> SharedString {
    cn([
        BASE,
        if has_icon { WITH_ICON } else { "" },
        variant.classes(),
    ])
}

/// The alert's variant, for its description.
#[derive(Clone, Copy)]
struct AlertContext(Prop<AlertVariant>);

/// The props of [`Alert`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertProps {
    /// `default` or `destructive`.
    #[prop(default)]
    pub variant: Prop<AlertVariant>,
    /// A Lucide icon at the alert's start, 16 points, beside the title.
    pub icon: Prop<Option<IconName>>,
    /// The title and description.
    pub children: Prop<Children>,
}

/// shadcn's Alert: a bordered callout, an icon beside its title and
/// description. Screen readers hear it as an alert.
#[component]
pub fn Alert(props: AlertProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    provide_context(AlertContext(props.variant));
    view()
        .role(AccessibilityRole::Alert)
        .app_class(move || {
            cls(alert_classes(
                props.variant.get(),
                props.icon.get().is_some(),
            ))
        })
        .child(dynamic(move || {
            props
                .icon
                .get()
                .map(|name| icon(name).app_class(cls(ICON)).into_element())
        }))
        .child(props.children)
}

impl AlertElement {
    /// `.variant(AlertVariant::Destructive)`.
    pub fn destructive(self) -> Self {
        self.variant(AlertVariant::Destructive)
    }
}

/// The props of [`AlertTitle`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertTitleProps {
    /// The title, on one line.
    pub title: Prop<Option<SharedString>>,
    /// Content after the title.
    pub children: Prop<Children>,
}

/// shadcn's AlertTitle: one line, medium weight.
#[component]
pub fn AlertTitle(props: AlertTitleProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    text(move || props.title.get().unwrap_or_default())
        .app_class(cls(TITLE))
        .child(props.children)
}

/// The props of [`AlertDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertDescriptionProps {
    /// The description's text (none when empty).
    pub description: Prop<Option<SharedString>>,
    /// Content after it: paragraphs (`leading-relaxed`), a list.
    pub children: Prop<Children>,
}

/// shadcn's AlertDescription: muted text under the title (destructive in a
/// destructive alert).
#[component]
pub fn AlertDescription(props: AlertDescriptionProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let variant = use_context::<AlertContext>().map(|c| c.0);
    let description = props.description;
    view()
        .app_class(move || {
            cls(match variant.map(|v| v.get()) {
                Some(AlertVariant::Destructive) => cn([DESCRIPTION, DESTRUCTIVE_DESCRIPTION]),
                _ => SharedString::from(DESCRIPTION),
            })
        })
        .child(dynamic(move || {
            description
                .get()
                .filter(|d| !d.is_empty())
                .map(|d| text(d).into_element())
        }))
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Alert>();
    registry.register::<AlertTitle>();
    registry.register::<AlertDescription>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = Vec::new();
        for variant in [AlertVariant::Default, AlertVariant::Destructive] {
            for has_icon in [false, true] {
                all.push(alert_classes(variant, has_icon));
            }
        }
        super::super::assert_known(all.iter().map(|s| s.as_ref()).chain([
            ICON,
            TITLE,
            DESCRIPTION,
            PARAGRAPH,
            DESTRUCTIVE_DESCRIPTION,
        ]));
    }
}
