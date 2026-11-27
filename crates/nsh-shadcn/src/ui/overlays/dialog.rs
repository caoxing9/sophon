//! [`Dialog`] and its parts, after shadcn/ui's `dialog.tsx` (new-york-v4).
//!
//! Behavior: the root keeps the open state (controlled with `open`, or its
//! own from `default_open`); its `trigger` slot toggles it (Radix's
//! `DialogTrigger asChild`); the content is presented in `nsh_base::dialog()`,
//! the core's modal layer, which keeps screen readers and the keyboard focus
//! to it and hears Escape. A press on the overlay, Escape, [`DialogClose`]
//! and the close button close it.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border` is `border border-border` (NSH has no global border color).
//! - The `data-[state=open|closed]:animate-in / animate-out fade-* zoom-*`
//!   classes and `duration-200` are tw-animate-css keyframes, which NSH does
//!   not have: the modal layer fades in and out as a whole
//!   (`ModalAnimation::Fade`), overlay and content together, without the
//!   content's `zoom-in-95`.
//! - `max-w-[calc(100%-2rem)]`: NSH has no `calc()` in sizes; at the `sm:`
//!   breakpoint and above (640 points) `sm:max-w-lg` takes its place anyway.
//! - `fixed top-[50%] left-[50%] translate-x-[-50%] translate-y-[-50%]`: the
//!   content is centered by a `fixed inset-0 flex items-center
//!   justify-center p-4` box around it instead. NSH's translate moves what
//!   is drawn, not the laid-out box that presses reach, so the lower half
//!   of a dialog taller than half the window was drawn but not pressable.
//! - The close button's `[&_svg]:…` size its icon: the icon takes `size-4`.
//!   Its `data-[state=open]:…` classes never match in shadcn (Radix's
//!   `Close` has no `data-state`) and are left out.
//! - `DialogOverlay` is drawn by the root, under the content, rather than by
//!   `DialogContent`: the content is the modal's own part, so the app's
//!   classes on it (`.app_class("sm:max-w-[425px]")`) style the card.
//! - `fixed` boxes are fixed to the modal layer, which covers the window.
//! - `text-base` is added to the content: a portal's content inherits the
//!   page body's text (the browser's 16 px, on Tailwind's base line height
//!   1.5), which NSH's text, 14 points by default, does not have.

use nsh_core::prelude::*;

use super::open::{OpenContext, close, trigger};
#[cfg(test)]
use crate::cn::cn;
use crate::icon::{IconName, icon};

/// shadcn's `DialogOverlay`.
pub(crate) const OVERLAY: &str = "fixed inset-0 z-50 bg-black/50";

/// shadcn's `DialogContent`, without its animation classes and centered
/// by the modal's [`CENTER`] rather than by `fixed top-[50%] left-[50%]
/// translate-x-[-50%] translate-y-[-50%]`.
const CONTENT: &str = "relative z-50 grid w-full \
    gap-4 rounded-lg border border-border bg-background p-6 \
    shadow-lg outline-none sm:max-w-lg text-base";

/// What holds a dialog's content in the middle of the window, over the
/// overlay. NSH's translate moves what is drawn, not the box that is laid
/// out and pressed: shadcn's `translate-[-50%]` centering left the lower
/// half of a dialog taller than half the window out of it, drawn but not
/// pressable. A flex box centers the box itself.
pub(crate) const CENTER: &str = "fixed inset-0 z-50 flex items-center justify-center p-4";

/// The close button in the corner of `DialogContent` and `SheetContent`.
pub(crate) const CLOSE: &str = "absolute top-4 right-4 rounded-xs opacity-70 \
    ring-offset-background transition-opacity hover:opacity-100 focus:ring-2 focus:ring-ring \
    focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none";

const HEADER: &str = "flex flex-col gap-2 text-center sm:text-left";
const FOOTER: &str = "flex flex-col-reverse gap-2 sm:flex-row sm:justify-end";
const TITLE: &str = "text-lg leading-none font-semibold";
const DESCRIPTION: &str = "text-sm text-muted-foreground";

/// The modal of a dialog or an alert dialog while `state` is open: the
/// overlay and `content` over the window. A press on the overlay closes
/// it when `overlay_closes`; Escape always does.
pub(crate) fn modal(
    state: OpenContext,
    role: AccessibilityRole,
    overlay_closes: bool,
    content: Prop<Children>,
) -> AnyElement {
    let shown = state.clone();
    when(
        move || shown.get(),
        move || {
            let title = state.title;
            let description = state.description;
            let cancel = state.clone();
            let press = state.clone();
            nsh_base::dialog()
                .role(role)
                .animation(ModalAnimation::Fade)
                .label(move || title.get().unwrap_or_default())
                .accessibility_description(move || description.get().unwrap_or_default())
                .close_on_escape(false)
                .on_cancel(move || cancel.set(false))
                .flex_1()
                .child(
                    view()
                        .app_class(Some(SharedString::from(OVERLAY)))
                        .focusable(false)
                        .on_press(move || {
                            if overlay_closes {
                                press.set(false)
                            }
                        }),
                )
                .child(
                    view()
                        .app_class(Some(SharedString::from(CENTER)))
                        .pointer_events(PointerEvents::BoxNone)
                        .child(content),
                )
        },
    )
    .into_element()
}

