//! [`Attachment`], [`AttachmentMedia`], [`AttachmentContent`],
//! [`AttachmentTitle`], [`AttachmentDescription`], [`AttachmentActions`],
//! [`AttachmentAction`], [`AttachmentTrigger`] and [`AttachmentGroup`],
//! after shadcn/ui's `attachment.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class:
//!
//! - `data-[state=…]:…`, `group-data-[state|size|orientation=…]/attachment:…`
//!   are `data-state-…:…` and `group-data-state-…/attachment:…` (NSH has
//!   the boolean `data-*` form only): an attachment projects `state-<state>`,
//!   `size-<size>` and `orientation-<orientation>`.
//! - `has-data-[slot=attachment-content]:…`, `has-data-[slot=attachment-media]:…`
//!   and `has-[>a,>button]:…` are `data-has-content:…`, `data-has-media:…`
//!   and `data-has-trigger:…`: NSH has no `has-*` variants, so the
//!   [`AttachmentContent`], [`AttachmentMedia`] and [`AttachmentTrigger`]
//!   inside tell their attachment, which projects those.
//! - `border` is `border border-border` (NSH has no global border color).
//! - The content's `flex-1` is `flex-auto`: NSH sizes a `w-fit` row from
//!   its children's flex bases, so a `flex-1` content would add nothing to
//!   the attachment's width and its title would be cut, where a browser
//!   fits the attachment to the whole title. In a full-width attachment
//!   both fill the room left the same.
//! - A vertical attachment without content is `w-24` (96 points). In
//!   shadcn's CSS the base's `w-fit` comes after the orientation's `w-24`
//!   (Tailwind orders the utilities of one property by name) and wins, so
//!   in a browser such a tile takes its image's natural width (up to the
//!   container's); NSH cannot size a `w-fit` box from a `w-full` image
//!   inside it (it collapses), so the later class, `w-24`, stays.
//! - `[&_svg…]:size-*` sizes the media's `icon`; `*:[img]:…` its `src`
//!   image; `*:data-[slot=spinner]:size-6!` is left out (the spinner is the
//!   app's child).
//! - `shimmer` (the title while uploading and processing) has no NSH
//!   equivalent: NSH clips no background to text.
//! - The group is a horizontal scroll view without a scroll bar
//!   (`overflow-x-auto scrollbar-none`); `snap-x snap-mandatory scroll-px-1`
//!   and `*:data-[slot=attachment]:snap-start` (snapping to each
//!   attachment), `scroll-fade-x` (a mask fading the edges) and
//!   `overscroll-x-contain` are left out.

use nsh_core::prelude::*;

use super::message::Counter;
use crate::cn::cn;
use crate::icon::{IconName, icon};
use crate::ui::button::{ButtonSize, ButtonVariant, button};

/// Where an attachment is in its upload: shadcn's `state`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum AttachmentState {
    /// Chosen, not uploading yet: a dashed border.
    Idle,
    /// Uploading: the image dimmed.
    Uploading,
    /// Being processed: the image dimmed.
    Processing,
    /// Failed: destructive colors.
    Error,
    /// Uploaded.
    #[default]
    Done,
}

/// How big an attachment is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum AttachmentSize {
    /// A 40-point media.
    #[default]
    Default,
    /// A 32-point media, extra small text.
    Sm,
    /// A 28-point media, tighter.
    Xs,
}

/// How an attachment lays out: shadcn's `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum AttachmentOrientation {
    /// Media, text and actions in a row.
    #[default]
    Horizontal,
    /// A tile: the media on top, the actions over its corner.
    Vertical,
}

/// How an attachment's media shows: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum AttachmentMediaVariant {
    /// An icon on the muted fill.
    #[default]
    Icon,
    /// An image filling the media, dimmed until the upload is done.
    Image,
}

/// shadcn's `attachmentVariants` base classes.
const ATTACHMENT: &str = "group/attachment relative flex w-fit max-w-full min-w-0 shrink-0 flex-wrap rounded-xl \
    border border-border bg-card text-card-foreground transition-colors focus-within:ring-1 \
    focus-within:ring-ring/50 data-has-trigger:hover:bg-muted/50 data-state-error:border-destructive/30 \
    data-state-idle:border-dashed";

