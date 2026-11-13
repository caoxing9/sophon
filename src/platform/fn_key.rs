//! Dictation's shortcut on macOS, heard by a local monitor of the keyboard
//! (it sees a key before the app's own bindings and the composer's editor
//! do, and the Fn key, a modifier those never hear):
//!
//! - Fn (🌐) on its own: when it goes down alone, comes up, or turns out
//!   part of a combination (another key or modifier with it).
//! - Any other shortcut chosen in settings: when it is pressed.
//! - While a new shortcut is being recorded in settings: the next
//!   combination pressed (Esc cancels, Delete clears it).
//!
//! Only while Sophon is the active app: a local monitor hears its own
//! events. The keys it acts on go no further into the app; what macOS
//! itself does with Fn is the system's.

use std::{cell::Cell, sync::Mutex};

use block2::RcBlock;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};

use crate::store::{Hotkey, MOD_ALT, MOD_CMD, MOD_CTRL, MOD_SHIFT};

/// macOS's key codes (`kVK_…`).
const FN_KEY: u16 = 63;
const ESCAPE: u16 = 53;
const DELETE: u16 = 51;

/// What the monitor heard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Heard {
    /// Fn down, alone.
    FnDown,
    /// Fn up again, alone all along.
    FnUp,
    /// Another key or modifier came with Fn: not Fn alone.
    FnCombined,
    /// The chosen shortcut (not Fn) was pressed.
    Shortcut,
    /// Recording: this combination was pressed.
    Recorded(Hotkey),
    /// Recording: Escape, nothing changes.
    Cancelled,
    /// Recording: Delete, no shortcut.
    Cleared,
}

/// What the monitor matches and whether it records, set from the engine's
/// thread, read on the main one.
struct Shared {
    shortcut: Option<(u16, u8)>,
    recording: bool,
}

static SHARED: Mutex<Shared> = Mutex::new(Shared { shortcut: None, recording: false });

/// The shortcut the monitor answers to (`None`: none, or Fn's, which it
/// always reports).
pub(crate) fn set_shortcut(key: Option<&Hotkey>) {
    if let Ok(mut s) = SHARED.lock() {
        s.shortcut = key.filter(|k| !k.is_fn()).map(|k| (k.code, k.mods));
    }
}

/// Records the next combination pressed (`true`), or stops recording.
pub(crate) fn record(on: bool) {
    if let Ok(mut s) = SHARED.lock() {
        s.recording = on;
    }
}

thread_local! {
    /// Fn is down, and nothing else happened since.
    static CLEAN: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "C" {
    /// `dispatch_get_main_queue()` is this symbol's address.
    static _dispatch_main_q: std::ffi::c_void;
    fn dispatch_async_f(queue: *const std::ffi::c_void, context: *mut std::ffi::c_void, work: extern "C" fn(*mut std::ffi::c_void));
}

/// The modifiers held, as [`Hotkey::mods`].
fn mods_of(flags: NSEventModifierFlags) -> u8 {
    let mut m = 0;
    for (flag, bit) in [
        (NSEventModifierFlags::Control, MOD_CTRL),
        (NSEventModifierFlags::Option, MOD_ALT),
        (NSEventModifierFlags::Shift, MOD_SHIFT),
        (NSEventModifierFlags::Command, MOD_CMD),
    ] {
        if flags.contains(flag) {
            m |= bit;
        }
    }
    m
}

/// A key's name as a shortcut shows it.
fn key_label(code: u16, event: &NSEvent) -> String {
    let named = match code {
        49 => "Space",
        36 => "↩",
        48 => "⇥",
        123 => "←",
        124 => "→",
        125 => "↓",
        126 => "↑",
        _ => "",
    };
    if !named.is_empty() {
        return named.to_owned();
    }
    if let Some(n) = function_key(code) {
        return format!("F{n}");
    }
    event.charactersIgnoringModifiers().map(|s| s.to_string().to_uppercase()).filter(|s| !s.trim().is_empty()).unwrap_or_else(|| format!("#{code}"))
}

/// F1–F12's number, by key code.
fn function_key(code: u16) -> Option<u8> {
    [122u16, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111].iter().position(|c| *c == code).map(|i| i as u8 + 1)
}

/// Calls `heard` (on the main thread) with what the keyboard does that
/// dictation's shortcut cares about. Once; the monitor stays for the
/// app's life.
pub(crate) fn watch(heard: impl Fn(Heard) + Send + 'static) {
    extern "C" fn install(context: *mut std::ffi::c_void) {
        // SAFETY: the box `watch` leaked for this one call.
        let heard = unsafe { Box::from_raw(context.cast::<Box<dyn Fn(Heard) + Send>>()) };
        let block = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit hands the monitor a live event.
            let e = unsafe { event.as_ref() };
            let (shortcut, recording) = SHARED.lock().map(|s| (s.shortcut, s.recording)).unwrap_or((None, false));
            let kind = e.r#type();
            if kind == NSEventType::KeyDown {
                let fn_was_alone = CLEAN.with(|c| c.replace(false));
                let (code, mods) = (e.keyCode(), mods_of(e.modifierFlags()));
                if recording {
                    match (code, mods) {
                        (ESCAPE, 0) => heard(Heard::Cancelled),
                        (DELETE, 0) => heard(Heard::Cleared),
                        // A shortcut has a modifier, or is a function key:
                        // a plain letter is for typing.
                        _ if mods != 0 || function_key(code).is_some() => {
                            let label = format!("{}{}", Hotkey::mods_label(mods), key_label(code, e));
                            heard(Heard::Recorded(Hotkey { code, mods, label }));
                        }
                        _ => {}
                    }
                    return std::ptr::null_mut();
                }
                if fn_was_alone {
                    heard(Heard::FnCombined);
                }
                if shortcut == Some((code, mods)) {
                    if !e.isARepeat() {
                        heard(Heard::Shortcut);
                    }
                    return std::ptr::null_mut();
                }
            } else if kind == NSEventType::FlagsChanged {
                let flags = e.modifierFlags();
                let others = mods_of(flags) != 0;
                if e.keyCode() == FN_KEY {
                    if flags.contains(NSEventModifierFlags::Function) {
                        if !others {
                            CLEAN.with(|c| c.set(true));
                            if !recording {
                                heard(Heard::FnDown);
                            }
                        }
                    } else if CLEAN.with(|c| c.replace(false)) {
                        heard(if recording { Heard::Recorded(Hotkey::fn_key()) } else { Heard::FnUp });
                    }
                    // Fn's own event stops here: the app has no use for it.
                    return std::ptr::null_mut();
                }
                // Another modifier while Fn is held: a combination.
                if CLEAN.with(|c| c.replace(false)) && !recording {
                    heard(Heard::FnCombined);
                }
            }
            event.as_ptr()
        });
        // SAFETY: the block returns the event it got, or null for one it
        // handled; the monitor is kept for the app's life (its token is
        // leaked on purpose).
        let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::FlagsChanged | NSEventMask::KeyDown, &block) };
        std::mem::forget(monitor);
    }
    let heard: Box<Box<dyn Fn(Heard) + Send>> = Box::new(Box::new(heard));
    // SAFETY: the main queue, where AppKit's monitors are added; the
    // context is the box above, taken back once.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, Box::into_raw(heard).cast(), install) };
}
