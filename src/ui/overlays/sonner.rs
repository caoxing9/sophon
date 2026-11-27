//! [`Toaster`] and [`toast`], after shadcn/ui's `sonner.tsx` (new-york-v4),
//! which wraps the `sonner` library with the theme's tokens
//! (`--normal-bg: var(--popover)`, `--normal-text: var(--popover-foreground)`,
//! `--normal-border: var(--border)`, `--border-radius: var(--radius)`) and
//! Lucide icons for the types.
//!
//! Behavior is `nsh_base`'s toasts: a [`nsh_base::ToastManager`] keeps the
//! toasts (sonner's 4 seconds each, paused while the stack is expanded),
//! [`nsh_base::ToastStack`] lays them out as sonner's collapsed layers
//! (14 points apart, each 5% narrower, three shown) and expands them into a
//! list under the pointer ([`nsh_base::ToastMotion::sonner`]), and a toast
//! is swiped away to the right.
//!
//! The look is sonner's default styles (`sonner/dist/styles.css` 2.0.8,
//! `[data-styled=true]`), written as classes on shadcn's tokens:
//!
//! - the toast: `padding: 16px`, `background: var(--normal-bg)`,
//!   `border: 1px solid var(--normal-border)`, `color: var(--normal-text)`,
//!   `border-radius: var(--border-radius)`, `box-shadow: 0 4px 12px
//!   rgba(0,0,0,.1)`, `width: 356px`, `font-size: 13px`, a row centered
//!   with `gap: 6px`;
//! - `[data-icon]`: 16 points, `margin: 0 4px 0 -3px`, its SVG `-1px`;
//! - `[data-content]`: a column, `gap: 2px`; `[data-title]` 500 on a 1.5
//!   line; `[data-description]` 400 on a 1.4 line in `#3f3f3f`
//!   (`hsl(0 0% 91%)` in the dark theme);
//! - `[data-button]`: 24 points tall, `padding: 0 8px`, `border-radius:
//!   4px`, 12 points, 500, the toast's text color as its background and its
//!   background as its text, at the end of the row (`margin-left: auto`);
//! - the toaster: fixed 24 points from its corner (16 below 600 points),
//!   356 wide.
//!
//! Sonner's enter (`translateY(100%)`, transparent, 400 ms) and exit are the
//! toast's starting and ending looks with a 400 ms transition, and its
//! layers' motion is the stack's springs. Sonner's `richColors`, `invert`,
//! close button, `expand` and `theme` props are not here: shadcn's demos
//! use none of them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use nsh_base::{Anchor, ToastManager, ToastMotion, ToastOptions, ToastStackState, ToastTransitionStatus};
use nsh_core::prelude::*;
use nsh_core::time::now;

#[cfg(test)]
use crate::cn::cn;
use crate::icon::{IconName, icon};

/// How long a toast shows: sonner's `TOAST_LIFETIME`.
const LIFETIME: Duration = Duration::from_millis(4000);

const TOAST: &str = "flex w-full items-center gap-1.5 rounded-lg border border-border bg-popover \
    p-4 text-[13px] text-popover-foreground shadow-[0px_4px_12px_rgba(0,0,0,0.1)] \
    transition duration-[400ms]";
const ICON: &str = "relative flex size-4 shrink-0 items-center justify-start -ml-[3px] mr-1";
const ICON_SVG: &str = "size-4 -ml-px";
const CONTENT: &str = "flex min-w-0 flex-1 flex-col gap-0.5";
const TITLE: &str = "font-medium leading-[1.5]";
const DESCRIPTION: &str = "font-normal leading-[1.4] text-[#3f3f3f] dark:text-[#e8e8e8]";
const BUTTON: &str = "ml-auto flex h-6 shrink-0 cursor-pointer items-center rounded-[4px] \
    bg-popover-foreground px-2 text-xs font-medium text-popover";
/// Sonner's enter and exit: a toast at the bottom comes up from below its
/// place, transparent, and goes back down.
const STARTING: &str = "opacity-0 translate-y-full";
const ENDING_BOTTOM: &str = "opacity-0 translate-y-full";
const ENDING_TOP: &str = "opacity-0 -translate-y-full";

/// What kind of toast it is: sonner's `toast.success(..)` and the others,
/// each with shadcn's icon.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ToastType {
    /// No icon.
    #[default]
    Default,
    /// `circle-check`.
    Success,
    /// `info`.
    Info,
    /// `triangle-alert`.
    Warning,
    /// `octagon-x`.
    Error,
    /// A spinning `loader-circle`.
    Loading,
}

