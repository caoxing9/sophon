//! Sophon: a native client for the Paseo daemon, built with NSH.
//!
//! `daemon` owns the WebSocket connection on tokio's threads; `store` holds
//! the UI state in signals; `ui` draws it.

use nsh_base::theme;
use nsh_core::{element::{AssetFile, AssetKind}, prelude::*};

pub mod daemon;
mod i18n;
mod platform;
mod store;
mod ui;
mod update;

/// The app's UI set (`nsh.json` `"rust"`).
#[native_module("sophon/native")]
fn native(registry: &mut Registry) {
    registry.register::<App>();
    // Only the logo ships: `assets!()` would ship the whole directory,
    // the app icon's sources included.
    registry.assets(&[
        AssetFile { uri: LOGO.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/logo@2x.png"), kind: AssetKind::Image },
        AssetFile { uri: LOADER_SKY.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loader/sky@2x.png"), kind: AssetKind::Image },
        AssetFile { uri: LOADER_CORONA.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loader/corona@2x.png"), kind: AssetKind::Image },
        AssetFile { uri: LOADER_MOON.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loader/moon@2x.png"), kind: AssetKind::Image },
    ]);
}

/// The app's native components on iOS: the pairing QR scanner.
#[cfg(target_os = "ios")]
#[native_components(ios)]
fn components(components: &nsh_platform_ios::Components) {
    platform::ios_scanner::register(components);
}

/// The app icon, small: the sidebar's mark.
pub(crate) const LOGO: ImageAsset = asset!("logo@2x.png");

/// The icon in layers, for the loading mark: its sky, its corona (which
/// turns) and its moon (`assets/app-icon/gen_icon.py … sky|corona|moon`).
pub(crate) const LOADER_SKY: ImageAsset = asset!("loader/sky@2x.png");
pub(crate) const LOADER_CORONA: ImageAsset = asset!("loader/corona@2x.png");
pub(crate) const LOADER_MOON: ImageAsset = asset!("loader/moon@2x.png");

/// The application's root component.
#[component]
pub fn App() -> impl IntoElement {
    store::load_prefs();
    start_connection();
    store::load_hosts();
    store::watch_notification_presses();
    store::watch_pairing_links();
    if is_desktop() {
        store::start_bridge();
        update::start();
    }
    let _ = request_notification_permission();
    view()
        .flex_1()
        .bg(move || theme().background)
        .text_color(move || theme().foreground)
        // A language change mounts the UI again, in the new language.
        .child(dynamic(|| {
            let _ = i18n::lang();
            Some(untracked(|| ui::shell().into_element()))
        }))
}

/// Starts the daemon connection for this surface and pumps its events into
/// the store, a batch per wake so a burst of deltas is one render.
fn start_connection() {
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();
    store::install_sender(command_tx);
    // A thread with its own small runtime: it works wherever NSH runs the
    // app (including the headless test runner, which has no tokio reactor),
    // and ends once this surface drops its channel ends. tokio's channels
    // are executor-agnostic, so the engine-thread pump below reads them.
    let spawned = std::thread::Builder::new().name("sophon-daemon".into()).spawn(move || {
        match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => rt.block_on(daemon::connection::run(event_tx, command_rx)),
            Err(e) => log::error!("cannot start the daemon connection: {e}"),
        }
    });
    if let Err(e) = spawned {
        log::error!("cannot start the daemon thread: {e}");
    }
    spawn(async move {
        while let Some(first) = event_rx.recv().await {
            batch(|| {
                store::apply(first);
                while let Ok(next) = event_rx.try_recv() {
                    store::apply(next);
                }
            });
        }
    });
}
