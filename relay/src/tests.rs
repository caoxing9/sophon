//! The relay over real sockets on a local port; the helpers serve the push
//! gateway's tests too.

use std::path::PathBuf;

use serde_json::Value;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite};

use super::*;

pub type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

const WAIT: Duration = Duration::from_secs(10);

/// A directory of this test alone.
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sophon-relay-test-{name}-{:016x}", rand::random::<u64>()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Starts the server on a free local port: its `host:port`.
pub async fn start(gateway: push::Gateway) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap().to_string();
    tokio::spawn(listen(listener, Shared::default(), Arc::new(gateway)));
    addr
}

async fn start_relay() -> String {
    let dir = temp_dir("relay");
    let addr = start(push::Gateway::new(&dir, None)).await;
    let _ = std::fs::remove_dir_all(dir);
    addr
}

pub async fn connect(addr: &str, path: &str) -> Ws {
    connect_async(format!("ws://{addr}{path}")).await.unwrap().0
}

/// The HTTP status a refused upgrade gets.
pub async fn refused(addr: &str, path: &str) -> u16 {
    match connect_async(format!("ws://{addr}{path}")).await {
        Err(tungstenite::Error::Http(response)) => response.status().as_u16(),
        other => panic!("{path} was not refused: {:?}", other.map(|_| ())),
    }
}

pub async fn next(ws: &mut Ws) -> Message {
    tokio::time::timeout(WAIT, ws.next()).await.expect("no frame in time").expect("socket ended").expect("socket failed")
}

pub async fn next_json(ws: &mut Ws) -> Value {
    match next(ws).await {
        Message::Text(t) => serde_json::from_str(&t).unwrap(),
        other => panic!("expected a text frame, got {other:?}"),
    }
}

/// The code of the Close frame that comes next.
pub async fn next_close(ws: &mut Ws) -> u16 {
    match next(ws).await {
        Message::Close(Some(frame)) => frame.code.into(),
        other => panic!("expected a close frame, got {other:?}"),
    }
}

pub async fn send(ws: &mut Ws, v: Value) {
    ws.send(text(v)).await.unwrap();
}

/// Nothing arrives for a moment.
pub async fn silent(ws: &mut Ws) -> bool {
    tokio::time::timeout(Duration::from_millis(300), ws.next()).await.is_err()
}

#[tokio::test]
async fn relay_passes_frames_between_a_client_and_its_data_socket() {
    let addr = start_relay().await;
    let mut control = connect(&addr, "/ws?serverId=srv&role=server&v=2").await;
    assert_eq!(next_json(&mut control).await, json!({ "type": "sync", "connectionIds": [] }));
    send(&mut control, json!({ "type": "ping" })).await;
    assert_eq!(next_json(&mut control).await, json!({ "type": "pong" }));

    let mut client = connect(&addr, "/ws?serverId=srv&role=client&connectionId=c1&v=2").await;
    assert_eq!(next_json(&mut control).await, json!({ "type": "connected", "connectionId": "c1" }));
    // Sent before the data socket is there: kept, then delivered.
    client.send(Message::Text("early".into())).await.unwrap();
    client.send(Message::Binary(vec![1, 2, 3].into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut data = connect(&addr, "/ws?serverId=srv&role=server&connectionId=c1&v=2").await;
    assert_eq!(next(&mut data).await, Message::Text("early".into()));
    assert_eq!(next(&mut data).await, Message::Binary(vec![1, 2, 3].into()));
    data.send(Message::Text("back".into())).await.unwrap();
    assert_eq!(next(&mut client).await, Message::Text("back".into()));

    client.close(None).await.unwrap();
    assert_eq!(next_json(&mut control).await, json!({ "type": "disconnected", "connectionId": "c1" }));
    assert_eq!(next_close(&mut data).await, 1001);
}

#[tokio::test]
async fn relay_replaces_an_older_control_socket() {
    let addr = start_relay().await;
    let mut old = connect(&addr, "/ws?serverId=srv&role=server&v=2").await;
    assert_eq!(next_json(&mut old).await["type"], "sync");
    let mut new = connect(&addr, "/ws?serverId=srv&role=server&v=2").await;
    assert_eq!(next_json(&mut new).await["type"], "sync");
    assert_eq!(next_close(&mut old).await, 1008);
}

#[tokio::test]
async fn requests_that_are_neither_service_are_refused() {
    let addr = start_relay().await;
    for path in [
        "/",
        "/nope?serverId=srv&role=server&v=2",
        "/ws?serverId=srv&role=server",
        "/ws?role=server&v=2",
        "/ws?serverId=srv&role=other&v=2",
        "/push",
        "/push?id=short",
        "/push?to=has.a.dot.in.it.and.is.long",
        "/push?id=aaaaaaaaaaaaaaaa&to=aaaaaaaaaaaaaaaa",
    ] {
        assert_eq!(refused(&addr, path).await, 400, "{path}");
    }
}
