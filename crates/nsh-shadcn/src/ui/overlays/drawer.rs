//! [`Drawer`] and its parts, after shadcn/ui's `drawer.tsx` (new-york-v4),
//! which wraps vaul: a panel at an edge (the bottom by default) with a
//! handle, dragged down to close.
//!
//! Behavior: the root keeps the open state and its `trigger` slot toggles
//! it; the content is `nsh_base::Sheet` (as [`super::Sheet`]'s), with
//! `close_on_swipe`: a bottom drawer follows the pointer or finger down and
//! closes past 30% of its height or on a flick, as vaul's does.
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border-t` / `-b` / `-l` / `-r` name their color (`border-border`).
//! - `data-[vaul-drawer-direction=…]:` and
//!   `group-data-[vaul-drawer-direction=…]/drawer-content:` select on the
//!   direction: the parts pick those classes from the content's `direction`
//!   (through context for the header and the handle). The handle's
//!   `hidden … group-data-[…=bottom]:block` is: drawn only at the bottom.
//!   The header's `md:text-left` is left out at the top and bottom, where
//!   Tailwind sorts `group-data-[…]:text-center` after it (it wins in
//!   shadcn at every width); NSH sorts breakpoints last.
//! - `max-h-[80vh]` is `max-h-[80%]`: NSH has no viewport units, and the
//!   panel is fixed to the window, whose height `%` measures.
//! - vaul's slide is `nsh_base::Sheet`'s motion (`SheetMotion`), its
//!   overlay's fade the sheet's backdrop fade.
//! - `DrawerContent`'s box is the sheet's panel, which the core places: the
//!   app's classes for it go in its `class` prop.
//! - `text-base` is added to the content: a portal's content inherits the
//!   page body's text (the browser's 16 px, on Tailwind's base line height
//!   1.5), which NSH's text, 14 points by default, does not have.

use nsh_core::prelude::*;

use super::open::{OpenContext, close, trigger};
use super::sheet::panel;
use crate::cn::cn;

/// The edge a drawer is on: vaul's `direction`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum DrawerDirection {
    /// Across the top.
    Top,
    /// Down the right side.
    Right,
    /// Across the bottom, with a handle.
    #[default]
    Bottom,
    /// Down the left side.
    Left,
}

impl DrawerDirection {
    fn placement(self) -> Placement {
        match self {
            DrawerDirection::Top => Placement::Top,
            DrawerDirection::Right => Placement::Right,
            DrawerDirection::Bottom => Placement::Bottom,
            DrawerDirection::Left => Placement::Left,
        }
    }

    fn classes(self) -> &'static str {
        match self {
            DrawerDirection::Top => {
                "inset-x-0 top-0 mb-24 max-h-[80%] rounded-b-lg border-b border-border"
            }
            DrawerDirection::Bottom => {
                "inset-x-0 bottom-0 mt-24 max-h-[80%] rounded-t-lg border-t border-border"
            }
            DrawerDirection::Right => {
                "inset-y-0 right-0 w-3/4 border-l border-border sm:max-w-sm"
            }
            DrawerDirection::Left => "inset-y-0 left-0 w-3/4 border-r border-border sm:max-w-sm",
        }
    }

    fn header(self) -> &'static str {
        match self {
            // Tailwind sorts `group-data-*` after the breakpoints, so the
            // centered text wins over `md:text-left` at every width.
            DrawerDirection::Top | DrawerDirection::Bottom => {
                "flex flex-col gap-0.5 p-4 text-center md:gap-1.5"
            }
            _ => "flex flex-col gap-0.5 p-4 md:gap-1.5 md:text-left",
        }
    }
}

const CONTENT: &str = "group/drawer-content fixed z-50 flex h-auto flex-col bg-background text-base";
/// The handle: `hidden` but at the bottom, where it is `block`.
const HANDLE: &str = "mx-auto mt-4 h-2 w-[100px] shrink-0 rounded-full bg-muted";
const FOOTER: &str = "mt-auto flex flex-col gap-2 p-4";
const TITLE: &str = "font-semibold text-foreground";
const DESCRIPTION: &str = "text-sm text-muted-foreground";

/// The content's direction, which its header reads.
#[derive(Clone, Copy)]
struct Direction(Prop<DrawerDirection>);

/// The props of [`Drawer`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DrawerProps {
    /// Open (controlled): the app owns the value and changes it in
    /// `on_open_change`. Without it the drawer keeps its own state.
    pub open: Prop<Option<bool>>,
    /// Open at first, while `open` is not set.
    #[prop(default)]
    pub default_open: Prop<bool>,
    /// What opens it (shadcn's `DrawerTrigger asChild`).
    pub trigger: Prop<Option<Slot>>,
    /// The [`DrawerContent`].
    pub children: Prop<Children>,
}

/// The events of a [`Drawer`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum DrawerEvent {
    /// It asks to open or close (the trigger, the overlay, a drag, Escape,
    /// a close button): set `open` to follow it.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's Drawer: a panel with a handle that slides up from the bottom.
#[component]
pub fn Drawer(props: DrawerProps, cx: Scope<DrawerEvent>) -> impl IntoElement {
    let state = OpenContext::new(props.open, props.default_open, move |open| {
        cx.emit(DrawerEvent::OpenChange(open))
    })
    .provide();
    view()
        .child(trigger(props.trigger, state))
        .child(props.children)
}

/// The props of [`DrawerContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DrawerContentProps {
    /// `bottom`, `top`, `left` or `right` (vaul's `direction`, which shadcn
    /// gives the root).
    #[prop(default)]
    pub direction: Prop<DrawerDirection>,
    /// The app's classes for the panel, after shadcn's (its `className`).
    pub class: Prop<Option<SharedString>>,
    /// What it shows, made on each opening.
    #[prop(lazy)]
    pub children: Prop<Children>,
}

