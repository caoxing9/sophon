//! The forms family of components. Each component is one file in this
//! directory; this module declares them, re-exports their public API (types
//! and constructors, which the crate root re-exports in turn) and registers
//! them for TSX.
//!
//! shadcn's `form.tsx` (react-hook-form's binding) is not ported: [`field`]
//! is the form layout, and the app checks values and shows [`field_error`].

use nsh_core::prelude::*;

mod button_group;
mod checkbox;
mod field;
mod input;
mod input_group;
mod input_otp;
mod native_select;
mod radio_group;
mod slider;
mod state;
mod switch;
mod toggle;
mod toggle_group;

pub use button_group::{
    ButtonGroup, ButtonGroupElement, ButtonGroupItem, ButtonGroupOrientation, ButtonGroupProps,
    ButtonGroupSeparator, ButtonGroupSeparatorElement, ButtonGroupSeparatorProps, ButtonGroupText,
    ButtonGroupTextElement, ButtonGroupTextProps,
};
pub use input_group::{
    InputGroup, InputGroupAddon, InputGroupAddonAlign, InputGroupAddonElement,
    InputGroupAddonProps, InputGroupButton, InputGroupButtonElement, InputGroupButtonEvent,
    InputGroupButtonProps, InputGroupButtonSize, InputGroupElement, InputGroupInput,
    InputGroupInputElement, InputGroupProps, InputGroupText, InputGroupTextElement,
    InputGroupTextProps, InputGroupTextarea, InputGroupTextareaElement,
};
pub use input_otp::{InputOtp, InputOtpElement, InputOtpEvent, InputOtpProps};
pub use native_select::{
    NativeSelect, NativeSelectElement, NativeSelectEvent, NativeSelectProps, NativeSelectSize,
};
pub use slider::{Slider, SliderElement, SliderEvent, SliderOrientation, SliderProps};
pub use toggle::{Toggle, ToggleElement, ToggleEvent, ToggleProps, ToggleSize, ToggleVariant};
pub use toggle_group::{
    ToggleGroup, ToggleGroupElement, ToggleGroupEvent, ToggleGroupItem, ToggleGroupItemElement,
    ToggleGroupItemProps, ToggleGroupProps, ToggleGroupType,
};

/// An option of a [`native_select`] (shadcn's `NativeSelectOption`; its
/// `group` is the `NativeSelectOptGroup` it is in).
pub type NativeSelectOption = nsh_base::SelectItem;

pub use checkbox::{Checkbox, CheckboxElement, CheckboxEvent, CheckboxProps};
pub use field::{
    Field, FieldContent, FieldContentElement, FieldContentProps, FieldDescription,
    FieldDescriptionElement, FieldDescriptionProps, FieldElement, FieldError, FieldErrorElement,
    FieldErrorProps, FieldGroup, FieldGroupElement, FieldGroupProps, FieldLabel, FieldLabelElement,
    FieldLabelProps, FieldLegend, FieldLegendElement, FieldLegendProps, FieldLegendVariant,
    FieldOrientation, FieldProps, FieldSeparator, FieldSeparatorElement, FieldSeparatorProps,
    FieldSet, FieldSetElement, FieldSetProps, FieldTitle, FieldTitleElement, FieldTitleProps,
};
pub use input::{
    Input, InputElement, InputEvent, InputProps, InputType, Textarea, TextareaElement,
    input_classes, textarea_classes,
};
pub use radio_group::{
    RadioGroup, RadioGroupElement, RadioGroupEvent, RadioGroupItem, RadioGroupItemElement,
    RadioGroupItemProps, RadioGroupProps,
};
pub use switch::{Switch, SwitchElement, SwitchEvent, SwitchProps, SwitchSize};

/// A text field: `input().placeholder(Some("Email".into())).email()`.
pub fn input() -> InputElement {
    input::input()
}

/// A text field of several lines, as tall as its text.
pub fn textarea() -> TextareaElement {
    input::textarea()
}

/// A checkbox: `checkbox().default_checked(true)`.
pub fn checkbox() -> CheckboxElement {
    checkbox::checkbox()
}

/// A switch: `switch().on_checked_change(..)`.
pub fn switch() -> SwitchElement {
    switch::switch()
}

/// A set of radio items, one checked: `radio_group().default_value(Some("a".into()))`.
pub fn radio_group() -> RadioGroupElement {
    radio_group::radio_group()
}

/// An option of a [`radio_group`], checked while the group holds `value`.
pub fn radio_group_item(value: impl Into<SharedString>) -> RadioGroupItemElement {
    radio_group::radio_group_item().value(value.into())
}

/// A set of fields with a legend.
pub fn field_set() -> FieldSetElement {
    field::field_set()
}

