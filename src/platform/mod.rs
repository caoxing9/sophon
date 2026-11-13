//! Native pieces of the app on each platform, beside what NSH provides.

pub(crate) mod fonts;
#[cfg(target_os = "macos")]
pub(crate) mod fn_key;

#[cfg(target_os = "android")]
pub(crate) mod android;
#[cfg(target_os = "ios")]
pub(crate) mod ios_camera;
#[cfg(target_os = "ios")]
pub(crate) mod ios_push;
#[cfg(target_os = "ios")]
pub(crate) mod ios_scanner;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub(crate) mod pdf;

/// The PDF view's name (`nsh_core::element::native`).
#[cfg_attr(not(any(target_os = "macos", target_os = "ios")), allow(dead_code))]
pub(crate) const PDF_VIEW: &str = "sophon/pdf";

/// The QR scanner component's name (`nsh_core::element::native`).
#[cfg_attr(not(target_os = "ios"), allow(dead_code))]
pub(crate) const QR_SCANNER: &str = "sophon/qr-scanner";

