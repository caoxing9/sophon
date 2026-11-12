//! A photo taken now, for the phone's composer: the system's camera screen
//! (`UIImagePickerController`), which NSH's image picker (the photo
//! library) does not open. The photo comes back as a JPEG no longer than
//! [`MAX_SIDE`] on a side, upright.
//!
//! The camera is asked for by the system the first time
//! (`NSCameraUsageDescription` in `nsh.json`, shared with the QR scanner).

use std::{cell::RefCell, ffi::c_void};

use objc2::{
    AnyThread, Encode, Encoding, MainThreadMarker, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, NSObject},
};
use objc2_foundation::NSString;

/// The photo's longer side at most, in pixels: what the picker gives too.
const MAX_SIDE: f64 = 2048.0;
/// `UIImagePickerControllerSourceTypeCamera`.
const SOURCE_CAMERA: isize = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

// SAFETY: the layouts and encodings of CoreGraphics' structs on 64-bit.
unsafe impl Encode for CGPoint {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for CGSize {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}
unsafe impl Encode for CGRect {
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
}

unsafe extern "C" {
    /// `dispatch_get_main_queue()` is this symbol's address.
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
}

/// Runs `work` on the main thread, later.
fn on_main(work: impl FnOnce() + Send + 'static) {
    extern "C" fn run(context: *mut c_void) {
        // SAFETY: the box `on_main` leaked for this one call.
        let work = unsafe { Box::from_raw(context.cast::<Box<dyn FnOnce() + Send>>()) };
        work();
    }
    let work: Box<Box<dyn FnOnce() + Send>> = Box::new(Box::new(work));
    // SAFETY: the main queue; the context is the box above, taken back once.
    unsafe { dispatch_async_f(&raw const _dispatch_main_q, Box::into_raw(work).cast(), run) };
}

#[link(name = "UIKit", kind = "framework")]
unsafe extern "C" {
    static UIImagePickerControllerOriginalImage: &'static NSString;
    fn UIGraphicsBeginImageContextWithOptions(size: CGSize, opaque: bool, scale: f64);
    fn UIGraphicsGetImageFromCurrentImageContext() -> *mut AnyObject;
    fn UIGraphicsEndImageContext();
    fn UIImageJPEGRepresentation(image: *const AnyObject, quality: f64) -> *mut AnyObject;
}

type Done = Box<dyn FnOnce(Option<Vec<u8>>) + Send>;

thread_local! {
    /// The camera screen that is up: its delegate (the picker does not
    /// retain it) and who waits for the photo.
    static OPEN: RefCell<Option<(Retained<Delegate>, Done)>> = const { RefCell::new(None) };
}

define_class!(
    /// The camera screen's delegate.
    #[unsafe(super(NSObject))]
    #[name = "SophonCameraDelegate"]
    struct Delegate;

    impl Delegate {
        #[unsafe(method(imagePickerController:didFinishPickingMediaWithInfo:))]
        fn picked(&self, picker: &AnyObject, info: &AnyObject) {
            // SAFETY: the info dictionary's documented key, a UIImage.
            let image: Option<Retained<AnyObject>> = unsafe { msg_send![info, objectForKey: UIImagePickerControllerOriginalImage] };
            finish(picker, image.and_then(|image| jpeg(&image)));
        }

        #[unsafe(method(imagePickerControllerDidCancel:))]
        fn cancelled(&self, picker: &AnyObject) {
            finish(picker, None);
        }
    }
);

/// Takes the camera screen down and hands the photo over.
fn finish(picker: &AnyObject, photo: Option<Vec<u8>>) {
    // SAFETY: a documented method; no completion block.
    let _: () = unsafe { msg_send![picker, dismissViewControllerAnimated: true, completion: std::ptr::null::<c_void>()] };
    if let Some((_, done)) = OPEN.with_borrow_mut(Option::take) {
        done(photo);
    }
}

/// `image` drawn upright into at most [`MAX_SIDE`] a side, as a JPEG: the
/// camera's own picture is some 12 megapixels, turned by a flag.
fn jpeg(image: &AnyObject) -> Option<Vec<u8>> {
    // SAFETY: documented UIKit drawing calls, on the main thread; the data
    // is copied before the pool lets it go.
    unsafe {
        let size: CGSize = msg_send![image, size];
        if size.width <= 0.0 || size.height <= 0.0 {
            return None;
        }
        let scale = (MAX_SIDE / size.width.max(size.height)).min(1.0);
        let fit = CGSize { width: (size.width * scale).round(), height: (size.height * scale).round() };
        UIGraphicsBeginImageContextWithOptions(fit, true, 1.0);
        let _: () = msg_send![image, drawInRect: CGRect { origin: CGPoint { x: 0.0, y: 0.0 }, size: fit }];
        let drawn = UIGraphicsGetImageFromCurrentImageContext();
        UIGraphicsEndImageContext();
        if drawn.is_null() {
            return None;
        }
        let data = UIImageJPEGRepresentation(drawn, 0.85);
        if data.is_null() {
            return None;
        }
        let (ptr, len): (*const u8, usize) = (msg_send![data, bytes], msg_send![data, length]);
        (!ptr.is_null() && len > 0).then(|| std::slice::from_raw_parts(ptr, len).to_vec())
    }
}

/// Whether this device has a camera to open (the simulator has none).
pub(crate) fn available() -> bool {
    let Some(class) = AnyClass::get(c"UIImagePickerController") else { return false };
    // SAFETY: a documented class method.
    unsafe { msg_send![class, isSourceTypeAvailable: SOURCE_CAMERA] }
}

/// The view controller on top, to present from.
fn top_controller() -> Option<Retained<AnyObject>> {
    // SAFETY: documented properties; the scenes of the app, their windows.
    unsafe {
        let app: Retained<AnyObject> = msg_send![AnyClass::get(c"UIApplication")?, sharedApplication];
        let scenes: Retained<AnyObject> = msg_send![&*app, connectedScenes];
        let scenes: Retained<AnyObject> = msg_send![&*scenes, allObjects];
        let count: usize = msg_send![&*scenes, count];
        let mut root: Option<Retained<AnyObject>> = None;
        for i in 0..count {
            let scene: Retained<AnyObject> = msg_send![&*scenes, objectAtIndex: i];
            let has_windows: bool = msg_send![&*scene, respondsToSelector: objc2::sel!(keyWindow)];
            if !has_windows {
                continue;
            }
            let window: Option<Retained<AnyObject>> = msg_send![&*scene, keyWindow];
            if let Some(window) = window {
                root = msg_send![&*window, rootViewController];
                if root.is_some() {
                    break;
                }
            }
        }
        let mut top = root?;
        loop {
            let presented: Option<Retained<AnyObject>> = msg_send![&*top, presentedViewController];
            match presented {
                Some(next) => top = next,
                None => return Some(top),
            }
        }
    }
}

/// Opens the camera; `done` hears the photo (a JPEG) or `None` when the
/// user cancelled or no camera opened. Called from any thread (the app's
/// code runs on the engine's, UIKit only on the main one); `done` is
/// called on the main thread.
pub(crate) fn take_photo(done: impl FnOnce(Option<Vec<u8>>) + Send + 'static) {
    on_main(move || present(Box::new(done)));
}

/// [`take_photo`], on the main thread.
fn present(done: Done) {
    let Some(_mtm) = MainThreadMarker::new() else { return done(None) };
    if !available() || OPEN.with_borrow(Option::is_some) {
        return done(None);
    }
    let Some(top) = top_controller() else { return done(None) };
    // SAFETY: documented UIKit calls on the main thread; the delegate is
    // kept in `OPEN` while the screen is up.
    unsafe {
        let Some(class) = AnyClass::get(c"UIImagePickerController") else { return done(None) };
        let picker: Retained<AnyObject> = msg_send![class, new];
        let delegate: Retained<Delegate> = msg_send![Delegate::alloc(), init];
        let _: () = msg_send![&*picker, setSourceType: SOURCE_CAMERA];
        let _: () = msg_send![&*picker, setDelegate: &*delegate];
        OPEN.with_borrow_mut(|open| *open = Some((delegate, done)));
        let _: () = msg_send![&*top, presentViewController: &*picker, animated: true, completion: std::ptr::null::<c_void>()];
    }
}
