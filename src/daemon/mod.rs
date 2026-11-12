//! Everything that talks to the daemon. Free of UI types, so it builds
//! and tests without a window.

/// Where Sophon keeps its settings, hosts, keys and ids: `~/.sophon`. A
/// `~/.victor` from before the rename moves there once. `SOPHON_DATA_DIR`
/// points it elsewhere.
pub fn data_dir() -> std::path::PathBuf {
    // Tests and screenshots keep their own state, not the user's.
    if let Some(dir) = std::env::var_os("SOPHON_DATA_DIR") {
        return dir.into();
    }
    // Android names no home: the app's files directory is made it.
    #[cfg(target_os = "android")]
    crate::platform::android::init();
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_else(|| ".".into());
    // On iOS HOME is the app's container, whose root a device's sandbox
    // does not let the app write (the simulator does): Application Support
    // is the place, kept across updates and reinstalls.
    if cfg!(target_os = "ios") {
        let dir = home.join("Library").join("Application Support").join("Sophon");
        let _ = std::fs::create_dir_all(&dir);
        return dir;
    }
    let dir = home.join(".sophon");
    static MIGRATED: std::sync::Once = std::sync::Once::new();
    MIGRATED.call_once(|| {
        let old = home.join(".victor");
        if !dir.exists() && old.is_dir() {
            let _ = std::fs::rename(&old, &dir);
        }
    });
    dir
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod bonjour;
#[cfg(not(target_arch = "wasm32"))]
pub mod bridge;
pub mod agents;
pub mod cache;
pub mod connection;
pub mod model;
pub mod relay;
pub mod rt;

pub use connection::{Command, Event};
pub use model::{Agent, Item, Permission, Project, Provider, Workspace};

/// Reads one of Sophon's files (`settings.json`, `hosts.json`, …): from
/// [`data_dir`], or in a browser from the page's storage.
pub fn read_file(name: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_storage()?.get_item(&format!("sophon/{name}")).ok().flatten()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(data_dir().join(name)).ok()
    }
}

/// Writes what [`read_file`] reads; natively readable by this user only
/// (the hosts file holds passwords).
pub fn write_file(name: &str, text: &str) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        web_storage().ok_or("no storage")?.set_item(&format!("sophon/{name}"), text).map_err(|e| format!("{e:?}"))
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = data_dir().join(name);
        let _ = path.parent().map(std::fs::create_dir_all);
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn web_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}
