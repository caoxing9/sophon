//! A value that is the app's when it passes one and the component's own
//! otherwise: React's controlled / uncontrolled props (`checked` and
//! `defaultChecked`), which Radix's primitives take and `nsh_base`'s
//! controls leave to the app.

use nsh_core::prelude::*;

/// The app's value (`controlled`), else the component's own, from the
/// default it was made with.
pub(crate) struct Controllable<T: 'static> {
    controlled: Prop<Option<T>>,
    own: Signal<T>,
}

impl<T: 'static> Clone for Controllable<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for Controllable<T> {}

impl<T: Clone + PartialEq + 'static> Controllable<T> {
    /// `controlled`, falling back to a value of the component's own that
    /// starts at `default`.
    pub(crate) fn new(controlled: Prop<Option<T>>, default: T) -> Self {
        Self {
            controlled,
            own: signal(default),
        }
    }

    /// The value (tracked).
    pub(crate) fn get(self) -> T {
        self.controlled.get().unwrap_or_else(|| self.own.get())
    }

    /// The value, read without subscribing.
    pub(crate) fn get_untracked(self) -> T {
        self.controlled
            .get_untracked()
            .unwrap_or_else(|| self.own.get_untracked())
    }

    /// The user asked for `value`: kept when the app does not control the
    /// value (the app sets its own from the event).
    pub(crate) fn set(self, value: T) {
        if self.controlled.get_untracked().is_none() {
            self.own.set(value);
        }
    }
}
