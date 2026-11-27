//! [`Item`] and its parts, after shadcn/ui's `item.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `border` is `border border-border` where shadcn relies on its global
//!   `* { @apply border-border }` (the icon media's).
//! - `[a]:transition-colors [a]:hover:bg-accent/50` style an item that is
//!   a link (`asChild` with an `<a>`): an item given an `href` is one
//!   (`nsh_base`'s `Link`), and takes them.
//! - `ItemMedia`'s `group-has-[[data-slot=item-description]]/item:translate-y-0.5`
//!   and `…:self-start` move the media to the top of an item that has a
//!   description: each [`ItemDescription`] tells the item it is there (a
//!   context), and the item's media take the classes.
//! - `ItemMedia`'s `[&_svg]:pointer-events-none`, the icon variant's
//!   `[&_svg:not([class*='size-'])]:size-4` and the image variant's
//!   `[&_img]:size-full [&_img]:object-cover` style its icon and image:
//!   NSH has no descendant selectors, so the `icon` and `src` props place
//!   them with those classes (an icon of the default variant is Lucide's
//!   24 points; `iconClass` sizes it, as shadcn's examples do with
//!   `className="size-5"`).
//! - `ItemContent`'s `[&+[data-slot=item-content]]:flex-none` (a second
//!   content block keeps its width) is the app's `flex-none` on it.
//! - `ItemTitle`'s and `ItemDescription`'s `[&>a]:…` style a link in them;
//!   a link there takes them as the app's classes.
//! - `ItemGroup` is a list for screen readers, as its `role="list"`.

use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

use super::cls;
use super::separator::{SeparatorOrientation, separator_classes};
use crate::cn::cn;
use crate::icon::{IconName, icon};

const ITEM: &str = "group/item flex flex-wrap items-center rounded-md border border-transparent \
    text-sm transition-colors duration-100 outline-none focus-visible:border-ring \
    focus-visible:ring-[3px] focus-visible:ring-ring/50";
/// `[a]:transition-colors [a]:hover:bg-accent/50`.
const LINK: &str = "transition-colors hover:bg-accent/50";
const GROUP: &str = "group/item-group flex flex-col";
const SEPARATOR: &str = "my-0";
const MEDIA: &str = "flex shrink-0 items-center justify-center gap-2";
/// `group-has-[[data-slot=item-description]]/item:translate-y-0.5 …:self-start`.
const MEDIA_BESIDE_DESCRIPTION: &str = "translate-y-0.5 self-start";
/// `[&_svg]:pointer-events-none`.
const MEDIA_ICON: &str = "pointer-events-none";
const CONTENT: &str = "flex flex-1 flex-col gap-1";
const TITLE: &str = "flex w-fit items-center gap-2 text-sm leading-snug font-medium";
const DESCRIPTION: &str =
    "line-clamp-2 text-sm leading-normal font-normal text-balance text-muted-foreground";
/// `[&>a]:underline [&>a]:underline-offset-4 [&>a:hover]:text-primary`.
#[cfg(test)]
const DESCRIPTION_LINK: &str = "underline underline-offset-4 hover:text-primary";
const ACTIONS: &str = "flex items-center gap-2";
const HEADER: &str = "flex basis-full items-center justify-between gap-2";
const FOOTER: &str = "flex basis-full items-center justify-between gap-2";

/// How an item looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ItemVariant {
    /// No fill, no border.
    #[default]
    Default,
    /// A border.
    Outline,
    /// A muted fill.
    Muted,
}

impl ItemVariant {
    #[cfg(test)]
    const ALL: [ItemVariant; 3] = [
        ItemVariant::Default,
        ItemVariant::Outline,
        ItemVariant::Muted,
    ];

    fn classes(self) -> &'static str {
        match self {
            ItemVariant::Default => "bg-transparent",
            ItemVariant::Outline => "border-border",
            ItemVariant::Muted => "bg-muted/50",
        }
    }
}

/// How roomy an item is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ItemSize {
    /// 16 points of padding.
    #[default]
    Default,
    /// Tighter.
    Sm,
}

impl ItemSize {
    fn classes(self) -> &'static str {
        match self {
            ItemSize::Default => "gap-4 p-4",
            ItemSize::Sm => "gap-2.5 px-4 py-3",
        }
    }
}

fn item_classes(variant: ItemVariant, size: ItemSize, link: bool) -> SharedString {
    cn([
        ITEM,
        if link { LINK } else { "" },
        variant.classes(),
        size.classes(),
    ])
}

