//! The sidebar, after shadcn/ui's `sidebar.tsx` (new-york-v4):
//! [`SidebarProvider`], [`Sidebar`], [`SidebarHeader`], [`SidebarContent`],
//! [`SidebarFooter`], [`SidebarGroup`], [`SidebarGroupLabel`],
//! [`SidebarGroupContent`], [`SidebarMenu`], [`SidebarMenuItem`],
//! [`SidebarMenuButton`], [`SidebarSeparator`], [`SidebarTrigger`] and
//! [`SidebarInset`]. It collapses off the canvas or to its icons, with
//! `mod+b` or its trigger.
//!
//! The classes are shadcn's, word for word, except where NSH has no such
//! class or the parts differ:
//!
//! - `group-data-[collapsible=…]`, `group-data-[variant=…]`,
//!   `group-data-[side=…]` and `peer-data-[…]` read the sidebar's state,
//!   which its parts know through context: each picks the classes of the
//!   state it is in.
//! - The CSS variables are written out (NSH has no `calc()`):
//!   `w-(--sidebar-width)` is `w-[256px]` (16rem), `--sidebar-width-icon`
//!   48 points, `calc(var(--sidebar-width-icon)+(--spacing(4)))` 64 and
//!   its `+2px` 66, `left-[calc(var(--sidebar-width)*-1)]` `left-[-256px]`.
//! - The sidebar's container is `fixed inset-y-0 h-svh` in shadcn: placed
//!   in the window. Here it is `absolute inset-y-0` in the sidebar, so the
//!   sidebar sits where its provider is laid out, and the provider clips
//!   (`overflow-hidden`) the sidebar that slides off the canvas.
//! - `md:block` / `md:flex` (the desktop layout) are always on: there is
//!   no mobile sheet (shadcn's `Sheet` under 768 points) and no
//!   `isMobile`.
//! - `transition-[width]`, `transition-[left,right,width]`,
//!   `transition-[margin,opacity]` and `transition-[width,height,padding]`:
//!   NSH's transitions do not animate layout, so the widths, margins and
//!   paddings change at once; the label's `opacity` fades (`transition`).
//! - `size-8!`, `p-2!`, `p-0!` of a collapsed sidebar's buttons are these
//!   classes' `!` forms.
//! - The outline button's `shadow-[0_0_0_1px_var(--sidebar-border)]` is
//!   `ring-1 ring-sidebar-border` (the same 1-point ring).
//! - `border` (the global border color) is `border-border`.
//! - `[&>svg]:size-4` and `[&>span:last-child]:truncate` are the button's
//!   `icon` (`size-4 shrink-0`) and its label's `truncate` (with
//!   `min-w-0 shrink`: NSH's flex items do not shrink by default).
//! - `SidebarMenuButton`'s `tooltip` (shown while collapsed) is left out:
//!   tooltips are another family's.
//! - `peer`, `peer/menu-button` and the menu action's `group-has-*` are left
//!   out: NSH has no `peer-*` variants (the parts read the state instead).
//! - `min-h-svh` is the provider's, as shadcn's; an app that places it in
//!   a box gives it the box's height (`.app_class("!min-h-0 h-full")`: a later `min-h-0` alone does not
//!   win over `min-h-svh` in NSH).

use std::rc::Rc;

use nsh_core::prelude::*;

use crate::cn::cn;
use crate::icon::{IconName, icon};
use crate::ui::button::{ButtonSize, ButtonVariant};

/// Which edge the sidebar is on: shadcn's `side`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SidebarSide {
    /// The left edge.
    #[default]
    Left,
    /// The right edge.
    Right,
}

/// How the sidebar looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SidebarVariant {
    /// Flush with the edge, a border on its inner side.
    #[default]
    Sidebar,
    /// A floating card.
    Floating,
    /// Beside an inset content card.
    Inset,
}

/// How the sidebar collapses: shadcn's `collapsible`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SidebarCollapsible {
    /// It slides off the canvas.
    #[default]
    Offcanvas,
    /// It narrows to its icons.
    Icon,
    /// It does not collapse.
    None,
}

/// How big a menu button is: shadcn's `size` of `SidebarMenuButton`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SidebarMenuButtonSize {
    /// 32 points tall.
    #[default]
    Default,
    /// 28 points tall, extra small text.
    Sm,
    /// 48 points tall.
    Lg,
}

/// How a menu button looks: shadcn's `variant` of `SidebarMenuButton`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum SidebarMenuButtonVariant {
    /// No fill until hovered.
    #[default]
    Default,
    /// On the background, a ring around it.
    Outline,
}

