//! The QR scanner the phone pairs with (`sophon/qr-scanner`): the back
//! camera's picture filling the component's box, and an event `scan`
//! (`{ value }`) for each code it reads. AVFoundation does the work; the
//! session stops with the component.
//!
//! The first session asks for the camera (`NSCameraUsageDescription` in
//! `nsh.json`); when access is denied the component emits `denied` instead,
//! and the screen asks the user to allow it in Settings.

use std::{
    cell::RefCell,
    ffi::c_void,
    rc::Rc,
};

use nsh_core::JsonValue;
use nsh_platform_ios::{ComponentContext, Components, NativeComponent};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, NSObject},
    sel,
};
use objc2_foundation::{NSArray, NSString};
use objc2_ui_kit::UIView;

pub(crate) const NAME: &str = super::QR_SCANNER;

#[link(name = "AVFoundation", kind = "framework")]
unsafe extern "C" {
    static AVMediaTypeVideo: &'static NSString;
    static AVMetadataObjectTypeQRCode: &'static NSString;
    static AVLayerVideoGravityResizeAspectFill: &'static NSString;
}

unsafe extern "C" {
    /// `dispatch_get_main_queue()` is this symbol's address.
    static _dispatch_main_q: c_void;
}

pub(crate) fn register(components: &Components) {
    components.register(NAME, Rc::new(|cx| Box::new(Scanner::new(cx)) as Box<dyn NativeComponent>));
}

fn class(name: &str) -> Option<&'static AnyClass> {
    AnyClass::get(&std::ffi::CString::new(name).ok()?)
}

// A view whose layer is the camera's preview layer, so it follows the
// view's bounds by itself.
define_class!(
    #[unsafe(super(UIView))]
    #[thread_kind = MainThreadOnly]
    #[name = "SophonScannerView"]
    struct PreviewView;

    impl PreviewView {
        #[unsafe(method(layerClass))]
        fn layer_class() -> &'static AnyClass {
            class("AVCaptureVideoPreviewLayer").unwrap_or_else(|| objc2::class!(CALayer))
        }
    }
);

struct DelegateIvars {
    cx: ComponentContext,
    last: RefCell<Option<String>>,
}

// Receives the codes the metadata output reads, on the main queue.
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "SophonScannerDelegate"]
    #[ivars = DelegateIvars]
    struct Delegate;

    impl Delegate {
        #[unsafe(method(captureOutput:didOutputMetadataObjects:fromConnection:))]
        fn did_output(&self, _output: &AnyObject, objects: &NSArray<AnyObject>, _connection: &AnyObject) {
            for object in objects.iter() {
                // SAFETY: an AVMetadataMachineReadableCodeObject's
                // documented property; nil for codes it cannot decode.
                let value: Option<Retained<NSString>> = unsafe { msg_send![&*object, stringValue] };
                let Some(value) = value.map(|v| v.to_string()) else { continue };
                let mut last = self.ivars().last.borrow_mut();
                if last.as_deref() == Some(value.as_str()) {
                    continue;
                }
                *last = Some(value.clone());
                self.ivars().cx.emit("scan", serde_json::json!({ "value": value }));
            }
        }
    }
);

struct Scanner {
    view: Retained<PreviewView>,
    session: Option<Retained<AnyObject>>,
    _delegate: Option<Retained<Delegate>>,
}

impl Scanner {
    fn new(cx: ComponentContext) -> Self {
        let mtm = MainThreadMarker::new().expect("components are made on the main thread");
        // SAFETY: a plain UIView subclass, made on the main thread.
        let view: Retained<PreviewView> = unsafe { msg_send![PreviewView::alloc(mtm), init] };
        // SAFETY: UIView's documented properties.
        unsafe {
            let black: Retained<AnyObject> = msg_send![objc2::class!(UIColor), blackColor];
            let _: () = msg_send![&*view, setBackgroundColor: &*black];
            let _: () = msg_send![&*view, setClipsToBounds: true];
        }
        let (session, delegate) = match Self::start(&view, &cx, mtm) {
            Some((s, d)) => (Some(s), Some(d)),
            None => (None, None),
        };
        Scanner { view, session, _delegate: delegate }
    }

