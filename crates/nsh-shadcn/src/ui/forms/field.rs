//! [`Field`] and its parts, after shadcn/ui's `field.tsx` (new-york-v4):
//! [`FieldSet`], [`FieldLegend`], [`FieldGroup`], [`Field`],
//! [`FieldContent`], [`FieldLabel`], [`FieldTitle`], [`FieldDescription`],
//! [`FieldSeparator`] and [`FieldError`]. They lay out a form (shadcn's
//! `form.tsx` is react-hook-form's binding and is not ported: Field is the
//! form layout, and the app checks its values and shows [`FieldError`]).
//!
//! shadcn composes `Label` (FieldLabel) and `Separator` (FieldSeparator);
//! they are drawn here with those components' classes (`label.tsx`,
//! `separator.tsx`), so this family depends on no other.
//!
//! The classes are shadcn's, word for word, except where they select
//! other elements, which NSH cannot (each part tells the others what CSS
//! would find through a context):
//!
//! - `data-[invalid=true]:` / `data-[disabled=true]:` are the `invalid` /
//!   `disabled` props of [`Field`]; its parts read them
//!   (`group-data-[disabled=true]/field:opacity-50`).
//! - Field's `[&>*]:w-full` (vertical): a column stretches its children in
//!   NSH as it does; the label, `w-fit` otherwise, is `w-full` there.
//! - Field's `[&>[data-slot=field-label]]:flex-auto` (horizontal): the
//!   label's own `flex-auto`. `has-[>[data-slot=field-content]]:items-start`
//!   follows a [`FieldContent`] inside it, and the checkbox's or radio's
//!   `mt-px` beside one is theirs.
//! - `responsive`: container queries (`@container/field-group`,
//!   `@md/field-group:`) are unknown to NSH; the window's `md:` breakpoint
//!   stands in for the field group's width.
//! - FieldLabel's `has-[>[data-slot=field]]:…` (a choice card: a label
//!   around a Field) applies when a [`Field`] is inside it, and
//!   `has-data-[state=checked]:…` while that field's control is checked;
//!   the inner field's `[&>*]:data-[slot=field]:p-4` is its own `p-4`.
//! - FieldDescription's `last:mt-0 nth-last-2:-mt-1`: `-mt-1 last:mt-0`
//!   (NSH has `last:` but no `nth-last-*`), so a description before the
//!   last two children of its parent pulls up too; `[[data-variant=legend]+&]:-mt-1.5`
//!   applies in a [`FieldSet`] whose legend is a `legend`.
//! - FieldSet's `has-[>[data-slot=radio-group]]:gap-3` follows a radio
//!   group placed in it; FieldGroup's `[&>[data-slot=field-group]]:gap-4`
//!   is a nested group's own.
//! - FieldLegend: a `<legend>` is laid out by the browser outside its
//!   fieldset's flex gap; here it is a flex item, so its `mb-3` less the
//!   set's gap is a negative margin.
//! - FieldSeparator: `-my-2 h-5`, the line `absolute inset-0 top-1/2`, as
//!   shadcn's; the line is `separator.tsx`'s `bg-border h-px w-full`.
//! - FieldError's list (`list-disc`, unknown to NSH) draws its bullets as
//!   text.
//! - `text-balance`, `[&>a]:…` (links in a description) are left out.
//!
//! A press on a [`FieldLabel`] activates its field's control (a checkbox,
//! a switch, a radio; focuses an input), as an HTML `<label htmlFor>`.

use std::rc::Rc;

use nsh_core::prelude::*;

use crate::cn::cn;

/// `label.tsx`'s classes (FieldLabel is a `Label`).
pub(crate) const LABEL: &str =
    "flex items-center gap-2 text-sm leading-none font-medium select-none";

/// The props of [`FieldSet`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldSetProps {
    /// The legend, description and fields.
    pub children: Prop<Children>,
}

/// What a part placed directly in a [`FieldSet`] or [`FieldGroup`] tells
/// it, and learns of it: CSS's child and sibling selectors.
#[derive(Clone, Copy)]
pub(crate) enum FieldParent {
    /// Directly in a FieldSet.
    Set(SetSignals),
    /// Directly in a FieldGroup.
    Group,
    /// Anywhere else.
    Other,
}

