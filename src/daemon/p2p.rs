//! Peer to peer between a device and its paired computer (iroh: QUIC,
//! hole punching, relayed by iroh's relays when it cannot be direct). Tried
//! before Sophon's relay, whose bandwidth every device shares.
//!
//! What goes over it is what the LAN bridge serves: one bidirectional
//! stream per session, a WebSocket over it, then the bridge's own Noise
//! handshake and pairing token (`bridge::serve`). iroh's TLS authenticates
//! the computer by its endpoint id, which the pairing link carries (`p2p=`);
//! the bridge still decides who may in, by token.

use std::cell::RefCell;

use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMap, RelayMode, RelayUrl, SecretKey, endpoint::presets};
use tokio::io::{Join, join};

/// The protocol a bridge's endpoint accepts.
pub const ALPN: &[u8] = b"sophon/bridge/1";

/// A session's stream: what iroh receives, and sends.
pub type Stream = Join<iroh::endpoint::RecvStream, iroh::endpoint::SendStream>;

/// The computer's side: its endpoint, under the key the bridge keeps (the
/// same id across restarts, the one pairing links name). Its sessions come
/// to `serve` as streams. `relay`: the relay of the server Sophon is set to
/// use (`sophon-relay`'s, self-hosted): its home, where its phones find it
/// and punch through, no one else's servers; `None`, n0's public ones.
pub async fn host<F, Fut>(secret: [u8; 32], relay: Option<&str>, serve: F) -> Result<(), String>
where
    F: Fn(Stream) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let endpoint = builder(relay)?.secret_key(SecretKey::from_bytes(&secret)).alpns(vec![ALPN.to_vec()]).bind().await.map_err(|e| e.to_string())?;
    log::info!("peer to peer as {}", endpoint.id().fmt_short());
    serve_on(&endpoint, serve).await;
    Ok(())
}

/// Each connection to `endpoint`, each stream the device opens on it a
/// session of its own; until the endpoint closes.
async fn serve_on<F, Fut>(endpoint: &Endpoint, serve: F)
where
    F: Fn(Stream) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    while let Some(incoming) = endpoint.accept().await {
        let serve = serve.clone();
        tokio::spawn(async move {
            let Ok(connection) = incoming.await else { return };
            while let Ok((send, recv)) = connection.accept_bi().await {
                tokio::spawn(serve(join(recv, send)));
            }
        });
    }
}

/// An endpoint's setup: on `relay` alone when one is given (the relay
/// server's own, in China: nothing overseas is asked, not n0's relays nor
/// its address lookup, so finding each other and punching through take the
/// least time), else n0's public relays and lookup.
fn builder(relay: Option<&str>) -> Result<iroh::endpoint::Builder, String> {
    Ok(match relay {
        Some(url) => {
            let url: RelayUrl = url.parse().map_err(|_| format!("Not a relay address: {url}"))?;
            Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Custom(RelayMap::from(url)))
        }
        None => Endpoint::builder(presets::N0),
    })
}

/// The id of the endpoint `secret` makes: what pairing links carry.
pub fn id_of(secret: &[u8; 32]) -> String {
    SecretKey::from_bytes(secret).public().to_string()
}

/// A new endpoint key, for a bridge that has none yet.
pub fn new_secret() -> [u8; 32] {
    SecretKey::generate().to_bytes()
}

thread_local! {
    /// The device's endpoint, made on the connection's thread (its runtime)
    /// and kept while its relay stays the same (the host's: a host with
    /// another one gets an endpoint on that one): binding one joins a
    /// relay, which takes a while.
    static CLIENT: RefCell<Option<(Option<String>, Endpoint)>> = const { RefCell::new(None) };
    static WARMING: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
}

/// The device's endpoint made now, in the background, for a host it may be
/// reached by (through `relay`): joining the relay takes a moment, better
/// spent before the LAN's look fails than after it. On the connection's
/// runtime.
pub fn warm(relay: Option<&str>) {
    let relay = relay.map(str::to_owned);
    if WARMING.with_borrow(|w| w.as_ref() == Some(&relay)) {
        return;
    }
    WARMING.with_borrow_mut(|w| *w = Some(relay.clone()));
    tokio::spawn(async move {
        if let Err(e) = client(relay.as_deref()).await {
            log::info!("peer to peer: {e}");
        }
    });
}

