//! [`MessageScrollerProvider`], [`MessageScroller`],
//! [`MessageScrollerViewport`], [`MessageScrollerContent`],
//! [`MessageScrollerItem`] and [`MessageScrollerButton`], after shadcn/ui's
//! `message-scroller.tsx` (new-york-v4) and the headless
//! `@shadcn/react/message-scroller` it wraps; [`use_message_scroller`] is
//! its `useMessageScroller`, `useMessageScrollerScrollable` and
//! `useMessageScrollerVisibility` in one handle.
//!
//! # Behavior
//!
//! shadcn's scroller is a `div` that scrolls, its rows real DOM nodes
//! measured on every change. Here the transcript is NSH's virtualized list
//! (its rows are the app's `virtual_list`, given to
//! [`MessageScrollerContent`]'s `items`), which lays out and recycles the
//! rows natively, keeps the rows on screen in place when older ones are
//! prepended, and scrolls to a row or to the end exactly while rows are
//! measured. On it, this reproduces the primitive's rules:
//!
//! - It opens at the provider's `default_scroll_position`: the end (the
//!   default), the start, or the last anchored turn (the end when that
//!   turn fits).
//! - A row appended among the `scroll_anchors` (the user's message: a new
//!   turn) is scrolled to the top of the viewport, `scroll_previous_item_peek`
//!   (64) points of the turn before it left showing above it, and a tail
//!   spacer under the last row reserves the room for that: the reply then
//!   streams into the space below the turn, and nothing moves.
//! - With `auto_scroll`, while the reader is at the end (within
//!   `scroll_edge_threshold`, 8 points), it follows the end as the content
//!   grows (a reply streaming in, the viewport shrinking); a reply that
//!   fills the tail spacer hands the anchored turn over to following.
//!   Scrolling up leaves the reader where they are; back at the end, it
//!   follows again. Without `auto_scroll` nothing follows. The viewport
//!   resizing (a composer growing) holds the anchored turn in place.
//! - [`MessageScrollerButton`] shows while there is content past its edge,
//!   and scrolls there (following again, with `auto_scroll`).
//! - [`use_message_scroller`] scrolls to the end, the start or a message,
//!   and tells which edges can be scrolled toward and which messages show.
//!
//! # Classes
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class:
//!
//! - The provider renders no element in shadcn; here it is a view (NSH has
//!   no `display: contents`): a column filling its parent
//!   (`flex size-full min-h-0 min-w-0 flex-col`).
//! - The viewport (`overflow-y-auto` in shadcn) is a view around the
//!   content, and the content (`flex h-max min-h-full flex-col gap-8`) the
//!   list that scrolls: its `gap` (32 points) and `padding` are props, a
//!   list's row gap and padding being its layout's, not classes.
//!   `scrollbar-thin`, `scrollbar-gutter-stable`, `overscroll-contain`,
//!   `contain-content`, `data-autoscrolling:scrollbar-none` and
//!   `data-pending-scroll:invisible` are left out (the list draws the
//!   platform's overlay scroll bar, and positions itself before its first
//!   frame). `scroll-fade-b` is a mask on the viewport fading the last
//!   `min(12%, 40px)` while there is content below, as shadcn's.
//! - An item's `[content-visibility:auto]` and
//!   `[contain-intrinsic-size:auto_10rem]` are left out: the list
//!   virtualizes. Its `messageId` and `scrollAnchor` are the content's
//!   `message_ids` and `scroll_anchors` (the list asks for rows as it
//!   needs them, so it knows them from the content).
//! - The button's `data-[active=…]:…` and `data-[direction=…]:…` are
//!   `data-active:…` and the direction's classes (NSH has the boolean
//!   `data-*` form only); they are on a view around the [`crate::Button`]
//!   (the button is the library's component); its
//!   `transition-[translate,scale,opacity]` is `transition`;
//!   `data-[direction=start]:[&_svg]:rotate-180` is the `arrow-up` icon;
//!   its `sr-only` label is the button's accessibility label.
//! - `role="log"` and `aria-relevant="additions"` are a polite live region
//!   on the content; `role="region"` with the label "Messages" is the
//!   label of the viewport (NSH has no region role).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::IconName;
use crate::ui::button::{ButtonSize, ButtonVariant, button};