/// What a FieldSet knows of its children.
#[derive(Clone, Copy)]
pub(crate) struct SetSignals {
    /// The variant of its legend, when it has one.
    legend: Signal<Option<FieldLegendVariant>>,
    /// It holds a radio group (`has-[>[data-slot=radio-group]]:gap-3`).
    choice_group: Signal<bool>,
}

impl FieldParent {
    /// The parent around the current part.
    pub(crate) fn current() -> Self {
        use_context::<FieldParent>().unwrap_or(FieldParent::Other)
    }

    /// A radio group was placed here.
    pub(crate) fn holds_choice_group(self) {
        if let FieldParent::Set(set) = self {
            set.choice_group.set(true);
        }
    }
}

/// shadcn's FieldSet: a `<fieldset>`, `flex flex-col gap-6`.
#[component]
pub fn FieldSet(props: FieldSetProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let set = SetSignals {
        legend: signal(None),
        choice_group: signal(false),
    };
    provide_context(FieldParent::Set(set));
    view()
        .role(AccessibilityRole::Group)
        .app_class(move || {
            Some(cn([
                "flex flex-col gap-6",
                if set.choice_group.get() { "gap-3" } else { "" },
            ]))
        })
        .child(props.children)
}

/// FieldLegend's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum FieldLegendVariant {
    /// A legend, `text-base`.
    #[default]
    Legend,
    /// A label's size, `text-sm`.
    Label,
}

/// The props of [`FieldLegend`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldLegendProps {
    /// The legend's text.
    pub label: Prop<Option<SharedString>>,
    /// `legend` or `label`.
    #[prop(default)]
    pub variant: Prop<FieldLegendVariant>,
}

/// Every class a legend draws with, before the app's; `set_gap` is its
/// set's gap in points (its `mb-3` is outside that gap in a browser).
pub(crate) fn legend_classes(variant: FieldLegendVariant, set_gap_3: bool) -> SharedString {
    cn([
        "mb-3 font-medium",
        match variant {
            FieldLegendVariant::Legend => "text-base",
            FieldLegendVariant::Label => "text-sm",
        },
        if set_gap_3 { "mb-0" } else { "-mb-3" },
    ])
}

/// shadcn's FieldLegend: the title of a [`FieldSet`].
#[component]
pub fn FieldLegend(props: FieldLegendProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let parent = FieldParent::current();
    if let FieldParent::Set(set) = parent {
        let variant = props.variant;
        effect(move || set.legend.set(Some(variant.get())));
    }
    let gap_3 = move || matches!(parent, FieldParent::Set(set) if set.choice_group.get());
    text(move || props.label.get().unwrap_or_default())
        .role(AccessibilityRole::Header)
        .app_class(move || Some(legend_classes(props.variant.get(), gap_3())))
}

/// The props of [`FieldGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldGroupProps {
    /// The fields.
    pub children: Prop<Children>,
}

/// shadcn's FieldGroup: a column of fields, `gap-7`.
#[component]
pub fn FieldGroup(props: FieldGroupProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let nested = matches!(FieldParent::current(), FieldParent::Group);
    provide_context(FieldParent::Group);
    view()
        .app_class(Some(cn([
            "group/field-group flex w-full flex-col gap-7",
            if nested { "gap-4" } else { "" },
        ])))
        .child(props.children)
}

/// Field's `orientation`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum FieldOrientation {
    /// The label above the control.
    #[default]
    Vertical,
    /// The control beside its label.
    Horizontal,
    /// Vertical, horizontal from the `md` breakpoint.
    Responsive,
}

impl FieldOrientation {
    fn classes(self, has_content: bool) -> &'static str {
        match (self, has_content) {
            (FieldOrientation::Vertical, _) => "flex-col",
            (FieldOrientation::Horizontal, false) => "flex-row items-center",
            (FieldOrientation::Horizontal, true) => "flex-row items-center items-start",
            (FieldOrientation::Responsive, false) => "flex-col md:flex-row md:items-center",
            (FieldOrientation::Responsive, true) => {
                "flex-col md:flex-row md:items-center md:items-start"
            }
        }
    }
}

