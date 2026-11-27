//! [`Sheet`] and its parts, after shadcn/ui's `sheet.tsx` (new-york-v4): a
//! panel at an edge of the window, over a dimmed overlay.
//!
//! Behavior: the root keeps the open state and its `trigger` slot toggles
//! it, as `Dialog`'s; the content is `nsh_base::Sheet`, which slides the
//! panel in from its edge as the overlay fades in (and back out), keeps
//! screen readers and the keyboard focus to it as a modal, and closes on a
//! press on the overlay and on Escape.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border-l` / `-r` / `-t` / `-b` name their color (`border-border`).
//! - The `data-[state=…]:animate-* slide-* duration-*` classes are
//!   tw-animate-css's: the panel's slide and the overlay's fade are
//!   `nsh_base::Sheet`'s motion (`SheetMotion`), on its own timing rather
//!   than 500 ms in and 300 ms out.
//! - `SheetContent`'s box is the sheet's panel, which the core places: the
//!   app's classes for it (shadcn's `className`, `w-[400px]`) go in its
//!   `class` prop.
//! - The close button's `data-[state=open]:bg-secondary` never matches in
//!   shadcn (Radix's `Close` has no `data-state`) and is left out.
//! - `text-base` is added to the content: a portal's content inherits the
//!   page body's text (the browser's 16 px, on Tailwind's base line height
//!   1.5), which NSH's text, 14 points by default, does not have.

use nsh_core::prelude::*;

use super::dialog::{OVERLAY, close_button};
use super::open::{OpenContext, close, trigger};
use crate::cn::cn;

/// The edge a sheet is on: shadcn's `side`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SheetSide {
    /// Across the top.
    Top,
    /// Down the right side.
    #[default]
    Right,
    /// Across the bottom.
    Bottom,
    /// Down the left side.
    Left,
}

impl SheetSide {
    pub(crate) fn placement(self) -> Placement {
        match self {
            SheetSide::Top => Placement::Top,
            SheetSide::Right => Placement::Right,
            SheetSide::Bottom => Placement::Bottom,
            SheetSide::Left => Placement::Left,
        }
    }

    fn classes(self) -> &'static str {
        match self {
            SheetSide::Right => {
                "inset-y-0 right-0 h-full w-3/4 border-l border-border sm:max-w-sm"
            }
            SheetSide::Left => "inset-y-0 left-0 h-full w-3/4 border-r border-border sm:max-w-sm",
            SheetSide::Top => "inset-x-0 top-0 h-auto border-b border-border",
            SheetSide::Bottom => "inset-x-0 bottom-0 h-auto border-t border-border",
        }
    }
}

const CONTENT: &str = "fixed z-50 flex flex-col gap-4 bg-background shadow-lg transition ease-in-out \
    text-base";

const CLOSE: &str = "absolute top-4 right-4 rounded-xs opacity-70 ring-offset-background \
    transition-opacity hover:opacity-100 focus:ring-2 focus:ring-ring focus:ring-offset-2 \
    focus:outline-hidden disabled:pointer-events-none";

const HEADER: &str = "flex flex-col gap-1.5 p-4";
const FOOTER: &str = "mt-auto flex flex-col gap-2 p-4";
const TITLE: &str = "font-semibold text-foreground";
const DESCRIPTION: &str = "text-sm text-muted-foreground";

/// The panel of a sheet or a drawer while `state` is open: `nsh_base::Sheet`
/// on `side` over shadcn's overlay, the panel drawn with `classes`, `before`
/// at its top (a drawer's handle) and `after` at its end (the close
/// button).
pub(crate) fn panel(
    state: OpenContext,
    side: impl Fn() -> Placement + 'static,
    classes: impl Fn() -> SharedString + 'static,
    swipe: bool,
    content: Prop<Children>,
    before: impl Fn() -> Option<AnyElement> + 'static,
    after: impl Fn() -> Option<AnyElement> + 'static,
) -> AnyElement {
    let shown = state.clone();
    let title = state.title;
    nsh_base::sheet()
        .open(move || Some(shown.get()))
        .side(side)
        .close_on_swipe(swipe)
        .accessibility_label(move || title.get())
        .backdrop_class(Some(SharedString::from(OVERLAY)))
        .popup_class(move || Some(classes()))
        .on_open_change(move |e: &nsh_base::SheetEvent| {
            if let nsh_base::SheetEvent::OpenChange(open, _) = e {
                state.set(*open)
            }
        })
        .child(dynamic(before))
        .child(content)
        .child(dynamic(after))
        .into_element()
}

/// The props of [`Sheet`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SheetProps {
    /// Open (controlled): the app owns the value and changes it in
    /// `on_open_change`. Without it the sheet keeps its own state.
    pub open: Prop<Option<bool>>,
    /// Open at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// What opens it (shadcn's `SheetTrigger asChild`).
    pub trigger: Prop<Option<Slot>>,
    /// The [`SheetContent`].
    pub children: Prop<Children>,
}

