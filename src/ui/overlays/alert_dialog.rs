//! [`AlertDialog`] and its parts, after shadcn/ui's `alert-dialog.tsx`
//! (new-york-v4): a styled dialog that asks for a decision, drawn by this
//! crate on `nsh_base::dialog()` as [`super::Dialog`] is. It is not
//! `nsh_base::AlertDialog`, which shows the system's alert.
//!
//! Behavior: as `Dialog`'s, announced as an alert dialog, and a press on
//! the overlay does not close it (Radix's `AlertDialog`): only Escape,
//! [`AlertDialogCancel`] and [`AlertDialogAction`] do.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border` is `border border-border`.
//! - The animation classes are tw-animate-css's: the modal layer fades in
//!   and out as a whole (`ModalAnimation::Fade`), without `zoom-in-95`.
//! - `max-w-[calc(100%-2rem)]` is left out (no `calc()` in NSH's sizes;
//!   `sm:max-w-lg` / `max-w-xs` take its place at the window sizes it acts
//!   at).
//! - `data-[size=…]:`, `group-data-[size=…]/alert-dialog-content:` and
//!   `has-data-[slot=alert-dialog-media]:` select on the content's `size`
//!   and on whether the header holds an [`AlertDialogMedia`]: the parts pick
//!   those classes from the `size` prop and the media through context (NSH
//!   has no group or `has-` selectors).
//! - `*:[svg:not([class*='size-'])]:size-8` sizes the media's icon: the
//!   `icon` prop's icon takes `size-8`.
//! - `text-base` is added to the content: a portal's content inherits the
//!   page body's text (the browser's 16 px, on Tailwind's base line height
//!   1.5), which NSH's text, 14 points by default, does not have.

use nsh_core::prelude::*;

use super::dialog::modal;
use super::open::{OpenContext, trigger};
use crate::cn::cn;
use crate::icon::{IconName, icon};
use crate::{ButtonSize, ButtonVariant};

const CONTENT: &str = "group/alert-dialog-content relative z-50 grid w-full \
    gap-4 rounded-lg border border-border bg-background p-6 \
    shadow-lg text-base";

/// How wide an alert dialog is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum AlertDialogSize {
    /// `sm:max-w-lg`, the text and actions at the start and end.
    #[default]
    Default,
    /// `max-w-xs`, centered, the actions side by side.
    Sm,
}

impl AlertDialogSize {
    fn content(self) -> &'static str {
        match self {
            AlertDialogSize::Default => "sm:max-w-lg",
            AlertDialogSize::Sm => "max-w-xs",
        }
    }

    fn header(self, media: bool) -> &'static str {
        match (self, media) {
            (AlertDialogSize::Default, false) => {
                "grid grid-rows-[auto_1fr] place-items-center gap-1.5 text-center \
                 sm:place-items-start sm:text-left"
            }
            (AlertDialogSize::Default, true) => {
                "grid grid-rows-[auto_1fr] place-items-center gap-1.5 text-center \
                 grid-rows-[auto_auto_1fr] gap-x-6 sm:place-items-start sm:text-left \
                 sm:grid-rows-[auto_1fr]"
            }
            (AlertDialogSize::Sm, false) => {
                "grid grid-rows-[auto_1fr] place-items-center gap-1.5 text-center"
            }
            (AlertDialogSize::Sm, true) => {
                "grid grid-rows-[auto_1fr] place-items-center gap-1.5 text-center \
                 grid-rows-[auto_auto_1fr] gap-x-6"
            }
        }
    }

    fn footer(self) -> &'static str {
        match self {
            AlertDialogSize::Default => "flex flex-col-reverse gap-2 sm:flex-row sm:justify-end",
            AlertDialogSize::Sm => {
                "flex flex-col-reverse gap-2 grid grid-cols-2 sm:flex-row sm:justify-end"
            }
        }
    }

    fn title(self, media: bool) -> &'static str {
        match (self, media) {
            (AlertDialogSize::Default, true) => "text-lg font-semibold sm:col-start-2",
            _ => "text-lg font-semibold",
        }
    }

    fn media(self) -> &'static str {
        match self {
            AlertDialogSize::Default => {
                "mb-2 inline-flex size-16 items-center justify-center rounded-md bg-muted sm:row-span-2"
            }
            AlertDialogSize::Sm => {
                "mb-2 inline-flex size-16 items-center justify-center rounded-md bg-muted"
            }
        }
    }
}