/// Where a transcript opens: shadcn's `defaultScrollPosition`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum MessageScrollerScrollPosition {
    /// At its first message.
    Start,
    /// At its last message.
    #[default]
    End,
    /// At its last anchored turn (often the user's last message), or at
    /// the end when that turn fits.
    LastAnchor,
}

/// Which edge a [`MessageScrollerButton`] scrolls to: shadcn's `direction`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum MessageScrollerButtonDirection {
    /// The first message, from the top of the viewport.
    Start,
    /// The last message, from the bottom of the viewport.
    #[default]
    End,
}

/// Where [`MessageScrollerHandle::scroll_to_message`] puts the message:
/// shadcn's `MessageScrollerScrollAlign`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum MessageScrollerScrollAlign {
    /// Its top at the top of the viewport.
    #[default]
    Start,
    /// Its middle in the middle.
    Center,
    /// Its bottom at the bottom.
    End,
    /// Where it is when it shows whole, else the nearest edge.
    Nearest,
}

/// Which edges the viewport can still scroll toward: shadcn's
/// `MessageScrollerScrollable`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MessageScrollerScrollable {
    /// There is content above.
    pub start: bool,
    /// There is content below.
    pub end: bool,
}

/// Which messages show: shadcn's `MessageScrollerVisibilityState`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageScrollerVisibility {
    /// The anchored turn the reader is in (the last anchor at or above the
    /// reading line), still set after it scrolled above the viewport.
    pub current_anchor_id: Option<SharedString>,
    /// The messages in the viewport (below the reading line), in order.
    pub visible_message_ids: Vec<SharedString>,
}

/// The height of a row not measured yet, as the list estimates it.
const ESTIMATED_ROW: f32 = 50.0;

/// What the viewport reports: where it is, how long the content is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Geometry {
    offset: f32,
    content: f32,
    viewport: f32,
}

/// The scroller's state, which its parts share.
#[derive(Clone, Copy)]
struct Scroller {
    list: ElementRef,
    auto_scroll: Signal<bool>,
    default_position: Signal<MessageScrollerScrollPosition>,
    edge_threshold: Signal<f32>,
    peek: Signal<f32>,
    margin: Signal<f32>,
    /// The content's layout: its gap and padding (set by the content).
    gap: Signal<f32>,
    padding: Signal<f32>,
    geometry: Signal<Geometry>,
    /// Following the end (with `auto_scroll`).
    following: Signal<bool>,
    /// The turn held at the reading line while its reply streams in.
    anchored: Signal<Option<SharedString>>,
    /// The rows' ids, and which of them are anchors.
    ids: Signal<Vec<SharedString>>,
    anchors: Signal<Vec<SharedString>>,
    /// How many rows the list has (from its cells while there are no ids).
    count: Signal<usize>,
    /// The rows' measured heights, by index (NaN: not measured).
    heights: Signal<Vec<f32>>,
    scrollable: Signal<MessageScrollerScrollable>,
    /// The first rows are in, and the opening position waits for the list
    /// to be laid out.
    opening: Signal<bool>,
}

impl Scroller {
    fn new() -> Self {
        Scroller {
            list: element_ref(),
            auto_scroll: signal(false),
            default_position: signal(MessageScrollerScrollPosition::End),
            edge_threshold: signal(8.0),
            peek: signal(64.0),
            margin: signal(0.0),
            gap: signal(32.0),
            padding: signal(0.0),
            geometry: signal(Geometry::default()),
            following: signal(false),
            anchored: signal(None),
            ids: signal(Vec::new()),
            anchors: signal(Vec::new()),
            count: signal(0),
            heights: signal(Vec::new()),
            scrollable: signal(MessageScrollerScrollable::default()),
            opening: signal(false),
        }
    }

    /// The scroller of the provider around the component being made, or
    /// a new one.
    fn current() -> Self {
        use_context::<Scroller>().unwrap_or_else(|| {
            let scroller = Scroller::new();
            provide_context(scroller);
            scroller
        })
    }

    fn index_of(self, id: &str) -> Option<usize> {
        self.ids.with_untracked(|ids| ids.iter().position(|i| i.as_ref() == id))
    }

    /// The height of row `index`, measured or estimated.
    fn height(heights: &[f32], index: usize) -> f32 {
        heights.get(index).copied().filter(|h| h.is_finite()).unwrap_or(ESTIMATED_ROW)
    }

