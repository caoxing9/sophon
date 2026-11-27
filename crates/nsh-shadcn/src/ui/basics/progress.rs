//! [`Progress`], after shadcn/ui's `progress.tsx` (new-york-v4), on
//! `nsh_base`'s `Progress` (the progress bar role and value for screen
//! readers, the indicator's width by the value).
//!
//! The classes are shadcn's, word for word, except the indicator's: shadcn
//! draws it as wide as the track (`h-full w-full flex-1`) and slides it
//! left by what is left to do (`translateX(-${100 - value}%)`) under the
//! track's `overflow-hidden`; `nsh_base`'s indicator is `value` percent of
//! the track wide instead (the same pixels: the indicator's left end is
//! clipped round, its right end square), so `w-full flex-1` are left out
//! (a `w-*` class there would win over the value). The track `nsh_base`
//! puts inside the bar fills it: `h-full w-full`.

use nsh_core::prelude::*;

use super::cls;

const ROOT: &str = "relative h-2 w-full overflow-hidden rounded-full bg-primary/20";
/// `nsh_base`'s track, filling the bar.
const TRACK: &str = "h-full w-full";
const INDICATOR: &str = "h-full bg-primary transition-all";

/// The props of [`Progress`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ProgressProps {
    /// How far the work is, from 0 to 100.
    #[prop(default)]
    pub value: Prop<f32>,
    /// What screen readers say: what is in progress.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// shadcn's Progress: an 8-point bar filling with the value.
#[component]
pub fn Progress(props: ProgressProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    nsh_base::progress()
        .value(props.value)
        .accessibility_label(props.accessibility_label)
        .app_class(cls(ROOT))
        .track_class(cls(TRACK))
        .indicator_class(cls(INDICATOR))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Progress>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::assert_known([ROOT, TRACK, INDICATOR]);
    }
}
