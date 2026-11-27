//! The chat family of components: shadcn/ui's newest parts for AI chat
//! transcripts. Each component is one file in this directory; this module
//! declares them, re-exports their public API (types and constructors,
//! which the crate root re-exports in turn) and registers them for TSX.
//!
//! They compose as shadcn's do: a [`MessageScroller`] of rows, each a
//! [`Message`] (an avatar beside [`Bubble`]s and [`Attachment`]s, a header,
//! a footer) or a [`Marker`].
//!
//! ```ignore
//! use nsh_shadcn as ui;
//!
//! ui::message().end().child(
//!     ui::message_content()
//!         .child(ui::bubble().child(ui::bubble_content("Deploying to prod real quick.")))
//!         .child(ui::message_footer().text(Some("Delivered".into()))),
//! )
//! ```

use nsh_core::prelude::*;

mod attachment;
mod bubble;
mod direction;
mod marker;
mod message;
mod message_scroller;

pub use attachment::{
    Attachment, AttachmentAction, AttachmentActionElement, AttachmentActionEvent, AttachmentActions,
    AttachmentActionsElement, AttachmentContent, AttachmentContentElement, AttachmentDescription,
    AttachmentDescriptionElement, AttachmentElement, AttachmentGroup, AttachmentGroupElement, AttachmentMedia,
    AttachmentMediaElement, AttachmentMediaVariant, AttachmentOrientation, AttachmentSize, AttachmentState,
    AttachmentTitle, AttachmentTitleElement, AttachmentTrigger, AttachmentTriggerElement, AttachmentTriggerEvent,
};
pub use bubble::{
    Bubble, BubbleAlign, BubbleContent, BubbleContentElement, BubbleContentEvent, BubbleElement, BubbleGroup,
    BubbleGroupElement, BubbleReactions, BubbleReactionsAlign, BubbleReactionsElement, BubbleReactionsSide,
    BubbleVariant,
};
pub use direction::{Direction, DirectionProvider, DirectionProviderElement, use_direction};
pub use marker::{
    Marker, MarkerContent, MarkerContentElement, MarkerElement, MarkerEvent, MarkerIcon, MarkerIconElement,
    MarkerVariant,
};
pub use message::{
    Message, MessageAlign, MessageAvatar, MessageAvatarElement, MessageContent, MessageContentElement,
    MessageElement, MessageFooter, MessageFooterElement, MessageGroup, MessageGroupElement, MessageHeader,
    MessageHeaderElement,
};
pub use message_scroller::{
    MessageScroller, MessageScrollerButton, MessageScrollerButtonDirection, MessageScrollerButtonElement,
    MessageScrollerContent, MessageScrollerContentElement, MessageScrollerElement, MessageScrollerHandle,
    MessageScrollerItem, MessageScrollerItemElement, MessageScrollerProvider, MessageScrollerProviderElement,
    MessageScrollerScrollAlign, MessageScrollerScrollPosition, MessageScrollerScrollable,
    MessageScrollerViewport, MessageScrollerViewportElement, MessageScrollerVisibility, use_message_scroller,
};

use crate::icon::IconName;

/// Consecutive messages of one sender: `message_group().child(message()…)`.
pub fn message_group() -> MessageGroupElement {
    message::message_group()
}

/// A message, on the start side; `.end()` for the user's own.
pub fn message() -> MessageElement {
    message::message()
}

/// A message's avatar: `message_avatar().child(avatar)`.
pub fn message_avatar() -> MessageAvatarElement {
    message::message_avatar()
}

/// The column of a message's parts.
pub fn message_content() -> MessageContentElement {
    message::message_content()
}

/// The sender's name above a message: `message_header("Olivia")`.
pub fn message_header(text: impl Into<SharedString>) -> MessageHeaderElement {
    message::message_header().text(Some(text.into()))
}

/// A line under a message: `message_footer().text(Some("Delivered".into()))`,
/// or actions as its children.
pub fn message_footer() -> MessageFooterElement {
    message::message_footer()
}

impl MessageElement {
    /// `.align(MessageAlign::End)`: the user's own message.
    pub fn end(self) -> Self {
        self.align(MessageAlign::End)
    }
}

/// Bubbles sent one after the other.
pub fn bubble_group() -> BubbleGroupElement {
    bubble::bubble_group()
}

/// A chat bubble (the primary fill); `.muted()`, `.ghost()`, ... for the
/// others, `.end()` on the end side.
pub fn bubble() -> BubbleElement {
    bubble::bubble()
}

/// What a bubble says: `bubble_content("It's a one-line change.")`.
pub fn bubble_content(text: impl Into<SharedString>) -> BubbleContentElement {
    bubble::bubble_content().text(Some(text.into()))
}