/// shadcn's `attachmentMediaVariants` base classes.
const MEDIA: &str = "relative flex aspect-square w-10 shrink-0 items-center justify-center overflow-hidden \
    rounded-lg bg-muted text-foreground group-data-orientation-vertical/attachment:w-full \
    group-data-size-sm/attachment:w-8 group-data-size-xs/attachment:w-7 \
    group-data-size-xs/attachment:rounded-md group-data-state-error/attachment:bg-destructive/10 \
    group-data-state-error/attachment:text-destructive";

/// shadcn's `[&_svg:not([class*='size-'])]:…` classes of the media, on its
/// icon.
const MEDIA_ICON: &str = "pointer-events-none size-4 group-data-orientation-vertical/attachment:size-6 \
    group-data-size-xs/attachment:size-3.5";

/// shadcn's `*:[img]:…` classes of the media, on its image.
const MEDIA_IMAGE: &str = "aspect-square w-full object-cover";

/// shadcn's `AttachmentContent` classes.
const CONTENT: &str = "max-w-full min-w-0 flex-auto leading-tight group-data-orientation-vertical/attachment:px-1";

/// shadcn's `AttachmentTitle` classes (`shimmer` left out).
const TITLE: &str = "block max-w-full min-w-0 truncate font-medium";

/// shadcn's `AttachmentDescription` classes.
const DESCRIPTION: &str = "mt-0.5 block min-w-0 truncate text-xs text-muted-foreground \
    group-data-state-error/attachment:text-destructive/80 max-w-full";

/// shadcn's `AttachmentActions` classes.
const ACTIONS: &str = "relative z-20 flex shrink-0 items-center \
    group-data-orientation-vertical/attachment:absolute group-data-orientation-vertical/attachment:top-3 \
    group-data-orientation-vertical/attachment:right-3 group-data-orientation-vertical/attachment:gap-1";

/// shadcn's `AttachmentTrigger` classes.
const TRIGGER: &str = "absolute inset-0 z-10 outline-none";

/// shadcn's `AttachmentGroup` classes that NSH draws: the scroll view's
/// and its row's.
const GROUP: &str = "flex min-w-0";
const GROUP_ROW: &str = "flex gap-3 py-1";

impl AttachmentSize {
    fn classes(self) -> &'static str {
        match self {
            AttachmentSize::Default => {
                "gap-2 text-sm data-has-content:px-2.5 data-has-content:py-2 data-has-media:p-2"
            }
            AttachmentSize::Sm => {
                "gap-2.5 text-xs data-has-content:px-2 data-has-content:py-1.5 data-has-media:p-1.5"
            }
            AttachmentSize::Xs => {
                "gap-1.5 rounded-lg text-xs data-has-content:px-1.5 data-has-content:py-1 data-has-media:p-1"
            }
        }
    }
}

impl AttachmentOrientation {
    fn classes(self) -> &'static str {
        match self {
            AttachmentOrientation::Horizontal => "min-w-40 items-center",
            AttachmentOrientation::Vertical => "w-24 flex-col data-has-content:w-30",
        }
    }
}

impl AttachmentMediaVariant {
    fn classes(self) -> &'static str {
        match self {
            AttachmentMediaVariant::Icon => "",
            AttachmentMediaVariant::Image => {
                "opacity-60 group-data-state-done/attachment:opacity-100 \
                 group-data-state-idle/attachment:opacity-100"
            }
        }
    }
}

/// Every class an attachment of `size` and `orientation` draws with,
/// before the app's.
fn attachment_classes(size: AttachmentSize, orientation: AttachmentOrientation) -> SharedString {
    cn([ATTACHMENT, size.classes(), orientation.classes()])
}

/// What an attachment knows of the parts inside it, for the classes
/// shadcn writes with `has-*`.
#[derive(Clone, Copy)]
struct AttachmentContext {
    content: Counter,
    media: Counter,
    trigger: Counter,
}

