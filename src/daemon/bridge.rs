//! Sophon's LAN bridge: phones on the same network reach this Mac's daemon
//! directly, without the relay round trip and without exposing the daemon.
//!
//! The daemon keeps listening on loopback only. The bridge listens on the
//! LAN, runs the same end to end handshake as the relay (Noise NK, with
//! the bridge's own static key from the pairing QR), then requires the
//! pairing token as the first sealed frame. Only then does it open a socket
//! to the local daemon, adding this machine's local credential to the
//! phone's hello, and forwards frames both ways.
//!
//! Two ways to a token: the pairing QR's (whoever sees this Mac's screen),
//! or one of a device's own, which a phone that found the bridge on the LAN
//! asks for and gets only once someone at this Mac allows it, both showing
//! the session's code. Every device connected is reported to the app, which
//! can cut it off or revoke it.

use std::{
    collections::HashMap,
    net::UdpSocket,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Notify, mpsc, oneshot},
};
use super::rt::{Message, from_ws, to_ws};

use super::{connection::discover, relay};

/// What a pairing QR carries about the bridge.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeInfo {
    /// `ip:port` on the LAN, then on Tailscale when it runs: comma
    /// separated, tried in order.
    pub addr: String,
    pub key: [u8; 32],
    pub token: String,
}

fn state_path() -> PathBuf {
    super::data_dir().join("bridge.json")
}

/// A device paired through the LAN, with a token of its own.
#[derive(Debug, Clone, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub token: String,
}

/// What the bridge tells the app.
pub enum BridgeEvent {
    /// A phone on the LAN asks to pair, showing `code`: the reply is `None`
    /// to refuse, else the relay link to hand it (`Some(None)`: LAN only).
    PairRequest { id: u64, device: String, code: String, reply: oneshot::Sender<Option<Option<String>>> },
    /// The asking phone left before an answer.
    PairEnded { id: u64 },
    Connected { conn: u64, device: String },
    Disconnected { conn: u64 },
}

/// A device connected now.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    pub conn: u64,
    pub device: String,
    /// Its paired device's id; `None` for the QR's token.
    pub device_id: Option<String>,
}

struct State {
    private: [u8; 32],
    public: [u8; 32],
    /// The pairing QR's token.
    token: String,
    port: u16,
    devices: Vec<Device>,
    conns: HashMap<u64, (Connection, Arc<Notify>)>,
    next: u64,
    /// A pairing request waits on someone at this Mac (one at a time).
    asking: bool,
    events: Option<mpsc::UnboundedSender<BridgeEvent>>,
}

static STATE: Mutex<Option<State>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.lock().ok()?.as_mut().map(f)
}

/// The bridge's keys, token, port and devices, kept so paired phones stay
/// paired.
fn load_or_create() -> State {
    let fresh = || {
        let (private, public) = relay::generate_key();
        let token = uuid::Uuid::new_v4().simple().to_string();
        State { private, public, token, port: 0, devices: vec![], conns: HashMap::new(), next: 1, asking: false, events: None }
    };
    let Ok(text) = std::fs::read_to_string(state_path()) else { return fresh() };
    let Ok(v) = serde_json::from_str::<Value>(&text) else { return fresh() };
    let k = |name: &str| -> Option<[u8; 32]> { B64.decode(v.get(name)?.as_str()?).ok()?.try_into().ok() };
    let (Some(private), Some(public), Some(token)) = (k("private"), k("public"), v.get("token").and_then(Value::as_str)) else {
        return fresh();
    };
    let devices = v
        .get("devices")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|d| {
            let s = |k: &str| d.get(k).and_then(Value::as_str).map(str::to_owned);
            Some(Device { id: s("id")?, name: s("name")?, token: s("token")? })
        })
        .collect();
    State {
        private,
        public,
        token: token.to_owned(),
        port: v.get("port").and_then(Value::as_u64).unwrap_or(0) as u16,
        devices,
        conns: HashMap::new(),
        next: 1,
        asking: false,
        events: None,
    }
}