    /// Where row `index` starts in the content.
    fn row_top(self, index: usize) -> f32 {
        let gap = self.gap.get_untracked();
        let tops: f32 = self
            .heights
            .with_untracked(|h| (0..index).map(|i| Self::height(h, i)).sum());
        self.padding.get_untracked() + tops + gap * index as f32
    }

    /// The tail spacer: the room under the last row that lets the anchored
    /// turn reach the reading line (shadcn's `getTailSpacerHeight`).
    fn spacer(self) -> f32 {
        let Some(anchor) = self.anchored.get() else {
            return 0.0;
        };
        let Some(at) = self.ids.with(|ids| ids.iter().position(|i| *i == anchor)) else {
            return 0.0;
        };
        let count = self.count.get();
        if at >= count {
            return 0.0;
        }
        let tail: f32 = self.heights.with(|h| (at..count).map(|i| Self::height(h, i)).sum());
        let tail = tail + self.gap.get() * (count - 1 - at) as f32 + self.padding.get();
        let room = self.geometry.with(|g| g.viewport) - self.peek.get() - self.margin.get() - tail;
        room.max(0.0).ceil()
    }

    fn scroll_to_end(self, animated: bool) {
        self.anchored.set(None);
        self.following.set(self.auto_scroll.get_untracked());
        self.list.scroll_to_end(animated);
    }

    fn scroll_to_start(self, animated: bool) {
        self.anchored.set(None);
        self.following.set(false);
        self.list.scroll_to_offset(px(0.0), animated);
    }

    /// Scrolls row `index` to the reading line, the previous one peeking
    /// above it when `peek`.
    fn scroll_to_row(self, index: usize, align: MessageScrollerScrollAlign, peek: bool, animated: bool) {
        self.following.set(false);
        let margin = self.margin.get_untracked() + if peek { self.peek.get_untracked() } else { 0.0 };
        let (position, offset) = match align {
            MessageScrollerScrollAlign::Start => (0.0, margin),
            MessageScrollerScrollAlign::Center => (0.5, 0.0),
            MessageScrollerScrollAlign::End => (1.0, -margin),
            MessageScrollerScrollAlign::Nearest => {
                let g = self.geometry.get_untracked();
                let top = self.row_top(index);
                let bottom = top + self.heights.with_untracked(|h| Self::height(h, index));
                if top < g.offset + margin {
                    (0.0, margin)
                } else if bottom > g.offset + g.viewport - margin {
                    (1.0, -margin)
                } else {
                    return;
                }
            }
        };
        self.list
            .scroll_to_index(index as u32, Percentage(position), px(offset), animated);
    }

    /// Hears the viewport: follows the end, and tells which edges can be
    /// scrolled to.
    fn watch(self) {
        watch_scroll(
            move || self.list.node(),
            move |e: &ScrollEvent| {
                let g = Geometry {
                    offset: e.offset.y.0,
                    content: e.content_size.height.0,
                    viewport: e.size.height.0,
                };
                let before = self.geometry.get_untracked();
                self.geometry.set(g);
                if self.opening.get_untracked() && g.viewport > 0.0 {
                    self.opening.set(false);
                    self.open();
                    return;
                }
                // The viewport resized (a composer growing, the window): hold
                // the anchored turn at the reading line, as shadcn's
                // `reanchorToAnchoredMessage` does.
                if g.viewport != before.viewport
                    && before.viewport > 0.0
                    && let Some(anchor) = self.anchored.get_untracked()
                    && let Some(index) = self.index_of(&anchor)
                    && untracked(|| self.spacer()) > 0.0
                {
                    self.scroll_to_row(index, MessageScrollerScrollAlign::Start, true, false);
                }
                let edge = self.edge_threshold.get_untracked();
                let at_end = g.offset >= (g.content - g.viewport).max(0.0) - edge;
                let auto = self.auto_scroll.get_untracked();
                if self.following.get_untracked() {
                    let scrolled_up = g.offset < before.offset - 0.5 && g.content >= before.content;
                    if scrolled_up || !auto {
                        self.following.set(false);
                    } else if !at_end {
                        // The content grew, or the viewport shrank: stay at the end.
                        self.list.scroll_to_end(false);
                    }
                } else if auto && at_end && untracked(|| self.spacer()) == 0.0 {
                    self.following.set(true);
                }
                let following = self.following.get_untracked();
                self.scrollable.set(MessageScrollerScrollable {
                    start: g.offset > edge,
                    end: !following && !at_end,
                });
            },
        );
    }