async fn client(relay: Option<&str>) -> Result<Endpoint, String> {
    let relay = relay.map(str::to_owned);
    if let Some(endpoint) = CLIENT.with_borrow(|c| c.as_ref().filter(|(r, _)| *r == relay).map(|(_, e)| e.clone())) {
        return Ok(endpoint);
    }
    let endpoint = builder(relay.as_deref())?.bind().await.map_err(|e| e.to_string())?;
    if let Some((_, old)) = CLIENT.with_borrow_mut(|c| c.replace((relay, endpoint.clone()))) {
        old.close().await;
    }
    Ok(endpoint)
}

/// The device's network changed (another Wi-Fi, cellular): its endpoint
/// looks for its paths again, rather than waiting to notice.
pub async fn network_changed() {
    if let Some(endpoint) = CLIENT.with_borrow(|c| c.as_ref().map(|(_, e)| e.clone())) {
        endpoint.network_change().await;
    }
}

/// A session's stream to the computer whose endpoint is `id`, direct when it
/// can be: found through `relay` (its home, the self-hosted one its link
/// names), else by its id (n0's address lookup).
pub async fn connect(id: &str, relay: Option<&str>) -> Result<Stream, String> {
    let id: EndpointId = id.parse().map_err(|_| "Not a peer to peer id".to_owned())?;
    let mut to = EndpointAddr::new(id);
    if let Some(url) = relay.and_then(|r| r.parse::<RelayUrl>().ok()) {
        to = to.with_relay_url(url);
    }
    open(&client(relay).await?, to).await
}

/// A peer to peer address as a URL (`iroh://<id>`, `?relay=` its relay):
/// what a session's endpoint names.
pub fn url(id: &str, relay: Option<&str>) -> String {
    match relay {
        Some(r) => format!("iroh://{id}?relay={}", r.replace('%', "%25").replace('&', "%26").replace('?', "%3F")),
        None => format!("iroh://{id}"),
    }
}

/// [`url`]'s id and relay back.
pub fn parse_url(url: &str) -> Option<(String, Option<String>)> {
    let rest = url.strip_prefix("iroh://")?;
    let (id, query) = rest.split_once('?').unwrap_or((rest, ""));
    let relay = query
        .split('&')
        .find_map(|kv| kv.strip_prefix("relay="))
        .map(|r| r.replace("%3F", "?").replace("%26", "&").replace("%25", "%"))
        .filter(|r| !r.is_empty());
    (!id.is_empty()).then(|| (id.to_owned(), relay))
}

