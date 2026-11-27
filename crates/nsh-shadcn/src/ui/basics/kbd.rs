//! [`Kbd`] and [`KbdGroup`], after shadcn/ui's `kbd.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `[&_svg:not([class*='size-'])]:size-3` sizes an icon in the key; NSH
//!   has no descendant selectors, so the `icon` prop's icon takes `size-3`.
//! - `[[data-slot=tooltip-content]_&]:bg-background/20
//!   [[data-slot=tooltip-content]_&]:text-background
//!   dark:[[data-slot=tooltip-content]_&]:bg-background/10` restyle a key
//!   inside a tooltip; NSH cannot select on an ancestor, so a key in a
//!   tooltip takes them as the app's classes.
//! - `KbdGroup` is a `<kbd>` in shadcn too; here it is a row the screen
//!   reader reads key by key.

use nsh_core::prelude::*;

use super::cls;
use crate::icon::{IconName, icon};

const KBD: &str = "pointer-events-none inline-flex h-5 w-fit min-w-5 items-center justify-center \
    gap-1 rounded-sm bg-muted px-1 font-sans text-xs font-medium text-muted-foreground select-none";
/// `[&_svg:not([class*='size-'])]:size-3`.
const ICON: &str = "size-3";
/// What a key in a tooltip adds.
#[cfg(test)]
const IN_TOOLTIP: &str = "bg-background/20 text-background dark:bg-background/10";
const GROUP: &str = "inline-flex items-center gap-1";

/// The props of [`Kbd`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct KbdProps {
    /// The key: `⌘`, `Ctrl`, `B`.
    pub key: Prop<Option<SharedString>>,
    /// A Lucide icon before the key's text, 12 points.
    pub icon: Prop<Option<IconName>>,
    /// What screen readers say, over the key's text (`"Command"` for `⌘`).
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Content after the key's text.
    pub children: Prop<Children>,
}

/// shadcn's Kbd: a key, as a shortcut names it.
#[component]
pub fn Kbd(props: KbdProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let key = props.key;
    view()
        .accessible(true)
        .role(AccessibilityRole::KeyboardKey)
        .label(move || {
            props
                .accessibility_label
                .get()
                .or_else(|| key.get())
                .unwrap_or_default()
        })
        .app_class(cls(KBD))
        .child(dynamic(move || {
            props
                .icon
                .get()
                .map(|name| icon(name).app_class(cls(ICON)).into_element())
        }))
        .child(dynamic(move || {
            key.get().map(|key| text(key).into_element())
        }))
        .child(props.children)
}

/// The props of [`KbdGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct KbdGroupProps {
    /// The keys, and what goes between them (`+`).
    pub children: Prop<Children>,
}

/// shadcn's KbdGroup: keys side by side, a shortcut.
#[component]
pub fn KbdGroup(props: KbdGroupProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(GROUP)).child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Kbd>();
    registry.register::<KbdGroup>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::assert_known([KBD, ICON, IN_TOOLTIP, GROUP]);
    }
}