/// The title of a [`field_set`].
pub fn field_legend(label: impl Into<SharedString>) -> FieldLegendElement {
    field::field_legend().label(Some(label.into()))
}

/// A column of fields.
pub fn field_group() -> FieldGroupElement {
    field::field_group()
}

/// A field: a label, its control, a description, an error.
pub fn field() -> FieldElement {
    field::field()
}

/// The label and description beside a control in a horizontal [`field`].
pub fn field_content() -> FieldContentElement {
    field::field_content()
}

/// The label of a field's control; pressing it activates the control.
pub fn field_label(label: impl Into<SharedString>) -> FieldLabelElement {
    field::field_label().label(Some(label.into()))
}

/// A label's look for a part that labels nothing (a choice card's title).
pub fn field_title(label: impl Into<SharedString>) -> FieldTitleElement {
    field::field_title().label(Some(label.into()))
}

/// Help text for a field.
pub fn field_description(label: impl Into<SharedString>) -> FieldDescriptionElement {
    field::field_description().label(Some(label.into()))
}

/// A line between groups of fields.
pub fn field_separator() -> FieldSeparatorElement {
    field::field_separator()
}

/// What is wrong with a field's value: one message or a list.
pub fn field_error<S: Into<SharedString>>(
    errors: impl IntoIterator<Item = S>,
) -> FieldErrorElement {
    field::field_error().errors(errors.into_iter().map(Into::into).collect::<Vec<_>>())
}

/// A slider: `slider().default_value(vec![50.0])`.
pub fn slider() -> SliderElement {
    slider::slider()
}

/// A toggle button: `toggle().icon(Some(IconName::Italic))`.
pub fn toggle() -> ToggleElement {
    toggle::toggle()
}

/// A group of toggles: `toggle_group().r#type(ToggleGroupType::Multiple)`.
pub fn toggle_group() -> ToggleGroupElement {
    toggle_group::toggle_group()
}

/// A toggle of a [`toggle_group`], on while the group holds `value`.
pub fn toggle_group_item(value: impl Into<SharedString>) -> ToggleGroupItemElement {
    toggle_group::toggle_group_item().value(value.into())
}

/// Buttons joined into one bar: `button_group().item(button("A").outline())`.
pub fn button_group() -> ButtonGroupElement {
    button_group::button_group()
}

/// A text joined to a [`button_group`]'s buttons.
pub fn button_group_text(label: impl Into<SharedString>) -> ButtonGroupTextElement {
    button_group::button_group_text().label(Some(label.into()))
}

/// A line between two buttons of a [`button_group`].
pub fn button_group_separator() -> ButtonGroupSeparatorElement {
    button_group::button_group_separator()
}

/// A select styled as a browser's: `native_select(vec![native_select_option("todo", "Todo")])`.
pub fn native_select(options: impl IntoIterator<Item = NativeSelectOption>) -> NativeSelectElement {
    native_select::native_select().options(options.into_iter().collect::<Vec<_>>())
}

/// An option of a [`native_select`]; `.group("Engineering")` puts it in a group.
pub fn native_select_option(
    value: impl Into<SharedString>,
    title: impl Into<SharedString>,
) -> NativeSelectOption {
    NativeSelectOption::new(value, title)
}

/// A one-time code of `max_length` digits: `input_otp(6).groups(2)`.
pub fn input_otp(max_length: u32) -> InputOtpElement {
    input_otp::input_otp().max_length(max_length)
}

/// A field with addons inside its border.
pub fn input_group() -> InputGroupElement {
    input_group::input_group()
}

/// An addon of an [`input_group`]: `.align(InputGroupAddonAlign::InlineEnd)`.
pub fn input_group_addon() -> InputGroupAddonElement {
    input_group::input_group_addon()
}

/// A small button in an addon: `input_group_button().label(Some("Search".into()))`.
pub fn input_group_button() -> InputGroupButtonElement {
    input_group::input_group_button()
}

/// Muted text in an addon.
pub fn input_group_text(label: impl Into<SharedString>) -> InputGroupTextElement {
    input_group::input_group_text().label(Some(label.into()))
}

/// The single-line field of an [`input_group`].
pub fn input_group_input() -> InputGroupInputElement {
    input_group::input_group_input()
}

/// The field of several lines of an [`input_group`].
pub fn input_group_textarea() -> InputGroupTextareaElement {
    input_group::input_group_textarea()
}

/// Registers this family's components for TSX (`nsh-shadcn`).
pub(crate) fn register(registry: &mut Registry) {
    button_group::register(registry);
    checkbox::register(registry);
    input_group::register(registry);
    input_otp::register(registry);
    native_select::register(registry);
    slider::register(registry);
    toggle::register(registry);
    toggle_group::register(registry);
    field::register(registry);
    input::register(registry);
    radio_group::register(registry);
    switch::register(registry);
}