/// What the parts of a [`Field`] share: its props, whether it has a
/// [`FieldContent`], and its control (what a press on its label does).
#[derive(Clone, Copy)]
pub(crate) struct FieldContext {
    orientation: Prop<FieldOrientation>,
    disabled: Prop<bool>,
    has_content: Signal<bool>,
    activate: Stored<Option<Rc<dyn Fn()>>>,
    checked: Signal<bool>,
}

impl FieldContext {
    /// A [`FieldContent`] is in the field.
    pub(crate) fn has_content(self) -> bool {
        self.has_content.get()
    }

    /// The field is laid out horizontally.
    pub(crate) fn horizontal(self) -> bool {
        self.orientation.get() == FieldOrientation::Horizontal
    }

    /// The field is disabled.
    pub(crate) fn disabled(self) -> bool {
        self.disabled.get()
    }

    /// The field's control: `activate` is what a press on its label does,
    /// `checked` its checked state (a choice card's border follows it).
    pub(crate) fn control(
        self,
        activate: impl Fn() + 'static,
        checked: impl Fn() -> bool + 'static,
    ) {
        self.activate.replace(Some(Rc::new(activate)));
        let field_checked = self.checked;
        effect(move || field_checked.set(checked()));
    }

    /// The field's control, given only what a press on the label does (an
    /// input's focus).
    pub(crate) fn control_press(self, activate: impl Fn() + 'static) {
        self.activate.replace(Some(Rc::new(activate)));
    }

    fn activate(self) {
        if let Some(activate) = self.activate.get() {
            activate();
        }
    }
}

/// What a [`FieldLabel`] tells a [`Field`] inside it (a choice card).
#[derive(Clone, Copy)]
struct ChoiceCard {
    field: Signal<Option<FieldContextCell>>,
}

/// A [`FieldContext`] in a signal (it holds no `PartialEq`).
#[derive(Clone, Copy)]
struct FieldContextCell(FieldContext);

impl PartialEq for FieldContextCell {
    fn eq(&self, other: &Self) -> bool {
        self.0.has_content == other.0.has_content
    }
}

/// The props of [`Field`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldProps {
    /// `vertical`, `horizontal` or `responsive`.
    #[prop(default)]
    pub orientation: Prop<FieldOrientation>,
    /// Its value fails the app's checks: its text turns destructive
    /// (shadcn's `data-invalid`).
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// Its label and title dim (shadcn's `data-disabled`); disable the
    /// control too.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The label, control, description, error.
    pub children: Prop<Children>,
}

/// Every class a field draws with, before the app's.
pub(crate) fn field_classes(
    orientation: FieldOrientation,
    has_content: bool,
    invalid: bool,
    in_card: bool,
) -> SharedString {
    cn([
        "group/field flex w-full gap-3",
        if invalid { "text-destructive" } else { "" },
        orientation.classes(has_content),
        if in_card { "p-4" } else { "" },
    ])
}

/// shadcn's Field: a label, its control, a description and an error.
#[component]
pub fn Field(props: FieldProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let context = FieldContext {
        orientation: props.orientation,
        disabled: props.disabled,
        has_content: signal(false),
        activate: stored(None),
        checked: signal(false),
    };
    let card = use_context::<ChoiceCard>();
    if let Some(card) = card {
        card.field.set(Some(FieldContextCell(context)));
    }
    provide_context(context);
    provide_context(FieldParent::Other);
    view()
        .role(AccessibilityRole::Group)
        .app_class(move || {
            Some(field_classes(
                props.orientation.get(),
                context.has_content.get(),
                props.invalid.get(),
                card.is_some(),
            ))
        })
        .child(props.children)
}

/// The props of [`FieldContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldContentProps {
    /// A label (or title) and its description.
    pub children: Prop<Children>,
}