/// The sidebar's state, which every part reads: shadcn's `useSidebar()`.
#[derive(Clone)]
struct SidebarContext {
    open: Memo<bool>,
    toggle: Rc<dyn Fn()>,
    side: Signal<SidebarSide>,
    variant: Signal<SidebarVariant>,
    collapsible: Signal<SidebarCollapsible>,
}

impl SidebarContext {
    fn find() -> Option<Self> {
        use_context::<SidebarContext>()
    }
}

/// The sidebar's state as a part reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct State {
    pub(crate) open: bool,
    pub(crate) side: SidebarSide,
    pub(crate) variant: SidebarVariant,
    pub(crate) collapsible: SidebarCollapsible,
}

impl State {
    /// `data-collapsible`: how it collapsed, while collapsed.
    fn collapsed(self) -> Option<SidebarCollapsible> {
        (!self.open && self.collapsible != SidebarCollapsible::None).then_some(self.collapsible)
    }
    fn icon(self) -> bool {
        self.collapsed() == Some(SidebarCollapsible::Icon)
    }
    fn offcanvas(self) -> bool {
        self.collapsed() == Some(SidebarCollapsible::Offcanvas)
    }
    fn boxed(self) -> bool {
        matches!(self.variant, SidebarVariant::Floating | SidebarVariant::Inset)
    }
}

fn state(ctx: &Option<SidebarContext>) -> State {
    ctx.as_ref().map_or(State { open: true, ..State::default() }, |c| State {
        open: c.open.get(),
        side: c.side.get(),
        variant: c.variant.get(),
        collapsible: c.collapsible.get(),
    })
}

/// shadcn's `SidebarProvider` wrapper.
pub(crate) fn wrapper_classes(state: State) -> SharedString {
    cn([
        "flex min-h-svh w-full overflow-hidden",
        if state.variant == SidebarVariant::Inset { "bg-sidebar" } else { "" },
    ])
}

/// shadcn's `Sidebar` with `collapsible="none"`.
pub(crate) const STATIC: &str = "flex h-full w-[256px] flex-col bg-sidebar text-sidebar-foreground";
/// shadcn's outer `Sidebar` element (`md:block`).
pub(crate) const OUTER: &str = "group block text-sidebar-foreground relative";

/// shadcn's `sidebar-gap`: the room the sidebar takes in the layout.
pub(crate) fn gap_classes(state: State) -> SharedString {
    cn([
        "relative w-[256px] bg-transparent duration-200 ease-linear",
        if state.offcanvas() { "w-0" } else { "" },
        match (state.icon(), state.boxed()) {
            (true, true) => "w-[64px]",
            (true, false) => "w-[48px]",
            _ => "",
        },
    ])
}

/// shadcn's `sidebar-container`.
pub(crate) fn container_classes(state: State) -> SharedString {
    cn([
        "absolute inset-y-0 z-10 h-full w-[256px] duration-200 ease-linear flex",
        match (state.side, state.offcanvas()) {
            (SidebarSide::Left, false) => "left-0",
            (SidebarSide::Left, true) => "left-[-256px]",
            (SidebarSide::Right, false) => "right-0",
            (SidebarSide::Right, true) => "right-[-256px]",
        },
        match (state.boxed(), state.icon(), state.side) {
            (true, true, _) => "p-2 w-[66px]",
            (true, false, _) => "p-2",
            (false, icon, side) => match (icon, side) {
                (true, SidebarSide::Left) => "w-[48px] border-r border-border",
                (true, SidebarSide::Right) => "w-[48px] border-l border-border",
                (false, SidebarSide::Left) => "border-r border-border",
                (false, SidebarSide::Right) => "border-l border-border",
            },
        },
    ])
}

/// shadcn's `sidebar-inner`.
pub(crate) fn inner_classes(state: State) -> SharedString {
    cn([
        "flex h-full w-full flex-col bg-sidebar",
        if state.variant == SidebarVariant::Floating {
            "rounded-lg border border-sidebar-border shadow-sm"
        } else {
            ""
        },
    ])
}

/// shadcn's `SidebarInset`.
pub(crate) fn inset_classes(state: State) -> SharedString {
    cn([
        "relative flex w-full flex-1 flex-col bg-background",
        match (state.variant == SidebarVariant::Inset, state.open) {
            (true, true) => "m-2 ml-0 rounded-xl shadow-sm",
            (true, false) => "m-2 ml-0 rounded-xl shadow-sm ml-2",
            _ => "",
        },
    ])
}

