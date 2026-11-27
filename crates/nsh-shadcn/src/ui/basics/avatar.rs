//! [`Avatar`], [`AvatarBadge`], [`AvatarGroup`] and [`AvatarGroupCount`],
//! after shadcn/ui's `avatar.tsx` (new-york-v4), on `nsh_base`'s `Avatar`
//! and `AvatarGroup` (the image's loading state, which decides when the
//! fallback shows, and what screen readers hear).
//!
//! The classes are shadcn's, word for word, except:
//!
//! - shadcn's `AvatarImage` and `AvatarFallback` are parts `nsh_base`'s
//!   avatar draws itself: here they are the avatar's `src` / `alt` and
//!   `fallback` props, their classes its image and fallback classes
//!   (`imageClass` and `fallbackClass` add the app's, as their
//!   `className`).
//! - `data-[size=lg]:size-10 data-[size=sm]:size-6` and the
//!   `group-data-[size=…]/avatar:` classes of the fallback and the badge
//!   follow the `size` prop (the avatar tells its badge its size: a
//!   context).
//! - `AvatarGroup`'s `*:data-[slot=avatar]:ring-2
//!   *:data-[slot=avatar]:ring-background` ring the avatars in it: the group
//!   tells them they are in one (a context), and they take the classes.
//! - `AvatarGroupCount`'s `group-has-data-[size=…]/avatar-group:` classes
//!   follow its own `size` prop (NSH cannot look at its siblings), and its
//!   `[&>svg]:…` sizes the `icon` prop's icon.
//! - `AvatarBadge`'s `[&>svg]:…` sizes the `icon` prop's icon.
//! - `AvatarGroup` shows every avatar it holds (`nsh_base`'s group would
//!   show three by default).

use nsh_core::element::ImageSrc;
use nsh_core::prelude::*;

use super::cls;
use crate::cn::cn;
use crate::icon::{IconName, icon};

const ROOT: &str = "relative flex size-8 shrink-0 overflow-hidden rounded-full select-none";
const IMAGE: &str = "aspect-square size-full";
const FALLBACK: &str = "flex size-full items-center justify-center rounded-full bg-muted text-sm text-muted-foreground";
/// `*:data-[slot=avatar]:ring-2 *:data-[slot=avatar]:ring-background`.
const IN_GROUP: &str = "ring-2 ring-background";
const BADGE: &str = "absolute right-0 bottom-0 z-10 inline-flex items-center justify-center \
    rounded-full bg-primary text-primary-foreground ring-2 ring-background select-none";
const GROUP: &str = "flex -space-x-2";
const GROUP_COUNT: &str = "relative flex size-8 shrink-0 items-center justify-center rounded-full \
    bg-muted text-sm text-muted-foreground ring-2 ring-background";

/// How big an avatar is: shadcn's `size`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum AvatarSize {
    /// 32 points.
    #[default]
    Default,
    /// 24 points, extra small initials.
    Sm,
    /// 40 points.
    Lg,
}

impl AvatarSize {
    #[cfg(test)]
    const ALL: [AvatarSize; 3] = [AvatarSize::Default, AvatarSize::Sm, AvatarSize::Lg];

    /// `data-[size=lg]:size-10 data-[size=sm]:size-6`.
    fn root(self) -> &'static str {
        match self {
            AvatarSize::Default => "",
            AvatarSize::Sm => "size-6",
            AvatarSize::Lg => "size-10",
        }
    }

    /// `group-data-[size=sm]/avatar:text-xs`.
    fn fallback(self) -> &'static str {
        match self {
            AvatarSize::Sm => "text-xs",
            _ => "",
        }
    }

    /// `group-data-[size=…]/avatar:size-*` of the badge.
    fn badge(self) -> &'static str {
        match self {
            AvatarSize::Sm => "size-2",
            AvatarSize::Default => "size-2.5",
            AvatarSize::Lg => "size-3",
        }
    }

    /// `group-data-[size=…]/avatar:[&>svg]:…` of the badge's icon.
    fn badge_icon(self) -> &'static str {
        match self {
            AvatarSize::Sm => "hidden",
            _ => "size-2",
        }
    }

    /// `group-has-data-[size=…]/avatar-group:size-*` of the count.
    fn count(self) -> &'static str {
        match self {
            AvatarSize::Default => "",
            AvatarSize::Sm => "size-6",
            AvatarSize::Lg => "size-10",
        }
    }

    /// `[&>svg]:size-4`, `group-has-data-[size=…]/avatar-group:[&>svg]:size-*`.
    fn count_icon(self) -> &'static str {
        match self {
            AvatarSize::Default => "size-4",
            AvatarSize::Sm => "size-3",
            AvatarSize::Lg => "size-5",
        }
    }
}

fn root_classes(size: AvatarSize, in_group: bool) -> SharedString {
    cn([ROOT, size.root(), if in_group { IN_GROUP } else { "" }])
}

/// The avatar's size, for its badge.
#[derive(Clone, Copy)]
struct AvatarContext(Prop<AvatarSize>);

/// Avatars inside an [`AvatarGroup`].
#[derive(Clone, Copy)]
struct InAvatarGroup;

