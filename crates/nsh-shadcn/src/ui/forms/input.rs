//! [`Input`] and [`Textarea`], after shadcn/ui's `input.tsx` and
//! `textarea.tsx` (new-york-v4).
//!
//! Both are NSH's native text field (`input`: UITextField / NSTextField,
//! UITextView / NSTextView when multiline), styled with shadcn's classes.
//! They are word for word, except:
//!
//! - `shrink` is added: a flex item shrinks in CSS by default, not in NSH,
//!   and shadcn's `w-full min-w-0` field shrinks beside a button.
//! - `transition-[color,box-shadow]` (an arbitrary property list, unknown
//!   to NSH) is `transition`.
//! - `selection:text-primary-foreground`: NSH's `selection:` takes the
//!   highlight's color only; the selected text keeps its color.
//! - `file:…` (the file button of `type="file"`) is left out: NSH's field
//!   has no file type (pick files with `pick_files`).
//! - `aria-invalid:` follows the `invalid` prop, which the field reports
//!   to screen readers as invalid.
//! - Textarea's `field-sizing-content` (unknown to NSH): the field's
//!   height follows its text's (`onContentSizeChange`), over `min-h-16`.
//!
//! `type` picks the keyboard and masking: `password` masks the text,
//! `email`, `number`, `tel`, `url` and `search` the keyboard and return
//! key.

use nsh_core::prelude::*;

use super::field::FieldContext;

/// shadcn's Input classes.
pub(crate) const INPUT: &str = "shrink h-9 w-full min-w-0 rounded-md border border-input bg-transparent \
    px-3 py-1 text-base shadow-xs transition outline-none selection:bg-primary \
    placeholder:text-muted-foreground disabled:pointer-events-none disabled:cursor-not-allowed \
    disabled:opacity-50 md:text-sm dark:bg-input/30 \
    focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 \
    aria-invalid:border-destructive aria-invalid:ring-destructive/20 \
    dark:aria-invalid:ring-destructive/40";

/// shadcn's Textarea classes.
pub(crate) const TEXTAREA: &str = "shrink flex min-h-16 w-full rounded-md border border-input \
    bg-transparent px-3 py-2 text-base shadow-xs transition outline-none \
    placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] \
    focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 \
    aria-invalid:border-destructive aria-invalid:ring-destructive/20 md:text-sm \
    dark:bg-input/30 dark:aria-invalid:ring-destructive/40";

/// shadcn's Input look for an app's own field (as shadcn exports
/// `buttonVariants` for its button's): an `input()` the app builds itself
/// (its font, its events) draws as [`Input`] with
/// `.app_class(Some(input_classes().into()))`.
pub fn input_classes() -> &'static str {
    INPUT
}

/// shadcn's Textarea look for an app's own multiline field, as
/// [`input_classes`] is the Input's.
pub fn textarea_classes() -> &'static str {
    TEXTAREA
}

/// What an input holds: the HTML `type`s NSH's field can be.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum InputType {
    /// Text.
    #[default]
    Text,
    /// An email address: the email keyboard, no capitals or corrections.
    Email,
    /// A password: masked.
    Password,
    /// A number: the numeric keyboard.
    Number,
    /// A search: the search return key.
    Search,
    /// A phone number: the phone keypad.
    Tel,
    /// A URL: the URL keyboard.
    Url,
}

