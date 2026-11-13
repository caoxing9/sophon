//! The Fn (🌐) key held on its own, on macOS: a modifier, which the app's
//! key bindings do not hear, so a local monitor of the modifier keys
//! watches for it and says when it goes down alone, comes up, or turns
//! out part of a combination (another key or modifier with it: Fn with an
//! arrow key, Fn+F-keys). Only while Sophon is the active app: a local
//! monitor hears its own events. Fn's own events go no further into the
//! app; what macOS itself does with the key is the system's.

use std::cell::Cell;

use block2::RcBlock;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};

/// macOS's key code of Fn (`kVK_Function`).
const FN_KEY: u16 = 63;

/// What the Fn key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FnKey {
    /// Down, alone.
    Down,
    /// Up again, alone all along.
    Up,
    /// Another key or modifier came with it: not Fn alone.
    Combined,
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

/// Calls `heard` (on the main thread) with what the Fn key does. Once; the
/// monitor stays for the app's life.
pub(crate) fn watch(heard: impl Fn(FnKey) + Send + 'static) {
    extern "C" fn install(context: *mut std::ffi::c_void) {
        // SAFETY: the box `watch` leaked for this one call.
        let heard = unsafe { Box::from_raw(context.cast::<Box<dyn Fn(FnKey) + Send>>()) };
        let block = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit hands the monitor a live event.
            let e = unsafe { event.as_ref() };
            let kind = e.r#type();
            let combined = |heard: &dyn Fn(FnKey)| {
                if CLEAN.with(|c| c.replace(false)) {
                    heard(FnKey::Combined);
                }
            };
            if kind == NSEventType::KeyDown {
                combined(&*heard);
            } else if kind == NSEventType::FlagsChanged {
                let flags = e.modifierFlags();
                let others = flags.intersects(
                    NSEventModifierFlags::Command | NSEventModifierFlags::Option | NSEventModifierFlags::Control | NSEventModifierFlags::Shift,
                );
                if e.keyCode() == FN_KEY {
                    if flags.contains(NSEventModifierFlags::Function) {
                        if !others {
                            CLEAN.with(|c| c.set(true));
                            heard(FnKey::Down);
                        }
                    } else if CLEAN.with(|c| c.replace(false)) {
                        heard(FnKey::Up);
                    }
                    // Fn's own event stops here: the app has no use for it.
                    return std::ptr::null_mut();
                }
                // Another modifier while Fn is held: a combination.
                combined(&*heard);
            }
            event.as_ptr()
        });
        // SAFETY: the block returns the event it got, or null for Fn's
        // own (handled); the monitor is kept for the app's life (its
        // token is leaked on purpose).
        let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::FlagsChanged | NSEventMask::KeyDown, &block) };
        std::mem::forget(monitor);
    }
    let heard: Box<Box<dyn Fn(FnKey) + Send>> = Box::new(Box::new(heard));
    // SAFETY: the main queue, where AppKit's monitors are added; the
    // context is the box above, taken back once.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, Box::into_raw(heard).cast(), install) };
}
