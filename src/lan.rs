//! Sophon on the LAN, both sides of it.
//!
//! This Mac: a phone that found it asks to pair, and someone here allows it
//! (comparing the code both show) or not; every device connected through the
//! bridge is announced and listed, and can be cut off or revoked.
//!
//! A phone (or another Mac): the computers nearby, found by Bonjour, and
//! pairing with one without its QR code.

use std::{cell::RefCell, collections::HashMap};

use nsh_core::prelude::*;

use crate::i18n::{t, tf};
use crate::store::{self, Modal};

// ------------------------------------------------------------ this device

/// What this device calls itself to the computer it pairs with.
pub(crate) fn device_name() -> String {
    #[cfg(target_os = "macos")]
    {
        static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        NAME.get_or_init(|| {
            std::process::Command::new("/usr/sbin/scutil")
                .args(["--get", "ComputerName"])
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| "Mac".into())
        })
        .clone()
    }
    #[cfg(target_os = "ios")]
    {
        if crate::ui::kit::MOBILE { "iPhone".into() } else { "iPad".into() }
    }
    #[cfg(target_os = "android")]
    {
        crate::platform::android::model().unwrap_or_else(|| "Android".into())
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
    {
        "Sophon".into()
    }
}

// ------------------------------------------------- this Mac: who connects

/// A device connected through the bridge now.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Connected {
    pub(crate) conn: u64,
    pub(crate) device: String,
    pub(crate) device_id: Option<String>,
    /// The space it reaches, by name; `None`: the first.
    pub(crate) space: Option<String>,
    /// Through the relay, else over the LAN.
    pub(crate) relayed: bool,
}

/// A device paired through the LAN.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Paired {
    pub(crate) id: String,
    pub(crate) name: String,
    /// The space it joined, by name; `None`: the first.
    pub(crate) space: Option<String>,
}

#[derive(Clone, Default)]
struct Devices {
    connected: Vec<Connected>,
    paired: Vec<Paired>,
}

pub(crate) fn connected() -> Vec<Connected> {
    global::<Devices>().with(|d| d.connected.clone())
}

pub(crate) fn paired() -> Vec<Paired> {
    global::<Devices>().with(|d| d.paired.clone())
}

/// A device's name as the bridge has it, "Unknown device" in the UI's
/// language (a phone too old to say its name).
fn shown(name: String) -> String {
    if name == "Unknown device" { t("Unknown device").to_owned() } else { name }
}

/// Reads the bridge's devices and connections again.
fn refresh() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use crate::daemon::bridge;
        // A space by its folder, named as saved here.
        let spaces = store::spaces();
        let name_of = |home: Option<String>| {
            let home = home?;
            spaces.iter().find(|h| crate::spaces::home_of(&h.url).is_some_and(|p| p.to_string_lossy() == home)).map(|h| h.label())
        };
        let connected = bridge::connections()
            .into_iter()
            .map(|c| Connected { conn: c.conn, device: shown(c.device), device_id: c.device_id, space: name_of(c.space), relayed: c.relayed })
            .collect();
        let paired = bridge::devices().into_iter().map(|d| Paired { id: d.id, name: d.name, space: name_of(d.space) }).collect();
        global::<Devices>().set(Devices { connected, paired });
    }
}

/// Lists the bridge's devices (the settings page opens).
pub(crate) fn load_devices() {
    refresh();
}

pub(crate) fn disconnect(conn: u64) {
    #[cfg(not(target_arch = "wasm32"))]
    crate::daemon::bridge::disconnect(conn);
    let _ = conn;
}

pub(crate) fn revoke(id: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    crate::daemon::bridge::revoke(id);
    let _ = id;
    refresh();
}

/// Every device must pair again, the QR's included.
pub(crate) fn revoke_all() {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = crate::daemon::bridge::revoke_all();
    refresh();
    store::notify(t("Every device must pair again").into());
}

#[cfg(not(target_arch = "wasm32"))]
type Reply = tokio::sync::oneshot::Sender<Option<crate::daemon::bridge::PairAnswer>>;
#[cfg(target_arch = "wasm32")]
type Reply = tokio::sync::oneshot::Sender<Option<()>>;

thread_local! {
    /// The pairing request waiting on someone here, and its answer's way back.
    static ASKING: RefCell<Option<(u64, Reply)>> = const { RefCell::new(None) };
    /// When each device last left, so a phone back from sleep is not
    /// announced again.
    static LEFT: RefCell<HashMap<String, f64>> = RefCell::new(HashMap::new());
}

