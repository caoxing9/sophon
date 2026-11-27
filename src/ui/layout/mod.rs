//! The layout family of components. Each component is one file in this
//! directory; this module declares them, re-exports their public API (types
//! and constructors, which the crate root re-exports in turn) and registers
//! them for TSX.

use nsh_core::prelude::*;

mod accordion;
mod calendar;
mod carousel;
mod chart;
mod collapsible;
mod resizable;
mod scroll_area;
mod sidebar;
mod tabs;

pub use accordion::{
    Accordion, AccordionContent, AccordionContentElement, AccordionElement, AccordionEvent,
    AccordionItem, AccordionItemElement, AccordionTrigger, AccordionTriggerElement,
};
pub use calendar::{
    Calendar, CalendarCaptionLayout, CalendarElement, CalendarEvent, CalendarMode,
};
pub use carousel::{
    Carousel, CarouselContent, CarouselContentElement, CarouselElement, CarouselEvent,
    CarouselItem, CarouselItemElement, CarouselNext, CarouselNextElement, CarouselOrientation,
    CarouselPrevious, CarouselPreviousElement,
};
pub use chart::{
    ChartConfigItem, ChartContainer, ChartContainerElement, ChartContainerEvent,
    ChartLegendAlign, ChartLegendContent, ChartLegendContentElement, ChartTooltipContent,
    ChartTooltipContentElement, ChartTooltipIndicator, ChartTooltipItem, chart_area,
    chart_band_scale, chart_bar, chart_line, chart_linear_scale,
};
pub use collapsible::{
    Collapsible, CollapsibleContent, CollapsibleContentElement, CollapsibleElement,
    CollapsibleEvent, CollapsibleTrigger, CollapsibleTriggerElement,
};
pub use resizable::{
    ResizableOrientation, ResizablePanel, ResizablePanelElement, ResizablePanelGroup,
    ResizablePanelGroupElement, ResizablePanelGroupEvent,
};
pub use scroll_area::{
    ScrollArea, ScrollAreaElement, ScrollAreaOrientation, ScrollAreaType, ScrollBar,
    ScrollBarElement,
};

pub use sidebar::{
    Sidebar, SidebarCollapsible, SidebarContent, SidebarContentElement, SidebarElement,
    SidebarFooter, SidebarFooterElement, SidebarGroup, SidebarGroupContent,
    SidebarGroupContentElement, SidebarGroupElement, SidebarGroupLabel, SidebarGroupLabelElement,
    SidebarHeader, SidebarHeaderElement, SidebarInset, SidebarInsetElement, SidebarMenu,
    SidebarMenuButton, SidebarMenuButtonElement, SidebarMenuButtonEvent, SidebarMenuButtonSize,
    SidebarMenuButtonVariant, SidebarMenuElement, SidebarMenuItem, SidebarMenuItemElement,
    SidebarProvider, SidebarProviderElement, SidebarProviderEvent, SidebarSeparator,
    SidebarSeparatorElement, SidebarSide, SidebarTrigger, SidebarTriggerElement, SidebarVariant,
};
pub use tabs::{
    Tabs, TabsContent, TabsContentElement, TabsElement, TabsEvent, TabsList, TabsListElement,
    TabsListVariant, TabsOrientation, TabsTrigger, TabsTriggerElement,
};

/// Tabs, `default_value` selected at first: `tabs("account").child(tabs_list()..)`.
pub fn tabs(default_value: impl Into<SharedString>) -> TabsElement {
    tabs::tabs().default_value(default_value.into())
}

/// The list of a [`Tabs`]' triggers.
pub fn tabs_list() -> TabsListElement {
    tabs::tabs_list()
}

/// A tab of a [`TabsList`]: `tabs_trigger("account", "Account")`.
pub fn tabs_trigger(value: impl Into<SharedString>, label: impl Into<SharedString>) -> TabsTriggerElement {
    tabs::tabs_trigger().value(value.into()).label(Some(label.into()))
}

/// What shows while the tab of `value` is selected.
pub fn tabs_content(value: impl Into<SharedString>) -> TabsContentElement {
    tabs::tabs_content().value(value.into())
}

impl TabsListElement {
    /// `.variant(TabsListVariant::Line)`.
    pub fn line(self) -> Self {
        self.variant(TabsListVariant::Line)
    }
}

impl TabsElement {
    /// `.orientation(TabsOrientation::Vertical)`.
    pub fn vertical(self) -> Self {
        self.orientation(TabsOrientation::Vertical)
    }
}

/// An accordion, one item open at a time: `accordion().collapsible(true)`;
/// `.multiple(true)` for several.
pub fn accordion() -> AccordionElement {
    accordion::accordion()
}

/// An item of an [`Accordion`], identified by `value`.
pub fn accordion_item(value: impl Into<SharedString>) -> AccordionItemElement {
    accordion::accordion_item().value(value.into())
}

/// The heading of an [`AccordionItem`], titled `label`.
pub fn accordion_trigger(label: impl Into<SharedString>) -> AccordionTriggerElement {
    accordion::accordion_trigger().label(Some(label.into()))
}

/// What an open [`AccordionItem`] shows.
pub fn accordion_content() -> AccordionContentElement {
    accordion::accordion_content()
}

/// A collapsible, closed at first.
pub fn collapsible() -> CollapsibleElement {
    collapsible::collapsible()
}

/// What opens and closes a [`Collapsible`]; `.variant(Some(..))` gives it
/// a button's look.
pub fn collapsible_trigger() -> CollapsibleTriggerElement {
    collapsible::collapsible_trigger()
}

