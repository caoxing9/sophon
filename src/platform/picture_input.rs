//! The picture viewer's mouse on macOS: its wheel zooms, and a click (a
//! press let go where it was) puts the viewer away. The viewer's own
//! drag gesture takes the presses AppKit gives the app, and NSH has no
//! wheel event, so a local monitor watches for them while a picture is
//! open; the viewer says where its own buttons are.

use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

use block2::RcBlock;
use objc2_app_kit::{NSEvent, NSEventMask, NSEventType};

/// What the monitor heard while a picture was open.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Heard {
    /// The wheel turned: zoom by this factor.
    Zoom(f32),
    /// A click, in the window (points from the top left).
    Click { x: f32, y: f32 },
}

static OPEN: AtomicBool = AtomicBool::new(false);
/// Where the last press went down (window points, from the bottom left).
static DOWN: Mutex<Option<(f64, f64)>> = Mutex::new(None);

/// A picture is open (`true`) or not.
pub(crate) fn set_open(open: bool) {
    OPEN.store(open, Ordering::Relaxed);
}

unsafe extern "C" {
    /// `dispatch_get_main_queue()` is this symbol's address.
    static _dispatch_main_q: std::ffi::c_void;
    fn dispatch_async_f(queue: *const std::ffi::c_void, context: *mut std::ffi::c_void, work: extern "C" fn(*mut std::ffi::c_void));
}

/// Calls `heard` (on the main thread) with the wheel's turns and the
/// clicks while a picture is open. Once; the monitor stays.
pub(crate) fn watch(heard: impl Fn(Heard) + Send + 'static) {
    extern "C" fn install(context: *mut std::ffi::c_void) {
        // SAFETY: the box `watch` leaked for this one call.
        let heard = unsafe { Box::from_raw(context.cast::<Box<dyn Fn(Heard) + Send>>()) };
        let block = RcBlock::new(move |event: std::ptr::NonNull<NSEvent>| -> *mut NSEvent {
            if !OPEN.load(Ordering::Relaxed) {
                return event.as_ptr();
            }
            // SAFETY: AppKit hands the monitor a live event.
            let e = unsafe { event.as_ref() };
            match e.r#type() {
                NSEventType::ScrollWheel => {
                    let dy = e.scrollingDeltaY() as f32;
                    // A trackpad's or a smooth wheel's points, a notched
                    // wheel's lines.
                    let factor = if e.hasPreciseScrollingDeltas() { (dy * 0.01).exp() } else { 1.15_f32.powf(dy) };
                    if dy != 0.0 {
                        heard(Heard::Zoom(factor));
                    }
                    return std::ptr::null_mut();
                }
                NSEventType::LeftMouseDown => {
                    let p = e.locationInWindow();
                    if let Ok(mut d) = DOWN.lock() {
                        *d = Some((p.x, p.y));
                    }
                }
                NSEventType::LeftMouseUp => {
                    let p = e.locationInWindow();
                    let down = DOWN.lock().ok().and_then(|mut d| d.take());
                    let still = down.is_some_and(|(x, y)| (x - p.x).abs() < 4.0 && (y - p.y).abs() < 4.0);
                    if still && let Some(window) = e.window(objc2::MainThreadMarker::new().expect("AppKit's monitors run on the main thread")) {
                        let height = window.contentView().map(|v| v.frame().size.height).unwrap_or(0.0);
                        heard(Heard::Click { x: p.x as f32, y: (height - p.y) as f32 });
                    }
                }
                _ => {}
            }
            event.as_ptr()
        });
        let mask = NSEventMask::ScrollWheel | NSEventMask::LeftMouseDown | NSEventMask::LeftMouseUp;
        // SAFETY: the block returns the event it got, or null for a wheel
        // turn it handled; the monitor is kept for the app's life.
        let monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(mask, &block) };
        std::mem::forget(monitor);
    }
    let heard: Box<Box<dyn Fn(Heard) + Send>> = Box::new(Box::new(heard));
    // SAFETY: the main queue, where AppKit's monitors are added; the
    // context is the box above, taken back once.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, Box::into_raw(heard).cast(), install) };
}
