//! Sophon's LAN bridge: phones on the same network reach this Mac's daemon
//! directly, without the relay round trip and without exposing the daemon.
//!
//! The daemon keeps listening on loopback only. The bridge listens on the
//! LAN, runs the same end to end handshake as the relay (Noise NK, with
//! the bridge's own static key from the pairing QR), then requires the
//! pairing token as the first sealed frame. Only then does it open a socket
//! to the local daemon, adding this machine's local credential to the
//! phone's hello, and forwards frames both ways.

use std::{net::UdpSocket, path::PathBuf, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::{TcpListener, TcpStream};
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

/// The bridge's key, token and port, kept so a paired phone stays paired.
fn load_or_create() -> ([u8; 32], [u8; 32], String, u16) {
    let path = state_path();
    if let Ok(text) = std::fs::read_to_string(&path)
        && let Ok(v) = serde_json::from_str::<Value>(&text)
    {
        let k = |name: &str| -> Option<[u8; 32]> { B64.decode(v.get(name)?.as_str()?).ok()?.try_into().ok() };
        if let (Some(private), Some(public), Some(token)) = (k("private"), k("public"), v.get("token").and_then(Value::as_str)) {
            let port = v.get("port").and_then(Value::as_u64).unwrap_or(0) as u16;
            return (private, public, token.to_owned(), port);
        }
    }
    let (private, public) = relay::generate_key();
    let token = uuid::Uuid::new_v4().simple().to_string();
    (private, public, token, 0)
}

fn save(private: &[u8; 32], public: &[u8; 32], token: &str, port: u16) {
    let path = state_path();
    let _ = path.parent().map(std::fs::create_dir_all);
    let json = json!({ "private": B64.encode(private), "public": B64.encode(public), "token": token, "port": port });
    if std::fs::write(&path, json.to_string()).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
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

/// Starts the bridge on its own thread; `None` when it cannot listen.
pub fn start() -> Option<BridgeInfo> {
    let (private, public, token, port) = load_or_create();
    let std_listener = std::net::TcpListener::bind(("0.0.0.0", port))
        .or_else(|_| std::net::TcpListener::bind(("0.0.0.0", 0)))
        .ok()?;
    let port = std_listener.local_addr().ok()?.port();
    save(&private, &public, &token, port);
    std_listener.set_nonblocking(true).ok()?;
    let ip = lan_ip()?;
    let mut addrs = vec![format!("{ip}:{port}")];
    if let Some(ts) = tailscale_ip() {
        addrs.push(format!("{ts}:{port}"));
    }
    let info = BridgeInfo { addr: addrs.join(","), key: public, token: token.clone() };
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
                    let token = token.clone();
                    tokio::spawn(async move {
                        if let Err(e) = serve(stream, private, &token).await {
                            log::info!("bridge client {peer}: {e}");
                        }
                    });
                }
            });
        })
        .ok()?;
    Some(info)
}

async fn serve(stream: TcpStream, private: [u8; 32], token: &str) -> Result<(), String> {
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

    // 2. The pairing token, sealed, before anything reaches the daemon.
    let authed = loop {
        let frame = tokio::time::timeout(Duration::from_secs(10), prx.next())
            .await
            .map_err(|_| "no token")?
            .ok_or("closed")?
            .map_err(|e| e.to_string())?;
        match unseal.open(from_ws(frame))? {
            None => ptx.send(to_ws(Message::Text(ready.clone()))).await.map_err(|e| e.to_string())?,
            Some(Message::Text(t)) => {
                let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                break matches!(v.get("type").and_then(Value::as_str), Some("sophon_bridge_auth" | "victor_bridge_auth"))
                    && v.get("token").and_then(Value::as_str) == Some(token);
            }
            Some(_) => break false,
        }
    };
    if !authed {
        return Err("wrong pairing token".into());
    }

    // 3. The local daemon, and frames both ways.
    let endpoint = discover();
    let (daemon, _) = tokio_tungstenite::connect_async(endpoint.url.as_str()).await.map_err(|e| e.to_string())?;
    let (mut dtx, mut drx) = daemon.split();
    let credential = endpoint.local_credential.clone();
    loop {
        tokio::select! {
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