const DESCRIPTION: &str = "text-sm text-muted-foreground";

/// What the parts inside the content read: its size, and whether the
/// header holds a media.
#[derive(Clone, Copy)]
struct Layout {
    size: Prop<AlertDialogSize>,
    media: Signal<bool>,
}

impl Layout {
    fn of() -> Option<Self> {
        use_context::<Layout>()
    }
    fn size(this: Option<Self>) -> AlertDialogSize {
        this.map(|l| l.size.get()).unwrap_or_default()
    }
    fn media(this: Option<Self>) -> bool {
        this.is_some_and(|l| l.media.get())
    }
}

/// The props of [`AlertDialog`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertDialogProps {
    /// Open (controlled): the app owns the value and changes it in
    /// `on_open_change`. Without it the alert dialog keeps its own state.
    pub open: Prop<Option<bool>>,
    /// Open at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// What opens it (shadcn's `AlertDialogTrigger asChild`).
    pub trigger: Prop<Option<Slot>>,
    /// The [`AlertDialogContent`], made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// The events of an [`AlertDialog`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum AlertDialogEvent {
    /// It asks to open or close (the trigger, Escape, an action or the
    /// cancel button): set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's AlertDialog: a dialog that interrupts with a decision to make.
#[component]
pub fn AlertDialog(props: AlertDialogProps, cx: Scope<AlertDialogEvent>) -> impl IntoElement {
    let state = OpenContext::new(props.open, props.default_open, move |open| {
        cx.emit(AlertDialogEvent::OpenChange(open))
    })
    .provide();
    view()
        .child(trigger(props.trigger, state.clone()))
        .child(modal(state, AccessibilityRole::AlertDialog, false, props.children))
}

/// The props of [`AlertDialogContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertDialogContentProps {
    /// `default` or `sm`.
    #[prop(default)]
    pub size: Prop<AlertDialogSize>,
    /// The header and the footer.
    pub children: Prop<Children>,
}

/// shadcn's AlertDialogContent: the card in the middle of the window.
#[component]
pub fn AlertDialogContent(props: AlertDialogContentProps) -> impl IntoElement {
    provide_context(Layout {
        size: props.size,
        media: signal(false),
    });
    view()
        .app_class(move || Some(cn([CONTENT, props.size.get().content()])))
        .child(props.children)
}

/// The props of the parts that only hold others ([`AlertDialogHeader`],
/// [`AlertDialogFooter`]).
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertDialogHeaderProps {
    /// What it holds.
    pub children: Prop<Children>,
}

/// shadcn's AlertDialogHeader: the media, the title and the description.
#[component]
pub fn AlertDialogHeader(props: AlertDialogHeaderProps) -> impl IntoElement {
    let layout = Layout::of();
    view()
        .app_class(move || {
            Some(SharedString::from(
                Layout::size(layout).header(Layout::media(layout)),
            ))
        })
        .child(props.children)
}

/// shadcn's AlertDialogFooter: the actions.
#[component]
pub fn AlertDialogFooter(props: AlertDialogHeaderProps) -> impl IntoElement {
    let layout = Layout::of();
    view()
        .app_class(move || Some(SharedString::from(Layout::size(layout).footer())))
        .child(props.children)
}

/// The props of [`AlertDialogTitle`] and [`AlertDialogDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertDialogTitleProps {
    /// What it says.
    pub text: Prop<SharedString>,
}

/// shadcn's AlertDialogTitle: it names the alert dialog for screen readers
/// too.
#[component]
pub fn AlertDialogTitle(props: AlertDialogTitleProps) -> impl IntoElement {
    OpenContext::of("AlertDialogTitle").register_title(move || Some(props.text.get()));
    let layout = Layout::of();
    text(props.text).app_class(move || {
        Some(SharedString::from(
            Layout::size(layout).title(Layout::media(layout)),
        ))
    })
}

