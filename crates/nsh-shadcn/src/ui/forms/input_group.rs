//! [`InputGroup`] and its parts, after shadcn/ui's `input-group.tsx`
//! (new-york-v4): [`InputGroupAddon`], [`InputGroupButton`],
//! [`InputGroupText`], [`InputGroupInput`] and [`InputGroupTextarea`].
//!
//! The group's classes are shadcn's, word for word; its selectors on its
//! children (`has-[>[data-align=…]]:…`, `has-[>textarea]:h-auto`,
//! `[&>input]:pl-2`, …) are the parts' context: an addon tells the group
//! its alignment, and the group and its control take the classes those
//! selectors give. The other changes:
//!
//! - `order-first` / `order-last` (unknown to NSH, which has no `order`):
//!   the parts show in the order they are given; put a start addon before
//!   the control, an end addon after it.
//! - `has-[[data-slot=input-group-control]:focus-visible]:…` is
//!   `focus-within:…` (a text field always shows its focus).
//! - The error state, `has-[[data-slot][aria-invalid=true]]:…`, is the
//!   group's `invalid` prop's; `group-data-[disabled=true]/input-group:`
//!   its `disabled` prop's (shadcn's `data-disabled`).
//! - Addon's `has-[>button]:ml-[-0.45rem]` follows an
//!   [`InputGroupButton`] in it; `[&>kbd]:…` (shadcn's Kbd, another
//!   family's) and `[.border-b]:pb-3` / `[.border-t]:pt-3` (a class the
//!   app adds) are the app's to add with the border.
//! - `rounded-[calc(var(--radius)-5px)]` (unknown to NSH) is the length it
//!   comes to, `rounded-[5px]`; `[&>svg…]:size-3.5` / `size-4` are the
//!   button's icon's classes.
//! - `transition-[color,box-shadow]` is `transition`; `resize-none` (a
//!   native field cannot be resized) is left out; `shrink` is added (CSS's
//!   default for a flex item).
//! - A press on an addon focuses the group's control, as shadcn's
//!   `onClick` does.

use nsh_core::prelude::*;

use super::input::{INPUT, InputEvent, InputProps, TEXTAREA, text_field, textarea_field};
use crate::cn::cn;
use crate::icon::{IconName, icon};
use crate::ui::button::{ButtonVariant, button as base_button};

/// shadcn's InputGroup classes.
const GROUP: &str = "group/input-group relative flex w-full items-center rounded-md border \
    border-input shadow-xs transition outline-none dark:bg-input/30 h-9 min-w-0 shrink \
    focus-within:border-ring focus-within:ring-[3px] focus-within:ring-ring/50";
/// shadcn's error classes, while invalid.
const GROUP_INVALID: &str = "border-destructive ring-destructive/20 dark:ring-destructive/40";
/// shadcn's addon base classes.
const ADDON: &str = "flex h-auto cursor-text items-center justify-center gap-2 py-1.5 text-sm \
    font-medium text-muted-foreground select-none";
/// shadcn's InputGroupText classes.
const TEXT: &str = "flex items-center gap-2 text-sm text-muted-foreground";
/// shadcn's InputGroupButton base classes.
const BUTTON: &str = "flex items-center gap-2 text-sm shadow-none";
/// shadcn's InputGroupInput classes after Input's.
const CONTROL_INPUT: &str = "flex-1 rounded-none border-0 bg-transparent shadow-none focus-visible:ring-0 dark:bg-transparent";
/// shadcn's InputGroupTextarea classes after Textarea's.
const CONTROL_TEXTAREA: &str = "flex-1 rounded-none border-0 bg-transparent py-3 shadow-none \
    focus-visible:ring-0 dark:bg-transparent";

/// Where an addon goes: shadcn's `align`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum InputGroupAddonAlign {
    /// Before the control, on its line.
    #[default]
    InlineStart,
    /// After the control, on its line.
    InlineEnd,
    /// Above the control, across the group.
    BlockStart,
    /// Below the control, across the group.
    BlockEnd,
}

/// What the parts of a group tell it.
#[derive(Clone, Copy)]
struct GroupContext {
    inline_start: Signal<bool>,
    inline_end: Signal<bool>,
    block_start: Signal<bool>,
    block_end: Signal<bool>,
    textarea: Signal<bool>,
    input: Signal<bool>,
    disabled: Prop<bool>,
    control: Stored<Option<ElementRef>>,
}

impl GroupContext {
    fn align(self, align: InputGroupAddonAlign) -> Signal<bool> {
        match align {
            InputGroupAddonAlign::InlineStart => self.inline_start,
            InputGroupAddonAlign::InlineEnd => self.inline_end,
            InputGroupAddonAlign::BlockStart => self.block_start,
            InputGroupAddonAlign::BlockEnd => self.block_end,
        }
    }

    fn blocks(self) -> bool {
        self.block_start.get() || self.block_end.get()
    }
}

