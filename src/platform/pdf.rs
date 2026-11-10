//! A PDF shown by the system (`sophon/pdf`): PDFKit's `PDFView` on the Mac
//! and on iOS, scrolling the pages continuously, scaled to the width, with
//! its own zoom and text selection. Prop `path`: a file on this device (the
//! preview's copy of the workspace's file).

use std::rc::Rc;

#[cfg(target_os = "macos")]
use nsh_platform_macos::{
    ComponentContext, Components, NativeComponent,
    objc2::{msg_send, rc::{Allocated, Retained}, runtime::{AnyClass, AnyObject, Bool}},
    objc2_app_kit::NSView as PlatformView,
    objc2_foundation::NSString,
};
#[cfg(target_os = "ios")]
use nsh_platform_ios::{ComponentContext, Components, NativeComponent};
#[cfg(target_os = "ios")]
use objc2::{msg_send, rc::{Allocated, Retained}, runtime::{AnyClass, AnyObject, Bool}};
#[cfg(target_os = "ios")]
use objc2_foundation::NSString;
#[cfg(target_os = "ios")]
use objc2_ui_kit::UIView as PlatformView;

use nsh_core::JsonValue;

#[link(name = "PDFKit", kind = "framework")]
unsafe extern "C" {}

pub(crate) const NAME: &str = super::PDF_VIEW;

/// `kPDFDisplaySinglePageContinuous`.
const CONTINUOUS: isize = 1;

fn class(name: &str) -> Option<&'static AnyClass> {
    AnyClass::get(&std::ffi::CString::new(name).ok()?)
}

struct Pdf {
    view: Retained<AnyObject>,
    path: Option<String>,
}

impl Pdf {
    fn new(_cx: ComponentContext) -> Self {
        let cls = class("PDFView").expect("PDFKit");
        // SAFETY: PDFView's own initializer and setters, on the main thread
        // components are made on.
        let view: Retained<AnyObject> = unsafe {
            let made: Allocated<AnyObject> = msg_send![cls, alloc];
            let view: Retained<AnyObject> = msg_send![made, init];
            let _: () = msg_send![&view, setAutoScales: Bool::YES];
            let _: () = msg_send![&view, setDisplayMode: CONTINUOUS];
            view
        };
        Pdf { view, path: None }
    }

    fn open(&mut self, path: &str) {
        if self.path.as_deref() == Some(path) {
            return;
        }
        self.path = Some(path.to_owned());
        let (Some(url_cls), Some(doc_cls)) = (class("NSURL"), class("PDFDocument")) else { return };
        // SAFETY: Foundation's and PDFKit's documented initializers.
        unsafe {
            let url: Retained<AnyObject> = msg_send![url_cls, fileURLWithPath: &*NSString::from_str(path)];
            let made: Allocated<AnyObject> = msg_send![doc_cls, alloc];
            let doc: Option<Retained<AnyObject>> = msg_send![made, initWithURL: &*url];
            let _: () = msg_send![&self.view, setDocument: doc.as_deref()];
        }
    }
}

impl NativeComponent for Pdf {
    #[cfg(target_os = "macos")]
    fn view(&self) -> Retained<PlatformView> {
        // SAFETY: PDFView is an NSView.
        unsafe { Retained::cast_unchecked(self.view.clone()) }
    }

    #[cfg(target_os = "ios")]
    fn view(&self) -> &PlatformView {
        // SAFETY: PDFView is a UIView, kept alive by `self.view`.
        unsafe { &*(Retained::as_ptr(&self.view) as *const PlatformView) }
    }

    fn set_prop(&mut self, name: &str, value: Option<&JsonValue>) {
        if name == "path"
            && let Some(path) = value.and_then(JsonValue::as_str)
        {
            self.open(path);
        }
    }
}

pub(crate) fn register(components: &Components) {
    components.register(NAME, Rc::new(|cx: ComponentContext| Box::new(Pdf::new(cx)) as Box<dyn NativeComponent>));
}
