//! Android: the app's directories and the phone's name, which no
//! environment variable gives.

use std::path::PathBuf;

/// The app's package (`dev.sophon.app`): the process is named after it.
fn package() -> Option<String> {
    let line = std::fs::read("/proc/self/cmdline").ok()?;
    let name = line.split(|b| *b == 0).next()?;
    // A process of its own within the package is `<package>:<name>`.
    let name = std::str::from_utf8(name).ok()?.split(':').next()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// Points `HOME` at the app's files directory and `TMPDIR` at its cache:
/// an app's process has neither, and `/` and `/data/local/tmp` (what they
/// fall back to) are not the app's to write. Once, as the app starts.
pub(crate) fn init() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let Some(package) = package() else { return };
        // The app's own directory, whichever user it runs as.
        let root = PathBuf::from("/data/data").join(package);
        let (files, cache) = (root.join("files"), root.join("cache"));
        let _ = std::fs::create_dir_all(&files);
        let _ = std::fs::create_dir_all(&cache);
        // SAFETY: called as the app's Rust starts (`native`, and before the
        // first read of the data directory), ahead of the threads Sophon
        // starts; nothing here reads the environment while it changes.
        unsafe {
            std::env::set_var("HOME", &files);
            std::env::set_var("TMPDIR", &cache);
        }
    });
}

/// What the phone is called: its model (`Pixel 8`).
pub(crate) fn model() -> Option<String> {
    unsafe extern "C" {
        fn __system_property_get(name: *const std::ffi::c_char, value: *mut std::ffi::c_char) -> std::ffi::c_int;
    }
    // PROP_VALUE_MAX.
    let mut value = [0u8; 92];
    // SAFETY: the name ends in NUL and the buffer holds PROP_VALUE_MAX
    // bytes, the most a property's value takes with its NUL.
    let len = unsafe { __system_property_get(c"ro.product.model".as_ptr(), value.as_mut_ptr().cast()) };
    let model = std::str::from_utf8(value.get(..usize::try_from(len).ok()?)?).ok()?.trim();
    (!model.is_empty()).then(|| model.to_owned())
}

/// Where a download goes: the shared Downloads folder, which the Files app
/// shows (an app writes its own files there without a permission from
/// Android 11 on).
pub(crate) fn downloads_dir() -> PathBuf {
    PathBuf::from("/storage/emulated/0/Download")
}