fn save(s: &State) {
    let path = state_path();
    let _ = path.parent().map(std::fs::create_dir_all);
    let devices: Vec<Value> = s.devices.iter().map(|d| json!({ "id": d.id, "name": d.name, "token": d.token })).collect();
    let json = json!({
        "private": B64.encode(s.private),
        "public": B64.encode(s.public),
        "token": s.token,
        "port": s.port,
        "devices": devices,
    });
    if std::fs::write(&path, json.to_string()).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

/// The bridge's addresses now: the LAN's, then Tailscale's.
fn addrs(port: u16) -> Option<String> {
    let mut list = vec![format!("{}:{port}", lan_ip()?)];
    if let Some(ts) = tailscale_ip() {
        list.push(format!("{ts}:{port}"));
    }
    Some(list.join(","))
}

/// What the pairing QR carries now.
pub fn info() -> Option<BridgeInfo> {
    with(|s| (s.port, s.public, s.token.clone())).and_then(|(port, key, token)| Some(BridgeInfo { addr: addrs(port)?, key, token }))
}

/// The devices paired through the LAN.
pub fn devices() -> Vec<Device> {
    with(|s| s.devices.clone()).unwrap_or_default()
}

/// The devices connected now.
pub fn connections() -> Vec<Connection> {
    let mut list: Vec<Connection> = with(|s| s.conns.values().map(|(c, _)| c.clone()).collect()).unwrap_or_default();
    list.sort_by_key(|c| c.conn);
    list
}

/// Cuts a connection off; the device may connect again.
pub fn disconnect(conn: u64) {
    if let Some(Some(n)) = with(|s| s.conns.get(&conn).map(|(_, n)| n.clone())) {
        n.notify_one();
    }
}

/// Forgets a device: its token stops working, its connections end.
pub fn revoke(device_id: &str) {
    let cut: Vec<Arc<Notify>> = with(|s| {
        s.devices.retain(|d| d.id != device_id);
        save(s);
        s.conns.values().filter(|(c, _)| c.device_id.as_deref() == Some(device_id)).map(|(_, n)| n.clone()).collect()
    })
    .unwrap_or_default();
    cut.iter().for_each(|n| n.notify_one());
}

/// Forgets every device and changes the QR's token: everything paired
/// before must pair again. Returns the new QR's bridge.
pub fn revoke_all() -> Option<BridgeInfo> {
    let cut: Vec<Arc<Notify>> = with(|s| {
        s.devices.clear();
        s.token = uuid::Uuid::new_v4().simple().to_string();
        save(s);
        s.conns.values().map(|(_, n)| n.clone()).collect()
    })
    .unwrap_or_default();
    cut.iter().for_each(|n| n.notify_one());
    info()
}

fn emit(e: BridgeEvent) {
    if let Some(Some(tx)) = with(|s| s.events.clone()) {
        let _ = tx.send(e);
    }
}

/// This Mac's address on the LAN: a private IPv4 address of a physical
/// interface (Wi-Fi / Ethernet first). The default route is not used: with
/// a VPN or a proxy's TUN mode it points at a virtual interface
/// (`utun*`, 198.18.0.0/15) that phones on the Wi-Fi cannot reach.
pub fn lan_ip() -> Option<String> {
    let mut best: Option<(u8, String)> = None;
    for iface in if_addrs::get_if_addrs().ok()? {
        let std::net::IpAddr::V4(ip) = iface.ip() else { continue };
        let o = ip.octets();
        let private = o[0] == 10 || (o[0] == 172 && (16..=31).contains(&o[1])) || (o[0] == 192 && o[1] == 168);
        let virtual_if = ["utun", "tun", "tap", "bridge", "vmnet", "docker", "llw", "awdl", "gif", "stf"]
            .iter()
            .any(|p| iface.name.starts_with(p));
        if ip.is_loopback() || !private || virtual_if {
            continue;
        }
        let rank = if iface.name.starts_with("en") { 0 } else { 1 };
        if best.as_ref().is_none_or(|(r, _)| rank < *r) {
            best = Some((rank, ip.to_string()));
        }
    }
    best.map(|(_, ip)| ip).or_else(|| {
        // Fall back to the default route's interface.
        let s = UdpSocket::bind("0.0.0.0:0").ok()?;
        s.connect("8.8.8.8:80").ok()?;
        let ip = s.local_addr().ok()?.ip();
        (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
    })
}

/// This Mac's Tailscale address (100.64.0.0/10), when Tailscale runs: a
/// phone on the tailnet reaches the bridge from anywhere, peer to peer.
pub fn tailscale_ip() -> Option<String> {
    if_addrs::get_if_addrs().ok()?.into_iter().find_map(|iface| match iface.ip() {
        std::net::IpAddr::V4(ip) if ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]) => Some(ip.to_string()),
        _ => None,
    })
}