/// Seconds now, for `LEFT`.
fn now() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// A device that left less than this long ago is not announced again.
const QUIET_SECS: f64 = 600.0;

/// Takes the bridge's events: requests to pair, devices coming and going.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn watch_bridge(mut events: tokio::sync::mpsc::UnboundedReceiver<crate::daemon::bridge::BridgeEvent>) {
    use crate::daemon::bridge::BridgeEvent;
    refresh();
    store::spawn_detached(async move {
        while let Some(e) = events.recv().await {
            match e {
                BridgeEvent::PairRequest { id, device, code, reply } => {
                    let device = shown(device);
                    ASKING.with_borrow_mut(|a| *a = Some((id, reply)));
                    store::open_modal(Modal::ApprovePairing { id, device: device.clone(), code });
                    // This Mac may be in another app's hands, or asleep in a
                    // corner: the system says it too.
                    let n = nsh_core::notifications::Notification::new(
                        format!("pair:{id}"),
                        t("Pairing request").to_owned(),
                        tf("“{}” asks to connect to this Mac", &[&device]),
                    );
                    let _ = show_notification(n);
                }
                BridgeEvent::PairEnded { id } => {
                    let mine = ASKING.with_borrow_mut(|a| a.take_if(|(x, _)| *x == id).is_some());
                    if matches!(store::modal(), Some(Modal::ApprovePairing { id: x, .. }) if x == id) {
                        store::close_modal();
                    }
                    if mine {
                        store::notify(t("The pairing request ended").into());
                    }
                }
                BridgeEvent::Connected { device, .. } => {
                    let device = shown(device);
                    let already = connected().iter().any(|c| c.device == device);
                    let recent = LEFT.with_borrow(|l| l.get(&device).is_some_and(|at| now() - at < QUIET_SECS));
                    refresh();
                    if !already && !recent {
                        let n = nsh_core::notifications::Notification::new(
                            format!("device:{device}"),
                            t("Device connected").to_owned(),
                            tf("“{}” connected to this Mac", &[&device]),
                        );
                        let _ = show_notification(n);
                        store::notify(tf("“{}” connected to this Mac", &[&device]));
                    }
                }
                BridgeEvent::Disconnected { conn } => {
                    if let Some(c) = connected().into_iter().find(|c| c.conn == conn) {
                        LEFT.with_borrow_mut(|l| l.insert(c.device, now()));
                    }
                    refresh();
                }
            }
        }
    });
}

/// Answers the pairing request: allowed, the phone joins `space` (a saved
/// space's host id; `None`: this Mac's first space) and gets the bridge's
/// place at the relay too (so it connects from anywhere, still through the
/// bridge).
pub(crate) fn answer_pairing(allow: bool, space: Option<String>) {
    let asking = ASKING.with_borrow_mut(Option::take);
    store::close_modal();
    let Some((_, reply)) = asking else { return };
    if !allow {
        let _ = reply.send(None);
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let space = space.and_then(|id| store::spaces().into_iter().find(|h| h.id == id));
        let home = space.as_ref().and_then(|h| crate::spaces::home_of(&h.url));
        let name = space.map(|h| format!("{} · {}", device_name(), h.label()));
        // The bridge's own place at the relay: away from home the device
        // still comes through the bridge.
        let relay = crate::daemon::bridge::relay_offer();
        let space = home.map(|h| h.to_string_lossy().into_owned());
        let _ = reply.send(Some(crate::daemon::bridge::PairAnswer { relay, space, name }));
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (reply, space);
    }
}

// --------------------------------------------- a phone: computers nearby

/// A computer found on the LAN.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Nearby {
    pub(crate) name: String,
    /// Its bridge's public key, base64.
    pub(crate) key: String,
    pub(crate) addr: String,
    /// The saved host it is, when paired already.
    pub(crate) host: Option<String>,
}

#[derive(Clone, Default)]
struct NearbyState {
    list: Vec<Nearby>,
    scanning: bool,
    scanned: bool,
}

/// The computers found nearby, this one left out, those paired marked.
pub(crate) fn nearby() -> Vec<Nearby> {
    let hosts = store::hosts();
    global::<NearbyState>().with(|n| n.list.clone()).into_iter().map(|mut n| {
        n.host = hosts.iter().find(|h| lan_key(&h.url).as_deref() == Some(n.key.as_str())).map(|h| h.id.clone());
        n
    }).collect()
}

pub(crate) fn scanning() -> bool {
    global::<NearbyState>().with(|n| n.scanning)
}

