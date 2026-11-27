//! The basics family of components. Each component is one file in this
//! directory; this module declares them, re-exports their public API (types
//! and constructors, which the crate root re-exports in turn) and registers
//! them for TSX.
//!
//! A constructor takes the part's required content: `badge("New")`,
//! `card_title("Login")`, `kbd("⌘")`; the parts that hold other parts take
//! none (`card()`, `table_row()`) and get them with `.child(..)`.

use nsh_core::prelude::*;

mod alert;
mod aspect_ratio;
mod avatar;
mod badge;
mod breadcrumb;
mod card;
mod empty;
mod item;
mod kbd;
mod label;
mod pagination;
mod progress;
mod separator;
mod skeleton;
mod spinner;
mod table;

pub use alert::{
    Alert, AlertDescription, AlertDescriptionElement, AlertElement, AlertTitle, AlertTitleElement,
    AlertVariant,
};
pub use aspect_ratio::{AspectRatio, AspectRatioElement};
pub use avatar::{
    Avatar, AvatarBadge, AvatarBadgeElement, AvatarElement, AvatarGroup, AvatarGroupCount,
    AvatarGroupCountElement, AvatarGroupElement, AvatarSize,
};
pub use badge::{Badge, BadgeElement, BadgeVariant};
pub use breadcrumb::{
    Breadcrumb, BreadcrumbElement, BreadcrumbEllipsis, BreadcrumbEllipsisElement, BreadcrumbItem,
    BreadcrumbItemElement, BreadcrumbLink, BreadcrumbLinkElement, BreadcrumbLinkEvent,
    BreadcrumbList, BreadcrumbListElement, BreadcrumbPage, BreadcrumbPageElement,
    BreadcrumbSeparator, BreadcrumbSeparatorElement,
};
pub use card::{
    Card, CardAction, CardActionElement, CardContent, CardContentElement, CardDescription,
    CardDescriptionElement, CardElement, CardFooter, CardFooterElement, CardHeader,
    CardHeaderElement, CardTitle, CardTitleElement,
};
pub use empty::{
    Empty, EmptyContent, EmptyContentElement, EmptyDescription, EmptyDescriptionElement,
    EmptyElement, EmptyHeader, EmptyHeaderElement, EmptyMedia, EmptyMediaElement,
    EmptyMediaVariant, EmptyTitle, EmptyTitleElement,
};
pub use item::{
    Item, ItemActions, ItemActionsElement, ItemContent, ItemContentElement, ItemDescription,
    ItemDescriptionElement, ItemElement, ItemEvent, ItemFooter, ItemFooterElement, ItemGroup,
    ItemGroupElement, ItemHeader, ItemHeaderElement, ItemMedia, ItemMediaElement, ItemMediaVariant,
    ItemSeparator, ItemSeparatorElement, ItemSize, ItemTitle, ItemTitleElement, ItemVariant,
};
pub use kbd::{Kbd, KbdElement, KbdGroup, KbdGroupElement};
pub use label::{Label, LabelElement, LabelEvent};
pub use pagination::{
    Pagination, PaginationContent, PaginationContentElement, PaginationElement, PaginationEllipsis,
    PaginationEllipsisElement, PaginationItem, PaginationItemElement, PaginationLink,
    PaginationLinkElement, PaginationLinkEvent, PaginationNext, PaginationNextElement,
    PaginationPrevious, PaginationPreviousElement,
};
pub use progress::{Progress, ProgressElement};
pub use separator::{Separator, SeparatorElement, SeparatorOrientation};
pub use skeleton::{Skeleton, SkeletonElement};
pub use spinner::{Spinner, SpinnerElement};
pub use table::{
    Table, TableBody, TableBodyElement, TableCaption, TableCaptionElement, TableCell,
    TableCellElement, TableElement, TableFooter, TableFooterElement, TableHead, TableHeadElement,
    TableHeader, TableHeaderElement, TableRow, TableRowElement,
};

// ---- Constructors ---------------------------------------------------------

/// A badge: `badge("New")`, `badge("Failed").destructive()`.
pub fn badge(label: impl Into<SharedString>) -> BadgeElement {
    badge::badge().label(Some(label.into()))
}

/// A card: `card().child(card_header()..).child(card_content()..)`.
pub fn card() -> CardElement {
    card::card()
}

/// A card's header: its title and description, and an `.action(..)` at
/// its top right.
pub fn card_header() -> CardHeaderElement {
    card::card_header()
}

/// A card's title.
pub fn card_title(title: impl Into<SharedString>) -> CardTitleElement {
    card::card_title().title(Some(title.into()))
}

/// A card's description, under its title.
pub fn card_description(description: impl Into<SharedString>) -> CardDescriptionElement {
    card::card_description().description(Some(description.into()))
}

/// What a card's header holds at its top right:
/// `card_header().action(card_action().child(..))`.
pub fn card_action() -> CardActionElement {
    card::card_action()
}