/// The props of [`Dialog`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DialogProps {
    /// Open (controlled): the app owns the value and changes it in
    /// `on_open_change`. Without it the dialog keeps its own state.
    pub open: Prop<Option<bool>>,
    /// Open at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// What opens it (shadcn's `DialogTrigger asChild`): pressing the app's
    /// element toggles the dialog.
    pub trigger: Prop<Option<Slot>>,
    /// The [`DialogContent`], made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// The events of a [`Dialog`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum DialogEvent {
    /// It asks to open or close (the trigger, the overlay, Escape, a close
    /// button): set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's Dialog: a window over the page, over a dimmed overlay.
#[component]
pub fn Dialog(props: DialogProps, cx: Scope<DialogEvent>) -> impl IntoElement {
    let state = OpenContext::new(props.open, props.default_open, move |open| {
        cx.emit(DialogEvent::OpenChange(open))
    })
    .provide();
    view()
        .child(trigger(props.trigger, state.clone()))
        .child(modal(state, AccessibilityRole::Dialog, true, props.children))
}

/// The props of [`DialogContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DialogContentProps {
    /// The close button in the top right corner (default true).
    #[prop(default = true)]
    pub show_close_button: Prop<bool>,
    /// The header, the body and the footer.
    pub children: Prop<Children>,
}

/// The close button of `DialogContent` and `SheetContent`: an `x`, "Close"
/// to screen readers.
pub(crate) fn close_button(classes: &'static str, state: OpenContext) -> View {
    view()
        .role(AccessibilityRole::Button)
        .label("Close")
        .cursor_pointer()
        .app_class(Some(SharedString::from(classes)))
        .on_press(move || state.set(false))
        .child(icon(IconName::X).app_class(Some(SharedString::from("size-4"))))
}

/// shadcn's DialogContent: the card in the middle of the window.
#[component]
pub fn DialogContent(props: DialogContentProps) -> impl IntoElement {
    let state = OpenContext::of("DialogContent");
    view()
        .app_class(Some(SharedString::from(CONTENT)))
        .child(props.children)
        .child(when(
            move || props.show_close_button.get(),
            move || close_button(CLOSE, state.clone()),
        ))
}

/// The props of [`DialogHeader`] and [`DialogFooter`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DialogHeaderProps {
    /// The title and the description.
    pub children: Prop<Children>,
}

/// shadcn's DialogHeader: the title over the description.
#[component]
pub fn DialogHeader(props: DialogHeaderProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(HEADER))).child(props.children)
}

/// The props of [`DialogFooter`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DialogFooterProps {
    /// An outline "Close" button after the children (default false).
    #[prop(default)]
    pub show_close_button: Prop<bool>,
    /// The actions.
    pub children: Prop<Children>,
}

/// shadcn's DialogFooter: the actions, at the end of a row from the `sm:`
/// breakpoint, stacked (the first at the bottom) below it.
#[component]
pub fn DialogFooter(props: DialogFooterProps) -> impl IntoElement {
    let state = OpenContext::of("DialogFooter");
    view()
        .app_class(Some(SharedString::from(FOOTER)))
        .child(props.children)
        .child(when(
            move || props.show_close_button.get(),
            move || {
                let state = state.clone();
                crate::button("Close").outline().on_press(move || state.set(false))
            },
        ))
}

/// The props of [`DialogTitle`] and [`DialogDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DialogTitleProps {
    /// What it says.
    pub text: Prop<SharedString>,
}

/// shadcn's DialogTitle: it names the dialog for screen readers too.
#[component]
pub fn DialogTitle(props: DialogTitleProps) -> impl IntoElement {
    OpenContext::of("DialogTitle").register_title(move || Some(props.text.get()));
    text(props.text).app_class(Some(SharedString::from(TITLE)))
}

/// shadcn's DialogDescription: muted text under the title, the dialog's
/// description for screen readers.
#[component]
pub fn DialogDescription(props: DialogTitleProps) -> impl IntoElement {
    OpenContext::of("DialogDescription").register_description(move || Some(props.text.get()));
    text(props.text).app_class(Some(SharedString::from(DESCRIPTION)))
}

/// The props of [`DialogClose`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DialogCloseProps {
    /// The app's element that closes the dialog when pressed (shadcn's
    /// `DialogClose asChild`): a button.
    pub button: Prop<Option<Slot>>,
}

/// shadcn's DialogClose: pressing its button closes the dialog.
#[component]
pub fn DialogClose(props: DialogCloseProps) -> impl IntoElement {
    view().child(close(props.button, "DialogClose"))
}

/// Every class a dialog draws with, for the unit tests.
#[cfg(test)]
pub(crate) fn all_classes() -> SharedString {
    cn([OVERLAY, CONTENT, CLOSE, HEADER, FOOTER, TITLE, DESCRIPTION, "size-4"])
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Dialog>();
    registry.register::<DialogContent>();
    registry.register::<DialogHeader>();
    registry.register::<DialogFooter>();
    registry.register::<DialogTitle>();
    registry.register::<DialogDescription>();
    registry.register::<DialogClose>();
}

#[cfg(test)]
mod tests {
    /// Every class a dialog can draw with is one NSH knows.
    #[test]
    fn every_class_is_known_to_nsh() {
        let unknown = crate::cn::unknown_classes(&super::all_classes());
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