    /// Sets up the session on the back camera and starts it; `None` (and
    /// `denied` or `unavailable` emitted) when the camera cannot be used.
    fn start(view: &PreviewView, cx: &ComponentContext, mtm: MainThreadMarker) -> Option<(Retained<AnyObject>, Retained<Delegate>)> {
        let device_class = class("AVCaptureDevice")?;
        // SAFETY: AVFoundation's documented API, on the main thread; the
        // classes were looked up and the constants are AVFoundation's.
        unsafe {
            // 2 denied, 1 restricted; 0 (not determined) asks when the
            // input is made.
            let status: isize = msg_send![device_class, authorizationStatusForMediaType: AVMediaTypeVideo];
            if status == 1 || status == 2 {
                cx.emit("denied", JsonValue::Null);
                return None;
            }
            let device: Option<Retained<AnyObject>> = msg_send![device_class, defaultDeviceWithMediaType: AVMediaTypeVideo];
            let Some(device) = device else {
                cx.emit("unavailable", JsonValue::Null);
                return None;
            };
            let input: Option<Retained<AnyObject>> = msg_send![
                class("AVCaptureDeviceInput")?,
                deviceInputWithDevice: &*device,
                error: std::ptr::null_mut::<*mut AnyObject>()
            ];
            let Some(input) = input else {
                cx.emit("denied", JsonValue::Null);
                return None;
            };
            let session: Retained<AnyObject> = msg_send![class("AVCaptureSession")?, new];
            let can: bool = msg_send![&*session, canAddInput: &*input];
            if !can {
                cx.emit("unavailable", JsonValue::Null);
                return None;
            }
            let _: () = msg_send![&*session, addInput: &*input];
            let output: Retained<AnyObject> = msg_send![class("AVCaptureMetadataOutput")?, new];
            let can: bool = msg_send![&*session, canAddOutput: &*output];
            if !can {
                cx.emit("unavailable", JsonValue::Null);
                return None;
            }
            let _: () = msg_send![&*session, addOutput: &*output];
            let delegate = Delegate::alloc(mtm).set_ivars(DelegateIvars { cx: cx.clone(), last: RefCell::new(None) });
            let delegate: Retained<Delegate> = msg_send![super(delegate), init];
            let _: () = msg_send![
                &*output,
                setMetadataObjectsDelegate: &*delegate,
                queue: &_dispatch_main_q as *const c_void
            ];
            // Types are set once the output is in the session.
            let types = NSArray::from_slice(&[AVMetadataObjectTypeQRCode]);
            let _: () = msg_send![&*output, setMetadataObjectTypes: &*types];

            let layer: Retained<AnyObject> = msg_send![view, layer];
            let _: () = msg_send![&*layer, setSession: &*session];
            let _: () = msg_send![&*layer, setVideoGravity: AVLayerVideoGravityResizeAspectFill];
            // startRunning blocks until the camera is up: off the main
            // thread.
            let _: () = msg_send![&*session, performSelectorInBackground: sel!(startRunning), withObject: std::ptr::null::<AnyObject>()];
            Some((session, delegate))
        }
    }
}

impl NativeComponent for Scanner {
    fn view(&self) -> &UIView {
        &self.view
    }

    fn set_prop(&mut self, _name: &str, _value: Option<&JsonValue>) {}

    fn command(&mut self, name: &str, _args: &JsonValue) {
        // `again`: read the same code again (after a failed pairing).
        if name == "again"
            && let Some(d) = &self._delegate
        {
            *d.ivars().last.borrow_mut() = None;
        }
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        if let Some(session) = &self.session {
            // SAFETY: AVCaptureSession's documented API.
            unsafe {
                let _: () = msg_send![&**session, performSelectorInBackground: sel!(stopRunning), withObject: std::ptr::null::<AnyObject>()];
            }
        }
    }
}
