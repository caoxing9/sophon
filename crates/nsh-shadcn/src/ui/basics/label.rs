//! [`Label`], after shadcn/ui's `label.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `peer-disabled:cursor-not-allowed peer-disabled:opacity-50` and
//!   `group-data-[disabled=true]:pointer-events-none
//!   group-data-[disabled=true]:opacity-50` dim the label of a disabled
//!   control; NSH has no `peer-` or `group-data-` selectors, so the
//!   `disabled` prop does (`cursor-not-allowed opacity-50`, and no presses).
//! - Radix's `htmlFor` (a click on the label is a click on its control) is
//!   the `onPress` event: the app presses the control.

use nsh_core::prelude::*;

use super::cls;
use crate::cn::cn;

const BASE: &str = "flex items-center gap-2 text-sm leading-none font-medium select-none";
/// `peer-disabled:…` / `group-data-[disabled=true]:…`.
const DISABLED: &str = "cursor-not-allowed opacity-50";

fn label_classes(disabled: bool) -> SharedString {
    cn([BASE, if disabled { DISABLED } else { "" }])
}

/// The props of [`Label`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct LabelProps {
    /// The label's text.
    pub text: Prop<Option<SharedString>>,
    /// The control it names is disabled: dimmed, and presses do nothing.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// Content after the text (shadcn's label can hold its control).
    pub children: Prop<Children>,
}

/// The events of a [`Label`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum LabelEvent {
    /// It was pressed: what HTML's `<label for>` does, the app does here
    /// (checks the checkbox, focuses the input).
    Press,
}

/// shadcn's Label: the name of a control beside it.
#[component]
pub fn Label(props: LabelProps, cx: Scope<LabelEvent>) -> impl IntoElement {
    let label = props.text;
    view()
        .app_class(move || cls(label_classes(props.disabled.get())))
        .on_press(move || {
            if !props.disabled.get_untracked() {
                cx.emit(LabelEvent::Press);
            }
        })
        .child(dynamic(move || {
            label.get().map(|label| text(label).into_element())
        }))
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Label>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let all = [label_classes(false), label_classes(true)];
        super::super::assert_known(all.iter().map(|s| s.as_ref()));
    }
}
