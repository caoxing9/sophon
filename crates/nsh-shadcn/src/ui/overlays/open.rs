//! The open state an overlay's root shares with its parts (Radix's
//! `Dialog.Root` context): whether it is open, a request to open or close
//! it, and the title and description its parts register, which name the
//! modal for screen readers.

use std::rc::Rc;

use nsh_core::prelude::*;

/// What the parts of a dialog, alert dialog, sheet or drawer find through
/// context: [`OpenContext::get`] and [`OpenContext::set`].
#[derive(Clone)]
pub(crate) struct OpenContext {
    get: Rc<dyn Fn() -> bool>,
    set: Rc<dyn Fn(bool)>,
    /// The title the `*Title` part shows: the modal's name.
    pub title: Signal<Option<SharedString>>,
    /// The text the `*Description` part shows: the modal's description.
    pub description: Signal<Option<SharedString>>,
}

impl OpenContext {
    /// The state of a root: controlled by `open` when the app sets it,
    /// else its own, starting at `default_open`. `changed` hears every
    /// request (the app follows it with `open`).
    pub fn new(
        open: Prop<Option<bool>>,
        default_open: Prop<bool>,
        changed: impl Fn(bool) + 'static,
    ) -> Self {
        let own = signal(default_open.get_untracked());
        let get = move || open.get().unwrap_or_else(|| own.get());
        let set = move |next: bool| {
            if untracked(get) == next {
                return;
            }
            if open.get_untracked().is_none() {
                own.set(next);
            }
            changed(next);
        };
        Self {
            get: Rc::new(get),
            set: Rc::new(set),
            title: signal(None),
            description: signal(None),
        }
    }

    /// The root's state, provided to what it makes from now on.
    pub fn provide(self) -> Self {
        provide_context(self.clone());
        self
    }

    /// The state of the nearest root: `part` names the part for the panic
    /// when there is none.
    pub fn of(part: &str) -> Self {
        use_context::<OpenContext>()
            .unwrap_or_else(|| panic!("{part} must be inside its root (Dialog, Sheet, …)"))
    }

    /// Whether it is open (tracked).
    pub fn get(&self) -> bool {
        (self.get)()
    }

    /// Asks to open or close it.
    pub fn set(&self, open: bool) {
        (self.set)(open)
    }

    /// A `*Title` part shows `title`: it names the modal while the part is
    /// there.
    pub fn register_title(&self, title: impl Fn() -> Option<SharedString> + 'static) {
        let slot = self.title;
        effect(move || slot.set(title()));
        cleanup(move || slot.set(None));
    }

    /// A `*Description` part shows `description`.
    pub fn register_description(&self, description: impl Fn() -> Option<SharedString> + 'static) {
        let slot = self.description;
        effect(move || slot.set(description()));
        cleanup(move || slot.set(None));
    }
}

/// A root's trigger: the app's element in the slot, pressed to toggle the
/// root, with the expanded state screen readers hear (Radix's
/// `*.Trigger`, `asChild`).
pub(crate) fn trigger(slot: Prop<Option<Slot>>, state: OpenContext) -> AnyElement {
    slot.place(move |trigger| {
        let expanded = state.clone();
        let toggle = state.clone();
        trigger
            .accessibility_state(move || AccessibilityState {
                expanded: Some(expanded.get()),
                has_popup: true,
                ..AccessibilityState::default()
            })
            .on_press(move || toggle.set(!untracked(|| toggle.get())))
    })
}

/// A part that closes its root when the app's element in `slot` is
/// pressed (Radix's `*.Close`, `asChild`).
pub(crate) fn close(slot: Prop<Option<Slot>>, part: &str) -> AnyElement {
    let state = OpenContext::of(part);
    slot.place(move |button| {
        let state = state.clone();
        button.on_press(move || state.set(false))
    })
}
