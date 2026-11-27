//! [`Empty`] and its parts, after shadcn/ui's `empty.tsx` (new-york-v4).
//!
//! The classes are shadcn's, word for word, except:
//!
//! - `EmptyMedia`'s `[&_svg]:pointer-events-none [&_svg]:shrink-0` and the
//!   icon variant's `[&_svg:not([class*='size-'])]:size-6` size an icon in
//!   it; NSH has no descendant selectors, so the `icon` prop's icon takes
//!   them.
//! - `EmptyDescription`'s `[&>a]:underline [&>a]:underline-offset-4
//!   [&>a:hover]:text-primary` style a link in it; a link there takes them
//!   as the app's classes.
//! - `Empty` adds `text-base`: text in it with no size of its own inherits
//!   the page's 16px / 1.5 in shadcn; NSH's default text is 14 points.
//! - `text-balance` lays lines out as NSH wraps any text: NSH has no
//!   balanced wrapping, so a description breaks where it fills its line.
//! - An `Empty` with a border (shadcn's examples add `border border-dashed`)
//!   names its color too: `border border-dashed border-border`.

use nsh_core::prelude::*;

use super::cls;
use crate::cn::cn;
use crate::icon::{IconName, icon};

const EMPTY: &str = "flex min-w-0 flex-1 flex-col items-center justify-center gap-6 rounded-lg \
    border-dashed p-6 text-center text-balance md:p-12 text-base";
const HEADER: &str = "flex max-w-sm flex-col items-center gap-2 text-center";
const MEDIA: &str = "mb-2 flex shrink-0 items-center justify-center";
/// `[&_svg]:pointer-events-none [&_svg]:shrink-0`.
const MEDIA_ICON: &str = "pointer-events-none shrink-0";
const TITLE: &str = "text-lg font-medium tracking-tight";
const DESCRIPTION: &str = "text-sm/relaxed text-muted-foreground";
/// `[&>a]:underline [&>a]:underline-offset-4 [&>a:hover]:text-primary`.
#[cfg(test)]
const DESCRIPTION_LINK: &str = "underline underline-offset-4 hover:text-primary";
const CONTENT: &str =
    "flex w-full max-w-sm min-w-0 flex-col items-center gap-4 text-sm text-balance";

/// How an empty state's media looks: shadcn's `variant`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum EmptyMediaVariant {
    /// The media as it is (an avatar, an image).
    #[default]
    Default,
    /// A 24-point icon on a muted 40-point tile.
    Icon,
}

impl EmptyMediaVariant {
    fn classes(self) -> &'static str {
        match self {
            EmptyMediaVariant::Default => "bg-transparent",
            EmptyMediaVariant::Icon => {
                "flex size-10 shrink-0 items-center justify-center rounded-lg bg-muted text-foreground"
            }
        }
    }

    /// `[&_svg:not([class*='size-'])]:size-6` (the icon's 16 points elsewhere).
    fn icon(self) -> &'static str {
        match self {
            EmptyMediaVariant::Default => "",
            EmptyMediaVariant::Icon => "size-6",
        }
    }
}

/// The props of the parts that hold only children: [`Empty`],
/// [`EmptyHeader`] and [`EmptyContent`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct EmptyPartProps {
    /// What the part holds.
    pub children: Prop<Children>,
}

/// shadcn's Empty: a centered column saying there is nothing here yet, and
/// what to do about it.
#[component]
pub fn Empty(props: EmptyPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(EMPTY)).child(props.children)
}

/// shadcn's EmptyHeader: the media, title and description.
#[component]
pub fn EmptyHeader(props: EmptyPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(HEADER)).child(props.children)
}

/// The props of [`EmptyMedia`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct EmptyMediaProps {
    /// `default` or `icon` (a tile).
    #[prop(default)]
    pub variant: Prop<EmptyMediaVariant>,
    /// A Lucide icon: 24 points on the `icon` tile.
    pub icon: Prop<Option<IconName>>,
    /// Other media: an avatar, an image.
    pub children: Prop<Children>,
}

/// shadcn's EmptyMedia: the picture above the title.
#[component]
pub fn EmptyMedia(props: EmptyMediaProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .app_class(move || cls(cn([MEDIA, props.variant.get().classes()])))
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(cls(cn([MEDIA_ICON, props.variant.get().icon()])))
                    .into_element()
            })
        }))
        .child(props.children)
}

/// The props of [`EmptyTitle`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct EmptyTitleProps {
    /// The title.
    pub title: Prop<Option<SharedString>>,
    /// Content after the title.
    pub children: Prop<Children>,
}

/// shadcn's EmptyTitle.
#[component]
pub fn EmptyTitle(props: EmptyTitleProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    text(move || props.title.get().unwrap_or_default())
        .app_class(cls(TITLE))
        .child(props.children)
}

/// The props of [`EmptyDescription`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct EmptyDescriptionProps {
    /// The description.
    pub description: Prop<Option<SharedString>>,
    /// Content after the description (a link).
    pub children: Prop<Children>,
}

/// shadcn's EmptyDescription: muted text under the title.
#[component]
pub fn EmptyDescription(props: EmptyDescriptionProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    text(move || props.description.get().unwrap_or_default())
        .app_class(cls(DESCRIPTION))
        .child(props.children)
}

/// shadcn's EmptyContent: the actions under the header.
#[component]
pub fn EmptyContent(props: EmptyPartProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view().app_class(cls(CONTENT)).child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Empty>();
    registry.register::<EmptyHeader>();
    registry.register::<EmptyMedia>();
    registry.register::<EmptyTitle>();
    registry.register::<EmptyDescription>();
    registry.register::<EmptyContent>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all = Vec::new();
        for variant in [EmptyMediaVariant::Default, EmptyMediaVariant::Icon] {
            all.push(cn([MEDIA, variant.classes()]));
            all.push(cn([MEDIA_ICON, variant.icon()]));
        }
        super::super::assert_known(all.iter().map(|s| s.as_ref()).chain([
            EMPTY,
            HEADER,
            TITLE,
            DESCRIPTION,
            DESCRIPTION_LINK,
            CONTENT,
            // What shadcn's outlined example adds.
            "border border-dashed border-border",
        ]));
    }
}
