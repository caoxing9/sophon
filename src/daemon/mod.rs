//! Everything that talks to the Paseo daemon. Free of UI types, so it builds
//! and tests without a window.

/// Where Sophon keeps its settings, hosts, keys and ids: `~/.sophon`. A
/// `~/.victor` from before the rename moves there once. `SOPHON_DATA_DIR`
/// points it elsewhere.
pub fn data_dir() -> std::path::PathBuf {
    // Tests and screenshots keep their own state, not the user's.
    if let Some(dir) = std::env::var_os("SOPHON_DATA_DIR") {
        return dir.into();
    }
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

pub mod bridge;
pub mod connection;
pub mod model;
pub mod relay;

pub use connection::{Command, Event};
pub use model::{Agent, Item, Permission, Project, Provider, Workspace};