/// shadcn's DrawerContent: the panel, its handle at the top.
#[component]
pub fn DrawerContent(props: DrawerContentProps) -> impl IntoElement {
    let state = OpenContext::of("DrawerContent");
    provide_context(Direction(props.direction));
    panel(
        state,
        move || props.direction.get().placement(),
        move || {
            let class = props.class.get();
            cn([CONTENT, props.direction.get().classes(), class.as_deref().unwrap_or("")])
        },
        true,
        props.children,
        move || {
            (props.direction.get() == DrawerDirection::Bottom).then(|| {
                view()
                    .app_class(Some(SharedString::from(HANDLE)))
                    .into_element()
            })
        },
        || None,
    )
}

/// The props of the parts that hold others ([`DrawerHeader`],
/// [`DrawerFooter`]).
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DrawerHeaderProps {
    /// What it holds.
    pub children: Prop<Children>,
}

/// shadcn's DrawerHeader: the title over the description, centered at the
/// top and bottom below the `md:` breakpoint.
#[component]
pub fn DrawerHeader(props: DrawerHeaderProps) -> impl IntoElement {
    let direction = use_context::<Direction>();
    view()
        .app_class(move || {
            let direction = direction.map(|d| d.0.get()).unwrap_or_default();
            Some(SharedString::from(direction.header()))
        })
        .child(props.children)
}

/// shadcn's DrawerFooter: the actions, stacked at the end of the panel.
#[component]
pub fn DrawerFooter(props: DrawerHeaderProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(FOOTER))).child(props.children)
}

/// The props of [`DrawerTitle`] and [`DrawerDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DrawerTitleProps {
    /// What it says.
    pub text: Prop<SharedString>,
}

/// shadcn's DrawerTitle: it names the drawer for screen readers too.
#[component]
pub fn DrawerTitle(props: DrawerTitleProps) -> impl IntoElement {
    OpenContext::of("DrawerTitle").register_title(move || Some(props.text.get()));
    text(props.text).app_class(Some(SharedString::from(TITLE)))
}

/// shadcn's DrawerDescription.
#[component]
pub fn DrawerDescription(props: DrawerTitleProps) -> impl IntoElement {
    OpenContext::of("DrawerDescription").register_description(move || Some(props.text.get()));
    text(props.text).app_class(Some(SharedString::from(DESCRIPTION)))
}

/// The props of [`DrawerClose`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct DrawerCloseProps {
    /// The app's element that closes the drawer when pressed (shadcn's
    /// `DrawerClose asChild`).
    pub button: Prop<Option<Slot>>,
}

/// shadcn's DrawerClose: pressing its button closes the drawer.
#[component]
pub fn DrawerClose(props: DrawerCloseProps) -> impl IntoElement {
    view().child(close(props.button, "DrawerClose"))
}

impl DrawerContentElement {
    /// `.direction(DrawerDirection::Top)`.
    pub fn top(self) -> Self {
        self.direction(DrawerDirection::Top)
    }
    /// `.direction(DrawerDirection::Left)`.
    pub fn left(self) -> Self {
        self.direction(DrawerDirection::Left)
    }
    /// `.direction(DrawerDirection::Right)`.
    pub fn right(self) -> Self {
        self.direction(DrawerDirection::Right)
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Drawer>();
    registry.register::<DrawerContent>();
    registry.register::<DrawerHeader>();
    registry.register::<DrawerFooter>();
    registry.register::<DrawerTitle>();
    registry.register::<DrawerDescription>();
    registry.register::<DrawerClose>();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every class a drawer can draw with is one NSH knows.
    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for direction in [
            DrawerDirection::Top,
            DrawerDirection::Right,
            DrawerDirection::Bottom,
            DrawerDirection::Left,
        ] {
            let classes = cn([
                super::super::dialog::OVERLAY,
                CONTENT,
                direction.classes(),
                direction.header(),
                HANDLE,
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