    /// Reacts to the rows changing: opens at the default position, and
    /// anchors a new turn.
    fn track_rows(self) {
        let before: Rc<RefCell<Vec<SharedString>>> = Rc::default();
        let before_count = Rc::new(Cell::new(0_usize));
        effect(move || {
            let count = self.count.get();
            let ids = self.ids.get();
            let anchors = self.anchors.get();
            untracked(|| {
                let previous = before.replace(ids.clone());
                let previous_count = before_count.replace(count);
                self.heights.update(|h| h.truncate(count));
                if count < previous_count
                    && let Some(anchor) = self.anchored.get()
                    && !ids.contains(&anchor)
                {
                    self.anchored.set(None);
                }
                if previous_count == 0 && count > 0 {
                    // Once the list is laid out (`watch`).
                    self.opening.set(true);
                    return;
                }
                if count <= previous_count || ids.is_empty() {
                    return;
                }
                // Rows after the previous last one are appended; rows
                // before the previous first one are prepended, which the
                // list keeps in place by itself.
                let start = match previous.last() {
                    Some(last) => match ids.iter().position(|i| i == last) {
                        Some(at) => at + 1,
                        None => return,
                    },
                    None => previous_count,
                };
                let new: Vec<usize> = (start..ids.len()).filter(|&i| anchors.contains(&ids[i])).collect();
                let Some(&first) = new.first() else {
                    return;
                };
                if self.auto_scroll.get() && self.following.get() && new.len() > 1 {
                    self.scroll_to_end(false);
                } else {
                    self.anchored.set(Some(ids[first].clone()));
                    self.scroll_to_row(first, MessageScrollerScrollAlign::Start, true, false);
                }
            });
        });
    }

    /// Scrolls to the opening position, once the first rows are in.
    fn open(self) {
        let (ids, anchors) = (self.ids.get_untracked(), self.anchors.get_untracked());
        match self.default_position.get_untracked() {
            MessageScrollerScrollPosition::Start => {}
            MessageScrollerScrollPosition::End => self.scroll_to_end(false),
            MessageScrollerScrollPosition::LastAnchor => {
                match ids.iter().rposition(|id| anchors.contains(id)) {
                    // Past the end when the last turn fits: the list stops at the end.
                    Some(at) => self.scroll_to_row(at, MessageScrollerScrollAlign::Start, true, false),
                    None => self.scroll_to_end(false),
                }
            }
        }
    }

    fn visibility(self) -> MessageScrollerVisibility {
        let g = self.geometry.get();
        let ids = self.ids.get();
        let anchors = self.anchors.get();
        let gap = self.gap.get();
        let line = g.offset + self.margin.get() + self.peek.get();
        let mut top = self.padding.get();
        let mut out = MessageScrollerVisibility::default();
        self.heights.with(|h| {
            for (i, id) in ids.iter().enumerate() {
                let bottom = top + Self::height(h, i);
                if anchors.contains(id) && top <= line {
                    out.current_anchor_id = Some(id.clone());
                }
                if bottom > line && top < g.offset + g.viewport {
                    out.visible_message_ids.push(id.clone());
                }
                top = bottom + gap;
            }
        });
        out
    }
}

/// A handle on the scroller around the component that asked for it
/// ([`use_message_scroller`]): its commands, and what it shows. Reading
/// [`Self::scrollable`] or [`Self::visibility`] in a binding follows them.
#[derive(Clone, Copy)]
pub struct MessageScrollerHandle(Scroller);

impl MessageScrollerHandle {
    /// Scrolls to the last message (following it again, with
    /// `auto_scroll`).
    pub fn scroll_to_end(&self, animated: bool) {
        self.0.scroll_to_end(animated);
    }

    /// Scrolls to the first message.
    pub fn scroll_to_start(&self, animated: bool) {
        self.0.scroll_to_start(animated);
    }

    /// Scrolls to the message `id` (one of the content's `message_ids`),
    /// placed as `align` says, `scroll_margin` from the edge. False when
    /// there is no such message.
    pub fn scroll_to_message(&self, id: &str, align: MessageScrollerScrollAlign, animated: bool) -> bool {
        let Some(index) = self.0.index_of(id) else {
            return false;
        };
        self.0.anchored.set(None);
        self.0.scroll_to_row(index, align, false, animated);
        true
    }