/// Starts the bridge on its own thread, advertised on the LAN; `None`
/// when it cannot listen. Its events come through the receiver.
pub fn start() -> Option<(BridgeInfo, mpsc::UnboundedReceiver<BridgeEvent>)> {
    let mut state = load_or_create();
    let std_listener = std::net::TcpListener::bind(("0.0.0.0", state.port))
        .or_else(|_| std::net::TcpListener::bind(("0.0.0.0", 0)))
        .ok()?;
    let port = std_listener.local_addr().ok()?.port();
    state.port = port;
    save(&state);
    std_listener.set_nonblocking(true).ok()?;
    let (tx, rx) = mpsc::unbounded_channel();
    state.events = Some(tx);
    let public = state.public;
    *STATE.lock().ok()? = Some(state);
    let info = info()?;
    #[cfg(target_os = "macos")]
    if let Err(e) = super::bonjour::advertise(port, &B64.encode(public)) {
        log::warn!("{e}: phones find this Mac by its QR only");
    }
    #[cfg(not(target_os = "macos"))]
    let _ = public;
    std::thread::Builder::new()
        .name("sophon-bridge".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt,
                Err(e) => return log::error!("bridge runtime: {e}"),
            };
            rt.block_on(async move {
                let Ok(listener) = TcpListener::from_std(std_listener) else { return };
                log::info!("LAN bridge listening on port {port}");
                loop {
                    let Ok((stream, peer)) = listener.accept().await else { continue };
                    tokio::spawn(async move {
                        if let Err(e) = serve(stream).await {
                            log::info!("bridge client {peer}: {e}");
                        }
                    });
                }
            });
        })
        .ok()?;
    Some((info, rx))
}

/// Who a token belongs to: the QR's (named as the phone says) or a
/// paired device's.
fn authorize(token: &str, said: &str) -> Option<(String, Option<String>)> {
    with(|s| {
        if token == s.token {
            return Some((said.to_owned(), None));
        }
        s.devices.iter().find(|d| d.token == token).map(|d| (d.name.clone(), Some(d.id.clone())))
    })
    .flatten()
}