/// A card's body.
pub fn card_content() -> CardContentElement {
    card::card_content()
}

/// A card's footer.
pub fn card_footer() -> CardFooterElement {
    card::card_footer()
}

/// A horizontal separator; `.vertical()` for a vertical one.
pub fn separator() -> SeparatorElement {
    separator::separator()
}

/// A label: `label("Email")`.
pub fn label(text: impl Into<SharedString>) -> LabelElement {
    label::label().text(Some(text.into()))
}

/// A key: `kbd("⌘")`, `kbd("Ctrl")`.
pub fn kbd(key: impl Into<SharedString>) -> KbdElement {
    kbd::kbd().key(Some(key.into()))
}

/// Keys side by side: `kbd_group().child(kbd("Ctrl")).child(kbd("B"))`.
pub fn kbd_group() -> KbdGroupElement {
    kbd::kbd_group()
}

/// A placeholder while content loads, sized by the app:
/// `skeleton().app_class("h-4 w-[250px]")`.
pub fn skeleton() -> SkeletonElement {
    skeleton::skeleton()
}

/// A spinner: a 16-point `loader-circle` turning.
pub fn spinner() -> SpinnerElement {
    spinner::spinner()
}

/// An alert: `alert().icon(Some(IconName::CircleCheck)).child(alert_title("Saved"))`.
pub fn alert() -> AlertElement {
    alert::alert()
}

/// An alert's title.
pub fn alert_title(title: impl Into<SharedString>) -> AlertTitleElement {
    alert::alert_title().title(Some(title.into()))
}

/// An alert's description, under its title.
pub fn alert_description(description: impl Into<SharedString>) -> AlertDescriptionElement {
    alert::alert_description().description(Some(description.into()))
}

/// An avatar showing `fallback` (initials: "CN") while its picture loads,
/// when it fails and without one: `avatar("CN").src(Some(url.into()))`.
pub fn avatar(fallback: impl Into<SharedString>) -> AvatarElement {
    avatar::avatar().fallback(Some(fallback.into()))
}

/// A dot at an avatar's bottom right: `avatar("CN").child(avatar_badge())`.
pub fn avatar_badge() -> AvatarBadgeElement {
    avatar::avatar_badge()
}

/// Avatars overlapping in a row.
pub fn avatar_group() -> AvatarGroupElement {
    avatar::avatar_group()
}

/// The count of the people an avatar group leaves out: `avatar_group_count("+3")`.
pub fn avatar_group_count(count: impl Into<SharedString>) -> AvatarGroupCountElement {
    avatar::avatar_group_count().count(Some(count.into()))
}

/// Content at a width-to-height `ratio`: `aspect_ratio(16.0 / 9.0).child(..)`.
pub fn aspect_ratio(ratio: f32) -> AspectRatioElement {
    aspect_ratio::aspect_ratio().ratio(ratio)
}

/// An empty state: `empty().child(empty_header()..).child(empty_content()..)`.
pub fn empty() -> EmptyElement {
    empty::empty()
}

/// An empty state's media, title and description.
pub fn empty_header() -> EmptyHeaderElement {
    empty::empty_header()
}

/// An empty state's picture: `empty_media().icon(Some(IconName::Inbox))`,
/// on a tile with `.variant(EmptyMediaVariant::Icon)`.
pub fn empty_media() -> EmptyMediaElement {
    empty::empty_media()
}

/// An empty state's title.
pub fn empty_title(title: impl Into<SharedString>) -> EmptyTitleElement {
    empty::empty_title().title(Some(title.into()))
}

/// An empty state's description.
pub fn empty_description(description: impl Into<SharedString>) -> EmptyDescriptionElement {
    empty::empty_description().description(Some(description.into()))
}

/// An empty state's actions, under its header.
pub fn empty_content() -> EmptyContentElement {
    empty::empty_content()
}

/// An item: `item().child(item_content()..).child(item_actions()..)`.
pub fn item() -> ItemElement {
    item::item()
}

/// Items in a list.
pub fn item_group() -> ItemGroupElement {
    item::item_group()
}

/// A line between the items of a group.
pub fn item_separator() -> ItemSeparatorElement {
    item::item_separator()
}

/// An item's picture: an icon, an avatar, an image.
pub fn item_media() -> ItemMediaElement {
    item::item_media()
}

/// An item's title and description.
pub fn item_content() -> ItemContentElement {
    item::item_content()
}

/// An item's title.
pub fn item_title(title: impl Into<SharedString>) -> ItemTitleElement {
    item::item_title().title(Some(title.into()))
}

/// An item's description, under its title.
pub fn item_description(description: impl Into<SharedString>) -> ItemDescriptionElement {
    item::item_description().description(Some(description.into()))
}

/// An item's buttons, at its end.
pub fn item_actions() -> ItemActionsElement {
    item::item_actions()
}

/// A row across the top of an item.
pub fn item_header() -> ItemHeaderElement {
    item::item_header()
}

