//! Sophon: a native client for a coding-agent daemon, built with NSH.
//!
//! `daemon` owns the WebSocket connection on tokio's threads; `store` holds
//! the UI state in signals; `ui` draws it.

use nsh_base::theme;
use nsh_core::{element::{AssetFile, AssetKind}, prelude::*};

#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
mod agent_pictures;
mod agent_update;
#[cfg(target_os = "macos")]
mod awake;
mod bundled_cli;
mod cadence;
pub mod daemon;
mod fold;
mod i18n;
mod lan;
mod platform;
mod prompt_link;
mod sent_pictures;
#[cfg(target_os = "macos")]
mod service_install;
mod spaces;
mod store;
mod models;
mod ui;
mod update;
mod usage;

/// Randomness on the web: NSH builds with getrandom's `custom` backend,
/// filled here from the page's `crypto.getRandomValues` (getrandom 0.2's
/// `js`), which the keys and ids draw from.
#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> Result<(), getrandom03::Error> {
    // SAFETY: getrandom passes a writable buffer of `len` bytes.
    let buf = unsafe { std::slice::from_raw_parts_mut(dest, len) };
    getrandom02::getrandom(buf).map_err(|_| getrandom03::Error::UNSUPPORTED)
}

/// The app's UI set (`nsh.json` `"rust"`).
#[native_module("sophon/native")]
fn native(registry: &mut Registry) {
    #[cfg(target_os = "android")]
    platform::android::init();
    registry.register::<App>();
    // Only the mark's layers ship: `assets!()` would ship the whole
    // directory, the app icon's sources included.
    registry.assets(&[
        AssetFile { uri: LOADER_SKY.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loader/sky@2x.png"), kind: AssetKind::Image },
        AssetFile { uri: LOADER_CORONA.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loader/corona@2x.png"), kind: AssetKind::Image },
        AssetFile { uri: LOADER_MOON.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loader/moon@2x.png"), kind: AssetKind::Image },
        AssetFile { uri: LOADER_SKY_LIGHT.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loader/sky-light@2x.png"), kind: AssetKind::Image },
        AssetFile { uri: LOADER_CORONA_LIGHT.url, source: concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loader/corona-light@2x.png"), kind: AssetKind::Image },
    ]);
}

/// The app's native components on iOS: the pairing QR scanner.
#[cfg(target_os = "ios")]
#[native_components(ios)]
fn components(components: &nsh_platform_ios::Components) {
    platform::ios_scanner::register(components);
    platform::pdf::register(components);
}

/// The app's native components on macOS: the PDF view.
#[cfg(target_os = "macos")]
#[native_components(macos)]
fn components(components: &nsh_platform_macos::Components) {
    platform::pdf::register(components);
}

/// The icon in layers, for the loading mark: its sky, its corona (which
/// turns) and its moon (`assets/app-icon/gen_icon.py … sky|corona|moon`).
pub(crate) const LOADER_SKY: ImageAsset = asset!("loader/sky@2x.png");
pub(crate) const LOADER_CORONA: ImageAsset = asset!("loader/corona@2x.png");
pub(crate) const LOADER_MOON: ImageAsset = asset!("loader/moon@2x.png");

/// The same for light pages: the daytime eclipse, a dawn sky and a
/// golden corona (`gen_icon.py … light`); the moon is shared.
pub(crate) const LOADER_SKY_LIGHT: ImageAsset = asset!("loader/sky-light@2x.png");
pub(crate) const LOADER_CORONA_LIGHT: ImageAsset = asset!("loader/corona-light@2x.png");

/// The application's root component.
#[component]
pub fn App() -> impl IntoElement {
    store::load_prefs();
    start_connection();
    store::load_hosts();
    store::load_local_models();
    // The phone's notifications are its own (remote ones, with actions):
    // their presses come from there, not through NSH.
    #[cfg(target_os = "ios")]
    store::start_push();
    #[cfg(not(target_os = "ios"))]
    store::watch_notification_presses();
    store::watch_pairing_links();
    // The web's desktop idiom is still a page: no bridge, no updates.
    if is_desktop() && !cfg!(target_arch = "wasm32") {
        // A Mac that only connects to others runs none of this side.
        store::start_bridge();
        store::start_spaces();
        store::watch_keep_awake();
        #[cfg(not(any(target_arch = "wasm32", target_os = "ios")))]
        store::watch_limits_for_push();
        update::start();
        update::preview();
    }
    // (The phone asks in `start_push`, and for Apple's token after.)
    #[cfg(not(target_os = "ios"))]
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
    #[cfg(not(target_arch = "wasm32"))]
    {
        let spawned = std::thread::Builder::new().name("sophon-daemon".into()).spawn(move || {
            match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt.block_on(daemon::connection::run(event_tx, command_rx)),
                Err(e) => log::error!("cannot start the daemon connection: {e}"),
            }
        });
        if let Err(e) = spawned {
            log::error!("cannot start the daemon thread: {e}");
        }
    }
    // A page has one thread: the connection is a task of the engine's,
    // on the page's WebSocket.
    #[cfg(target_arch = "wasm32")]
    spawn(daemon::connection::run(event_tx, command_rx));
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
