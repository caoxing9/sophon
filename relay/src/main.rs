//! Sophon's relay: the daemon's relay protocol (version 2) on a server of your
//! own, so a phone away from home reaches its computer through a nearby
//! machine instead of an overseas one.
//!
//! The relay only forwards frames. Everything it carries is end-to-end
//! encrypted between the phone and the computer (Noise, keyed by the pairing
//! QR code), so it reads nothing and needs no secrets.
//!
//! The computer's service (the daemon, `daemon.relay.endpoint`) holds a
//! control socket per server id: `/ws?serverId=S&role=server&v=2`. A phone
//! connects as a client (`role=client`, a connection id given or made up);
//! the relay tells the control socket (`{"type":"connected","connectionId"}`),
//! and the service opens a data socket for that connection
//! (`role=server&connectionId=C`). Frames then pass between the client and
//! that data socket; the client's frames sent before it arrives wait in a
//! small buffer. This is the behavior of the default Cloudflare relay
//! (`packages/relay/src/cloudflare-adapter.ts`), on a plain socket server.
//!
//! Usage: `sophon-relay [ADDR]` (default `0.0.0.0:8443`; `RELAY_ADDR` too).

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use tokio_tungstenite::tungstenite::{
    Message,
    handshake::server::{Request, Response},
    http::StatusCode,
    protocol::{CloseFrame, frame::coding::CloseCode},
};

/// Client frames kept for a connection whose data socket is not there yet.
const BUFFER: usize = 200;

type Tx = mpsc::UnboundedSender<Message>;

#[derive(Clone, PartialEq, Debug)]
enum Role {
    /// The service's control socket of a server id.
    Control,
    /// The service's data socket of a connection.
    Data(String),
    /// A phone's socket of a connection.
    Client(String),
}

struct Socket {
    id: u64,
    tx: Tx,
}

/// One server id's sockets.
#[derive(Default)]
struct Room {
    control: Option<Socket>,
    data: HashMap<String, Socket>,
    clients: HashMap<String, Vec<Socket>>,
    pending: HashMap<String, Vec<Message>>,
}

#[derive(Default)]
struct State {
    rooms: HashMap<String, Room>,
    next_id: u64,
}

type Shared = Arc<Mutex<State>>;

fn close(code: u16, reason: &str) -> Message {
    Message::Close(Some(CloseFrame { code: CloseCode::from(code), reason: reason.to_owned().into() }))
}

fn text(v: serde_json::Value) -> Message {
    Message::Text(v.to_string().into())
}

impl Room {
    fn notify_control(&mut self, msg: serde_json::Value) {
        if let Some(c) = &self.control
            && c.tx.send(text(msg)).is_err()
        {
            self.control = None;
        }
    }

    fn connection_ids(&self) -> Vec<String> {
        self.clients.iter().filter(|(_, s)| !s.is_empty()).map(|(id, _)| id.clone()).collect()
    }

    fn is_empty(&self) -> bool {
        self.control.is_none() && self.data.is_empty() && self.clients.values().all(Vec::is_empty)
    }
}

/// What a socket asked for, from its URL.
struct Wanted {
    server_id: String,
    role: Role,
}

fn parse(uri: &str) -> Result<Wanted, &'static str> {
    let (path, query) = uri.split_once('?').unwrap_or((uri, ""));
    if path != "/ws" {
        return Err("Not found");
    }
    let mut server_id = None;
    let mut role = None;
    let mut connection_id = String::new();
    let mut version = "1".to_owned();
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        match k {
            "serverId" => server_id = Some(v.to_owned()).filter(|s| !s.is_empty()),
            "role" => role = Some(v.to_owned()),
            "connectionId" => connection_id = v.trim().to_owned(),
            "v" => version = v.trim().to_owned(),
            _ => {}
        }
    }
    if version != "2" {
        return Err("This relay speaks version 2 only");
    }
    let server_id = server_id.ok_or("Missing serverId parameter")?;
    let role = match role.as_deref() {
        Some("server") if connection_id.is_empty() => Role::Control,
        Some("server") => Role::Data(connection_id),
        Some("client") => Role::Client(if connection_id.is_empty() {
            format!("conn_{:016x}", rand::random::<u64>())
        } else {
            connection_id
        }),
        _ => return Err("Missing or invalid role parameter"),
    };
    Ok(Wanted { server_id, role })
}

async fn serve(stream: TcpStream, state: Shared) {
    let peer = stream.peer_addr().ok();
    let mut wanted: Result<Wanted, &'static str> = Err("Expected WebSocket upgrade");
    let accepted = tokio_tungstenite::accept_hdr_async(stream, |req: &Request, resp: Response| {
        wanted = parse(&req.uri().to_string());
        match &wanted {
            Ok(_) => Ok(resp),
            Err(e) => {
                let mut r = tokio_tungstenite::tungstenite::http::Response::new(Some((*e).to_owned()));
                *r.status_mut() = StatusCode::BAD_REQUEST;
                Err(r)
            }
        }
    })
    .await;
    let (Ok(ws), Ok(Wanted { server_id, role })) = (accepted, wanted) else { return };
    log::info!("{role:?} of {server_id} connected from {peer:?}");

    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let id = {
        let mut s = state.lock().unwrap();
        s.next_id += 1;
        s.next_id
    };
    register(&state, &server_id, &role, Socket { id, tx: tx.clone() });

    // Writes what the room sends this socket; a Close ends it.
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let closing = matches!(msg, Message::Close(_));
            if sink.send(msg).await.is_err() || closing {
                break;
            }
        }
        let _ = sink.close().await;
    });

    if let Role::Client(conn) = &role {
        nudge(state.clone(), server_id.clone(), conn.clone());
    }

    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            Message::Text(_) | Message::Binary(_) => forward(&state, &server_id, &role, &tx, msg),
            Message::Close(_) => break,
            _ => {}
        }
    }

    unregister(&state, &server_id, &role, id);
    drop(tx);
    let _ = writer.await;
    log::info!("{role:?} of {server_id} disconnected");
}