pub(crate) const HEADER: &str = "flex flex-col gap-2 p-2";
pub(crate) const FOOTER: &str = "flex flex-col gap-2 p-2";
pub(crate) const SEPARATOR: &str = "shrink-0 h-px w-full mx-2 w-auto bg-sidebar-border";
pub(crate) const GROUP: &str = "relative flex w-full min-w-0 flex-col p-2";
pub(crate) const GROUP_CONTENT: &str = "w-full text-sm";
pub(crate) const MENU: &str = "flex w-full min-w-0 flex-col gap-1";
pub(crate) const MENU_ITEM: &str = "group/menu-item relative";

/// shadcn's `SidebarContent`.
pub(crate) fn content_classes(state: State) -> SharedString {
    cn([
        "flex min-h-0 flex-1 flex-col gap-2",
        if state.icon() { "overflow-hidden" } else { "" },
    ])
}

/// shadcn's `SidebarGroupLabel`.
pub(crate) fn group_label_classes(state: State) -> SharedString {
    cn([
        "flex h-8 shrink-0 items-center rounded-md px-2 text-xs font-medium \
         text-sidebar-foreground/70 ring-sidebar-ring outline-hidden transition duration-200 \
         ease-linear focus-visible:ring-2",
        if state.icon() { "-mt-8 opacity-0" } else { "" },
    ])
}

/// shadcn's `sidebarMenuButtonVariants`, active or not.
pub(crate) fn menu_button_classes(
    variant: SidebarMenuButtonVariant,
    size: SidebarMenuButtonSize,
    active: bool,
    state: State,
) -> SharedString {
    cn([
        "flex w-full items-center gap-2 overflow-hidden rounded-md p-2 text-left text-sm \
         ring-sidebar-ring outline-hidden hover:bg-sidebar-accent \
         hover:text-sidebar-accent-foreground focus-visible:ring-2 active:bg-sidebar-accent \
         active:text-sidebar-accent-foreground disabled:pointer-events-none disabled:opacity-50 \
         aria-expanded:hover:bg-sidebar-accent aria-expanded:hover:text-sidebar-accent-foreground",
        if state.icon() { "size-8! p-2!" } else { "" },
        if active {
            "bg-sidebar-accent font-medium text-sidebar-accent-foreground"
        } else {
            ""
        },
        match variant {
            SidebarMenuButtonVariant::Default => {
                "hover:bg-sidebar-accent hover:text-sidebar-accent-foreground"
            }
            SidebarMenuButtonVariant::Outline => {
                "bg-background ring-1 ring-sidebar-border hover:bg-sidebar-accent \
                 hover:text-sidebar-accent-foreground hover:ring-sidebar-accent"
            }
        },
        match size {
            SidebarMenuButtonSize::Default => "h-8 text-sm",
            SidebarMenuButtonSize::Sm => "h-7 text-xs",
            SidebarMenuButtonSize::Lg => {
                if state.icon() {
                    "h-12 text-sm p-0!"
                } else {
                    "h-12 text-sm"
                }
            }
        },
    ])
}

/// The props of [`SidebarProvider`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SidebarProviderProps {
    /// Open (expanded), held by the app; without it the provider keeps its
    /// own, from `defaultOpen`.
    pub open: Prop<Option<bool>>,
    /// Open at first (default true).
    #[prop(default = true)]
    pub default_open: Prop<bool>,
    /// The [`Sidebar`] and the [`SidebarInset`].
    pub children: Prop<Children>,
}

/// The events of [`SidebarProvider`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum SidebarProviderEvent {
    /// The sidebar opened or collapsed.
    #[args(open)]
    OpenChange(bool),
}

/// shadcn's SidebarProvider: the sidebar's state, and the row of the
/// sidebar and the content beside it. `mod+b` toggles it.
#[component]
pub fn SidebarProvider(props: SidebarProviderProps, cx: Scope<SidebarProviderEvent>) -> impl IntoElement {
    let own = signal(props.default_open.get_untracked());
    let open = computed(move || props.open.get().unwrap_or_else(|| own.get()));
    let toggle: Rc<dyn Fn()> = Rc::new(move || {
        let next = !open.get_untracked();
        own.set(next);
        cx.emit(SidebarProviderEvent::OpenChange(next));
    });
    let ctx = SidebarContext {
        open,
        toggle: toggle.clone(),
        side: signal(SidebarSide::Left),
        variant: signal(SidebarVariant::Sidebar),
        collapsible: signal(SidebarCollapsible::Offcanvas),
    };
    provide_context(ctx.clone());
    on_key("mod+b", move |_| toggle());
    let ctx = Some(ctx);
    view()
        .app_class(move || Some(wrapper_classes(state(&ctx))))
        .child(props.children)
}

