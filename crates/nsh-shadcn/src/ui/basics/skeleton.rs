//! [`Skeleton`], after shadcn/ui's `skeleton.tsx` (new-york-v4): its
//! classes word for word.

use nsh_core::prelude::*;

use super::cls;

const SKELETON: &str = "animate-pulse rounded-md bg-accent";

/// The props of [`Skeleton`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct SkeletonProps {
    /// Content inside the placeholder (rarely any).
    pub children: Prop<Children>,
}

/// shadcn's Skeleton: a pulsing block standing for content that loads. The
/// app sizes and shapes it (`h-4 w-[250px]`, `size-12 rounded-full`).
#[component]
pub fn Skeleton(props: SkeletonProps, _cx: Scope<NoEvent>) -> impl IntoElement {
    view()
        .accessibility_elements_hidden(true)
        .app_class(cls(SKELETON))
        .child(props.children)
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Skeleton>();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        super::super::assert_known([SKELETON]);
    }
}