async fn serve(stream: TcpStream) -> Result<(), String> {
    let private = with(|s| s.private).ok_or("no bridge")?;
    let phone = tokio_tungstenite::accept_async(stream).await.map_err(|e| e.to_string())?;
    let (mut ptx, mut prx) = phone.split();

    // 1. The handshake: the phone's hello, our ready (the phone resends its
    //    hello until it sees the ready; repeats are answered alike).
    let (ready, mut seal, mut unseal) = loop {
        let frame = tokio::time::timeout(Duration::from_secs(10), prx.next())
            .await
            .map_err(|_| "no hello")?
            .ok_or("closed")?
            .map_err(|e| e.to_string())?;
        if let Message::Text(t) = from_ws(frame)
            && let Ok(done) = relay::respond(&private, t.as_str())
        {
            break done;
        }
    };
    ptx.send(to_ws(Message::Text(ready.clone()))).await.map_err(|e| e.to_string())?;

    // 2. The pairing token, sealed, before anything reaches the daemon; or
    //    a request to pair.
    let first = loop {
        let frame = tokio::time::timeout(Duration::from_secs(10), prx.next())
            .await
            .map_err(|_| "no token")?
            .ok_or("closed")?
            .map_err(|e| e.to_string())?;
        match unseal.open(from_ws(frame))? {
            None => ptx.send(to_ws(Message::Text(ready.clone()))).await.map_err(|e| e.to_string())?,
            Some(Message::Text(t)) => break serde_json::from_str::<Value>(t.as_str()).unwrap_or(Value::Null),
            Some(_) => return Err("unexpected frame".into()),
        }
    };
    let said = first.get("device").and_then(Value::as_str).filter(|d| !d.trim().is_empty()).unwrap_or("Unknown device").to_owned();
    let (device, device_id) = match first.get("type").and_then(Value::as_str) {
        Some("sophon_bridge_auth" | "victor_bridge_auth") => {
            let token = first.get("token").and_then(Value::as_str).unwrap_or_default();
            authorize(token, &said).ok_or("wrong pairing token")?
        }
        Some("sophon_pair_request") => {
            let reply = pair(&said, &seal, &mut prx).await;
            let sealed = seal.seal(Message::Text(reply.to_string()))?;
            ptx.send(to_ws(sealed)).await.map_err(|e| e.to_string())?;
            return Ok(());
        }
        _ => return Err("no pairing token".into()),
    };

    // Known now: reported while it stays, and cut off when asked.
    let cut = Arc::new(Notify::new());
    let conn = with(|s| {
        let conn = s.next;
        s.next += 1;
        s.conns.insert(conn, (Connection { conn, device: device.clone(), device_id }, cut.clone()));
        conn
    })
    .ok_or("no bridge")?;
    emit(BridgeEvent::Connected { conn, device });
    let result = forward(&mut ptx, &mut prx, &mut seal, &mut unseal, &cut).await;
    with(|s| s.conns.remove(&conn));
    emit(BridgeEvent::Disconnected { conn });
    result
}

type PhoneTx = futures_util::stream::SplitSink<tokio_tungstenite::WebSocketStream<TcpStream>, tokio_tungstenite::tungstenite::Message>;
type PhoneRx = futures_util::stream::SplitStream<tokio_tungstenite::WebSocketStream<TcpStream>>;

/// Asks the app whether `device` may pair, and answers the phone: a link
/// with a token of its own, or a refusal.
async fn pair(device: &str, seal: &relay::Sender, prx: &mut PhoneRx) -> Value {
    let refused = json!({ "type": "sophon_pair_denied" });
    let Some(code) = seal.pairing_code() else { return refused };
    let (tx, rx) = oneshot::channel();
    let id = with(|s| {
        if s.asking {
            return None;
        }
        s.asking = true;
        let id = s.next;
        s.next += 1;
        Some(id)
    })
    .flatten();
    let Some(id) = id else { return json!({ "type": "sophon_pair_denied", "busy": true }) };
    emit(BridgeEvent::PairRequest { id, device: device.to_owned(), code, reply: tx });
    // The answer, unless the phone leaves or no one answers in time.
    let answer = tokio::select! {
        a = rx => a.ok().flatten(),
        _ = async { while let Some(Ok(_)) = prx.next().await {} } => None,
        _ = tokio::time::sleep(Duration::from_secs(180)) => None,
    };
    with(|s| s.asking = false);
    let Some(relay_link) = answer else {
        emit(BridgeEvent::PairEnded { id });
        return refused;
    };
    let made = with(|s| {
        let d = Device { id: uuid::Uuid::new_v4().simple().to_string(), name: device.to_owned(), token: uuid::Uuid::new_v4().simple().to_string() };
        s.devices.push(d.clone());
        save(s);
        (d, s.port, s.public)
    });
    let Some((d, port, public)) = made else { return refused };
    let Some(lan) = addrs(port) else { return refused };
    let link = relay::PairingLink::format(Some((&lan, &public, &d.token)), relay_link.as_deref());
    json!({ "type": "sophon_pair_ok", "link": link })
}