/// Every class the group draws with, before the app's.
pub(crate) fn group_classes(textarea: bool, blocks: bool, invalid: bool) -> SharedString {
    cn([
        GROUP,
        if textarea { "h-auto" } else { "" },
        if blocks { "h-auto flex-col" } else { "" },
        if invalid { GROUP_INVALID } else { "" },
    ])
}

/// The props of [`InputGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct InputGroupProps {
    /// Its control fails the app's checks: a destructive border.
    #[prop(default)]
    pub invalid: Prop<bool>,
    /// Its addons dim (shadcn's `data-disabled`); disable the control too.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// The addons and the control, in the order they show.
    pub children: Prop<Children>,
}

/// shadcn's InputGroup: a field with addons (icons, text, buttons) inside
/// its border.
#[component]
pub fn InputGroup(props: InputGroupProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let group = GroupContext {
        inline_start: signal(false),
        inline_end: signal(false),
        block_start: signal(false),
        block_end: signal(false),
        textarea: signal(false),
        input: signal(false),
        disabled: props.disabled,
        control: stored(None),
    };
    provide_context(group);
    view()
        .role(AccessibilityRole::Group)
        .app_class(move || {
            Some(group_classes(
                group.textarea.get(),
                group.blocks(),
                props.invalid.get(),
            ))
        })
        .child(props.children)
}

/// What an addon's button tells it.
#[derive(Clone, Copy)]
struct AddonContext {
    has_button: Signal<bool>,
}

/// The props of [`InputGroupAddon`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct InputGroupAddonProps {
    /// `inline-start`, `inline-end`, `block-start` or `block-end`.
    #[prop(default)]
    pub align: Prop<InputGroupAddonAlign>,
    /// A Lucide icon first.
    pub icon: Prop<Option<IconName>>,
    /// Text after the icon.
    pub label: Prop<Option<SharedString>>,
    /// Content after the text: [`InputGroupText`], [`InputGroupButton`].
    pub children: Prop<Children>,
}

/// Every class an addon draws with, before the app's.
pub(crate) fn addon_classes(
    align: InputGroupAddonAlign,
    has_button: bool,
    beside_input: bool,
    disabled: bool,
) -> SharedString {
    let align_classes = match align {
        InputGroupAddonAlign::InlineStart => "pl-3",
        InputGroupAddonAlign::InlineEnd => "pr-3",
        InputGroupAddonAlign::BlockStart => "w-full justify-start px-3 pt-3",
        InputGroupAddonAlign::BlockEnd => "w-full justify-start px-3 pb-3",
    };
    let button = match (align, has_button) {
        (InputGroupAddonAlign::InlineStart, true) => "ml-[-0.45rem]",
        (InputGroupAddonAlign::InlineEnd, true) => "mr-[-0.45rem]",
        _ => "",
    };
    let input = match (align, beside_input) {
        (InputGroupAddonAlign::BlockStart, true) => "pt-2.5",
        (InputGroupAddonAlign::BlockEnd, true) => "pb-2.5",
        _ => "",
    };
    cn([
        ADDON,
        align_classes,
        button,
        input,
        if disabled { "opacity-50" } else { "" },
    ])
}

/// shadcn's InputGroupAddon: an icon, text or buttons in the group.
#[component]
pub fn InputGroupAddon(props: InputGroupAddonProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let group = use_context::<GroupContext>();
    if let Some(group) = group {
        let align = props.align;
        effect(move || group.align(align.get()).set(true));
    }
    let addon = AddonContext {
        has_button: signal(false),
    };
    provide_context(addon);
    view()
        .role(AccessibilityRole::Group)
        .on_press(move || {
            if let Some(control) = group.and_then(|g| g.control.get()) {
                control.focus();
            }
        })
        .app_class(move || {
            Some(addon_classes(
                props.align.get(),
                addon.has_button.get(),
                group.is_some_and(|g| g.input.get()),
                group.is_some_and(|g| g.disabled.get()),
            ))
        })
        .child(dynamic(move || {
            props.icon.get().map(|name| icon(name).into_element())
        }))
        .child(dynamic(move || {
            props.label.get().map(|label| text(label).into_element())
        }))
        .child(props.children)
}

/// How big an addon's button is: shadcn's InputGroupButton `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum InputGroupButtonSize {
    /// 24 points tall.
    #[default]
    Xs,
    /// 32 points tall.
    Sm,
    /// A 24-point square.
    IconXs,
    /// A 32-point square.
    IconSm,
}

impl InputGroupButtonSize {
    fn classes(self) -> &'static str {
        match self {
            InputGroupButtonSize::Xs => "h-6 gap-1 rounded-[5px] px-2",
            InputGroupButtonSize::Sm => "h-8 gap-1.5 rounded-md px-2.5",
            InputGroupButtonSize::IconXs => "size-6 rounded-[5px] p-0",
            InputGroupButtonSize::IconSm => "size-8 p-0",
        }
    }

    fn icon(self) -> &'static str {
        match self {
            InputGroupButtonSize::Xs => "size-3.5",
            _ => "size-4",
        }
    }
}

/// Every class a button draws with, after Button's.
pub(crate) fn button_classes(size: InputGroupButtonSize) -> SharedString {
    cn([BUTTON, size.classes()])
}