async fn open(endpoint: &Endpoint, to: impl Into<iroh::EndpointAddr>) -> Result<Stream, String> {
    let connection = endpoint.connect(to, ALPN).await.map_err(|e| e.to_string())?;
    let (send, recv) = connection.open_bi().await.map_err(|e| e.to_string())?;
    Ok(join(recv, send))
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures_util::{SinkExt, StreamExt};
    use iroh::{EndpointAddr, RelayMode};
    use tokio_tungstenite::tungstenite::Message;

    /// An endpoint on loopback only: no relay, no address lookup.
    async fn local(secret: Option<[u8; 32]>) -> Endpoint {
        let mut b = Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled).bind_addr("127.0.0.1:0").unwrap();
        if let Some(secret) = secret {
            b = b.secret_key(SecretKey::from_bytes(&secret)).alpns(vec![ALPN.to_vec()]);
        }
        b.bind().await.unwrap()
    }

    /// A device's WebSocket over a stream to the computer's endpoint, two
    /// sessions on one connection each their own; the computer known by
    /// the id its kept key makes.
    #[tokio::test]
    async fn sessions_go_peer_to_peer() {
        let secret = new_secret();
        let computer = local(Some(secret)).await;
        assert_eq!(computer.id().to_string(), id_of(&secret));
        let at = computer.bound_sockets().into_iter().find(|a| a.is_ipv4()).unwrap();
        let serving = computer.clone();
        tokio::spawn(async move {
            serve_on(&serving, |stream| async move {
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(Message::Text(t))) = ws.next().await {
                    ws.send(Message::Text(format!("echo {t}").into())).await.unwrap();
                }
            })
            .await
        });

        let device = local(None).await;
        let to = EndpointAddr::new(id_of(&secret).parse().unwrap()).with_ip_addr(at);
        for n in ["one", "two"] {
            let stream = open(&device, to.clone()).await.unwrap();
            let (mut ws, _) = tokio_tungstenite::client_async("ws://sophon.p2p/ws", stream).await.unwrap();
            ws.send(Message::Text(n.into())).await.unwrap();
            let Some(Ok(Message::Text(back))) = ws.next().await else { panic!("no answer") };
            assert_eq!(back.as_str(), format!("echo {n}"));
        }

        // Another endpoint at that address is not the computer paired.
        let other = EndpointAddr::new(id_of(&new_secret()).parse().unwrap()).with_ip_addr(at);
        assert!(tokio::time::timeout(std::time::Duration::from_secs(5), open(&device, other)).await.map_or(true, |r| r.is_err()));
    }

    /// Through a relay of one's own alone (as the relay server's, in
    /// China): neither side asks n0 anything, the device is given no
    /// address of the computer, only its id and relay, and finds it there.
    #[tokio::test]
    async fn sessions_go_through_a_relay_of_ones_own() {
        let (map, relay, _server) = iroh::test_utils::run_relay_server().await.unwrap();
        let own = |secret: Option<[u8; 32]>| {
            let map = map.clone();
            async move {
                let mut b = Endpoint::builder(presets::Minimal)
                    .relay_mode(RelayMode::Custom(map))
                    .ca_tls_config(iroh_relay::tls::CaTlsConfig::insecure_skip_verify());
                if let Some(secret) = secret {
                    b = b.secret_key(SecretKey::from_bytes(&secret)).alpns(vec![ALPN.to_vec()]);
                }
                b.bind().await.unwrap()
            }
        };
        let secret = new_secret();
        let computer = own(Some(secret)).await;
        computer.online().await;
        let serving = computer.clone();
        tokio::spawn(async move {
            serve_on(&serving, |stream| async move {
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                while let Some(Ok(Message::Text(t))) = ws.next().await {
                    ws.send(Message::Text(format!("echo {t}").into())).await.unwrap();
                }
            })
            .await
        });
        let device = own(None).await;
        let to = EndpointAddr::new(id_of(&secret).parse().unwrap()).with_relay_url(relay);
        let stream = tokio::time::timeout(std::time::Duration::from_secs(10), open(&device, to)).await.expect("in time").unwrap();
        let (mut ws, _) = tokio_tungstenite::client_async("ws://sophon.p2p/ws", stream).await.unwrap();
        ws.send(Message::Text("hi".into())).await.unwrap();
        let Some(Ok(Message::Text(back))) = ws.next().await else { panic!("no answer") };
        assert_eq!(back.as_str(), "echo hi");
    }

    #[test]
    fn a_peer_to_peer_url_carries_its_relay() {
        let id = id_of(&new_secret());
        assert_eq!(parse_url(&url(&id, None)), Some((id.clone(), None)));
        let relay = "https://47.119.154.134:3443/";
        assert_eq!(parse_url(&url(&id, Some(relay))), Some((id.clone(), Some(relay.to_owned()))));
        assert_eq!(parse_url("ws://x"), None);
    }

    #[test]
    fn an_id_is_the_same_for_a_key_kept() {
        let secret = new_secret();
        assert_eq!(id_of(&secret), id_of(&secret));
        assert!(id_of(&secret).parse::<EndpointId>().is_ok());
        assert_ne!(id_of(&secret), id_of(&new_secret()));
    }
}