/// The props of [`Attachment`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentProps {
    /// `idle`, `uploading`, `processing`, `error` or `done`.
    #[prop(default)]
    pub state: Prop<AttachmentState>,
    /// `default`, `sm` or `xs`.
    #[prop(default)]
    pub size: Prop<AttachmentSize>,
    /// `horizontal` or `vertical`.
    #[prop(default)]
    pub orientation: Prop<AttachmentOrientation>,
    /// Its [`AttachmentMedia`], [`AttachmentContent`],
    /// [`AttachmentActions`] and [`AttachmentTrigger`].
    pub children: Prop<Children>,
}

/// A file in a message or a composer: its icon or image, its name and
/// details, its actions.
#[component]
pub fn Attachment(props: AttachmentProps) -> impl IntoElement {
    let parts = AttachmentContext {
        content: Counter::new(),
        media: Counter::new(),
        trigger: Counter::new(),
    };
    provide_context(parts);
    let state = move |s: AttachmentState| move || props.state.get() == s;
    let size = move |s: AttachmentSize| move || props.size.get() == s;
    view()
        .group_named("attachment")
        .app_class(move || Some(attachment_classes(props.size.get(), props.orientation.get())))
        .project_data("state-idle", state(AttachmentState::Idle))
        .project_data("state-uploading", state(AttachmentState::Uploading))
        .project_data("state-processing", state(AttachmentState::Processing))
        .project_data("state-error", state(AttachmentState::Error))
        .project_data("state-done", state(AttachmentState::Done))
        .project_data("size-sm", size(AttachmentSize::Sm))
        .project_data("size-xs", size(AttachmentSize::Xs))
        .project_data("orientation-vertical", move || {
            props.orientation.get() == AttachmentOrientation::Vertical
        })
        .project_data("has-content", move || parts.content.any())
        .project_data("has-media", move || parts.media.any())
        .project_data("has-trigger", move || parts.trigger.any())
        .child(props.children)
}

/// The props of [`AttachmentMedia`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentMediaProps {
    /// `icon` or `image`.
    #[prop(default)]
    pub variant: Prop<AttachmentMediaVariant>,
    /// A Lucide icon (the file's kind).
    pub icon: Prop<Option<IconName>>,
    /// The image of an `image` media: a URL, a `data:` URI.
    pub src: Prop<Option<SharedString>>,
    /// What screen readers say for the image.
    pub alt: Prop<Option<SharedString>>,
    /// Another glyph in place of `icon` (a spinner).
    pub children: Prop<Children>,
}

/// The file's icon or a preview of its image, in a square.
#[component]
pub fn AttachmentMedia(props: AttachmentMediaProps) -> impl IntoElement {
    if let Some(attachment) = use_context::<AttachmentContext>() {
        attachment.media.count_while(|| true);
    }
    view()
        .app_class(move || Some(cn([MEDIA, props.variant.get().classes()])))
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(Some(SharedString::from(MEDIA_ICON)))
                    .into_element()
            })
        }))
        .child(dynamic(move || {
            props.src.get().map(|src| {
                let alt = props.alt.get();
                image(src)
                    .app_class(Some(SharedString::from(MEDIA_IMAGE)))
                    .accessible(alt.is_some())
                    .optional_label(alt)
                    .into_element()
            })
        }))
        .child(props.children)
}

/// The props of [`AttachmentContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentContentProps {
    /// An [`AttachmentTitle`] and an [`AttachmentDescription`].
    pub children: Prop<Children>,
}

/// The file's name and details.
#[component]
pub fn AttachmentContent(props: AttachmentContentProps) -> impl IntoElement {
    if let Some(attachment) = use_context::<AttachmentContext>() {
        attachment.content.count_while(|| true);
    }
    view().app_class(Some(SharedString::from(CONTENT))).child(props.children)
}

/// The props of [`AttachmentTitle`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentTitleProps {
    /// The file's name.
    pub text: Prop<SharedString>,
}

/// The file's name, on one line, cut with an ellipsis.
#[component]
pub fn AttachmentTitle(props: AttachmentTitleProps) -> impl IntoElement {
    text(move || props.text.get()).app_class(Some(SharedString::from(TITLE)))
}

/// The props of [`AttachmentDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentDescriptionProps {
    /// Its kind and size, or how its upload goes.
    pub text: Prop<SharedString>,
}

/// The file's details, muted, on one line.
#[component]
pub fn AttachmentDescription(props: AttachmentDescriptionProps) -> impl IntoElement {
    text(move || props.text.get()).app_class(Some(SharedString::from(DESCRIPTION)))
}