/// Frames both ways between the phone and the local daemon, until either
/// side closes or the app cuts the phone off.
async fn forward(ptx: &mut PhoneTx, prx: &mut PhoneRx, seal: &mut relay::Sender, unseal: &mut relay::Receiver, cut: &Notify) -> Result<(), String> {
    // 3. The local daemon, and frames both ways.
    let endpoint = discover();
    let (daemon, _) = tokio_tungstenite::connect_async(endpoint.url.as_str()).await.map_err(|e| e.to_string())?;
    let (mut dtx, mut drx) = daemon.split();
    let credential = endpoint.local_credential.clone();
    loop {
        tokio::select! {
            _ = cut.notified() => break,
            frame = prx.next() => {
                let Some(Ok(frame)) = frame else { break };
                let Some(mut plain) = unseal.open(from_ws(frame))? else { continue };
                // The phone's hello gets this machine's credential, which a
                // daemon with a password requires.
                if let (Message::Text(t), Some(cred)) = (&plain, &credential)
                    && let Ok(mut v) = serde_json::from_str::<Value>(t.as_str())
                    && v.get("type").and_then(Value::as_str) == Some("hello")
                {
                    v["auth"] = json!({ "kind": "localCredential", "token": cred });
                    plain = Message::Text(v.to_string());
                }
                if let Message::Close(_) = plain { break; }
                dtx.send(to_ws(plain)).await.map_err(|e| e.to_string())?;
            }
            frame = drx.next() => {
                let Some(Ok(frame)) = frame else { break };
                let frame = from_ws(frame);
                if let Message::Close(_) = frame { break; }
                let sealed = seal.seal(frame)?;
                ptx.send(to_ws(sealed)).await.map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Why a pairing request came to nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum PairFailure {
    /// Someone at the Mac said no, or no one answered in time.
    Refused,
    /// The Mac is answering another device's request.
    Busy,
    Failed(String),
}

/// Asks the bridge at `addr` (whose key is `key`) to pair this device,
/// blocking (on its own runtime): `on_code` gets the session's code as soon
/// as it is known, for the person to compare with the Mac's. The answer is
/// a pairing link with a token of this device's own. `stop` withdraws the
/// request (the Mac's prompt closes).
pub fn request_pair(addr: &str, key: [u8; 32], device: &str, on_code: impl FnOnce(String), stop: oneshot::Receiver<()>) -> Result<String, PairFailure> {
    let fail = |e: String| PairFailure::Failed(e);
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| fail(e.to_string()))?;
    rt.block_on(async move {
        let url = format!("ws://{addr}/ws");
        let (ws, _) = tokio::time::timeout(Duration::from_secs(5), tokio_tungstenite::connect_async(url.as_str()))
            .await
            .map_err(|_| fail("The computer did not answer".into()))?
            .map_err(|e| fail(e.to_string()))?;
        let (mut tx, mut rx) = ws.split();
        let offer = relay::RelayOffer { server_id: "bridge".into(), daemon_key: key, endpoint: addr.to_owned(), use_tls: false, password: None };
        let mut hs = relay::Handshake::start(&offer).map_err(fail)?;
        tx.send(to_ws(Message::Text(hs.hello().to_owned()))).await.map_err(|e| fail(e.to_string()))?;
        let (mut seal, mut unseal) = loop {
            let frame = tokio::time::timeout(Duration::from_secs(10), rx.next())
                .await
                .map_err(|_| fail("No answer to the handshake".into()))?
                .ok_or_else(|| fail("Closed".into()))?
                .map_err(|e| fail(e.to_string()))?;
            if let Message::Text(t) = from_ws(frame) {
                match hs.finish(t.as_str()) {
                    Ok(Some(pair)) => break pair,
                    Ok(None) => return Err(fail("Handshake failed".into())),
                    Err((back, e)) if e.is_empty() => hs = back,
                    Err((_, e)) => return Err(fail(e)),
                }
            }
        };
        on_code(seal.pairing_code().ok_or_else(|| fail("The computer's Sophon is too old to pair this way".into()))?);
        let ask = json!({ "type": "sophon_pair_request", "device": device }).to_string();
        tx.send(to_ws(seal.seal(Message::Text(ask)).map_err(fail)?)).await.map_err(|e| fail(e.to_string()))?;
        // As long as the Mac's own wait, and a little more.
        let wait = async {
            loop {
                let frame = tokio::time::timeout(Duration::from_secs(200), rx.next())
                    .await
                    .map_err(|_| PairFailure::Refused)?
                    .ok_or(PairFailure::Refused)?
                    .map_err(|e| fail(e.to_string()))?;
                if let Some(Message::Text(t)) = unseal.open(from_ws(frame)).map_err(fail)? {
                    return Ok(serde_json::from_str::<Value>(t.as_str()).unwrap_or(Value::Null));
                }
            }
        };
        let answer = tokio::select! {
            a = wait => a?,
            _ = stop => {
                let _ = tx.close().await;
                return Err(PairFailure::Refused);
            }
        };
        match answer.get("type").and_then(Value::as_str) {
            Some("sophon_pair_ok") => answer.get("link").and_then(Value::as_str).map(str::to_owned).ok_or(PairFailure::Refused),
            _ if answer.get("busy").and_then(Value::as_bool) == Some(true) => Err(PairFailure::Busy),
            _ => Err(PairFailure::Refused),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bridge of its own (its own data folder, not the user's): a phone
    /// asks to pair, both ends show one code, allowed it gets a link with a
    /// token of its own that the bridge accepts; refused it gets nothing;
    /// revoked the token stops working. Reaches no daemon.
    #[test]
    #[ignore]
    fn pairing_on_the_lan_live() {
        let dir = std::env::temp_dir().join(format!("sophon-bridge-test-{}", std::process::id()));
        // SAFETY: set before any thread of this test reads it.
        unsafe { std::env::set_var("SOPHON_DATA_DIR", &dir) };
        let (info, mut events) = start().expect("bridge");
        let port: u16 = info.addr.split(',').next().unwrap().rsplit(':').next().unwrap().parse().unwrap();
        let addr = format!("127.0.0.1:{port}");

        let offer = json!({ "v": 2, "serverId": "srv_test", "daemonPublicKeyB64": B64.encode([7u8; 32]), "relay": { "endpoint": "relay.paseo.sh:443", "useTls": true } });
        let relay_link = format!("https://app.paseo.sh/#offer={}", base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(offer.to_string()));
        // The Mac's side: answers each request as told, reporting its code.
        let (answers_tx, answers_rx) = std::sync::mpsc::channel::<bool>();
        let (codes_tx, codes_rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            rt.block_on(async move {
                while let Some(e) = events.recv().await {
                    if let BridgeEvent::PairRequest { code, reply, device, .. } = e {
                        assert_eq!(device, "Test Phone");
                        codes_tx.send(code).unwrap();
                        let allow = answers_rx.recv().unwrap();
                        let _ = reply.send(allow.then(|| Some(relay_link.clone())));
                    }
                }
            });
        });

        let ask = |allow: bool| {
            answers_tx.send(allow).unwrap();
            let mut phone_code = String::new();
            let (_stop_tx, stop) = oneshot::channel();
            let r = request_pair(&addr, info.key, "Test Phone", |c| phone_code = c, stop);
            (r, phone_code)
        };

        let (link, phone_code) = ask(true);
        let link = link.expect("allowed");
        assert_eq!(phone_code, codes_rx.recv().unwrap(), "both ends show one code");
        assert_eq!(phone_code.len(), 7);
        let parsed = relay::PairingLink::parse(&link).unwrap();
        let (_, key, token) = parsed.lan.unwrap();
        assert_eq!(key, info.key);
        assert_ne!(token, info.token, "a token of its own, not the QR's");
        assert_eq!(parsed.relay.map(|r| r.server_id).as_deref(), Some("srv_test"), "the relay comes along");
        let (name, id) = authorize(&token, "ignored").expect("the bridge takes its token");
        assert_eq!(name, "Test Phone");
        assert_eq!(devices().len(), 1);

        let (refused, _) = ask(false);
        assert_eq!(refused, Err(PairFailure::Refused));
        let _ = codes_rx.recv();
        assert_eq!(devices().len(), 1, "a refusal pairs nothing");

        revoke(&id.unwrap());
        assert!(authorize(&token, "x").is_none(), "revoked");
        let qr = info.token.clone();
        assert!(authorize(&qr, "QR Phone").is_some());
        let fresh = revoke_all().unwrap();
        assert!(authorize(&qr, "x").is_none(), "the QR's token changed");
        assert!(authorize(&fresh.token, "x").is_some());
        let _ = std::fs::remove_dir_all(dir);
    }
}