/// The props of [`InputGroupButton`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct InputGroupButtonProps {
    /// The text on the button.
    pub label: Prop<Option<SharedString>>,
    /// A Button `variant` (default `ghost`).
    #[prop(default = ButtonVariant::Ghost)]
    pub variant: Prop<ButtonVariant>,
    /// `xs`, `sm`, `icon-xs` or `icon-sm`.
    #[prop(default)]
    pub size: Prop<InputGroupButtonSize>,
    /// A Lucide icon after the label (alone in the `icon-*` sizes).
    pub icon: Prop<Option<IconName>>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// What screen readers say (an icon button's name).
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// The events of an [`InputGroupButton`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum InputGroupButtonEvent {
    /// It was pressed.
    Press,
}

/// shadcn's InputGroupButton: a small Button in an addon.
#[component]
pub fn InputGroupButton(
    props: InputGroupButtonProps,
    cx: Scope<InputGroupButtonEvent>,
) -> impl IntoElement {
    if let Some(addon) = use_context::<AddonContext>() {
        addon.has_button.set(true);
    }
    base_button()
        .label(props.label)
        .variant(props.variant)
        .disabled(props.disabled)
        .accessibility_label(props.accessibility_label)
        .on_press(move || cx.emit(InputGroupButtonEvent::Press))
        .app_class(move || Some(button_classes(props.size.get())))
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(Some(SharedString::from(props.size.get().icon())))
                    .into_element()
            })
        }))
}

/// The props of [`InputGroupText`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct InputGroupTextProps {
    /// A Lucide icon before the text.
    pub icon: Prop<Option<IconName>>,
    /// The text.
    pub label: Prop<Option<SharedString>>,
}

/// shadcn's InputGroupText: muted text in an addon (`https://`, `USD`).
#[component]
pub fn InputGroupText(props: InputGroupTextProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .app_class(Some(SharedString::from(TEXT)))
        .child(dynamic(move || {
            props.icon.get().map(|name| icon(name).into_element())
        }))
        .child(dynamic(move || {
            props.label.get().map(|label| text(label).into_element())
        }))
}

/// Every class the group's input draws with, before the app's.
pub(crate) fn input_classes(
    start: bool,
    end: bool,
    block_start: bool,
    block_end: bool,
) -> SharedString {
    cn([
        INPUT,
        CONTROL_INPUT,
        if start { "pl-2" } else { "" },
        if end { "pr-2" } else { "" },
        if block_start { "pb-3" } else { "" },
        if block_end { "pt-3" } else { "" },
    ])
}

/// shadcn's InputGroupInput: the group's single-line field.
#[component]
pub fn InputGroupInput(props: InputProps, cx: Scope<InputEvent>) -> impl IntoElement {
    let group = use_context::<GroupContext>();
    let field_ref = element_ref();
    if let Some(group) = group {
        group.input.set(true);
        group.control.replace(Some(field_ref));
    }
    text_field(&props, cx, false, field_ref, move || match group {
        Some(g) => input_classes(
            g.inline_start.get(),
            g.inline_end.get(),
            g.block_start.get(),
            g.block_end.get(),
        ),
        None => input_classes(false, false, false, false),
    })
}

/// How much taller than its text the group's textarea is: its `py-3`.
const CONTROL_TEXTAREA_CHROME: f32 = 2.0 * 12.0;

/// shadcn's InputGroupTextarea: the group's field of several lines.
#[component]
pub fn InputGroupTextarea(props: InputProps, cx: Scope<InputEvent>) -> impl IntoElement {
    let group = use_context::<GroupContext>();
    let field_ref = element_ref();
    if let Some(group) = group {
        group.textarea.set(true);
        group.control.replace(Some(field_ref));
    }
    textarea_field(&props, cx, field_ref, CONTROL_TEXTAREA_CHROME, || {
        cn([TEXTAREA, CONTROL_TEXTAREA])
    })
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<InputGroup>();
    registry.register::<InputGroupAddon>();
    registry.register::<InputGroupButton>();
    registry.register::<InputGroupText>();
    registry.register::<InputGroupInput>();
    registry.register::<InputGroupTextarea>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = vec![
            group_classes(true, true, true),
            input_classes(true, true, true, true),
            cn([TEXTAREA, CONTROL_TEXTAREA]),
            SharedString::from(TEXT),
        ];
        for align in [
            InputGroupAddonAlign::InlineStart,
            InputGroupAddonAlign::InlineEnd,
            InputGroupAddonAlign::BlockStart,
            InputGroupAddonAlign::BlockEnd,
        ] {
            all.push(addon_classes(align, true, true, true));
        }
        for size in [
            InputGroupButtonSize::Xs,
            InputGroupButtonSize::Sm,
            InputGroupButtonSize::IconXs,
            InputGroupButtonSize::IconSm,
        ] {
            all.push(cn([&button_classes(size), size.icon()]));
        }
        let mut unknown = Vec::new();
        for classes in all {
            unknown.extend(crate::cn::unknown_classes(&classes));
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