/// Whether a search has finished, so "none found" means something.
pub(crate) fn scanned() -> bool {
    global::<NearbyState>().with(|n| n.scanned)
}

/// A pairing link's bridge key, base64.
pub(crate) fn lan_key(url: &str) -> Option<String> {
    use base64::Engine;
    let (_, key, _) = crate::daemon::relay::PairingLink::parse(url)?.lan?;
    Some(base64::engine::general_purpose::STANDARD.encode(key))
}

/// Looks for computers on the LAN (a couple of seconds, on a thread).
pub(crate) fn scan() {
    if global::<NearbyState>().with_untracked(|n| n.scanning) {
        return;
    }
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        global::<NearbyState>().update(|n| n.scanning = true);
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        std::thread::spawn(move || {
            let _ = tx.send(crate::daemon::bonjour::browse(std::time::Duration::from_secs(2)));
        });
        store::spawn_detached(async move {
            let found = rx.recv().await.unwrap_or_default();
            #[cfg(target_os = "macos")]
            let own = crate::daemon::bridge::info().map(|b| {
                use base64::Engine;
                base64::engine::general_purpose::STANDARD.encode(b.key)
            });
            #[cfg(not(target_os = "macos"))]
            let own: Option<String> = None;
            let list = found
                .into_iter()
                .filter(|f| own.as_deref() != Some(f.key.as_str()))
                .map(|f| Nearby { name: f.name, key: f.key, addr: f.addr, host: None })
                .collect();
            global::<NearbyState>().set(NearbyState { list, scanning: false, scanned: true });
        });
    }
}

/// Pairing with a computer nearby, as it goes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Pairing {
    pub(crate) name: String,
    /// The code to compare with the computer's, once the session is up.
    pub(crate) code: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Default)]
struct PairingState(Pairing);

thread_local! {
    /// Withdraws the request under way.
    static WITHDRAW: RefCell<Option<tokio::sync::oneshot::Sender<()>>> = const { RefCell::new(None) };
}

/// Withdraws a request still waiting (its dialog closed).
pub(crate) fn cancel_pairing() {
    if let Some(stop) = WITHDRAW.with_borrow_mut(Option::take) {
        let _ = stop.send(());
    }
}

pub(crate) fn pairing() -> Pairing {
    global::<PairingState>().with(|p| p.0.clone())
}

/// Connects to a nearby computer: one paired already at once, another by
/// asking it to pair (someone there allows it).
pub(crate) fn connect(n: Nearby) {
    if let Some(id) = n.host {
        store::switch_host(Some(id));
        return;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use base64::Engine;
        let Some(key) = base64::engine::general_purpose::STANDARD.decode(&n.key).ok().and_then(|k| <[u8; 32]>::try_from(k).ok()) else {
            return;
        };
        global::<PairingState>().set(PairingState(Pairing { name: n.name.clone(), code: None, error: None }));
        store::open_modal(Modal::PairNearby);
        enum Step {
            Code(String),
            Done(Result<String, crate::daemon::bridge::PairFailure>),
        }
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let (stop_tx, stop) = tokio::sync::oneshot::channel();
        WITHDRAW.with_borrow_mut(|w| *w = Some(stop_tx));
        let addr = n.addr.clone();
        std::thread::spawn(move || {
            let code_tx = tx.clone();
            let on_code = move |code| {
                let _ = code_tx.send(Step::Code(code));
            };
            let result = crate::daemon::bridge::request_pair(&addr, key, &device_name(), on_code, stop);
            let _ = tx.send(Step::Done(result));
        });
        let name = n.name;
        store::spawn_detached(async move {
            use crate::daemon::bridge::PairFailure;
            while let Some(step) = rx.recv().await {
                match step {
                    Step::Code(code) => global::<PairingState>().update(|p| p.0.code = Some(code)),
                    Step::Done(Ok(link)) => {
                        WITHDRAW.with_borrow_mut(|w| *w = None);
                        store::close_modal();
                        store::add_host(&name, &link, "");
                        store::notify(t("Paired").into());
                    }
                    Step::Done(Err(e)) => {
                        WITHDRAW.with_borrow_mut(|w| *w = None);
                        let error = match e {
                            PairFailure::Refused => t("The computer did not allow it").to_owned(),
                            PairFailure::Busy => t("The computer is answering another device; try again shortly").to_owned(),
                            PairFailure::Failed(e) => tf("Cannot pair: {}", &[&e]),
                        };
                        global::<PairingState>().update(|p| p.0.error = Some(error));
                    }
                }
            }
        });
    }
}
