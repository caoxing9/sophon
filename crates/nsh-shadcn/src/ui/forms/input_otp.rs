//! [`InputOtp`], after shadcn/ui's `input-otp.tsx` (new-york-v4):
//! shadcn's `InputOTP`, `InputOTPGroup`, `InputOTPSlot` and
//! `InputOTPSeparator` in one component.
//!
//! Behavior is `nsh_base`'s `OtpInput`: one native field over the cells
//! takes the keys (typing, paste, the one-time-code autofill), the cells
//! show the code. It draws shadcn's parts: the container takes
//! `containerClassName`'s classes, each group InputOTPGroup's, each cell
//! InputOTPSlot's, the caret the fake caret's. Instead of composing groups,
//! slots and separators, the app gives `maxLength` and `groups` (6 and 2:
//! two groups of three) and a separator is drawn between groups. The
//! classes are shadcn's, word for word, except:
//!
//! - `data-[active=true]:` is `focus:`: NSH's variant of the cell that
//!   takes the next digit while the field has the focus.
//! - `has-disabled:opacity-50` is `disabled:opacity-50` (the input is the
//!   disabled element).
//! - `aria-invalid:` is the `invalid` prop's: its classes apply while it
//!   is set.
//! - The caret's `animate-caret-blink duration-1000` is NSH's own blink of
//!   the caret (500 ms shown, 500 hidden: the same second).
//! - `transition-all` stands; the cells' `first:` / `last:` are NSH's.
//! - The separators (`InputOTPSeparator`, a `MinusIcon` at Lucide's 24
//!   points) are drawn over the container, which `nsh_base` lays out
//!   alone: the gap between two groups is the separator's width and the two
//!   gaps around it, and the separator sits in its middle. A cell's width
//!   other than shadcn's `w-9` moves them off.
//! - NSH's code is digits only: `input-otp`'s `pattern` (letters too) is
//!   not ported.

use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};

/// shadcn's container classes (`containerClassName`).
const CONTAINER: &str = "flex items-center gap-2 disabled:opacity-50";
/// The container's gap with separators: `gap-2`, a 24-point separator,
/// `gap-2`.
const CONTAINER_SEPARATED: &str = "gap-10";
/// shadcn's InputOTPGroup classes.
const GROUP: &str = "flex items-center";
/// shadcn's InputOTPSlot classes.
const SLOT: &str = "relative flex h-9 w-9 items-center justify-center border-y border-r \
    border-input text-sm shadow-xs transition-all outline-none first:rounded-l-md first:border-l \
    last:rounded-r-md focus:z-10 focus:border-ring focus:ring-[3px] focus:ring-ring/50 \
    dark:bg-input/30";
/// shadcn's `aria-invalid:` classes, while invalid.
const SLOT_INVALID: &str = "border-destructive focus:border-destructive \
    focus:ring-destructive/20 dark:focus:ring-destructive/40";
/// shadcn's fake caret.
const CARET: &str = "h-4 w-px bg-foreground";
/// The layer the separators are drawn on.
const SEPARATORS: &str = "pointer-events-none absolute inset-0 flex flex-row items-center gap-2";
/// A slot's width (`w-9`).
const SLOT_WIDTH: f32 = 36.0;

/// The classes of the container, a group, a cell and the caret.
pub(crate) fn otp_classes(separated: bool, invalid: bool) -> [SharedString; 4] {
    [
        cn([CONTAINER, if separated { CONTAINER_SEPARATED } else { "" }]),
        SharedString::from(GROUP),
        cn([SLOT, if invalid { SLOT_INVALID } else { "" }]),
        SharedString::from(CARET),
    ]
}

/// The props of [`InputOtp`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct InputOtpProps {
    /// How many digits the code has: one cell each.
    #[prop(default = 6)]
    pub max_length: Prop<u32>,
    /// How many groups the cells are split into, a separator between two.
    #[prop(default = 1)]
    pub groups: Prop<u32>,
    /// The code (controlled): the app keeps it and sets the one `onChange`
    /// reports. Without it the input keeps its own, from `defaultValue`.
    pub value: Prop<Option<SharedString>>,
    /// The code at first, while `value` is not set.
    pub default_value: Prop<Option<SharedString>>,
    /// No focus, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// A destructive border on the cells (shadcn's `aria-invalid`).
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// What screen readers say for the field.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of an [`InputOtp`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum InputOtpEvent {
    /// The user changed the code: the code now.
    #[args(value)]
    Change(SharedString),
    /// The user filled the last cell: the code.
    #[args(value)]
    Complete(SharedString),
}

/// shadcn's InputOTP: `<InputOTP maxLength={6}>` with its groups, slots
/// and separators.
#[component]
pub fn InputOtp(props: InputOtpProps, cx: Scope<InputOtpEvent>) -> impl IntoElement {
    let separated = move || props.groups.get() > 1;
    let classes = move || otp_classes(separated(), props.invalid.get());
    let field = nsh_base::otp_input()
        .length(props.max_length)
        .groups(props.groups)
        .default_value(props.default_value)
        .disabled(props.disabled)
        .accessibility_label(props.accessibility_label)
        .on_change(move |code: &SharedString| cx.emit(InputOtpEvent::Change(code.clone())))
        .on_complete(move |done: &nsh_base::OtpInputCompleteEvent| {
            cx.emit(InputOtpEvent::Complete(done.value.clone()))
        })
        .app_class(move || Some(classes()[0].clone()))
        .group_class(move || Some(classes()[1].clone()))
        .cell_class(move || Some(classes()[2].clone()))
        .caret_class(move || Some(classes()[3].clone()));
    let controlled = props.value;
    let field = if controlled.get_untracked().is_some() {
        field.value(move || controlled.get())
    } else {
        field
    };
    // The separators between the groups, over the container.
    let separators = dynamic(move || {
        let groups = props.groups.get().max(1) as usize;
        if groups < 2 {
            return None;
        }
        let length = props.max_length.get() as usize;
        let per_group = length.div_ceil(groups).max(1);
        let groups = length.div_ceil(per_group);
        let mut layer = view().app_class(Some(SharedString::from(SEPARATORS)));
        for g in 0..groups {
            if g > 0 {
                layer = layer.child(
                    view()
                        .accessibility_elements_hidden(true)
                        .child(icon(IconName::Minus).size(px(24.0))),
                );
            }
            let cells = per_group.min(length - g * per_group);
            layer = layer.child(view().w(px(cells as f32 * SLOT_WIDTH)));
        }
        Some(layer.into_element())
    });
    view()
        .app_class(Some(SharedString::from("relative flex w-fit")))
        .child(field)
        .child(separators)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<InputOtp>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = Vec::new();
        for separated in [false, true] {
            for invalid in [false, true] {
                let all = format!("{} {SEPARATORS}", otp_classes(separated, invalid).join(" "));
                unknown.extend(crate::cn::unknown_classes(&all));
            }
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
