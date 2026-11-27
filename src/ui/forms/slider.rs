//! [`Slider`], after shadcn/ui's `slider.tsx` (new-york-v4).
//!
//! Behavior is `nsh_base`'s `Slider` (presses and drags on the track and
//! the thumbs, the arrow keys, Page Up / Down, Home / End, steps, screen
//! readers' adjustable thumbs), one thumb per value as Radix's. Its parts
//! take shadcn's: the slider shadcn's `SliderPrimitive.Root`, the
//! indicator (the rail the values map onto) `SliderPrimitive.Track`, the
//! range `SliderPrimitive.Range`, each thumb `SliderPrimitive.Thumb`; the
//! track (`nsh_base`'s press area around the rail, which Radix's root is)
//! lays the rail out in the root (`flex flex-1 items-center`). The classes
//! are shadcn's, word for word, except:
//!
//! - `data-[orientation=…]:` are the `orientation` prop's,
//!   `data-[disabled]:` is `disabled:`.
//! - `touch-none` (unknown to NSH, which has no browser gestures to stop)
//!   is left out.
//! - The rail's `overflow-hidden` clips the range to the rail's rounded
//!   ends in a browser; here the thumbs are inside the rail, which must not
//!   clip them, so the range is `rounded-full` instead (its ends are the
//!   rail's or under a thumb).
//! - `transition-[color,box-shadow]` (unknown to NSH) is `transition`.
//! - The thumb's `disabled:pointer-events-none disabled:opacity-50` are
//!   left out: Radix's thumb is a `<span>`, which `:disabled` never
//!   matches, so only the root dims a disabled slider (NSH's thumb would
//!   dim twice).
//! - A thumb is centered on its value; Radix keeps a thumb inside the rail
//!   at its ends (half a thumb's width in at `min` and `max`, the same in
//!   the middle).

use nsh_base::SliderValue;
use nsh_core::geometry::Axis;
use nsh_core::prelude::*;

use crate::cn::cn;

/// shadcn's Root classes.
const ROOT: &str = "relative flex w-full items-center select-none disabled:opacity-50";
const ROOT_VERTICAL: &str = "h-full min-h-44 w-auto flex-col";
/// The press area around the rail.
const TRACK: &str = "flex flex-1 items-center";
const TRACK_VERTICAL: &str = "flex-col self-stretch";
/// shadcn's Track classes (the rail), without `overflow-hidden`.
const RAIL: &str = "relative grow rounded-full bg-muted";
const RAIL_HORIZONTAL: &str = "h-1.5 w-full";
const RAIL_VERTICAL: &str = "h-full w-1.5";
/// shadcn's Range classes, and `rounded-full`.
const RANGE: &str = "absolute bg-primary rounded-full";
const RANGE_HORIZONTAL: &str = "h-full";
const RANGE_VERTICAL: &str = "w-full";
/// shadcn's Thumb classes.
const THUMB: &str = "block size-4 shrink-0 rounded-full border border-primary bg-white shadow-sm \
    ring-ring/50 transition hover:ring-4 focus-visible:ring-4 focus-visible:outline-hidden";

/// Which way a slider runs: Radix's `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SliderOrientation {
    /// From `min` at the start to `max` at the end.
    #[default]
    Horizontal,
    /// From `min` at the bottom to `max` at the top.
    Vertical,
}

/// The classes of the root, the track, the rail, the range and a thumb.
pub(crate) fn slider_classes(orientation: SliderOrientation) -> [SharedString; 5] {
    let vertical = orientation == SliderOrientation::Vertical;
    [
        cn([ROOT, if vertical { ROOT_VERTICAL } else { "" }]),
        cn([TRACK, if vertical { TRACK_VERTICAL } else { "" }]),
        cn([
            RAIL,
            if vertical {
                RAIL_VERTICAL
            } else {
                RAIL_HORIZONTAL
            },
        ]),
        cn([
            RANGE,
            if vertical {
                RANGE_VERTICAL
            } else {
                RANGE_HORIZONTAL
            },
        ]),
        SharedString::from(THUMB),
    ]
}

/// The props of [`Slider`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SliderProps {
    /// The values (controlled), one per thumb: `[50]`, or `[25, 75]` for a
    /// range. The app keeps them and sets the ones `onValueChange` asks
    /// for. Without them the slider keeps its own, from `defaultValue`.
    pub value: Prop<Option<Vec<f64>>>,
    /// The values at first, while `value` is not set (default `[min]`).
    #[prop(default)]
    pub default_value: Prop<Vec<f64>>,
    /// The smallest value (default `0`).
    #[prop(default)]
    pub min: Prop<f64>,
    /// The largest value (default `100`).
    #[prop(default = 100.0)]
    pub max: Prop<f64>,
    /// The step values snap to (default `1`).
    #[prop(default = 1.0)]
    pub step: Prop<f64>,
    /// `horizontal` or `vertical`; read when the slider is made.
    #[prop(default)]
    pub orientation: Prop<SliderOrientation>,
    /// No changes, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// What screen readers say for the thumbs.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of a [`Slider`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum SliderEvent {
    /// The user moved a thumb: the values now (Radix's `onValueChange`).
    #[args(value)]
    ValueChange(Vec<f64>),
    /// The user's change ended: the values it ended at (Radix's
    /// `onValueCommit`).
    #[args(value)]
    ValueCommit(Vec<f64>),
}

fn to_slider(values: &[f64], min: f64) -> SliderValue {
    match values {
        [] => SliderValue::Single(min),
        [one] => SliderValue::Single(*one),
        [start, end, ..] => SliderValue::Range(*start, *end),
    }
}

fn to_values(value: &SliderValue) -> Vec<f64> {
    match *value {
        SliderValue::Single(v) => vec![v],
        SliderValue::Range(a, b) => vec![a, b],
    }
}

/// shadcn's Slider: `<Slider defaultValue={[50]} max={100} step={1} />`.
#[component]
pub fn Slider(props: SliderProps, cx: Scope<SliderEvent>) -> impl IntoElement {
    let orientation = props.orientation.get_untracked();
    let classes = slider_classes(orientation);
    let controlled = props.value;
    let min = props.min;
    let slider = nsh_base::slider()
        .default_value(to_slider(
            &props.default_value.get_untracked(),
            min.get_untracked(),
        ))
        .min(props.min)
        .max(props.max)
        .step(props.step)
        .axis(match orientation {
            SliderOrientation::Horizontal => Axis::Horizontal,
            SliderOrientation::Vertical => Axis::Vertical,
        })
        .disabled(props.disabled)
        .accessibility_label(props.accessibility_label)
        .on_change(move |value: &SliderValue| cx.emit(SliderEvent::ValueChange(to_values(value))))
        .on_change_end(move |value: &SliderValue| {
            cx.emit(SliderEvent::ValueCommit(to_values(value)))
        })
        .app_class(Some(classes[0].clone()))
        .track_class(Some(classes[1].clone()))
        .indicator_class(Some(classes[2].clone()))
        .range_class(Some(classes[3].clone()))
        .thumb_class(Some(classes[4].clone()));
    if controlled.get_untracked().is_some() {
        slider.value(move || controlled.get().map(|v| to_slider(&v, min.get())))
    } else {
        slider
    }
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Slider>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        for orientation in [SliderOrientation::Horizontal, SliderOrientation::Vertical] {
            let all = slider_classes(orientation).join(" ");
            let unknown = crate::cn::unknown_classes(&all);
            assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
        }
    }
}