    /// The edges the viewport can scroll toward (shadcn's
    /// `useMessageScrollerScrollable`). While it follows the end, the end
    /// is not one.
    pub fn scrollable(&self) -> MessageScrollerScrollable {
        self.0.scrollable.get()
    }

    /// The messages in view and the turn the reader is in (shadcn's
    /// `useMessageScrollerVisibility`), from the rows' measured heights.
    pub fn visibility(&self) -> MessageScrollerVisibility {
        self.0.visibility()
    }
}

/// The scroller around the component being made (a [`MessageScrollerProvider`]
/// or a [`MessageScroller`] above it): shadcn's `useMessageScroller`,
/// `useMessageScrollerScrollable` and `useMessageScrollerVisibility`.
pub fn use_message_scroller() -> Option<MessageScrollerHandle> {
    use_context::<Scroller>().map(MessageScrollerHandle)
}

/// The provider's classes (a view of its own here: see the module doc).
const PROVIDER: &str = "flex size-full min-h-0 min-w-0 flex-col";

/// shadcn's `MessageScroller` classes.
const ROOT: &str = "group/message-scroller relative flex size-full min-h-0 flex-col overflow-hidden";

/// shadcn's `MessageScrollerViewport` classes NSH draws.
const VIEWPORT: &str = "size-full min-h-0 min-w-0";

/// shadcn's `MessageScrollerContent` classes NSH draws on the list (`gap-8`
/// is the `gap` prop).
const CONTENT: &str = "flex min-h-full flex-col";

/// shadcn's `MessageScrollerItem` classes NSH draws.
const ITEM: &str = "min-w-0 shrink-0";

/// shadcn's `MessageScrollerButton` classes on the view around the button.
const BUTTON_FRAME: &str = "absolute inset-s-1/2 -translate-x-1/2 transition rtl:translate-x-1/2 \
    pointer-events-none scale-95 opacity-0 duration-400 ease-[cubic-bezier(0.7,0,0.84,0)] \
    data-active:pointer-events-auto data-active:translate-y-0 data-active:scale-100 data-active:opacity-100 \
    data-active:duration-200 data-active:ease-[cubic-bezier(0.23,1,0.32,1)]";

/// shadcn's `data-[direction=…]:…` classes of the button.
fn direction_classes(direction: MessageScrollerButtonDirection) -> &'static str {
    match direction {
        MessageScrollerButtonDirection::End => "bottom-4 translate-y-full",
        MessageScrollerButtonDirection::Start => "top-4 -translate-y-full",
    }
}

/// shadcn's classes of the button itself.
const BUTTON: &str = "border-border bg-background text-foreground hover:bg-muted hover:text-foreground";

/// The props of [`MessageScrollerProvider`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageScrollerProviderProps {
    /// Follow new content at the end while the reader is at the end.
    #[prop(default)]
    pub auto_scroll: Prop<bool>,
    /// Where the transcript opens, once its first rows are in.
    #[prop(default)]
    pub default_scroll_position: Prop<MessageScrollerScrollPosition>,
    /// How close to an edge, in points, still counts as at it.
    #[prop(default = 8.0)]
    pub scroll_edge_threshold: Prop<f32>,
    /// How much of the turn before a newly anchored one stays in view
    /// above it, in points.
    #[prop(default = 64.0)]
    pub scroll_previous_item_peek: Prop<f32>,
    /// The margin at the edge a message is scrolled to, in points.
    #[prop(default = 0.0)]
    pub scroll_margin: Prop<f32>,
    /// The [`MessageScroller`], and the controls around it that use
    /// [`use_message_scroller`].
    pub children: Prop<Children>,
}

/// Owns the scroll state of a transcript: its opening position, following
/// the end, anchoring new turns. The [`MessageScroller`] inside draws it.
#[component]
pub fn MessageScrollerProvider(props: MessageScrollerProviderProps) -> impl IntoElement {
    let scroller = Scroller::new();
    effect(move || scroller.auto_scroll.set(props.auto_scroll.get()));
    effect(move || scroller.default_position.set(props.default_scroll_position.get()));
    effect(move || scroller.edge_threshold.set(props.scroll_edge_threshold.get()));
    effect(move || scroller.peek.set(props.scroll_previous_item_peek.get()));
    effect(move || scroller.margin.set(props.scroll_margin.get()));
    provide_context(scroller);
    view().app_class(Some(SharedString::from(PROVIDER))).child(props.children)
}