/// The props of [`AttachmentActions`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentActionsProps {
    /// [`AttachmentAction`]s.
    pub children: Prop<Children>,
}

/// The attachment's buttons, at its end (over the corner of a tile), above
/// its trigger.
#[component]
pub fn AttachmentActions(props: AttachmentActionsProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(ACTIONS))).child(props.children)
}

/// The props of [`AttachmentAction`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentActionProps {
    /// Its icon.
    pub icon: Prop<Option<IconName>>,
    /// What it does, which screen readers say: "Remove report.pdf".
    pub accessibility_label: Prop<Option<SharedString>>,
    /// A [`crate::Button`] variant; shadcn's default is `ghost`.
    #[prop(default = ButtonVariant::Ghost)]
    pub variant: Prop<ButtonVariant>,
    /// A [`crate::Button`] size; shadcn's default is `icon-xs`.
    #[prop(default = ButtonSize::IconXs)]
    pub size: Prop<ButtonSize>,
}

/// The events of an [`AttachmentAction`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum AttachmentActionEvent {
    /// It was pressed.
    Press,
}

/// An icon button of the attachment: remove, cancel, retry, download.
#[component]
pub fn AttachmentAction(props: AttachmentActionProps, cx: Scope<AttachmentActionEvent>) -> impl IntoElement {
    button()
        .variant(props.variant)
        .size(props.size)
        .icon(props.icon)
        .accessibility_label(props.accessibility_label)
        .on_press(move || cx.emit(AttachmentActionEvent::Press))
}

/// The props of [`AttachmentTrigger`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentTriggerProps {
    /// What a press does, which screen readers say: "Open report.pdf".
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of an [`AttachmentTrigger`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum AttachmentTriggerEvent {
    /// It was pressed.
    Press,
}

/// The whole attachment as one button (open, preview), under its actions.
#[component]
pub fn AttachmentTrigger(props: AttachmentTriggerProps, cx: Scope<AttachmentTriggerEvent>) -> impl IntoElement {
    if let Some(attachment) = use_context::<AttachmentContext>() {
        attachment.trigger.count_while(|| true);
    }
    view()
        .app_class(Some(SharedString::from(TRIGGER)))
        .role(AccessibilityRole::Button)
        .optional_label(move || props.accessibility_label.get())
        .cursor_pointer()
        .on_press(move || cx.emit(AttachmentTriggerEvent::Press))
}

/// The props of [`AttachmentGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AttachmentGroupProps {
    /// The [`Attachment`]s, side by side.
    pub children: Prop<Children>,
}

/// Attachments side by side, scrolling sideways when they do not fit.
#[component]
pub fn AttachmentGroup(props: AttachmentGroupProps) -> impl IntoElement {
    scroll()
        .horizontal()
        .scrollbar(false)
        .app_class(Some(SharedString::from(GROUP)))
        .child(view().app_class(Some(SharedString::from(GROUP_ROW))).child(props.children))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Attachment>();
    registry.register::<AttachmentMedia>();
    registry.register::<AttachmentContent>();
    registry.register::<AttachmentTitle>();
    registry.register::<AttachmentDescription>();
    registry.register::<AttachmentActions>();
    registry.register::<AttachmentAction>();
    registry.register::<AttachmentTrigger>();
    registry.register::<AttachmentGroup>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = vec![cn([
            MEDIA,
            MEDIA_ICON,
            MEDIA_IMAGE,
            CONTENT,
            TITLE,
            DESCRIPTION,
            ACTIONS,
            TRIGGER,
            GROUP,
            GROUP_ROW,
        ])];
        for variant in [AttachmentMediaVariant::Icon, AttachmentMediaVariant::Image] {
            all.push(cn([MEDIA, variant.classes()]));
        }
        for size in [AttachmentSize::Default, AttachmentSize::Sm, AttachmentSize::Xs] {
            for orientation in [AttachmentOrientation::Horizontal, AttachmentOrientation::Vertical] {
                all.push(attachment_classes(size, orientation));
            }
        }
        for classes in all {
            let unknown = crate::cn::unknown_classes(&classes);
            assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
        }
    }
}