impl ToastType {
    fn icon(self) -> Option<IconName> {
        match self {
            ToastType::Default => None,
            ToastType::Success => Some(IconName::CircleCheck),
            ToastType::Info => Some(IconName::Info),
            ToastType::Warning => Some(IconName::TriangleAlert),
            ToastType::Error => Some(IconName::OctagonX),
            ToastType::Loading => Some(IconName::LoaderCircle),
        }
    }
}

/// Where the toaster is: sonner's `position`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PropEnum)]
#[non_exhaustive]
pub enum ToasterPosition {
    /// `top-left`.
    TopLeft,
    /// `top-center`.
    TopCenter,
    /// `top-right`.
    TopRight,
    /// `bottom-left`.
    BottomLeft,
    /// `bottom-center`.
    BottomCenter,
    /// `bottom-right` (sonner's default).
    #[default]
    BottomRight,
}

impl ToasterPosition {
    fn anchor(self) -> Anchor {
        match self {
            ToasterPosition::TopLeft => Anchor::TopLeft,
            ToasterPosition::TopCenter => Anchor::TopCenter,
            ToasterPosition::TopRight => Anchor::TopRight,
            ToasterPosition::BottomLeft => Anchor::BottomLeft,
            ToasterPosition::BottomCenter => Anchor::BottomCenter,
            ToasterPosition::BottomRight => Anchor::BottomRight,
        }
    }

    fn top(self) -> bool {
        matches!(
            self,
            ToasterPosition::TopLeft | ToasterPosition::TopCenter | ToasterPosition::TopRight
        )
    }

    /// Sonner's `[data-sonner-toaster]`: fixed `--offset` (24px) from its
    /// corner, `--width` (356px) wide; below 600 points, 16 from the edges
    /// and as wide as they leave.
    fn classes(self) -> &'static str {
        match self {
            ToasterPosition::TopLeft => {
                "fixed top-6 left-6 w-[356px] max-[600px]:top-4 max-[600px]:left-4 \
                 max-[600px]:right-4 max-[600px]:w-auto"
            }
            ToasterPosition::TopCenter => {
                "fixed top-6 left-1/2 w-[356px] -translate-x-1/2 max-[600px]:top-4 \
                 max-[600px]:left-4 max-[600px]:right-4 max-[600px]:w-auto max-[600px]:translate-x-0"
            }
            ToasterPosition::TopRight => {
                "fixed top-6 right-6 w-[356px] max-[600px]:top-4 max-[600px]:left-4 \
                 max-[600px]:right-4 max-[600px]:w-auto"
            }
            ToasterPosition::BottomLeft => {
                "fixed bottom-6 left-6 w-[356px] max-[600px]:bottom-4 max-[600px]:left-4 \
                 max-[600px]:right-4 max-[600px]:w-auto"
            }
            ToasterPosition::BottomCenter => {
                "fixed bottom-6 left-1/2 w-[356px] -translate-x-1/2 max-[600px]:bottom-4 \
                 max-[600px]:left-4 max-[600px]:right-4 max-[600px]:w-auto max-[600px]:translate-x-0"
            }
            ToasterPosition::BottomRight => {
                "fixed right-6 bottom-6 w-[356px] max-[600px]:bottom-4 max-[600px]:left-4 \
                 max-[600px]:right-4 max-[600px]:w-auto"
            }
        }
    }
}

/// A toast's id: what [`SonnerToast::show`] returns and [`dismiss`] takes.
pub type ToastId = u64;

/// What a toast shows.
#[derive(Clone, Debug, PartialEq)]
struct Shown {
    title: SharedString,
    description: Option<SharedString>,
    kind: ToastType,
    action: Option<SharedString>,
}

/// The toasts there are: one store per surface, as sonner's one store per
/// page; the [`Toaster`] shows them.
/// What the actions of the toasts run, by toast.
type Actions = HashMap<ToastId, Rc<dyn Fn()>>;

#[derive(Clone, Default)]
struct Store {
    manager: Rc<RefCell<Option<ToastManager<ToastId, Shown>>>>,
    actions: Rc<RefCell<Actions>>,
    next: Rc<RefCell<ToastId>>,
    /// Counts the changes the toaster shows.
    version: u64,
}

fn store() -> Signal<Store> {
    global::<Store>()
}

/// Runs `f` on the manager, then shows what it changed.
fn change<R>(f: impl FnOnce(&mut ToastManager<ToastId, Shown>) -> R) -> R {
    let store = store();
    let result = store.with_untracked(|s| {
        let mut manager = s.manager.borrow_mut();
        f(manager.get_or_insert_with(|| ToastManager::new(ToastMotion::sonner())))
    });
    store.update(|s| s.version += 1);
    result
}