/// shadcn's FieldContent: the label and description beside a control.
#[component]
pub fn FieldContent(props: FieldContentProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    if let Some(field) = use_context::<FieldContext>() {
        field.has_content.set(true);
    }
    provide_context(FieldParent::Other);
    view()
        .app_class(Some(SharedString::from(
            "group/field-content flex flex-1 flex-col gap-1.5 leading-snug",
        )))
        .child(props.children)
}

/// The props of [`FieldLabel`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldLabelProps {
    /// The label's text.
    pub label: Prop<Option<SharedString>>,
    /// Content after the text: a [`Field`] makes it a choice card.
    pub children: Prop<Children>,
}

/// Every class a label draws with, before the app's.
pub(crate) fn label_classes(
    vertical: bool,
    horizontal: bool,
    disabled: bool,
    card: bool,
    checked: bool,
) -> SharedString {
    cn([
        LABEL,
        "group/field-label flex w-fit gap-2 leading-snug",
        if disabled { "opacity-50" } else { "" },
        if vertical { "w-full" } else { "" },
        if horizontal { "flex-auto" } else { "" },
        if card {
            "w-full flex-col rounded-md border border-border"
        } else {
            ""
        },
        if card && checked {
            "border-primary bg-primary/5 dark:bg-primary/10"
        } else {
            ""
        },
    ])
}

/// shadcn's FieldLabel: a `Label` for its field's control. A press on it
/// activates the control.
#[component]
pub fn FieldLabel(props: FieldLabelProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let outer = use_context::<FieldContext>();
    let card = ChoiceCard {
        field: signal(None),
    };
    provide_context(card);
    provide_context(FieldParent::Other);
    let inner = move || card.field.get().map(|cell| cell.0);
    view()
        .on_press(move || {
            if let Some(field) = inner().or(outer) {
                field.activate();
            }
        })
        .app_class(move || {
            let card_field = inner();
            Some(label_classes(
                outer.is_some_and(|f| f.orientation.get() == FieldOrientation::Vertical),
                outer.is_some_and(|f| f.horizontal()),
                outer.is_some_and(|f| f.disabled()),
                card_field.is_some(),
                card_field.is_some_and(|f| f.checked.get()),
            ))
        })
        .child(dynamic(move || {
            props.label.get().map(|label| text(label).into_element())
        }))
        .child(props.children)
}

/// The props of [`FieldTitle`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldTitleProps {
    /// The title's text.
    pub label: Prop<Option<SharedString>>,
    /// Content after the text.
    pub children: Prop<Children>,
}

/// shadcn's FieldTitle: a label's look on a part that labels nothing (a
/// choice card's title).
#[component]
pub fn FieldTitle(props: FieldTitleProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let field = use_context::<FieldContext>();
    view()
        .app_class(move || {
            Some(cn([
                "flex w-fit flex-row items-center gap-2 text-sm leading-snug font-medium",
                if field.is_some_and(|f| f.disabled()) {
                    "opacity-50"
                } else {
                    ""
                },
            ]))
        })
        .child(dynamic(move || {
            props.label.get().map(|label| text(label).into_element())
        }))
        .child(props.children)
}

/// The props of [`FieldDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldDescriptionProps {
    /// The description's text.
    pub label: Prop<Option<SharedString>>,
}

/// Every class a description draws with, before the app's.
pub(crate) fn description_classes(after_legend: bool) -> SharedString {
    cn([
        "text-sm leading-normal font-normal text-muted-foreground",
        "-mt-1 last:mt-0",
        if after_legend { "-mt-1.5" } else { "" },
    ])
}

/// shadcn's FieldDescription: help text under a label or a control.
#[component]
pub fn FieldDescription(props: FieldDescriptionProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let parent = FieldParent::current();
    text(move || props.label.get().unwrap_or_default()).app_class(move || {
        let after_legend = matches!(
            parent,
            FieldParent::Set(set) if set.legend.get() == Some(FieldLegendVariant::Legend)
        );
        Some(description_classes(after_legend))
    })
}

