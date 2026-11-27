//! [`ResizablePanelGroup`], [`ResizablePanel`] and the handles between
//! them, after shadcn/ui's `resizable.tsx` (new-york-v4), on `nsh_base`'s
//! `ResizablePanelGroup` and `ResizablePanel` (the drags, the panels'
//! sizes, the handles' accessibility).
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class or the parts differ:
//!
//! - The handles are the group's: `nsh_base` places one over each
//!   boundary between two panels, a band that takes the drag, and draws
//!   shadcn's `ResizableHandle` classes as its line; `withHandle` (the
//!   grip) is the group's `with_handle`. A handle takes no room of its
//!   own (shadcn's is a 1-point flex item).
//! - `aria-[orientation=vertical]:` / `aria-[orientation=horizontal]:` are
//!   the group's `orientation`, which the component knows.
//! - `after:` (the handle's wider hit area) is the band's own.
//! - `[&[aria-orientation=horizontal]>div]:rotate-90` turns the grip in a
//!   vertical group.
//! - `border` is `border border-border`.
//! - react-resizable-panels sizes panels in percents of the group:
//!   `default_size` is a percent, as theirs; `min_size` and `max_size` are
//!   points (`nsh_base`'s), `0` and none by default.

use nsh_base::ResizeHandleContext;
use nsh_core::geometry::Axis;
use nsh_core::prelude::*;

#[cfg(test)]
use crate::cn::cn;
use crate::icon::{IconName, icon};

/// The direction a group lays its panels out: shadcn's `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ResizableOrientation {
    /// Side by side, the handles upright.
    #[default]
    Horizontal,
    /// Stacked, the handles across.
    Vertical,
}

/// shadcn's `ResizablePanelGroup` classes.
pub(crate) fn group_classes(orientation: ResizableOrientation) -> &'static str {
    match orientation {
        ResizableOrientation::Horizontal => "flex h-full w-full",
        ResizableOrientation::Vertical => "flex h-full w-full flex-col",
    }
}

/// shadcn's `ResizableHandle` classes: the line on the boundary.
pub(crate) fn handle_classes(orientation: ResizableOrientation) -> &'static str {
    match orientation {
        ResizableOrientation::Horizontal => "relative flex w-px items-center justify-center bg-border",
        ResizableOrientation::Vertical => {
            "relative flex w-px items-center justify-center bg-border h-px w-full"
        }
    }
}

/// shadcn's grip: the `withHandle` box.
pub(crate) fn grip_classes(orientation: ResizableOrientation) -> &'static str {
    match orientation {
        ResizableOrientation::Horizontal => {
            "z-10 flex h-4 w-3 items-center justify-center rounded-xs border border-border bg-border"
        }
        ResizableOrientation::Vertical => {
            "z-10 flex h-4 w-3 items-center justify-center rounded-xs border border-border bg-border rotate-90"
        }
    }
}

/// The grip's icon.
pub(crate) const GRIP_ICON: &str = "size-2.5";

/// The props of [`ResizablePanelGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ResizablePanelGroupProps {
    /// `horizontal` (side by side) or `vertical` (stacked).
    #[prop(default)]
    pub orientation: Prop<ResizableOrientation>,
    /// A grip on each handle (shadcn's `withHandle`).
    #[prop(default)]
    pub with_handle: Prop<bool>,
    /// No drags.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The [`ResizablePanel`]s, its direct children.
    pub children: Prop<Children>,
}

/// The events of [`ResizablePanelGroup`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ResizablePanelGroupEvent {
    /// A drag ended: every panel's size along the group, in points.
    #[args(sizes)]
    Layout(Vec<Points>),
}

/// shadcn's ResizablePanelGroup: panels the user resizes by dragging the
/// handles between them.
#[component]
pub fn ResizablePanelGroup(
    props: ResizablePanelGroupProps,
    cx: Scope<ResizablePanelGroupEvent>,
) -> impl IntoElement {
    let orientation = props.orientation.get_untracked();
    let with_handle = props.with_handle;
    nsh_base::resizable_panel_group()
        .axis(match orientation {
            ResizableOrientation::Horizontal => Axis::Horizontal,
            ResizableOrientation::Vertical => Axis::Vertical,
        })
        .disabled(props.disabled)
        .handle_class(Some(SharedString::from(handle_classes(orientation))))
        .render_handle(move |_: &ResizeHandleContext| {
            dynamic(move || {
                with_handle.get().then(|| {
                    view()
                        .app_class(Some(SharedString::from(grip_classes(orientation))))
                        .child(
                            icon(IconName::GripVertical)
                                .app_class(Some(SharedString::from(GRIP_ICON))),
                        )
                        .into_element()
                })
            })
        })
        .on_resize(move |sizes: &Vec<Points>| cx.emit(ResizablePanelGroupEvent::Layout(sizes.clone())))
        .app_class(Some(SharedString::from(group_classes(orientation))))
        .child(props.children)
}

/// The props of [`ResizablePanel`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ResizablePanelProps {
    /// Its share of the group at first, in percent (`50.0`); the panels
    /// without one share what the others leave.
    pub default_size: Prop<Option<f32>>,
    /// The smallest it gets, in points (default 0).
    #[prop(default)]
    pub min_size: Prop<Points>,
    /// The largest it gets, in points.
    pub max_size: Prop<Option<Points>>,
    /// The content.
    pub children: Prop<Children>,
}

/// shadcn's ResizablePanel: a panel of a [`ResizablePanelGroup`].
#[component]
pub fn ResizablePanel(props: ResizablePanelProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    // A percent is a basis in proportion to the others': when every panel
    // has one, `nsh_base` grows them by their bases, so they share the
    // group in these proportions.
    nsh_base::resizable_panel()
        .size(move || props.default_size.get().map(px))
        .min_size(props.min_size)
        .max_size(props.max_size)
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry
        .register::<ResizablePanelGroup>()
        .register::<ResizablePanel>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for o in [ResizableOrientation::Horizontal, ResizableOrientation::Vertical] {
            let all = cn([group_classes(o), handle_classes(o), grip_classes(o), GRIP_ICON]);
            unknown.extend(crate::cn::unknown_classes(&all));
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
