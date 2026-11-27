//! [`MessageGroup`], [`Message`], [`MessageAvatar`], [`MessageContent`],
//! [`MessageHeader`] and [`MessageFooter`], after shadcn/ui's
//! `message.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class:
//!
//! - `data-[align=end]:…` and `group-data-[align=end]/message:…` are
//!   `data-align-end:…` and `group-data-align-end/message:…`: NSH has the
//!   boolean `data-*` form only, so a message projects `align-end` while its
//!   `align` is `end`.
//! - `group-has-data-[slot=message-footer]/message:…` (the avatar moves up
//!   past a footer) and `group-has-data-[variant=ghost]/message:…` (header
//!   and footer lose their padding beside a ghost bubble) are
//!   `group-data-has-footer/message:…` and `group-data-has-ghost/message:…`:
//!   NSH has no `has-*` variants, so a [`MessageFooter`] and a ghost
//!   [`super::Bubble`] tell the message around them, which projects
//!   `has-footer` / `has-ghost`.
//! - `group-data-[align=end]/message:*:data-slot:self-end` (every part in
//!   the content goes to the end) is `group-data-align-end/message:items-end`
//!   on the content: NSH has no child selectors. The parts are the
//!   content's children, so they go to the end alike; a child that is not
//!   one of this library's parts goes to the end too.
//! - `wrap-break-word` is left out: NSH has no such class.

use std::cell::Cell;
use std::rc::Rc;

use nsh_core::prelude::*;

#[cfg(test)]
use crate::cn::cn;

/// What a message knows of the parts inside it, for the classes shadcn
/// writes with `has-*`.
#[derive(Clone, Copy)]
pub(crate) struct MessageContext {
    /// How many [`MessageFooter`]s it holds.
    pub(crate) footers: Counter,
    /// How many ghost bubbles it holds.
    pub(crate) ghosts: Counter,
}

/// A count of parts inside a part, which shadcn's `has-*` variants ask
/// about.
#[derive(Clone, Copy)]
pub(crate) struct Counter(Signal<u32>);

impl Counter {
    pub(crate) fn new() -> Self {
        Counter(signal(0))
    }

    /// At least one part is counted.
    pub(crate) fn any(self) -> bool {
        self.0.get() > 0
    }

    /// Counts the component being made while `on` holds, until it goes.
    pub(crate) fn count_while(self, on: impl Fn() -> bool + 'static) {
        let counted = Rc::new(Cell::new(false));
        {
            let counted = counted.clone();
            effect(move || {
                let on = on();
                if on != counted.get() {
                    counted.set(on);
                    untracked(|| self.0.update(|n| if on { *n += 1 } else { *n = n.saturating_sub(1) }));
                }
            });
        }
        cleanup(move || {
            if counted.get() {
                self.0.update(|n| *n = n.saturating_sub(1));
            }
        });
    }
}

/// Which side a message is on: shadcn's `align`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum MessageAlign {
    /// The start (left, right to left: right): the other side's messages.
    #[default]
    Start,
    /// The end: the user's own messages, avatar after the content.
    End,
}

/// shadcn's `MessageGroup` classes.
const GROUP: &str = "flex min-w-0 flex-col gap-2";

/// shadcn's `Message` classes.
const MESSAGE: &str = "group/message relative flex w-full min-w-0 gap-2 text-sm data-align-end:flex-row-reverse";

/// shadcn's `MessageAvatar` classes.
const AVATAR: &str = "flex w-fit min-w-8 shrink-0 items-center justify-center self-end overflow-hidden \
    rounded-full bg-muted group-data-has-footer/message:-translate-y-8";

/// shadcn's `MessageContent` classes.
const CONTENT: &str = "flex w-full min-w-0 flex-col gap-2.5 group-data-align-end/message:items-end";

/// shadcn's `MessageHeader` classes.
const HEADER: &str = "flex max-w-full min-w-0 items-center px-3 text-xs font-medium text-muted-foreground \
    group-data-has-ghost/message:px-0";