/// How many descriptions the item holds, for its media.
#[derive(Clone, Copy)]
struct ItemContext {
    descriptions: Signal<u32>,
}

/// The props of [`Item`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ItemProps {
    /// `default`, `outline` or `muted`.
    #[prop(default)]
    pub variant: Prop<ItemVariant>,
    /// `default` or `sm`.
    #[prop(default)]
    pub size: Prop<ItemSize>,
    /// Where the item goes (shadcn's `asChild` with an `<a href>`): given
    /// when the item is made, it is a link, which presses open and hovers
    /// highlight.
    pub href: Prop<Option<SharedString>>,
    /// Its media, content, actions, header and footer.
    pub children: Prop<Children>,
}

/// The events of an [`Item`].
#[derive(Debug, Clone, PartialEq, Event)]
pub enum ItemEvent {
    /// An item with an `href` was pressed.
    Press,
}

/// shadcn's Item: a row of media, a title and description, and actions.
#[component]
pub fn Item(props: ItemProps, cx: Scope<ItemEvent>) -> impl IntoElement {
    provide_context(ItemContext {
        descriptions: signal(0),
    });
    let classes = move |link| cls(item_classes(props.variant.get(), props.size.get(), link));
    match props.href.get_untracked() {
        Some(_) => nsh_base::link()
            .href(props.href)
            .app_class(move || classes(true))
            .on_press(move || cx.emit(ItemEvent::Press))
            .child(props.children)
            .into_element(),
        None => view()
            .app_class(move || classes(false))
            .child(props.children)
            .into_element(),
    }
}

impl ItemElement {
    /// `.variant(ItemVariant::Outline)`.
    pub fn outline(self) -> Self {
        self.variant(ItemVariant::Outline)
    }
    /// `.variant(ItemVariant::Muted)`.
    pub fn muted(self) -> Self {
        self.variant(ItemVariant::Muted)
    }
    /// `.size(ItemSize::Sm)`.
    pub fn sm(self) -> Self {
        self.size(ItemSize::Sm)
    }
}

/// The props of the parts that hold only children: [`ItemGroup`],
/// [`ItemContent`], [`ItemActions`], [`ItemHeader`] and [`ItemFooter`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ItemPartProps {
    /// What the part holds.
    pub children: Prop<Children>,
}

/// shadcn's ItemGroup: items in a column, a list for screen readers.
#[component]
pub fn ItemGroup(props: ItemPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .role(AccessibilityRole::List)
        .app_class(cls(GROUP))
        .child(props.children)
}

/// The props of [`ItemSeparator`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ItemSeparatorProps {}

/// shadcn's ItemSeparator: a horizontal [`super::Separator`] between the
/// items of a group.
#[component]
pub fn ItemSeparator(_props: ItemSeparatorProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .accessibility_elements_hidden(true)
        .app_class(cls(cn([
            separator_classes(SeparatorOrientation::Horizontal).as_ref(),
            SEPARATOR,
        ])))
}

/// How an item's media looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ItemMediaVariant {
    /// The media as it is.
    #[default]
    Default,
    /// A 16-point icon on a bordered 32-point tile.
    Icon,
    /// A 40-point image, cropped to fill.
    Image,
}

impl ItemMediaVariant {
    #[cfg(test)]
    const ALL: [ItemMediaVariant; 3] = [
        ItemMediaVariant::Default,
        ItemMediaVariant::Icon,
        ItemMediaVariant::Image,
    ];

    fn classes(self) -> &'static str {
        match self {
            ItemMediaVariant::Default => "bg-transparent",
            ItemMediaVariant::Icon => "size-8 rounded-sm border border-border bg-muted",
            ItemMediaVariant::Image => "size-10 overflow-hidden rounded-sm",
        }
    }

    /// `[&_svg:not([class*='size-'])]:size-4` of the icon tile; Lucide's 24
    /// points elsewhere.
    fn icon(self) -> &'static str {
        match self {
            ItemMediaVariant::Icon => "size-4",
            _ => "size-6",
        }
    }
}

/// `[&_img]:size-full [&_img]:object-cover`.
const MEDIA_IMAGE: &str = "size-full object-cover";

