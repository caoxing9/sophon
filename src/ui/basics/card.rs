//! [`Card`] and its parts, after shadcn/ui's `card.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border` is `border border-border` (shadcn's global
//!   `* { @apply border-border }`; NSH has no global rules).
//! - `CardHeader`'s `@container/card-header` is left out: NSH has no
//!   container queries, and nothing in a card queries it.
//! - `has-data-[slot=card-action]:grid-cols-[1fr_auto]` follows the
//!   header's `action` slot: NSH has no `:has()`, so the action is a prop
//!   of the header, which takes the second column while it holds one.
//! - `Card` adds `text-base`: shadcn's card text with no size of its own
//!   (the title) inherits the page's 16px / 1.5; NSH's default text is 14
//!   points.
//! - `[.border-b]:pb-6` (header) and `[.border-t]:pt-6` (footer) pad a
//!   header or footer the app gave a border; NSH cannot select on the
//!   app's classes, so an app adding `border-b` adds `pb-6` with it.

use nsh_core::prelude::*;

use super::cls;

const CARD: &str = "flex flex-col gap-6 rounded-xl border border-border bg-card py-6 \
    text-card-foreground shadow-sm text-base";
const HEADER: &str = "grid auto-rows-min grid-rows-[auto_auto] items-start gap-2 px-6";
/// `has-data-[slot=card-action]:grid-cols-[1fr_auto]`.
const HEADER_WITH_ACTION: &str = "grid-cols-[1fr_auto]";
const TITLE: &str = "leading-none font-semibold";
const DESCRIPTION: &str = "text-sm text-muted-foreground";
const ACTION: &str = "col-start-2 row-span-2 row-start-1 self-start justify-self-end";
const CONTENT: &str = "px-6";
const FOOTER: &str = "flex items-center px-6";

/// The props of [`Card`], [`CardAction`], [`CardContent`] and
/// [`CardFooter`]: their content.
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CardPartProps {
    /// What the part holds.
    pub children: Prop<Children>,
}

/// shadcn's Card: a bordered surface holding a header, content and a
/// footer.
#[component]
pub fn Card(props: CardPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(CARD)).child(props.children)
}

/// The props of [`CardHeader`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CardHeaderProps {
    /// A [`CardAction`] at the header's top right, beside the title and
    /// description (shadcn's `CardAction` child).
    pub action: Prop<Option<Slot>>,
    /// The title and description.
    pub children: Prop<Children>,
}

/// shadcn's CardHeader: the title and description, in a grid with the
/// action beside them.
#[component]
pub fn CardHeader(props: CardHeaderProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let action = props.action;
    view()
        .app_class(move || {
            cls(match action.get() {
                Some(_) => format!("{HEADER} {HEADER_WITH_ACTION}"),
                None => HEADER.to_owned(),
            })
        })
        .child(props.children)
        .child(action.place(|slot| slot))
}

/// The props of [`CardTitle`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CardTitleProps {
    /// The title.
    pub title: Prop<Option<SharedString>>,
    /// Content after the title.
    pub children: Prop<Children>,
}

/// shadcn's CardTitle.
#[component]
pub fn CardTitle(props: CardTitleProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    text(move || props.title.get().unwrap_or_default())
        .app_class(cls(TITLE))
        .child(props.children)
}

/// The props of [`CardDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct CardDescriptionProps {
    /// The description.
    pub description: Prop<Option<SharedString>>,
    /// Content after the description.
    pub children: Prop<Children>,
}

/// shadcn's CardDescription: muted text under the title.
#[component]
pub fn CardDescription(props: CardDescriptionProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    text(move || props.description.get().unwrap_or_default())
        .app_class(cls(DESCRIPTION))
        .child(props.children)
}

/// shadcn's CardAction: what the header holds at its top right (a button,
/// a link). Give it to [`CardHeader`]'s `action`.
#[component]
pub fn CardAction(props: CardPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(ACTION)).child(props.children)
}

/// shadcn's CardContent: the card's body.
#[component]
pub fn CardContent(props: CardPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(CONTENT)).child(props.children)
}

/// shadcn's CardFooter: a row under the content.
#[component]
pub fn CardFooter(props: CardPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(FOOTER)).child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Card>();
    registry.register::<CardHeader>();
    registry.register::<CardTitle>();
    registry.register::<CardDescription>();
    registry.register::<CardAction>();
    registry.register::<CardContent>();
    registry.register::<CardFooter>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::assert_known([
            CARD,
            HEADER,
            HEADER_WITH_ACTION,
            TITLE,
            DESCRIPTION,
            ACTION,
            CONTENT,
            FOOTER,
            // What an app adds with a border (`[.border-b]:pb-6`, `[.border-t]:pt-6`).
            "border-b pb-6 border-t pt-6",
        ]);
    }
}
