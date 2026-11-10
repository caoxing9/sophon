//! The kit's components for TSX screens: `import { Card, Segmented, ... }
//! from "sophon/native"`. Each is the Rust kit's own function under a
//! component's props and events, so a TSX screen and a Rust one look the same
//! by construction, and a change to the kit changes both.
//!
//! They run in the core, as every Rust component does: unlike a module's
//! plain functions, they may read the store's signals.

use nsh_core::prelude::*;

use super::controls as kit;
use crate::ui::shared::settings::row;

/// The props of [`Card`].
#[derive(Clone, PartialEq, Props)]
pub struct CardProps {
    /// Its rows.
    pub children: Prop<Children>,
}

/// A flat card of rows: the surface, a hairline edge, rounded corners.
#[component]
pub fn Card(props: CardProps) -> impl IntoElement {
    kit::card().child(props.children)
}

/// The props of [`Section`].
#[derive(Clone, PartialEq, Props)]
pub struct SectionProps {
    /// The label above it.
    pub title: Prop<SharedString>,
    /// Its cards.
    pub children: Prop<Children>,
}

/// A titled group of cards.
#[component]
pub fn Section(props: SectionProps) -> impl IntoElement {
    view().gap(px(super::S3)).child(kit::section_label(move || props.title.get())).child(props.children)
}

/// The props of [`SettingsRow`].
#[derive(Clone, PartialEq, Props)]
pub struct SettingsRowProps {
    /// What it sets.
    pub title: Prop<SharedString>,
    /// A line on what it does, under the title.
    pub hint: Prop<Option<SharedString>>,
    /// The card's first row: no hairline above it.
    #[prop(default = false)]
    pub first: Prop<bool>,
    /// The control, after the title.
    pub children: Prop<Children>,
}

/// A settings row: the title and its hint on the leading side, the control
/// trailing; a hairline above all but the first.
#[component]
pub fn SettingsRow(props: SettingsRowProps) -> impl IntoElement {
    view().child(dynamic(move || {
        let first = props.first.get();
        Some(untracked(move || {
            row(move || props.title.get(), move || props.hint.get().unwrap_or_default(), view().child(props.children), first).into_element()
        }))
    }))
}

/// One of a [`Segmented`]'s options.
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct SegmentOption {
    /// What `onChange` reports when it is chosen.
    pub value: SharedString,
    /// What it says.
    pub label: SharedString,
}

/// The props of [`Segmented`].
#[derive(Clone, PartialEq, Props)]
pub struct SegmentedProps {
    /// The options, in order.
    pub options: Prop<Vec<SegmentOption>>,
    /// The chosen option's value.
    pub value: Prop<SharedString>,
}

/// The events of a [`Segmented`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum SegmentedEvent {
    /// An option was chosen: its value. The app sets `value` from it.
    #[args(value)]
    Change(SharedString),
}

/// Segments with one thumb that springs to the chosen one.
#[component]
pub fn Segmented(props: SegmentedProps, cx: Scope<SegmentedEvent>) -> impl IntoElement {
    view().child(dynamic(move || {
        let options = props.options.get();
        Some(untracked(move || {
            let options = options.into_iter().map(|o| (o.value, o.label)).collect();
            kit::segmented(options, move || props.value.get(), move |v: SharedString| cx.emit(SegmentedEvent::Change(v))).into_element()
        }))
    }))
}