fn register(state: &Shared, server_id: &str, role: &Role, socket: Socket) {
    let mut s = state.lock().unwrap();
    let room = s.rooms.entry(server_id.to_owned()).or_default();
    match role {
        Role::Control => {
            if let Some(old) = room.control.replace(socket) {
                let _ = old.tx.send(close(1008, "Replaced by new connection"));
            }
            let ids = room.connection_ids();
            room.notify_control(json!({ "type": "sync", "connectionIds": ids }));
        }
        Role::Data(conn) => {
            for frame in room.pending.remove(conn).unwrap_or_default() {
                let _ = socket.tx.send(frame);
            }
            if let Some(old) = room.data.insert(conn.clone(), socket) {
                let _ = old.tx.send(close(1008, "Replaced by new connection"));
            }
        }
        Role::Client(conn) => {
            room.clients.entry(conn.clone()).or_default().push(socket);
            room.notify_control(json!({ "type": "connected", "connectionId": conn }));
        }
    }
}

fn forward(state: &Shared, server_id: &str, role: &Role, own: &Tx, msg: Message) {
    let mut s = state.lock().unwrap();
    let Some(room) = s.rooms.get_mut(server_id) else { return };
    match role {
        // The control socket's only frames are keepalives.
        Role::Control => {
            if let Message::Text(t) = &msg
                && serde_json::from_str::<serde_json::Value>(t).ok().and_then(|v| v.get("type").cloned()) == Some(json!("ping"))
            {
                let _ = own.send(text(json!({ "type": "pong" })));
            }
        }
        Role::Client(conn) => match room.data.get(conn) {
            Some(data) => {
                let _ = data.tx.send(msg);
            }
            None => {
                let pending = room.pending.entry(conn.clone()).or_default();
                pending.push(msg);
                if pending.len() > BUFFER {
                    let extra = pending.len() - BUFFER;
                    pending.drain(..extra);
                }
            }
        },
        Role::Data(conn) => {
            for client in room.clients.get(conn).into_iter().flatten() {
                let _ = client.tx.send(msg.clone());
            }
        }
    }
}

fn unregister(state: &Shared, server_id: &str, role: &Role, id: u64) {
    let mut s = state.lock().unwrap();
    let Some(room) = s.rooms.get_mut(server_id) else { return };
    match role {
        Role::Control => {
            if room.control.as_ref().is_some_and(|c| c.id == id) {
                room.control = None;
            }
        }
        Role::Data(conn) => {
            if room.data.get(conn).is_some_and(|d| d.id == id) {
                room.data.remove(conn);
                for client in room.clients.get(conn).into_iter().flatten() {
                    let _ = client.tx.send(close(1012, "Server disconnected"));
                }
            }
        }
        Role::Client(conn) => {
            let left = room.clients.get_mut(conn).map(|list| {
                list.retain(|c| c.id != id);
                list.len()
            });
            if left == Some(0) {
                room.clients.remove(conn);
                room.pending.remove(conn);
                if let Some(data) = room.data.remove(conn) {
                    let _ = data.tx.send(close(1001, "Client disconnected"));
                }
                room.notify_control(json!({ "type": "disconnected", "connectionId": conn }));
            }
        }
    }
    if room.is_empty() {
        s.rooms.remove(server_id);
    }
}

/// A phone waiting for the service: remind the control socket after 10 s,
/// and drop it 5 s later if the service still does not open the data
/// socket (it reconnects fresh), as the default relay does.
fn nudge(state: Shared, server_id: String, conn: String) {
    tokio::spawn(async move {
        let waiting = |state: &Shared| {
            let s = state.lock().unwrap();
            s.rooms.get(&server_id).is_some_and(|r| r.clients.get(&conn).is_some_and(|c| !c.is_empty()) && !r.data.contains_key(&conn))
        };
        tokio::time::sleep(Duration::from_secs(10)).await;
        if !waiting(&state) {
            return;
        }
        {
            let mut s = state.lock().unwrap();
            if let Some(room) = s.rooms.get_mut(&server_id) {
                let ids = room.connection_ids();
                room.notify_control(json!({ "type": "sync", "connectionIds": ids }));
            }
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
        if !waiting(&state) {
            return;
        }
        let mut s = state.lock().unwrap();
        if let Some(room) = s.rooms.get_mut(&server_id)
            && let Some(control) = room.control.take()
        {
            let _ = control.tx.send(close(1011, "Control unresponsive"));
        }
    });
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let addr = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("RELAY_ADDR").ok())
        .unwrap_or_else(|| "0.0.0.0:8443".into());
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {addr}: {e}");
            std::process::exit(1);
        }
    };
    log::info!("sophon-relay listening on {addr}");
    let state = Shared::default();
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                if let Ok((stream, _)) = accepted {
                    let _ = stream.set_nodelay(true);
                    tokio::spawn(serve(stream, state.clone()));
                }
            }
            _ = tokio::signal::ctrl_c() => break,
        }
    }
}