/// The props of [`ItemMedia`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ItemMediaProps {
    /// `default`, `icon` (a tile) or `image`.
    #[prop(default)]
    pub variant: Prop<ItemMediaVariant>,
    /// A Lucide icon.
    pub icon: Prop<Option<IconName>>,
    /// The app's classes of the icon (`size-5`).
    pub icon_class: Prop<Option<SharedString>>,
    /// An image, filling the media (the `image` variant's).
    pub src: Prop<Option<ImageSrc>>,
    /// Other media: an avatar.
    pub children: Prop<Children>,
}

/// shadcn's ItemMedia: the icon, avatar or image at an item's start.
#[component]
pub fn ItemMedia(props: ItemMediaProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let item = use_context::<ItemContext>();
    let beside_description = move || item.is_some_and(|i| i.descriptions.get() > 0);
    view()
        .app_class(move || {
            cls(cn([
                MEDIA,
                if beside_description() {
                    MEDIA_BESIDE_DESCRIPTION
                } else {
                    ""
                },
                props.variant.get().classes(),
            ]))
        })
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(move || {
                        cls(cn([
                            MEDIA_ICON,
                            props.variant.get().icon(),
                            props.icon_class.get().as_deref().unwrap_or(""),
                        ]))
                    })
                    .into_element()
            })
        }))
        .child(dynamic(move || {
            props
                .src
                .get()
                .map(|src| image(src).app_class(cls(MEDIA_IMAGE)).into_element())
        }))
        .child(props.children)
}

/// shadcn's ItemContent: the title and description, taking the room left.
#[component]
pub fn ItemContent(props: ItemPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(CONTENT)).child(props.children)
}

/// The props of [`ItemTitle`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ItemTitleProps {
    /// The title.
    pub title: Prop<Option<SharedString>>,
    /// Content after the title (a badge).
    pub children: Prop<Children>,
}

/// shadcn's ItemTitle.
#[component]
pub fn ItemTitle(props: ItemTitleProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let title = props.title;
    view()
        .app_class(cls(TITLE))
        .child(dynamic(move || title.get().map(|t| text(t).into_element())))
        .child(props.children)
}

/// The props of [`ItemDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ItemDescriptionProps {
    /// The description, on two lines at most.
    pub description: Prop<Option<SharedString>>,
    /// Content after the description.
    pub children: Prop<Children>,
}

/// shadcn's ItemDescription: muted text under the title, two lines at
/// most.
#[component]
pub fn ItemDescription(props: ItemDescriptionProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    if let Some(item) = use_context::<ItemContext>() {
        item.descriptions.update(|n| *n += 1);
        cleanup(move || item.descriptions.update(|n| *n = n.saturating_sub(1)));
    }
    text(move || props.description.get().unwrap_or_default())
        .app_class(cls(DESCRIPTION))
        .child(props.children)
}

/// shadcn's ItemActions: buttons at an item's end.
#[component]
pub fn ItemActions(props: ItemPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(ACTIONS)).child(props.children)
}

/// shadcn's ItemHeader: a row across the item's top.
#[component]
pub fn ItemHeader(props: ItemPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(HEADER)).child(props.children)
}

/// shadcn's ItemFooter: a row across the item's bottom.
#[component]
pub fn ItemFooter(props: ItemPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(FOOTER)).child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Item>();
    registry.register::<ItemGroup>();
    registry.register::<ItemSeparator>();
    registry.register::<ItemMedia>();
    registry.register::<ItemContent>();
    registry.register::<ItemTitle>();
    registry.register::<ItemDescription>();
    registry.register::<ItemActions>();
    registry.register::<ItemHeader>();
    registry.register::<ItemFooter>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all: Vec<SharedString> = Vec::new();
        for variant in ItemVariant::ALL {
            for size in [ItemSize::Default, ItemSize::Sm] {
                all.push(item_classes(variant, size, false));
                all.push(item_classes(variant, size, true));
            }
        }
        for variant in ItemMediaVariant::ALL {
            all.push(cn([MEDIA, MEDIA_BESIDE_DESCRIPTION, variant.classes()]));
            all.push(cn([MEDIA_ICON, variant.icon()]));
        }
        super::super::assert_known(all.iter().map(|s| s.as_ref()).chain([
            GROUP,
            SEPARATOR,
            MEDIA_IMAGE,
            CONTENT,
            TITLE,
            DESCRIPTION,
            DESCRIPTION_LINK,
            ACTIONS,
            HEADER,
            FOOTER,
        ]));
    }
}
