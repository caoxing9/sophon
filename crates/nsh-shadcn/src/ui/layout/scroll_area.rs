//! [`ScrollArea`] and [`ScrollBar`], after shadcn/ui's `scroll-area.tsx`
//! (new-york-v4): a native scroll view with `nsh_base`'s `Scrollbar` (the
//! thumb's length and place, dragging it, pressing the track, when it
//! shows) in place of the platform's indicator.
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class:
//!
//! - Radix places the bar with inline styles (`position: absolute; top: 0;
//!   right: 0; bottom: 0`, `overflow: hidden` on the root); those are
//!   classes here (`absolute inset-y-0 right-0`, `overflow-hidden`).
//! - The viewport's `rounded-[inherit]` is the root's clip.
//! - A scroll view scrolls along one axis: shadcn's `<ScrollBar
//!   orientation="horizontal" />` child is the area's
//!   `orientation="horizontal"`, which scrolls sideways and draws that
//!   bar. The area draws its bar itself; the app adds none.
//! - Radix's `type` is `scroll_type`; `hover` (the default) shows the bar
//!   while the pointer is over the area or it scrolls.
//! - `transition-[color,box-shadow]` (NSH has no arbitrary transition
//!   properties) is `transition`, which animates colors and shadows.
//! - `touch-none` is left out: the bar claims the touches on it itself.
//! - Radix's thumb is at least 18 points long (`min_thumb_length`).

use nsh_base::{ScrollbarMode, ScrollbarTarget};
use nsh_core::geometry::Axis;
use nsh_core::prelude::*;

use crate::cn::cn;

/// The direction an area scrolls: its bar's `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ScrollAreaOrientation {
    /// Up and down, the bar on the right.
    #[default]
    Vertical,
    /// Sideways, the bar at the bottom.
    Horizontal,
}

/// When the bar shows: Radix's `type`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ScrollAreaType {
    /// While the content overflows.
    Auto,
    /// Always, while the content overflows.
    Always,
    /// While it scrolls.
    Scroll,
    /// While the pointer is over the area, and while it scrolls.
    #[default]
    Hover,
}

/// shadcn's root classes, and the clip Radix's inline style sets.
pub(crate) const ROOT: &str = "relative overflow-hidden";
/// shadcn's viewport classes.
pub(crate) const VIEWPORT: &str = "size-full transition outline-none \
    focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1";
/// shadcn's thumb classes.
pub(crate) const THUMB: &str = "relative flex-1 rounded-full bg-border";

/// shadcn's `ScrollBar` classes, and the place Radix's inline style sets.
pub(crate) fn bar_classes(orientation: ScrollAreaOrientation) -> SharedString {
    cn([
        "flex p-px transition-colors select-none",
        match orientation {
            ScrollAreaOrientation::Vertical => {
                "h-full w-2.5 border-l border-l-transparent absolute inset-y-0 right-0"
            }
            ScrollAreaOrientation::Horizontal => {
                "h-2.5 flex-col border-t border-t-transparent absolute inset-x-0 bottom-0"
            }
        },
    ])
}

/// What a [`ScrollBar`] finds of its [`ScrollArea`].
#[derive(Clone, Copy)]
struct AreaContext {
    target: ElementRef,
    kind: Prop<ScrollAreaType>,
    hovered: Signal<bool>,
}

/// The props of [`ScrollArea`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ScrollAreaProps {
    /// `vertical` (the default) or `horizontal`.
    #[prop(default)]
    pub orientation: Prop<ScrollAreaOrientation>,
    /// When the bar shows: `hover` (the default), `scroll`, `auto` or
    /// `always` (Radix's `type`).
    #[prop(default)]
    pub scroll_type: Prop<ScrollAreaType>,
    /// What scrolls.
    pub children: Prop<Children>,
}

/// shadcn's ScrollArea: content that scrolls, with a thin bar of
/// shadcn's look.
#[component]
pub fn ScrollArea(props: ScrollAreaProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let target = element_ref();
    let hovered = signal(false);
    let orientation = props.orientation.get_untracked();
    provide_context(AreaContext {
        target,
        kind: props.scroll_type,
        hovered,
    });
    view()
        .app_class(Some(SharedString::from(ROOT)))
        .on_mouse_enter(move |_: &MouseEvent| hovered.set(true))
        .on_mouse_leave(move |_: &MouseEvent| hovered.set(false))
        .child(
            scroll()
                .element_ref(&target)
                .scrollbar(false)
                .when(orientation == ScrollAreaOrientation::Horizontal, |s| s.horizontal())
                .app_class(Some(SharedString::from(VIEWPORT)))
                .child(props.children),
        )
        .child(scroll_bar().orientation(orientation))
}

/// The props of [`ScrollBar`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ScrollBarProps {
    /// `vertical` (on the right) or `horizontal` (at the bottom).
    #[prop(default)]
    pub orientation: Prop<ScrollAreaOrientation>,
}

/// shadcn's ScrollBar: the bar of its [`ScrollArea`], which draws it.
#[component]
pub fn ScrollBar(props: ScrollBarProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let area = use_context::<AreaContext>();
    let orientation = props.orientation.get_untracked();
    let target = area.map_or(ScrollbarTarget::None, |a| ScrollbarTarget::Ref(a.target));
    nsh_base::scrollbar()
        .scroll(target)
        .axis(match orientation {
            ScrollAreaOrientation::Vertical => Axis::Vertical,
            ScrollAreaOrientation::Horizontal => Axis::Horizontal,
        })
        .mode(move || match area {
            None => ScrollbarMode::Scrolling,
            Some(a) => match a.kind.get() {
                ScrollAreaType::Auto | ScrollAreaType::Always => ScrollbarMode::Always,
                ScrollAreaType::Scroll => ScrollbarMode::Scrolling,
                ScrollAreaType::Hover if a.hovered.get() => ScrollbarMode::Always,
                ScrollAreaType::Hover => ScrollbarMode::Scrolling,
            },
        })
        .min_thumb_length(px(18.0))
        .thumb_class(Some(SharedString::from(THUMB)))
        .app_class(Some(bar_classes(orientation)))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<ScrollArea>().register::<ScrollBar>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = vec![
            SharedString::from(ROOT),
            SharedString::from(VIEWPORT),
            SharedString::from(THUMB),
        ];
        all.push(bar_classes(ScrollAreaOrientation::Vertical));
        all.push(bar_classes(ScrollAreaOrientation::Horizontal));
        let unknown: Vec<_> = all.iter().flat_map(|c| crate::cn::unknown_classes(c)).collect();
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