/// shadcn's `MessageFooter` classes.
const FOOTER: &str = "flex max-w-full min-w-0 items-center px-3 text-xs font-medium text-muted-foreground \
    group-data-has-ghost/message:px-0 group-data-align-end/message:justify-end";

/// The props of [`MessageGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageGroupProps {
    /// The [`Message`]s, one after the other.
    pub children: Prop<Children>,
}

/// Consecutive messages of one sender, closer together than the turns of
/// a conversation.
#[component]
pub fn MessageGroup(props: MessageGroupProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(GROUP))).child(props.children)
}

/// The props of [`Message`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageProps {
    /// `start` (the other side's) or `end` (the user's own).
    #[prop(default)]
    pub align: Prop<MessageAlign>,
    /// A [`MessageAvatar`] and a [`MessageContent`].
    pub children: Prop<Children>,
}

/// One message of a conversation: an avatar beside its content, on the
/// start or the end side.
#[component]
pub fn Message(props: MessageProps) -> impl IntoElement {
    let context = MessageContext {
        footers: Counter::new(),
        ghosts: Counter::new(),
    };
    provide_context(context);
    view()
        .group_named("message")
        .app_class(Some(SharedString::from(MESSAGE)))
        .project_data("align-end", move || props.align.get() == MessageAlign::End)
        .project_data("has-footer", move || context.footers.any())
        .project_data("has-ghost", move || context.ghosts.any())
        .child(props.children)
}

/// The props of [`MessageAvatar`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageAvatarProps {
    /// The avatar (shadcn puts an `Avatar` here); empty, it keeps the
    /// avatar's width so grouped messages line up.
    pub children: Prop<Children>,
}

/// The sender's avatar, at the bottom of the message, beside its last
/// bubble (above a footer).
#[component]
pub fn MessageAvatar(props: MessageAvatarProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(AVATAR))).child(props.children)
}

/// The props of [`MessageContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageContentProps {
    /// The message's parts: a header, bubbles, attachments, a footer.
    pub children: Prop<Children>,
}

/// The column of a message's parts, at the message's side.
#[component]
pub fn MessageContent(props: MessageContentProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(CONTENT))).child(props.children)
}

/// The props of [`MessageHeader`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageHeaderProps {
    /// The text: the sender's name.
    pub text: Prop<Option<SharedString>>,
    /// Content after the text.
    pub children: Prop<Children>,
}

/// A line above a message's bubbles: who sent it.
#[component]
pub fn MessageHeader(props: MessageHeaderProps) -> impl IntoElement {
    view()
        .app_class(Some(SharedString::from(HEADER)))
        .child(dynamic(move || props.text.get().map(|t| text(t).into_element())))
        .child(props.children)
}

/// The props of [`MessageFooter`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageFooterProps {
    /// The text: "Delivered", "Read".
    pub text: Prop<Option<SharedString>>,
    /// Content after the text: actions (ghost icon buttons), a status.
    pub children: Prop<Children>,
}

/// A line under a message's bubbles: its status or its actions. The
/// message's avatar moves up to the bubbles above it.
#[component]
pub fn MessageFooter(props: MessageFooterProps) -> impl IntoElement {
    if let Some(message) = use_context::<MessageContext>() {
        message.footers.count_while(|| true);
    }
    view()
        .app_class(Some(SharedString::from(FOOTER)))
        .child(dynamic(move || props.text.get().map(|t| text(t).into_element())))
        .child(props.children)
}

/// Every class string of this file, for the test.
#[cfg(test)]
pub(crate) fn all_classes() -> SharedString {
    cn([GROUP, MESSAGE, AVATAR, CONTENT, HEADER, FOOTER])
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<MessageGroup>();
    registry.register::<Message>();
    registry.register::<MessageAvatar>();
    registry.register::<MessageContent>();
    registry.register::<MessageHeader>();
    registry.register::<MessageFooter>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let unknown = crate::cn::unknown_classes(&all_classes());
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
