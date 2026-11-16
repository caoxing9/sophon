//! Whether the device has a network, and when the one it uses changes
//! (Wi-Fi to cellular, back online): Network.framework's path monitor. The
//! connection takes a change as its cue to check its socket or try again at
//! once (`Command::NetworkChanged`), and the connection strip says when the
//! device itself is offline.

use std::{ffi::c_void, sync::OnceLock};

use block2::RcBlock;
use tokio::sync::mpsc;

/// The device's network now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct NetState {
    /// A path to the internet (or the local network) is there.
    pub(crate) online: bool,
    pub(crate) wifi: bool,
    pub(crate) cellular: bool,
}

// `nw_path_status_t`, `nw_interface_type_t`.
const SATISFIED: i32 = 1;
const WIFI: i32 = 1;
const CELLULAR: i32 = 2;
const WIRED: i32 = 3;
// `QOS_CLASS_UTILITY`.
const UTILITY: isize = 0x11;

#[link(name = "Network", kind = "framework")]
unsafe extern "C" {
    fn nw_path_monitor_create() -> *mut c_void;
    fn nw_path_monitor_set_update_handler(monitor: *mut c_void, handler: &block2::Block<dyn Fn(*mut c_void)>);
    fn nw_path_monitor_set_queue(monitor: *mut c_void, queue: *mut c_void);
    fn nw_path_monitor_start(monitor: *mut c_void);
    fn nw_path_get_status(path: *mut c_void) -> i32;
    fn nw_path_uses_interface_type(path: *mut c_void, kind: i32) -> bool;
}

unsafe extern "C" {
    fn dispatch_get_global_queue(identifier: isize, flags: usize) -> *mut c_void;
}

static RECEIVER: OnceLock<std::sync::Mutex<Option<mpsc::UnboundedReceiver<NetState>>>> = OnceLock::new();

/// Starts the monitor (once): the device's network as it is, then each
/// change. `None` when taken already.
pub(crate) fn watch() -> Option<mpsc::UnboundedReceiver<NetState>> {
    RECEIVER
        .get_or_init(|| {
            let (tx, rx) = mpsc::unbounded_channel();
            let handler = RcBlock::new(move |path: *mut c_void| {
                if path.is_null() {
                    return;
                }
                // SAFETY: `path` is the monitor's path, valid during the call.
                let state = unsafe {
                    let wired = nw_path_uses_interface_type(path, WIRED);
                    NetState {
                        online: nw_path_get_status(path) == SATISFIED,
                        wifi: nw_path_uses_interface_type(path, WIFI) || wired,
                        cellular: nw_path_uses_interface_type(path, CELLULAR),
                    }
                };
                let _ = tx.send(state);
            });
            // SAFETY: the documented calls; the monitor lives as long as the
            // app (never cancelled or released), and copies the handler.
            unsafe {
                let monitor = nw_path_monitor_create();
                if !monitor.is_null() {
                    nw_path_monitor_set_update_handler(monitor, &handler);
                    nw_path_monitor_set_queue(monitor, dispatch_get_global_queue(UTILITY, 0));
                    nw_path_monitor_start(monitor);
                }
            }
            std::sync::Mutex::new(Some(rx))
        })
        .lock()
        .ok()?
        .take()
}

#[cfg(test)]
mod tests {
    /// The monitor says how the network is at once (this Mac's, live).
    #[test]
    fn the_monitor_says_the_network_at_once() {
        let mut rx = super::watch().expect("the monitor");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let first = rt.block_on(async { tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv()).await });
        let state = first.expect("an answer within 3 s").expect("a state");
        eprintln!("{state:?}");
        // Taken: a second watcher gets none.
        assert!(super::watch().is_none());
    }
}