/// The props of [`Sidebar`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SidebarProps {
    /// `left` (the default) or `right`.
    #[prop(default)]
    pub side: Prop<SidebarSide>,
    /// `sidebar` (the default), `floating` or `inset`.
    #[prop(default)]
    pub variant: Prop<SidebarVariant>,
    /// `offcanvas` (the default), `icon` or `none`.
    #[prop(default)]
    pub collapsible: Prop<SidebarCollapsible>,
    /// Its header, content and footer.
    pub children: Prop<Children>,
}

/// shadcn's Sidebar: the panel at the edge of its [`SidebarProvider`].
#[component]
pub fn Sidebar(props: SidebarProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let ctx = SidebarContext::find();
    if let Some(c) = &ctx {
        let c = c.clone();
        effect(move || {
            c.side.set(props.side.get());
            c.variant.set(props.variant.get());
            c.collapsible.set(props.collapsible.get());
        });
    }
    if props.collapsible.get_untracked() == SidebarCollapsible::None {
        return view()
            .app_class(Some(SharedString::from(STATIC)))
            .child(props.children)
            .into_element();
    }
    let (a, b, c) = (ctx.clone(), ctx.clone(), ctx);
    view()
        .app_class(Some(SharedString::from(OUTER)))
        .child(view().app_class(move || Some(gap_classes(state(&a)))))
        .child(
            view().app_class(move || Some(container_classes(state(&b)))).child(
                view()
                    .app_class(move || Some(inner_classes(state(&c))))
                    .child(props.children),
            ),
        )
        .into_element()
}

/// The props of the sidebar's plain parts.
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SidebarPartProps {
    /// The content.
    pub children: Prop<Children>,
}

/// shadcn's SidebarHeader: the top of the sidebar.
#[component]
pub fn SidebarHeader(props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(Some(SharedString::from(HEADER))).child(props.children)
}

/// shadcn's SidebarFooter: the bottom of the sidebar.
#[component]
pub fn SidebarFooter(props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(Some(SharedString::from(FOOTER))).child(props.children)
}

/// shadcn's SidebarContent: the groups between the header and the footer,
/// which scroll (`overflow-auto`).
#[component]
pub fn SidebarContent(props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let ctx = SidebarContext::find();
    scroll()
        .scrollbar(false)
        .app_class(move || Some(content_classes(state(&ctx))))
        .child(props.children)
}

/// shadcn's SidebarGroup: a section of the sidebar.
#[component]
pub fn SidebarGroup(props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(Some(SharedString::from(GROUP))).child(props.children)
}

/// The props of [`SidebarGroupLabel`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SidebarGroupLabelProps {
    /// Its text.
    #[prop(default)]
    pub label: Prop<SharedString>,
}

/// shadcn's SidebarGroupLabel: a group's title, hidden while collapsed to
/// icons.
#[component]
pub fn SidebarGroupLabel(props: SidebarGroupLabelProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let ctx = SidebarContext::find();
    view()
        .app_class(move || Some(group_label_classes(state(&ctx))))
        .child(text(move || props.label.get()))
}

/// shadcn's SidebarGroupContent: a group's menu.
#[component]
pub fn SidebarGroupContent(props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(Some(SharedString::from(GROUP_CONTENT))).child(props.children)
}

/// shadcn's SidebarMenu: a list of items.
#[component]
pub fn SidebarMenu(props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .role(AccessibilityRole::List)
        .app_class(Some(SharedString::from(MENU)))
        .child(props.children)
}

/// shadcn's SidebarMenuItem: an item of a [`SidebarMenu`].
#[component]
pub fn SidebarMenuItem(props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(Some(SharedString::from(MENU_ITEM))).child(props.children)
}

/// The props of [`SidebarMenuButton`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SidebarMenuButtonProps {
    /// Its text.
    pub label: Prop<Option<SharedString>>,
    /// A Lucide icon before the text (all that shows while collapsed to
    /// icons).
    pub icon: Prop<Option<IconName>>,
    /// The page it leads to is the one shown (`isActive`).
    #[prop(default)]
    pub is_active: Prop<bool>,
    /// `default` or `outline`.
    #[prop(default)]
    pub variant: Prop<SidebarMenuButtonVariant>,
    /// `default`, `sm` or `lg`.
    #[prop(default)]
    pub size: Prop<SidebarMenuButtonSize>,
    /// No presses, half opacity.
    #[prop(default)]
    pub disabled: Prop<bool>,
    /// Content after the label.
    pub children: Prop<Children>,
}

/// The events of [`SidebarMenuButton`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum SidebarMenuButtonEvent {
    /// It was pressed.
    Press,
}

