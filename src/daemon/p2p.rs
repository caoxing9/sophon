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

use iroh::{Endpoint, EndpointId, SecretKey, endpoint::presets};
use tokio::io::{Join, join};

/// The protocol a bridge's endpoint accepts.
pub const ALPN: &[u8] = b"sophon/bridge/1";

/// A session's stream: what iroh receives, and sends.
pub type Stream = Join<iroh::endpoint::RecvStream, iroh::endpoint::SendStream>;

/// The computer's side: its endpoint, under the key the bridge keeps (the
/// same id across restarts, the one pairing links name). Its sessions come
/// to `serve` as streams.
pub async fn host<F, Fut>(secret: [u8; 32], serve: F) -> Result<(), String>
where
    F: Fn(Stream) -> Fut + Clone + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let endpoint = Endpoint::builder(presets::N0)
        .secret_key(SecretKey::from_bytes(&secret))
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(|e| e.to_string())?;
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

/// The id of the endpoint `secret` makes: what pairing links carry.
pub fn id_of(secret: &[u8; 32]) -> String {
    SecretKey::from_bytes(secret).public().to_string()
}

/// A new endpoint key, for a bridge that has none yet.
pub fn new_secret() -> [u8; 32] {
    SecretKey::generate().to_bytes()
}

thread_local! {
    /// The device's endpoint, made once on the connection's thread (its
    /// runtime) and kept: binding one joins a relay, which takes a while.
    static CLIENT: RefCell<Option<Endpoint>> = const { RefCell::new(None) };
}

/// The device's endpoint made now, in the background, for a host it may
/// be reached by: joining iroh's relay takes a moment, better spent before
/// the LAN's look fails than after it. On the connection's runtime.
pub fn warm() {
    thread_local!(static WARMING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) });
    if !WARMING.replace(true) {
        tokio::spawn(async {
            if let Err(e) = client().await {
                log::info!("peer to peer: {e}");
            }
        });
    }
}

async fn client() -> Result<Endpoint, String> {
    if let Some(endpoint) = CLIENT.with_borrow(Clone::clone) {
        return Ok(endpoint);
    }
    let endpoint = Endpoint::bind(presets::N0).await.map_err(|e| e.to_string())?;
    CLIENT.with_borrow_mut(|c| *c = Some(endpoint.clone()));
    Ok(endpoint)
}

/// The device's network changed (another Wi-Fi, cellular): its endpoint
/// looks for its paths again, rather than waiting to notice.
pub async fn network_changed() {
    if let Some(endpoint) = CLIENT.with_borrow(Clone::clone) {
        endpoint.network_change().await;
    }
}

/// A session's stream to the computer whose endpoint is `id`: found by its
/// id (iroh's address lookup), direct when it can be.
pub async fn connect(id: &str) -> Result<Stream, String> {
    let id: EndpointId = id.parse().map_err(|_| "Not a peer to peer id".to_owned())?;
    open(&client().await?, id).await
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

    #[test]
    fn an_id_is_the_same_for_a_key_kept() {
        let secret = new_secret();
        assert_eq!(id_of(&secret), id_of(&secret));
        assert!(id_of(&secret).parse::<EndpointId>().is_ok());
        assert_ne!(id_of(&secret), id_of(&new_secret()));
    }
}