/// shadcn's AlertDialogDescription.
#[component]
pub fn AlertDialogDescription(props: AlertDialogTitleProps) -> impl IntoElement {
    OpenContext::of("AlertDialogDescription").register_description(move || Some(props.text.get()));
    text(props.text).app_class(Some(SharedString::from(DESCRIPTION)))
}

/// The props of [`AlertDialogMedia`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertDialogMediaProps {
    /// A Lucide icon in it, `size-8`.
    pub icon: Prop<Option<IconName>>,
    /// Other content (an image).
    pub children: Prop<Children>,
}

/// shadcn's AlertDialogMedia: an icon or an image in a muted square,
/// before the title.
#[component]
pub fn AlertDialogMedia(props: AlertDialogMediaProps) -> impl IntoElement {
    let layout = Layout::of();
    if let Some(layout) = layout {
        layout.media.set(true);
        let media = layout.media;
        cleanup(move || media.set(false));
    }
    view()
        .app_class(move || Some(SharedString::from(Layout::size(layout).media())))
        .child(dynamic(move || {
            props
                .icon
                .get()
                .map(|name| icon(name).app_class(Some(SharedString::from("size-8"))).into_element())
        }))
        .child(props.children)
}

/// The props of [`AlertDialogAction`] and [`AlertDialogCancel`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AlertDialogActionProps {
    /// The button's text.
    pub label: Prop<SharedString>,
    /// The button's variant (`default` for an action, `outline` for the
    /// cancel button).
    pub variant: Prop<Option<ButtonVariant>>,
    /// The button's size.
    #[prop(default)]
    pub size: Prop<ButtonSize>,
}

/// The events of [`AlertDialogAction`] and [`AlertDialogCancel`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum AlertDialogActionEvent {
    /// It was pressed; the alert dialog closes.
    Press,
}

fn action_button(
    props: AlertDialogActionProps,
    cx: Scope<AlertDialogActionEvent>,
    fallback: ButtonVariant,
    part: &str,
) -> crate::ButtonElement {
    let state = OpenContext::of(part);
    crate::ui::button::button()
        .label(move || Some(props.label.get()))
        .variant(move || props.variant.get().unwrap_or(fallback))
        .size(props.size)
        .on_press(move || {
            cx.emit(AlertDialogActionEvent::Press);
            state.set(false);
        })
}

/// shadcn's AlertDialogAction: a [`crate::Button`] (`default`) that
/// closes the alert dialog after its press.
#[component]
pub fn AlertDialogAction(
    props: AlertDialogActionProps,
    cx: Scope<AlertDialogActionEvent>,
) -> impl IntoElement {
    action_button(props, cx, ButtonVariant::Default, "AlertDialogAction")
}

/// shadcn's AlertDialogCancel: an `outline` [`crate::Button`] that closes
/// the alert dialog.
#[component]
pub fn AlertDialogCancel(
    props: AlertDialogActionProps,
    cx: Scope<AlertDialogActionEvent>,
) -> impl IntoElement {
    action_button(props, cx, ButtonVariant::Outline, "AlertDialogCancel")
}

impl AlertDialogContentElement {
    /// `.size(AlertDialogSize::Sm)`.
    pub fn sm(self) -> Self {
        self.size(AlertDialogSize::Sm)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<AlertDialog>();
    registry.register::<AlertDialogContent>();
    registry.register::<AlertDialogHeader>();
    registry.register::<AlertDialogFooter>();
    registry.register::<AlertDialogTitle>();
    registry.register::<AlertDialogDescription>();
    registry.register::<AlertDialogMedia>();
    registry.register::<AlertDialogAction>();
    registry.register::<AlertDialogCancel>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every class an alert dialog can draw with is one NSH knows.
    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for size in [AlertDialogSize::Default, AlertDialogSize::Sm] {
            for media in [false, true] {
                let classes = cn([
                    super::super::dialog::OVERLAY,
                    CONTENT,
                    size.content(),
                    size.header(media),
                    size.footer(),
                    size.title(media),
                    size.media(),
                    DESCRIPTION,
                    "size-8",
                ]);
                for class in crate::cn::unknown_classes(&classes) {
                    if !unknown.contains(&class) {
                        unknown.push(class);
                    }
                }
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