/// A row across the bottom of an item.
pub fn item_footer() -> ItemFooterElement {
    item::item_footer()
}

/// A breadcrumb trail: `breadcrumb().child(breadcrumb_list()..)`.
pub fn breadcrumb() -> BreadcrumbElement {
    breadcrumb::breadcrumb()
}

/// A breadcrumb's items and separators, in a row.
pub fn breadcrumb_list() -> BreadcrumbListElement {
    breadcrumb::breadcrumb_list()
}

/// A step of a breadcrumb: a link or the page.
pub fn breadcrumb_item() -> BreadcrumbItemElement {
    breadcrumb::breadcrumb_item()
}

/// A link to a page up the trail: `breadcrumb_link("Home").href("/")`.
pub fn breadcrumb_link(label: impl Into<SharedString>) -> BreadcrumbLinkElement {
    breadcrumb::breadcrumb_link().label(Some(label.into()))
}

/// The page the user is on, at the trail's end.
pub fn breadcrumb_page(label: impl Into<SharedString>) -> BreadcrumbPageElement {
    breadcrumb::breadcrumb_page().label(Some(label.into()))
}

/// The chevron between two steps; `.icon(IconName::Slash)` for another.
pub fn breadcrumb_separator() -> BreadcrumbSeparatorElement {
    breadcrumb::breadcrumb_separator()
}

/// The steps left out, as `…`.
pub fn breadcrumb_ellipsis() -> BreadcrumbEllipsisElement {
    breadcrumb::breadcrumb_ellipsis()
}

/// A pagination: `pagination().child(pagination_content()..)`.
pub fn pagination() -> PaginationElement {
    pagination::pagination()
}

/// A pagination's items, in a row.
pub fn pagination_content() -> PaginationContentElement {
    pagination::pagination_content()
}

/// One of a pagination's items: a link, Previous, Next, an ellipsis.
pub fn pagination_item() -> PaginationItemElement {
    pagination::pagination_item()
}

/// A link to a page: `pagination_link("2").is_active(true)`.
pub fn pagination_link(label: impl Into<SharedString>) -> PaginationLinkElement {
    pagination::pagination_link().label(Some(label.into()))
}

/// The link to the previous page.
pub fn pagination_previous() -> PaginationPreviousElement {
    pagination::pagination_previous()
}

/// The link to the next page.
pub fn pagination_next() -> PaginationNextElement {
    pagination::pagination_next()
}

/// The pages left out, as `…`.
pub fn pagination_ellipsis() -> PaginationEllipsisElement {
    pagination::pagination_ellipsis()
}

/// A progress bar at `value` percent.
pub fn progress(value: f32) -> ProgressElement {
    progress::progress().value(value)
}

/// A table: `table().child(table_header()..).child(table_body()..)`.
pub fn table() -> TableElement {
    table::table()
}

/// A table's rows of column headers.
pub fn table_header() -> TableHeaderElement {
    table::table_header()
}

/// A table's rows of data.
pub fn table_body() -> TableBodyElement {
    table::table_body()
}

/// A table's footer rows: a total.
pub fn table_footer() -> TableFooterElement {
    table::table_footer()
}

/// A row of a table.
pub fn table_row() -> TableRowElement {
    table::table_row()
}

/// A column's header cell: `table_head("Invoice")`.
pub fn table_head(text: impl Into<SharedString>) -> TableHeadElement {
    table::table_head().text(Some(text.into()))
}

/// A cell: `table_cell("INV001")`.
pub fn table_cell(text: impl Into<SharedString>) -> TableCellElement {
    table::table_cell().text(Some(text.into()))
}

/// A line under a table (place it last: shadcn's `caption-bottom`).
pub fn table_caption(caption: impl Into<SharedString>) -> TableCaptionElement {
    table::table_caption().caption(Some(caption.into()))
}

/// Registers this family's components for TSX (`nsh-shadcn`).
pub(crate) fn register(registry: &mut Registry) {
    alert::register(registry);
    aspect_ratio::register(registry);
    avatar::register(registry);
    badge::register(registry);
    breadcrumb::register(registry);
    card::register(registry);
    empty::register(registry);
    item::register(registry);
    kbd::register(registry);
    label::register(registry);
    pagination::register(registry);
    progress::register(registry);
    separator::register(registry);
    skeleton::register(registry);
    spinner::register(registry);
    table::register(registry);
}

/// Asserts that NSH knows every class of `strings`: an unknown one would be
/// dropped at run time with only a log line.
#[cfg(test)]
pub(crate) fn assert_known<'a>(strings: impl IntoIterator<Item = &'a str>) {
    let mut unknown = Vec::new();
    for classes in strings {
        for class in crate::cn::unknown_classes(classes) {
            if !unknown.contains(&class) {
                unknown.push(class);
            }
        }
    }
    assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
}

/// A class string as an `app_class` value.
pub(crate) fn cls(classes: impl Into<SharedString>) -> Option<SharedString> {
    Some(classes.into())
}