/// What an open [`Collapsible`] shows.
pub fn collapsible_content() -> CollapsibleContentElement {
    collapsible::collapsible_content()
}

/// A scroll area: `scroll_area().app_class("h-72 w-48 rounded-md border")`.
pub fn scroll_area() -> ScrollAreaElement {
    scroll_area::scroll_area()
}

/// Panels side by side (`.vertical()`: stacked) the user resizes.
pub fn resizable_panel_group() -> ResizablePanelGroupElement {
    resizable::resizable_panel_group()
}

/// A panel of a [`ResizablePanelGroup`], `default_size` percent of it.
pub fn resizable_panel(default_size: f32) -> ResizablePanelElement {
    resizable::resizable_panel().default_size(Some(default_size))
}

impl ResizablePanelGroupElement {
    /// `.orientation(ResizableOrientation::Vertical)`.
    pub fn vertical(self) -> Self {
        self.orientation(ResizableOrientation::Vertical)
    }
}

impl ScrollAreaElement {
    /// `.orientation(ScrollAreaOrientation::Horizontal)`.
    pub fn horizontal(self) -> Self {
        self.orientation(ScrollAreaOrientation::Horizontal)
    }
}

/// A carousel: `carousel().child(carousel_content()..).child(carousel_previous())..`.
pub fn carousel() -> CarouselElement {
    carousel::carousel()
}

/// The viewport of a [`Carousel`]'s slides.
pub fn carousel_content() -> CarouselContentElement {
    carousel::carousel_content()
}

/// A slide of a [`Carousel`].
pub fn carousel_item() -> CarouselItemElement {
    carousel::carousel_item()
}

/// The button to a [`Carousel`]'s previous slide.
pub fn carousel_previous() -> CarouselPreviousElement {
    carousel::carousel_previous()
}

/// The button to a [`Carousel`]'s next slide.
pub fn carousel_next() -> CarouselNextElement {
    carousel::carousel_next()
}

impl CarouselElement {
    /// `.orientation(CarouselOrientation::Vertical)`.
    pub fn vertical(self) -> Self {
        self.orientation(CarouselOrientation::Vertical)
    }
}

/// A calendar to pick a date: `calendar().default_selected(date)`;
/// `.mode(CalendarMode::Range)` for a range.
pub fn calendar() -> CalendarElement {
    calendar::calendar()
}

/// A chart of `config`'s series: `chart_container(config).data(rows)..`.
pub fn chart_container(config: Vec<ChartConfigItem>) -> ChartContainerElement {
    chart::chart_container().config(config)
}

/// A chart's tooltip: its label and a row per series.
pub fn chart_tooltip_content() -> ChartTooltipContentElement {
    chart::chart_tooltip_content()
}

/// A chart's legend.
pub fn chart_legend_content() -> ChartLegendContentElement {
    chart::chart_legend_content()
}

/// The state of a sidebar and the row it is laid out in, open at first.
pub fn sidebar_provider() -> SidebarProviderElement {
    sidebar::sidebar_provider()
}

/// A sidebar, in a [`SidebarProvider`].
pub fn sidebar() -> SidebarElement {
    sidebar::sidebar()
}

/// The top of a [`Sidebar`].
pub fn sidebar_header() -> SidebarHeaderElement {
    sidebar::sidebar_header()
}

/// The groups of a [`Sidebar`], which scroll.
pub fn sidebar_content() -> SidebarContentElement {
    sidebar::sidebar_content()
}

/// The bottom of a [`Sidebar`].
pub fn sidebar_footer() -> SidebarFooterElement {
    sidebar::sidebar_footer()
}

/// A section of a [`Sidebar`].
pub fn sidebar_group() -> SidebarGroupElement {
    sidebar::sidebar_group()
}

/// A [`SidebarGroup`]'s title.
pub fn sidebar_group_label(label: impl Into<SharedString>) -> SidebarGroupLabelElement {
    sidebar::sidebar_group_label().label(label.into())
}

/// A [`SidebarGroup`]'s menu.
pub fn sidebar_group_content() -> SidebarGroupContentElement {
    sidebar::sidebar_group_content()
}

/// A list of [`SidebarMenuItem`]s.
pub fn sidebar_menu() -> SidebarMenuElement {
    sidebar::sidebar_menu()
}

/// An item of a [`SidebarMenu`].
pub fn sidebar_menu_item() -> SidebarMenuItemElement {
    sidebar::sidebar_menu_item()
}

/// An item's button: `sidebar_menu_button("Inbox").icon(Some(IconName::Inbox))`.
pub fn sidebar_menu_button(label: impl Into<SharedString>) -> SidebarMenuButtonElement {
    sidebar::sidebar_menu_button().label(Some(label.into()))
}

/// A line between a [`Sidebar`]'s parts.
pub fn sidebar_separator() -> SidebarSeparatorElement {
    sidebar::sidebar_separator()
}

/// The button that opens and collapses the sidebar.
pub fn sidebar_trigger() -> SidebarTriggerElement {
    sidebar::sidebar_trigger()
}

/// The content beside a [`Sidebar`].
pub fn sidebar_inset() -> SidebarInsetElement {
    sidebar::sidebar_inset()
}

/// Registers this family's components for TSX (`nsh-shadcn`).
pub(crate) fn register(registry: &mut Registry) {
    tabs::register(registry);
    accordion::register(registry);
    collapsible::register(registry);
    scroll_area::register(registry);
    resizable::register(registry);
    carousel::register(registry);
    calendar::register(registry);
    chart::register(registry);
    sidebar::register(registry);
}
