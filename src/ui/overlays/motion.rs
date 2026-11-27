//! The entrance and exit of the anchored popups (popover, hover card,
//! tooltip): shadcn's tw-animate-css classes (`animate-in fade-in-0
//! zoom-in-95 slide-in-from-*-2`, `animate-out fade-out-0 zoom-out-95`)
//! redone with NSH's popup transitions: the popup's starting and ending
//! looks (`popupStartingClass` / `popupEndingClass`), between which its
//! `transition` runs for tw-animate's 150 ms on CSS's `ease`, scaled about
//! the side that faces the trigger (Radix's `--radix-*-transform-origin`).

use nsh_core::prelude::*;

/// The popup's own look: the transition, from the side facing the trigger.
pub(crate) fn transition(side: Placement) -> &'static str {
    match side {
        Placement::Bottom => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)] origin-top",
        Placement::Top => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)] origin-bottom",
        Placement::Left => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)] origin-right",
        Placement::Right => "transition duration-150 ease-[cubic-bezier(0.25,0.1,0.25,1)] origin-left",
    }
}

/// Where it comes from: `fade-in-0 zoom-in-95` and
/// `data-[side=…]:slide-in-from-…-2`.
pub(crate) fn starting(side: Placement) -> &'static str {
    match side {
        Placement::Bottom => "opacity-0 scale-95 -translate-y-2",
        Placement::Top => "opacity-0 scale-95 translate-y-2",
        Placement::Left => "opacity-0 scale-95 translate-x-2",
        Placement::Right => "opacity-0 scale-95 -translate-x-2",
    }
}

/// Where it goes: `fade-out-0 zoom-out-95`.
pub(crate) const ENDING: &str = "opacity-0 scale-95";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        for side in [Placement::Top, Placement::Bottom, Placement::Left, Placement::Right] {
            for classes in [transition(side), starting(side), ENDING] {
                let unknown = crate::cn::unknown_classes(classes);
                assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
            }
        }
    }
}