/// The props of [`MessageScroller`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageScrollerProps {
    /// A [`MessageScrollerViewport`] and a [`MessageScrollerButton`].
    pub children: Prop<Children>,
}

/// The frame of a transcript: it fills its parent (give the parent a
/// height), and holds the viewport and the scroll button over it.
#[component]
pub fn MessageScroller(props: MessageScrollerProps) -> impl IntoElement {
    let _ = Scroller::current();
    view().app_class(Some(SharedString::from(ROOT))).child(props.children)
}

/// The props of [`MessageScrollerViewport`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageScrollerViewportProps {
    /// What screen readers call the transcript.
    pub accessibility_label: Prop<Option<SharedString>>,
    /// A [`MessageScrollerContent`].
    pub children: Prop<Children>,
}

/// The visible part of the transcript, its last 40 points fading while
/// there is more below.
#[component]
pub fn MessageScrollerViewport(props: MessageScrollerViewportProps) -> impl IntoElement {
    let scroller = Scroller::current();
    // shadcn's `scroll-fade-b`: the last min(12%, 40px) fade out, less as
    // the end comes within 96 points.
    let stops = move || {
        let g = scroller.geometry.get();
        let left = ((g.content - g.viewport).max(0.0) - g.offset).max(0.0);
        let fade = (g.viewport * 0.12).min(40.0) * (left / 96.0).min(1.0);
        let end = g.viewport.max(1.0);
        vec![
            ColorStop::from((black(), px(0.0))),
            ColorStop::from((black(), px(end - fade))),
            ColorStop::from((
                if fade > 0.0 { transparent_black() } else { black() },
                px(end),
            )),
        ]
    };
    view()
        .app_class(Some(SharedString::from(VIEWPORT)))
        .optional_label(move || Some(props.accessibility_label.get().unwrap_or_else(|| "Messages".into())))
        .mask_linear(deg(180.0), stops)
        .child(props.children)
}

/// The props of [`MessageScrollerContent`].
#[derive(Clone, PartialEq, Props)]
pub struct MessageScrollerContentProps {
    /// The transcript's rows, oldest first: the app's `virtual_list` (in
    /// TSX `each` and a row renderer), each row a message, a marker, a
    /// typing indicator.
    pub items: Items,
    /// The rows' ids, in order (shadcn's `messageId` of each item): how
    /// the scroller tells appended rows from prepended ones, and what
    /// [`MessageScrollerHandle::scroll_to_message`] takes.
    #[prop(default)]
    pub message_ids: Prop<Vec<SharedString>>,
    /// The ids of the rows that start a turn (shadcn's `scrollAnchor`
    /// items; usually the user's messages). One appended is scrolled near
    /// the top.
    #[prop(default)]
    pub scroll_anchors: Prop<Vec<SharedString>>,
    /// Space between rows, in points (shadcn's `gap-8`).
    #[prop(default = 32.0)]
    pub gap: Prop<f32>,
    /// Space around the rows, in points.
    #[prop(default = 0.0)]
    pub padding: Prop<f32>,
}

/// The transcript: the rows, a polite live region for new messages.
#[component]
pub fn MessageScrollerContent(props: MessageScrollerContentProps) -> impl IntoElement {
    let scroller = Scroller::current();
    effect(move || scroller.ids.set(props.message_ids.get()));
    effect(move || scroller.anchors.set(props.scroll_anchors.get()));
    effect(move || scroller.gap.set(props.gap.get()));
    effect(move || scroller.padding.set(props.padding.get()));
    effect(move || {
        let ids = props.message_ids.get();
        if !ids.is_empty() {
            scroller.count.set(ids.len());
        }
    });
    let spacer = computed(move || scroller.spacer());
    // A reply that consumed the tail spacer has filled the viewport: with
    // `auto_scroll`, following the end takes over from holding its turn
    // (shadcn's hand-off on the spacer's last point going).
    let had_spacer = Rc::new(Cell::new(false));
    effect(move || {
        let room = spacer.get();
        let before = had_spacer.replace(room > 0.0);
        untracked(|| {
            if before && room == 0.0 && scroller.auto_scroll.get() && scroller.anchored.get().is_some() {
                scroller.scroll_to_end(false);
            }
        });
    });
    scroller.watch();
    scroller.track_rows();
    let has_ids = move || props.message_ids.with(|ids| !ids.is_empty());
    view()
        .app_class(Some(SharedString::from(VIEWPORT)))
        .accessibility_live_region(|| nsh_core::LiveRegion::Polite)
        .list(
            props.items,
            move |list| {
                list.app_class(Some(SharedString::from(CONTENT)))
                    .flex_1()
                    .row_gap(move || px(props.gap.get()))
                    .content_padding(move || px(props.padding.get()))
                    .element_ref(&scroller.list)
            },
            move |row: Child, at: Position| {
                let measured = signal(f32::NAN);
                effect(move || {
                    let (index, height) = (at.index(), measured.get());
                    if height.is_finite() {
                        untracked(|| {
                            scroller.heights.update(|h| {
                                if h.len() <= index {
                                    h.resize(index + 1, f32::NAN);
                                }
                                h[index] = height;
                            })
                        });
                    }
                });
                effect(move || {
                    let count = at.count();
                    if !untracked(has_ids) {
                        scroller.count.set(count);
                    }
                });
                view()
                    .pb(move || px(if at.is_last() { spacer.get() } else { 0.0 }))
                    .child(
                        view()
                            .on_layout(move |b: &Bounds<Points>| measured.set(b.size.height.0))
                            .child(row),
                    )
            },
        )
}