/// Reactions over a bubble's edge: `bubble_reactions().child(text("👍"))`.
pub fn bubble_reactions() -> BubbleReactionsElement {
    bubble::bubble_reactions()
}

/// A line of the conversation that is not a message; `.separator()`,
/// `.border()` for the others.
pub fn marker() -> MarkerElement {
    marker::marker()
}

/// A marker's icon: `marker_icon(IconName::GitBranch)`.
pub fn marker_icon(icon: IconName) -> MarkerIconElement {
    marker::marker_icon().icon(Some(icon))
}

/// What a marker says: `marker_content("Conversation compacted")`.
pub fn marker_content(text: impl Into<SharedString>) -> MarkerContentElement {
    marker::marker_content().text(Some(text.into()))
}

/// A file: `attachment().child(attachment_media(..)).child(attachment_content()…)`.
pub fn attachment() -> AttachmentElement {
    attachment::attachment()
}

/// An attachment's icon (`.icon(Some(IconName::File))`) or image
/// (`.variant(AttachmentMediaVariant::Image).src(Some(url))`).
pub fn attachment_media() -> AttachmentMediaElement {
    attachment::attachment_media()
}

/// An attachment's name and details.
pub fn attachment_content() -> AttachmentContentElement {
    attachment::attachment_content()
}

/// An attachment's name: `attachment_title("report.pdf")`.
pub fn attachment_title(text: impl Into<SharedString>) -> AttachmentTitleElement {
    attachment::attachment_title().text(text.into())
}

/// An attachment's details: `attachment_description("PDF · 2.4 MB")`.
pub fn attachment_description(text: impl Into<SharedString>) -> AttachmentDescriptionElement {
    attachment::attachment_description().text(text.into())
}

/// An attachment's buttons.
pub fn attachment_actions() -> AttachmentActionsElement {
    attachment::attachment_actions()
}

/// An attachment's icon button: `attachment_action(IconName::X, "Remove report.pdf")`.
pub fn attachment_action(icon: IconName, label: impl Into<SharedString>) -> AttachmentActionElement {
    attachment::attachment_action()
        .icon(Some(icon))
        .accessibility_label(Some(label.into()))
}

/// The whole attachment as a button: `attachment_trigger("Open report.pdf")`.
pub fn attachment_trigger(label: impl Into<SharedString>) -> AttachmentTriggerElement {
    attachment::attachment_trigger().accessibility_label(Some(label.into()))
}

/// Attachments side by side, scrolling sideways.
pub fn attachment_group() -> AttachmentGroupElement {
    attachment::attachment_group()
}

impl AttachmentElement {
    /// `.size(AttachmentSize::Sm)`.
    pub fn sm(self) -> Self {
        self.size(AttachmentSize::Sm)
    }
    /// `.size(AttachmentSize::Xs)`.
    pub fn xs(self) -> Self {
        self.size(AttachmentSize::Xs)
    }
    /// `.orientation(AttachmentOrientation::Vertical)`: a tile.
    pub fn vertical(self) -> Self {
        self.orientation(AttachmentOrientation::Vertical)
    }
}

/// The scroll state of a transcript, around its scroller and the controls
/// that use it.
pub fn message_scroller_provider() -> MessageScrollerProviderElement {
    message_scroller::message_scroller_provider()
}

/// A transcript's frame.
pub fn message_scroller() -> MessageScrollerElement {
    message_scroller::message_scroller()
}

/// A transcript's visible part.
pub fn message_scroller_viewport() -> MessageScrollerViewportElement {
    message_scroller::message_scroller_viewport()
}

/// A transcript's rows: `message_scroller_content(virtual_list(count, row).keys(..))`,
/// with `.message_ids(..)` and `.scroll_anchors(..)`.
pub fn message_scroller_content(rows: VirtualList) -> MessageScrollerContentElement {
    message_scroller::message_scroller_content().items(rows)
}

/// A row of a transcript.
pub fn message_scroller_item() -> MessageScrollerItemElement {
    message_scroller::message_scroller_item()
}

/// The scroll-to-end button (`.direction(MessageScrollerButtonDirection::Start)`
/// for the start).
pub fn message_scroller_button() -> MessageScrollerButtonElement {
    message_scroller::message_scroller_button()
}

/// Lays its content out in `direction`: `direction_provider(Direction::Rtl)`.
pub fn direction_provider(direction: Direction) -> DirectionProviderElement {
    direction::direction_provider().direction(direction)
}

/// Registers this family's components for TSX (`nsh-shadcn`).
pub(crate) fn register(registry: &mut Registry) {
    message::register(registry);
    bubble::register(registry);
    attachment::register(registry);
    marker::register(registry);
    message_scroller::register(registry);
    direction::register(registry);
}
