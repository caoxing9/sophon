//! What TSX screens reach of Sophon: `import { Prefs } from "sophon/native"`.
//! Rust keeps the state; TSX reads it and hears every change through
//! `Prefs.changed(..)`, and changes it through the setters, so a TSX screen
//! and a Rust one show the same thing at all times.
//!
//! The state is signals, which only the core's side of a call reaches: the
//! setters are ops (they take `cx`, run at the commit) and `changed` is a
//! task of the core (`async`); a plain function runs inside the JS call,
//! outside the reactive graph, and could not read them.

use nsh_core::prelude::*;

use crate::store::{self, KeepAwake, SendBehavior};

/// Sophon's preferences, as TSX sees them: each a name the value is saved
/// under (`"system"`, `"steer"`, ...).
#[derive(Debug, Clone, Default, PartialEq, JsObject)]
pub struct PrefsState {
    /// `auto` (the system's), `en` or `zh`.
    pub language: SharedString,
    /// `system`, `light` or `dark`.
    pub appearance: SharedString,
    /// What Return does while an agent runs: `steer`, `interrupt` or `queue`.
    pub send_behavior: SharedString,
    /// `off`, `on` or `display` (the screen stays on too).
    pub keep_awake: SharedString,
    /// Whether this computer can keep itself awake (a Mac).
    pub can_keep_awake: bool,
}

fn state() -> PrefsState {
    let s = store::settings();
    PrefsState {
        language: crate::i18n::lang().as_str().into(),
        appearance: match store::appearance() {
            nsh_core::platform::Appearance::Light => "light",
            nsh_core::platform::Appearance::Dark => "dark",
            _ => "system",
        }
        .into(),
        send_behavior: match s.send_behavior {
            SendBehavior::Steer => "steer",
            SendBehavior::Interrupt => "interrupt",
            SendBehavior::Queue => "queue",
        }
        .into(),
        keep_awake: match s.keep_awake {
            KeepAwake::Off => "off",
            KeepAwake::On => "on",
            KeepAwake::Display => "display",
        }
        .into(),
        can_keep_awake: cfg!(target_os = "macos"),
    }
}

/// Sophon's preferences: read, changed and listened to from TSX.
#[module]
pub struct Prefs;

#[module]
impl Prefs {
    /// The preferences once they differ from `since` (at once when they
    /// do already): a screen awaits it in a loop, from an empty state, to
    /// read them and then hear each change.
    pub async fn changed(since: PrefsState) -> PrefsState {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let tx = std::cell::RefCell::new(Some(tx));
        let owner = Owner::detached();
        owner.run(|| {
            effect(move || {
                let now = state();
                if now != since
                    && let Some(tx) = tx.borrow_mut().take()
                {
                    let _ = tx.send(now);
                }
            });
        });
        let now = rx.await.unwrap_or_default();
        owner.dispose();
        now
    }

    /// Sets the interface's language: `auto` (the system's), `en` or `zh`.
    pub fn set_language(value: String, _cx: &mut ModuleCx) {
        store::set_language(crate::i18n::Lang::parse(&value));
    }

    /// Sets the appearance: `system`, `light` or `dark`.
    pub fn set_appearance(value: String, _cx: &mut ModuleCx) {
        store::set_appearance(match value.as_str() {
            "light" => nsh_core::platform::Appearance::Light,
            "dark" => nsh_core::platform::Appearance::Dark,
            _ => nsh_core::platform::Appearance::System,
        });
    }

    /// Sets what Return does while an agent runs: `steer`, `interrupt` or
    /// `queue`.
    pub fn set_send_behavior(value: String, _cx: &mut ModuleCx) {
        let b = match value.as_str() {
            "interrupt" => SendBehavior::Interrupt,
            "queue" => SendBehavior::Queue,
            _ => SendBehavior::Steer,
        };
        store::edit_settings(|s| s.send_behavior = b);
    }

    /// Sets whether the Mac stays awake while its agents run: `off`, `on`
    /// or `display`.
    pub fn set_keep_awake(value: String, _cx: &mut ModuleCx) {
        let k = match value.as_str() {
            "off" => KeepAwake::Off,
            "display" => KeepAwake::Display,
            _ => KeepAwake::On,
        };
        store::edit_settings(|s| s.keep_awake = k);
    }

    /// `text` in the interface's language (Sophon's own translations); a
    /// plain call, which touches no signal.
    pub fn t(text: String) -> String {
        crate::i18n::tr(&text)
    }
}