/// A toast to show, sonner's `toast(title, options)`: `toast("Saved").show()`,
/// `toast("Event has been created").description("…").action("Undo", undo).show()`.
#[must_use = "a toast shows with .show()"]
pub struct SonnerToast {
    shown: Shown,
    id: Option<ToastId>,
    duration: Option<Duration>,
    on_action: Option<Rc<dyn Fn()>>,
}

/// A toast titled `title`, shown by [`SonnerToast::show`] in the page's
/// [`Toaster`].
pub fn toast(title: impl Into<SharedString>) -> SonnerToast {
    SonnerToast {
        shown: Shown {
            title: title.into(),
            description: None,
            kind: ToastType::Default,
            action: None,
        },
        id: None,
        duration: Some(LIFETIME),
        on_action: None,
    }
}

impl SonnerToast {
    /// Text under the title.
    pub fn description(mut self, description: impl Into<SharedString>) -> Self {
        self.shown.description = Some(description.into());
        self
    }

    /// A button at its end: pressing it runs `on_press` and dismisses the
    /// toast.
    pub fn action(mut self, label: impl Into<SharedString>, on_press: impl Fn() + 'static) -> Self {
        self.shown.action = Some(label.into());
        self.on_action = Some(Rc::new(on_press));
        self
    }

    /// Its kind (and icon).
    pub fn kind(mut self, kind: ToastType) -> Self {
        self.shown.kind = kind;
        self
    }

    /// `toast.success(..)`.
    pub fn success(self) -> Self {
        self.kind(ToastType::Success)
    }
    /// `toast.info(..)`.
    pub fn info(self) -> Self {
        self.kind(ToastType::Info)
    }
    /// `toast.warning(..)`.
    pub fn warning(self) -> Self {
        self.kind(ToastType::Warning)
    }
    /// `toast.error(..)`.
    pub fn error(self) -> Self {
        self.kind(ToastType::Error)
    }
    /// `toast.loading(..)`: it stays until it is replaced or dismissed.
    pub fn loading(self) -> Self {
        Self {
            duration: None,
            ..self.kind(ToastType::Loading)
        }
    }

    /// How long it shows (default 4 seconds); `None`: until dismissed.
    pub fn duration(mut self, duration: Option<Duration>) -> Self {
        self.duration = duration;
        self
    }

    /// Replaces the toast `id` where it is (sonner's `id` option): a
    /// loading toast becoming its result.
    pub fn id(mut self, id: ToastId) -> Self {
        self.id = Some(id);
        self
    }

    /// Shows it; its id.
    pub fn show(self) -> ToastId {
        let s = store();
        let id = self.id.unwrap_or_else(|| {
            s.with_untracked(|s| {
                let mut next = s.next.borrow_mut();
                *next += 1;
                *next
            })
        });
        s.with_untracked(|s| {
            let mut actions = s.actions.borrow_mut();
            match self.on_action {
                Some(action) => actions.insert(id, action),
                None => actions.remove(&id),
            }
        });
        let shown = self.shown;
        let duration = self.duration;
        change(|m| m.push(id, shown, ToastOptions::timeout(duration), now()));
        id
    }
}

/// Dismisses the toast `id`: it runs its exit and goes.
pub fn dismiss(id: ToastId) {
    change(|m| m.dismiss(&id, now()));
}

/// Dismisses every toast.
pub fn dismiss_all() {
    change(|m| m.dismiss_all(now()));
}

/// The props of [`Toaster`].
#[derive(Debug, Clone, PartialEq, Props)]
pub struct ToasterProps {
    /// Where the toasts show (default `bottom-right`).
    #[prop(default)]
    pub position: Prop<ToasterPosition>,
    /// How many show at once, collapsed (default 3).
    #[prop(default = 3)]
    pub visible_toasts: Prop<u32>,
}