/// shadcn's SidebarMenuButton: an item's button, its icon and label.
#[component]
pub fn SidebarMenuButton(props: SidebarMenuButtonProps, cx: Scope<SidebarMenuButtonEvent>) -> impl IntoElement {
    let ctx = SidebarContext::find();
    let label = props.label;
    view()
        .role(AccessibilityRole::Button)
        .cursor_pointer()
        .disabled(props.disabled)
        .accessibility_state(move || nsh_core::AccessibilityState {
            selected: props.is_active.get(),
            ..Default::default()
        })
        .label(move || label.get().unwrap_or_default())
        .on_press(move || cx.emit(SidebarMenuButtonEvent::Press))
        .app_class(move || {
            Some(menu_button_classes(
                props.variant.get(),
                props.size.get(),
                props.is_active.get(),
                state(&ctx),
            ))
        })
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(Some(SharedString::from("size-4 shrink-0")))
                    .into_element()
            })
        }))
        .child(dynamic(move || {
            label.get().map(|label| {
                text(label)
                    .app_class(Some(SharedString::from("min-w-0 shrink truncate")))
                    .into_element()
            })
        }))
        .child(props.children)
}

/// shadcn's SidebarSeparator: a line between the sidebar's parts.
#[component]
pub fn SidebarSeparator(_props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .accessibility_elements_hidden(true)
        .app_class(Some(SharedString::from(SEPARATOR)))
}

/// The props of [`SidebarTrigger`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SidebarTriggerProps {
    /// What screen readers say (shadcn's `sr-only` text).
    pub accessibility_label: Prop<Option<SharedString>>,
}

/// shadcn's SidebarTrigger: a ghost icon button that opens and collapses
/// the sidebar.
#[component]
pub fn SidebarTrigger(props: SidebarTriggerProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let toggle = SidebarContext::find().map(|c| c.toggle);
    crate::ui::button::button()
        .variant(ButtonVariant::Ghost)
        .size(ButtonSize::Icon)
        .icon(Some(IconName::PanelLeft))
        .accessibility_label(move || {
            props
                .accessibility_label
                .get()
                .or_else(|| Some("Toggle Sidebar".into()))
        })
        .app_class(Some(SharedString::from("size-7")))
        .on_press(move || {
            if let Some(toggle) = &toggle {
                toggle();
            }
        })
}

/// shadcn's SidebarInset: the content beside an `inset` sidebar (the page
/// beside any other).
#[component]
pub fn SidebarInset(props: SidebarPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let ctx = SidebarContext::find();
    view()
        .app_class(move || Some(inset_classes(state(&ctx))))
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry
        .register::<SidebarProvider>()
        .register::<Sidebar>()
        .register::<SidebarHeader>()
        .register::<SidebarFooter>()
        .register::<SidebarContent>()
        .register::<SidebarGroup>()
        .register::<SidebarGroupLabel>()
        .register::<SidebarGroupContent>()
        .register::<SidebarMenu>()
        .register::<SidebarMenuItem>()
        .register::<SidebarMenuButton>()
        .register::<SidebarSeparator>()
        .register::<SidebarTrigger>()
        .register::<SidebarInset>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all: Vec<SharedString> =
            [STATIC, OUTER, HEADER, FOOTER, SEPARATOR, GROUP, GROUP_CONTENT, MENU, MENU_ITEM, "size-4 shrink-0 min-w-0 shrink truncate size-7"]
                .into_iter()
                .map(SharedString::from)
                .collect();
        for open in [true, false] {
            for side in [SidebarSide::Left, SidebarSide::Right] {
                for variant in [SidebarVariant::Sidebar, SidebarVariant::Floating, SidebarVariant::Inset] {
                    for collapsible in [
                        SidebarCollapsible::Offcanvas,
                        SidebarCollapsible::Icon,
                        SidebarCollapsible::None,
                    ] {
                        let s = State { open, side, variant, collapsible };
                        all.extend([
                            wrapper_classes(s),
                            gap_classes(s),
                            container_classes(s),
                            inner_classes(s),
                            inset_classes(s),
                            content_classes(s),
                            group_label_classes(s),
                        ]);
                        for v in [SidebarMenuButtonVariant::Default, SidebarMenuButtonVariant::Outline] {
                            for size in [
                                SidebarMenuButtonSize::Default,
                                SidebarMenuButtonSize::Sm,
                                SidebarMenuButtonSize::Lg,
                            ] {
                                for active in [false, true] {
                                    all.push(menu_button_classes(v, size, active, s));
                                }
                            }
                        }
                    }
                }
            }
        }
        let mut unknown: Vec<String> = all.iter().flat_map(|c| crate::cn::unknown_classes(c)).collect();
        unknown.dedup();
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
