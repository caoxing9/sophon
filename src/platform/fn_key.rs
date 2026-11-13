//! The Fn (🌐) key pressed on its own, on macOS: a modifier, which the
//! app's key bindings do not hear, so a local monitor of the modifier keys
//! watches for it. A press counts when Fn goes down and comes up with no
//! other key pressed and no other modifier held meanwhile (Fn with an
//! arrow key, or Fn+F-keys, is not one). Only while Sophon is the active
//! app: a local monitor hears its own events.

use std::cell::Cell;

use block2::RcBlock;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventModifierFlags, NSEventType};

/// macOS's key code of Fn (`kVK_Function`).
const FN_KEY: u16 = 63;

thread_local! {
    /// Fn is down, and nothing else happened since.
    static CLEAN: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "C" {
    /// `dispatch_get_main_queue()` is this symbol's address.
    static _dispatch_main_q: std::ffi::c_void;
    fn dispatch_async_f(queue: *const std::ffi::c_void, context: *mut std::ffi::c_void, work: extern "C" fn(*mut std::ffi::c_void));
}

/// Calls `pressed` (on the main thread) each time Fn is pressed on its
/// own. Once; the monitor stays for the app's life.
pub(crate) fn watch(pressed: impl Fn() + Send + 'static) {
    extern "C" fn install(context: *mut std::ffi::c_void) {
        // SAFETY: the box `watch` leaked for this one call.
        let pressed = unsafe { Box::from_raw(context.cast::<Box<dyn Fn() + Send>>()) };
        let block = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
            // SAFETY: AppKit hands the monitor a live event.
            let e = unsafe { event.as_ref() };
            let kind = e.r#type();
            if kind == NSEventType::KeyDown {
                CLEAN.with(|c| c.set(false));
            } else if kind == NSEventType::FlagsChanged {
                let flags = e.modifierFlags();
                let others = flags.intersects(
                    NSEventModifierFlags::Command | NSEventModifierFlags::Option | NSEventModifierFlags::Control | NSEventModifierFlags::Shift,
                );
                let fn_down = flags.contains(NSEventModifierFlags::Function);
                if e.keyCode() == FN_KEY {
                    if fn_down {
                        CLEAN.with(|c| c.set(!others));
                    } else if CLEAN.with(|c| c.replace(false)) && !others {
                        pressed();
                    }
                } else {
                    // Another modifier while Fn is held: a combination.
                    CLEAN.with(|c| c.set(false));
                }
            }
            event.as_ptr()
        });
        // SAFETY: the block returns the event it got; the monitor is kept
        // for the app's life (its token is leaked on purpose).
        let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(NSEventMask::FlagsChanged | NSEventMask::KeyDown, &block) };
        std::mem::forget(monitor);
    }
    let pressed: Box<Box<dyn Fn() + Send>> = Box::new(Box::new(pressed));
    // SAFETY: the main queue, where AppKit's monitors are added; the
    // context is the box above, taken back once.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, Box::into_raw(pressed).cast(), install) };
}