/// The props of [`FieldSeparator`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldSeparatorProps {
    /// Text on the line ("Or continue with").
    pub label: Prop<Option<SharedString>>,
}

/// shadcn's FieldSeparator classes, then the line's (`separator.tsx`'s
/// horizontal Separator and FieldSeparator's), then the label's.
const SEPARATOR: &str = "relative -my-2 h-5 text-sm";
const SEPARATOR_LINE: &str = "shrink-0 bg-border h-px w-full absolute inset-0 top-1/2";
const SEPARATOR_LABEL: &str =
    "relative mx-auto block w-fit bg-background px-2 text-muted-foreground";

/// shadcn's FieldSeparator: a line between groups of fields, with an
/// optional label on it.
#[component]
pub fn FieldSeparator(props: FieldSeparatorProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .app_class(Some(SharedString::from(SEPARATOR)))
        .child(view().app_class(Some(SharedString::from(SEPARATOR_LINE))))
        .child(dynamic(move || {
            props.label.get().map(|label| {
                text(label)
                    .app_class(Some(SharedString::from(SEPARATOR_LABEL)))
                    .into_element()
            })
        }))
}

/// The props of [`FieldError`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct FieldErrorProps {
    /// The error messages: one is shown as it is, several as a list, the
    /// same message once. None: nothing is shown.
    #[prop(default)]
    pub errors: Prop<Vec<SharedString>>,
}

/// shadcn's FieldError: what is wrong with the field's value.
#[component]
pub fn FieldError(props: FieldErrorProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    dynamic(move || {
        let mut errors: Vec<SharedString> = Vec::new();
        for error in props.errors.get() {
            if !error.is_empty() && !errors.contains(&error) {
                errors.push(error);
            }
        }
        if errors.is_empty() {
            return None;
        }
        let root = view()
            .role(AccessibilityRole::Alert)
            .app_class(Some(SharedString::from(
                "text-sm font-normal text-destructive",
            )));
        Some(if errors.len() == 1 {
            root.child(text(errors[0].clone())).into_element()
        } else {
            let mut list = view().app_class(Some(SharedString::from("ml-4 flex flex-col gap-1")));
            for error in errors {
                list = list.child(
                    view()
                        .flex_row()
                        .child(text("•").w(px(16.0)).ml(px(-16.0)))
                        .child(text(error)),
                );
            }
            root.child(list).into_element()
        })
    })
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<FieldSet>();
    registry.register::<FieldLegend>();
    registry.register::<FieldGroup>();
    registry.register::<Field>();
    registry.register::<FieldContent>();
    registry.register::<FieldLabel>();
    registry.register::<FieldTitle>();
    registry.register::<FieldDescription>();
    registry.register::<FieldSeparator>();
    registry.register::<FieldError>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = vec![
            SharedString::from("flex flex-col gap-6 gap-3"),
            SharedString::from("group/field-group flex w-full flex-col gap-7 gap-4"),
            SharedString::from("group/field-content flex flex-1 flex-col gap-1.5 leading-snug"),
            SharedString::from(
                "flex w-fit flex-row items-center gap-2 text-sm leading-snug font-medium opacity-50",
            ),
            SharedString::from("text-sm font-normal text-destructive ml-4 flex flex-col gap-1"),
            SharedString::from(SEPARATOR),
            SharedString::from(SEPARATOR_LINE),
            SharedString::from(SEPARATOR_LABEL),
            description_classes(false),
            description_classes(true),
        ];
        for variant in [FieldLegendVariant::Legend, FieldLegendVariant::Label] {
            all.push(legend_classes(variant, false));
            all.push(legend_classes(variant, true));
        }
        for orientation in [
            FieldOrientation::Vertical,
            FieldOrientation::Horizontal,
            FieldOrientation::Responsive,
        ] {
            all.push(field_classes(orientation, false, false, false));
            all.push(field_classes(orientation, true, true, true));
        }
        all.push(label_classes(true, false, false, false, false));
        all.push(label_classes(false, true, true, true, true));
        for classes in all {
            let unknown = crate::cn::unknown_classes(&classes);
            assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
        }
    }
}