/// The events of a [`Sheet`] (and a drawer).
#[derive(Debug, Clone, PartialEq, Event)]
pub enum SheetEvent {
    /// It asks to open or close (the trigger, the overlay, Escape, a close
    /// button): set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's Sheet: a panel that slides in from an edge of the window.
#[component]
pub fn Sheet(props: SheetProps, cx: Scope<SheetEvent>) -> impl IntoElement {
    let state = OpenContext::new(props.open, props.default_open, move |open| {
        cx.emit(SheetEvent::OpenChange(open))
    })
    .provide();
    view()
        .child(trigger(props.trigger, state))
        .child(props.children)
}

/// The props of [`SheetContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SheetContentProps {
    /// `top`, `right`, `bottom` or `left`.
    #[prop(default)]
    pub side: Prop<SheetSide>,
    /// The close button in the top right corner (default true).
    #[prop(default = true)]
    pub show_close_button: Prop<bool>,
    /// The app's classes for the panel, after shadcn's (its `className`).
    pub class: Prop<Option<SharedString>>,
    /// The header, the body and the footer, made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// shadcn's SheetContent: the panel.
#[component]
pub fn SheetContent(props: SheetContentProps) -> impl IntoElement {
    let state = OpenContext::of("SheetContent");
    let closer = state.clone();
    panel(
        state,
        move || props.side.get().placement(),
        move || {
            let class = props.class.get();
            cn([CONTENT, props.side.get().classes(), class.as_deref().unwrap_or("")])
        },
        false,
        props.children,
        || None,
        move || {
            props
                .show_close_button
                .get()
                .then(|| close_button(CLOSE, closer.clone()).into_element())
        },
    )
}

/// The props of the parts that hold others ([`SheetHeader`],
/// [`SheetFooter`]).
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SheetHeaderProps {
    /// What it holds.
    pub children: Prop<Children>,
}

/// shadcn's SheetHeader: the title over the description.
#[component]
pub fn SheetHeader(props: SheetHeaderProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(HEADER))).child(props.children)
}

/// shadcn's SheetFooter: the actions, stacked at the bottom of the panel.
#[component]
pub fn SheetFooter(props: SheetHeaderProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(FOOTER))).child(props.children)
}

/// The props of [`SheetTitle`] and [`SheetDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SheetTitleProps {
    /// What it says.
    pub text: Prop<SharedString>,
}

/// shadcn's SheetTitle: it names the sheet for screen readers too.
#[component]
pub fn SheetTitle(props: SheetTitleProps) -> impl IntoElement {
    OpenContext::of("SheetTitle").register_title(move || Some(props.text.get()));
    text(props.text).app_class(Some(SharedString::from(TITLE)))
}

/// shadcn's SheetDescription.
#[component]
pub fn SheetDescription(props: SheetTitleProps) -> impl IntoElement {
    OpenContext::of("SheetDescription").register_description(move || Some(props.text.get()));
    text(props.text).app_class(Some(SharedString::from(DESCRIPTION)))
}

/// The props of [`SheetClose`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SheetCloseProps {
    /// The app's element that closes the sheet when pressed (shadcn's
    /// `SheetClose asChild`).
    pub button: Prop<Option<Slot>>,
}

/// shadcn's SheetClose: pressing its button closes the sheet.
#[component]
pub fn SheetClose(props: SheetCloseProps) -> impl IntoElement {
    view().child(close(props.button, "SheetClose"))
}

impl SheetContentElement {
    /// `.side(SheetSide::Top)`.
    pub fn top(self) -> Self {
        self.side(SheetSide::Top)
    }
    /// `.side(SheetSide::Bottom)`.
    pub fn bottom(self) -> Self {
        self.side(SheetSide::Bottom)
    }
    /// `.side(SheetSide::Left)`.
    pub fn left(self) -> Self {
        self.side(SheetSide::Left)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Sheet>();
    registry.register::<SheetContent>();
    registry.register::<SheetHeader>();
    registry.register::<SheetFooter>();
    registry.register::<SheetTitle>();
    registry.register::<SheetDescription>();
    registry.register::<SheetClose>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every class a sheet can draw with is one NSH knows.
    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for side in [SheetSide::Top, SheetSide::Right, SheetSide::Bottom, SheetSide::Left] {
            let classes = cn([
                OVERLAY,
                CONTENT,
                side.classes(),
                CLOSE,
                HEADER,
                FOOTER,
                TITLE,
                DESCRIPTION,
            ]);
            for class in crate::cn::unknown_classes(&classes) {
                if !unknown.contains(&class) {
                    unknown.push(class);
                }
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