/// shadcn's Toaster: where the page's toasts show. Put one in the page;
/// [`toast`] shows a toast in it from anywhere.
#[component]
pub fn Toaster(props: ToasterProps) -> impl IntoElement {
    let store = store();
    let state = ToastStackState::new();
    let shown = computed(move || {
        let limit = props.visible_toasts.get() as usize;
        store.with(|s| {
            s.manager
                .borrow()
                .as_ref()
                .map(|m| {
                    m.visible(limit.max(1))
                        .map(|(id, t, status)| (*id, t.clone(), status))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    // The lifecycle advances while toasts are mounted, paused while the
    // stack is expanded.
    set_interval(Duration::from_millis(50), move || {
        let paused = untracked(|| state.is_expanded());
        let advanced = store.with_untracked(|s| {
            s.manager
                .borrow_mut()
                .as_mut()
                .map(|m| m.advance(now(), paused))
        });
        if let Some(advanced) = advanced.filter(|a| a.changed) {
            store.with_untracked(|s| {
                let mut actions = s.actions.borrow_mut();
                for (id, _) in &advanced.removed {
                    actions.remove(id);
                }
            });
            store.update(|s| s.version += 1);
        }
    });
    let newest = move || {
        untracked(|| shown.get())
            .iter()
            .rev()
            .find(|(_, _, status)| *status != ToastTransitionStatus::Ending)
            .map(|(id, _, _)| *id)
    };
    overlay()
        .app_class(move || Some(SharedString::from(props.position.get().classes())))
        .child(
            nsh_base::toast_stack()
                .placement(move || props.position.get().anchor())
                .state(state)
                .app_class(Some(SharedString::from("w-full")))
                .on_cancel(move || {
                    if let Some(id) = newest() {
                        dismiss(id);
                    }
                })
                .child(each(
                    move || shown.get(),
                    |(id, _, _)| *id,
                    move |item, _| toast_item(item, props.position),
                )),
        )
}

/// One toast of the stack: sonner's `[data-sonner-toast]`.
fn toast_item(
    item: ReadSignal<(ToastId, Shown, ToastTransitionStatus)>,
    position: Prop<ToasterPosition>,
) -> impl IntoElement {
    let id = item.get_untracked().0;
    let shown = move || item.get().1;
    nsh_base::toast()
        .transition_status(move || item.get().2)
        .accessibility_label(move || {
            let shown = shown();
            Some(match shown.description {
                Some(d) => SharedString::from(format!("{}. {}", shown.title, d)),
                None => shown.title,
            })
        })
        .on_dismiss_request(move || dismiss(id))
        .app_class(Some(SharedString::from(TOAST)))
        .starting_class(Some(SharedString::from(STARTING)))
        .ending_class(move || {
            Some(SharedString::from(if position.get().top() {
                ENDING_TOP
            } else {
                ENDING_BOTTOM
            }))
        })
        .swiping_class(Some(SharedString::from("transition-none")))
        .child(dynamic(move || {
            shown().kind.icon().map(|name| {
                let spin = if shown().kind == ToastType::Loading {
                    " animate-spin"
                } else {
                    ""
                };
                view()
                    .app_class(Some(SharedString::from(ICON)))
                    .child(icon(name).app_class(Some(SharedString::from(format!("{ICON_SVG}{spin}")))))
                    .into_element()
            })
        }))
        .child(
            view()
                .app_class(Some(SharedString::from(CONTENT)))
                .child(text(move || shown().title).app_class(Some(SharedString::from(TITLE))))
                .child(dynamic(move || {
                    shown().description.map(|d| {
                        text(d)
                            .app_class(Some(SharedString::from(DESCRIPTION)))
                            .into_element()
                    })
                })),
        )
        .child(dynamic(move || {
            shown().action.map(|label| {
                view()
                    .role(AccessibilityRole::Button)
                    .label(label.clone())
                    .app_class(Some(SharedString::from(BUTTON)))
                    .on_press(move || {
                        let action = store().with_untracked(|s| s.actions.borrow().get(&id).cloned());
                        if let Some(action) = action {
                            action();
                        }
                        dismiss(id);
                    })
                    .child(text(label))
                    .into_element()
            })
        }))
}

pub(crate) fn register(registry: &mut Registry) {
    registry.register::<Toaster>();
}

/// Every class a toast draws with, for the unit tests.
#[cfg(test)]
fn all_classes() -> SharedString {
    cn([
        TOAST,
        ICON,
        ICON_SVG,
        "animate-spin",
        CONTENT,
        TITLE,
        DESCRIPTION,
        BUTTON,
        STARTING,
        ENDING_BOTTOM,
        ENDING_TOP,
        "transition-none w-full",
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_class_is_known_to_nsh() {
        let mut unknown = crate::cn::unknown_classes(&all_classes());
        for position in [
            ToasterPosition::TopLeft,
            ToasterPosition::TopCenter,
            ToasterPosition::TopRight,
            ToasterPosition::BottomLeft,
            ToasterPosition::BottomCenter,
            ToasterPosition::BottomRight,
        ] {
            unknown.extend(crate::cn::unknown_classes(position.classes()));
        }
        assert!(unknown.is_empty(), "classes NSH does not know: {unknown:?}");
    }
}