/// The props of [`Avatar`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AvatarProps {
    /// The picture (shadcn's `AvatarImage` `src`): a URL, an `asset:/` URI,
    /// a `data:` URI. It shows once it has loaded.
    pub src: Prop<Option<ImageSrc>>,
    /// Who it is (the image's `alt`): what screen readers say.
    pub alt: Prop<Option<SharedString>>,
    /// What shows while the picture loads, when it fails and without one
    /// (shadcn's `AvatarFallback`): initials, "CN".
    pub fallback: Prop<Option<SharedString>>,
    /// `default`, `sm` or `lg`.
    #[prop(default)]
    pub size: Prop<AvatarSize>,
    /// The app's classes of the image (`AvatarImage`'s `className`).
    pub image_class: Prop<Option<SharedString>>,
    /// The app's classes of the fallback (`AvatarFallback`'s `className`).
    pub fallback_class: Prop<Option<SharedString>>,
    /// What shows over the picture: an [`AvatarBadge`].
    pub children: Prop<Children>,
}

/// shadcn's Avatar: a round picture of a person, or their initials.
#[component]
pub fn Avatar(props: AvatarProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let in_group = use_context::<InAvatarGroup>().is_some();
    provide_context(AvatarContext(props.size));
    let fallback = props.fallback;
    nsh_base::avatar()
        .src(props.src)
        .accessibility_label(props.alt)
        .app_class(move || cls(root_classes(props.size.get(), in_group)))
        .image_class(move || {
            cls(cn([
                IMAGE,
                props.image_class.get().as_deref().unwrap_or(""),
            ]))
        })
        .fallback_class(move || {
            cls(cn([
                FALLBACK,
                props.size.get().fallback(),
                props.fallback_class.get().as_deref().unwrap_or(""),
            ]))
        })
        .fallback(text(move || fallback.get().unwrap_or_default()))
        .child(props.children)
}

impl AvatarElement {
    /// `.size(AvatarSize::Sm)`.
    pub fn sm(self) -> Self {
        self.size(AvatarSize::Sm)
    }
    /// `.size(AvatarSize::Lg)`.
    pub fn lg(self) -> Self {
        self.size(AvatarSize::Lg)
    }
}

/// The props of [`AvatarBadge`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AvatarBadgeProps {
    /// A Lucide icon in the badge (none on a small avatar).
    pub icon: Prop<Option<IconName>>,
    /// Content of the badge.
    pub children: Prop<Children>,
}

/// shadcn's AvatarBadge: a dot at the bottom right of the [`Avatar`] it is
/// in (a presence, a count), sized by the avatar's size.
#[component]
pub fn AvatarBadge(props: AvatarBadgeProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let size = use_context::<AvatarContext>().map(|c| c.0);
    let size = move || size.map(|s| s.get()).unwrap_or_default();
    view()
        .app_class(move || cls(cn([BADGE, size().badge()])))
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(cls(size().badge_icon()))
                    .into_element()
            })
        }))
        .child(props.children)
}

/// The props of [`AvatarGroup`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AvatarGroupProps {
    /// What screen readers say of the group ("Ada, Grace and 5 others").
    pub accessibility_label: Prop<Option<SharedString>>,
    /// The avatars, and an [`AvatarGroupCount`] last.
    pub children: Prop<Children>,
}

/// shadcn's AvatarGroup: avatars overlapping in a row, each ringed in the
/// background color.
#[component]
pub fn AvatarGroup(props: AvatarGroupProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    provide_context(InAvatarGroup);
    nsh_base::avatar_group()
        .limit(u32::MAX)
        .accessibility_label(props.accessibility_label)
        .app_class(cls(GROUP))
        .child(props.children)
}

/// The props of [`AvatarGroupCount`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct AvatarGroupCountProps {
    /// The count: "+3".
    pub count: Prop<Option<SharedString>>,
    /// The size of the group's avatars (shadcn reads it from them).
    #[prop(default)]
    pub size: Prop<AvatarSize>,
    /// A Lucide icon in place of the count.
    pub icon: Prop<Option<IconName>>,
    /// Content after the count.
    pub children: Prop<Children>,
}

/// shadcn's AvatarGroupCount: a round count of the people a group leaves
/// out, after its avatars.
#[component]
pub fn AvatarGroupCount(props: AvatarGroupCountProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    let count = props.count;
    view()
        .app_class(move || cls(cn([GROUP_COUNT, props.size.get().count()])))
        .child(dynamic(move || {
            props.icon.get().map(|name| {
                icon(name)
                    .app_class(cls(props.size.get().count_icon()))
                    .into_element()
            })
        }))
        .child(dynamic(move || count.get().map(|c| text(c).into_element())))
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Avatar>();
    registry.register::<AvatarBadge>();
    registry.register::<AvatarGroup>();
    registry.register::<AvatarGroupCount>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut all: Vec<SharedString> = Vec::new();
        for size in AvatarSize::ALL {
            all.push(root_classes(size, false));
            all.push(root_classes(size, true));
            all.push(cn([FALLBACK, size.fallback()]));
            all.push(cn([BADGE, size.badge(), size.badge_icon()]));
            all.push(cn([GROUP_COUNT, size.count(), size.count_icon()]));
        }
        super::super::assert_known(all.iter().map(|s| s.as_ref()).chain([IMAGE, GROUP]));
    }
}