/// The props of a text field ([`Input`], [`Textarea`] and the input
/// group's controls).
#[derive(Debug, Clone, PartialEq, Props)]
pub struct InputProps {
    /// The text (controlled): the app keeps it and sets the one `onChange`
    /// reports. Without it the field keeps its own, from `defaultValue`.
    pub value: Prop<Option<SharedString>>,
    /// The text at first, while `value` is not set.
    pub default_value: Prop<Option<SharedString>>,
    /// Shown while the field is empty.
    pub placeholder: Prop<Option<SharedString>>,
    /// `text`, `email`, `password`, `number`, `search`, `tel` or `url`.
    #[prop(default)]
    pub r#type: Prop<InputType>,
    /// No focus, no editing, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The text can be selected and copied, not changed.
    #[prop(default)]
    pub readonly: Prop<bool>,
    /// The value fails the app's checks: a destructive border and ring
    /// (shadcn's `aria-invalid`).
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// What screen readers say: what the field is for, when no label
    /// names it.
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of a text field.
#[derive(Debug, Clone, PartialEq, Event)]
pub enum InputEvent {
    /// The user changed the text: the text now.
    #[args(value)]
    Change(SharedString),
    /// Return was pressed in a single-line field: its text.
    #[args(value)]
    Submit(SharedString),
    /// It took the keyboard focus.
    Focus,
    /// It lost the keyboard focus.
    Blur,
}

/// A native text field with `props`, drawing with `classes`; `multiline`
/// for a textarea. A [`super::Field`] around it focuses it when its label
/// is pressed.
pub(crate) fn text_field(
    props: &InputProps,
    cx: Scope<InputEvent>,
    multiline: bool,
    field_ref: ElementRef,
    classes: impl Fn() -> SharedString + 'static,
) -> nsh_core::element::Input {
    if let Some(field) = use_context::<FieldContext>() {
        field.control_press(move || field_ref.focus());
    }
    let kind = props.r#type;
    let invalid = props.invalid;
    let controlled = props.value;
    let field = nsh_core::element::input()
        .element_ref(&field_ref)
        .multiline(multiline)
        .placeholder({
            let placeholder = props.placeholder;
            move || placeholder.get().map(|p| p.to_string()).unwrap_or_default()
        })
        .masked(move || kind.get() == InputType::Password)
        .keyboard_type(move || match kind.get() {
            InputType::Email => KeyboardType::Email,
            InputType::Number => KeyboardType::Numeric,
            InputType::Tel => KeyboardType::Phone,
            InputType::Url => KeyboardType::Url,
            _ => KeyboardType::Default,
        })
        .auto_capitalize(move || match kind.get() {
            InputType::Text if multiline => AutoCapitalize::Sentences,
            InputType::Text => AutoCapitalize::Sentences,
            _ => AutoCapitalize::None,
        })
        .auto_correct(move || matches!(kind.get(), InputType::Text | InputType::Search))
        .return_key_type(move || match kind.get() {
            InputType::Search => ReturnKeyType::Search,
            _ => ReturnKeyType::Default,
        })
        .disabled(props.disabled)
        .readonly(props.readonly)
        .optional_label(props.accessibility_label)
        .accessibility_state(move || AccessibilityState {
            invalid: invalid.get(),
            ..AccessibilityState::default()
        })
        .app_class(move || Some(classes()))
        .on_change(move |text: &str| {
            cx.emit(InputEvent::Change(SharedString::from(text.to_owned())))
        })
        .on_submit(move |text: &str| {
            cx.emit(InputEvent::Submit(SharedString::from(text.to_owned())))
        })
        .on_focus(move || cx.emit(InputEvent::Focus))
        .on_blur(move || cx.emit(InputEvent::Blur));
    if controlled.get_untracked().is_some() {
        field.value(move || controlled.get().map(|v| v.to_string()).unwrap_or_default())
    } else {
        field.default_value(
            props
                .default_value
                .get_untracked()
                .map(|v| v.to_string())
                .unwrap_or_default(),
        )
    }
}

/// shadcn's Input: `<Input type="email" placeholder="Email" />`.
#[component]
pub fn Input(props: InputProps, cx: Scope<InputEvent>) -> impl IntoElement {
    text_field(&props, cx, false, element_ref(), || {
        SharedString::from(INPUT)
    })
}

/// How much taller than its text a textarea is: its `py-2` and border.
pub(crate) const TEXTAREA_CHROME: f32 = 2.0 * 8.0 + 2.0;

/// shadcn's Textarea: `<Textarea placeholder="Type your message here." />`.
#[component]
pub fn Textarea(props: InputProps, cx: Scope<InputEvent>) -> impl IntoElement {
    textarea_field(&props, cx, element_ref(), TEXTAREA_CHROME, || {
        SharedString::from(TEXTAREA)
    })
}

/// A textarea drawing with `classes`, as tall as its text (CSS's
/// `field-sizing: content`), its classes' `min-h-*` the least.
pub(crate) fn textarea_field(
    props: &InputProps,
    cx: Scope<InputEvent>,
    field_ref: ElementRef,
    chrome: f32,
    classes: impl Fn() -> SharedString + 'static,
) -> nsh_core::element::Input {
    let height = signal(0.0_f32);
    text_field(props, cx, true, field_ref, classes)
        .on_content_size_change(move |size: &Size<Points>| height.set(size.height.0))
        .h(move || px(height.get() + chrome))
}

impl InputElement {
    /// `.r#type(InputType::Email)`.
    pub fn email(self) -> Self {
        self.r#type(InputType::Email)
    }
    /// `.r#type(InputType::Password)`.
    pub fn password(self) -> Self {
        self.r#type(InputType::Password)
    }
}

/// Every class an input or a textarea can draw with, for the tests.
#[cfg(test)]
pub(crate) fn all_classes() -> SharedString {
    crate::cn::cn([INPUT, TEXTAREA])
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Input>();
    registry.register::<Textarea>();
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_class_is_known_to_nsh() {
        let unknown = crate::cn::unknown_classes(&super::all_classes());
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