/// The props of [`MessageScrollerItem`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageScrollerItemProps {
    /// The row: a [`super::Message`], a [`super::Marker`].
    pub children: Prop<Children>,
}

/// One row of the transcript, as the app renders it in the content's
/// `items`.
#[component]
pub fn MessageScrollerItem(props: MessageScrollerItemProps) -> impl IntoElement {
    view().app_class(Some(SharedString::from(ITEM))).child(props.children)
}

/// The props of [`MessageScrollerButton`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct MessageScrollerButtonProps {
    /// `end` (the default: the newest message) or `start`.
    #[prop(default)]
    pub direction: Prop<MessageScrollerButtonDirection>,
    /// A [`crate::Button`] variant; shadcn's default is `secondary`.
    #[prop(default = ButtonVariant::Secondary)]
    pub variant: Prop<ButtonVariant>,
    /// A [`crate::Button`] size; shadcn's default is `icon-sm`.
    #[prop(default = ButtonSize::IconSm)]
    pub size: Prop<ButtonSize>,
    /// What screen readers say ("Scroll to end", "Scroll to start").
    pub accessibility_label: Prop<Option<SharedString>>,
    /// Scroll smoothly (shadcn's default) or jump.
    #[prop(default = true)]
    pub animated: Prop<bool>,
}

/// A round button over the edge of the viewport that shows while there
/// is content past that edge, and scrolls there.
#[component]
pub fn MessageScrollerButton(props: MessageScrollerButtonProps) -> impl IntoElement {
    let scroller = Scroller::current();
    let end = move || props.direction.get() == MessageScrollerButtonDirection::End;
    let active = move || {
        let s = scroller.scrollable.get();
        if end() { s.end } else { s.start }
    };
    view()
        .app_class(move || Some(cn([BUTTON_FRAME, direction_classes(props.direction.get())])))
        .project_data("active", active)
        .child(
            button()
                .variant(props.variant)
                .size(props.size)
                .icon(move || Some(if end() { IconName::ArrowDown } else { IconName::ArrowUp }))
                .accessibility_label(move || {
                    Some(props.accessibility_label.get().unwrap_or_else(|| {
                        if end() { "Scroll to end" } else { "Scroll to start" }.into()
                    }))
                })
                .disabled(move || !active())
                .app_class(Some(SharedString::from(BUTTON)))
                .on_press(move || {
                    let animated = props.animated.get_untracked();
                    if end() {
                        scroller.scroll_to_end(animated);
                    } else {
                        scroller.scroll_to_start(animated);
                    }
                }),
        )
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<MessageScrollerProvider>();
    registry.register::<MessageScroller>();
    registry.register::<MessageScrollerViewport>();
    registry.register::<MessageScrollerContent>();
    registry.register::<MessageScrollerItem>();
    registry.register::<MessageScrollerButton>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = vec![cn([PROVIDER, ROOT, VIEWPORT, CONTENT, ITEM, BUTTON])];
        for direction in [MessageScrollerButtonDirection::Start, MessageScrollerButtonDirection::End] {
            all.push(cn([BUTTON_FRAME, direction_classes(direction)]));
        }
        for classes in all {
            let unknown = crate::cn::unknown_classes(&classes);
            assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
        }
    }
}

