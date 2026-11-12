//! The push gateway (`GET /push`, see `docs/push.md`): a computer sends it
//! notifications sealed to the gateway's key, and it sends them to APNs with
//! the app's signing key, which only the gateway holds. A paired device sends
//! it actions sealed to the computer, which it passes on unread.

use std::{
    collections::{HashMap, VecDeque},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use crypto_box::{PublicKey, SalsaBox, SecretKey, aead::Aead};
use futures_util::{SinkExt, StreamExt};
use p256::ecdsa::{Signature, SigningKey, signature::Signer};
use serde_json::{Value, json};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, protocol::WebSocketConfig},
};

use crate::{Tx, close, text};

/// The largest frame a socket may send.
pub const MAX_FRAME: usize = 16 * 1024;
/// Pushes a socket may send in a minute.
const SOCKET_PUSHES: usize = 60;
/// Pushes one device token may receive in a minute.
const TOKEN_PUSHES: usize = 30;
const MINUTE: Duration = Duration::from_secs(60);
/// How long a provider token is used (APNs refuses one older than an hour).
const JWT_LIFE: Duration = Duration::from_secs(50 * 60);
const APNS_PRODUCTION: &str = "https://api.push.apple.com";
const APNS_DEVELOPMENT: &str = "https://api.sandbox.push.apple.com";
const DEFAULT_TOPIC: &str = "dev.sophon.app";
const DEFAULT_STATE_DIR: &str = "/var/lib/sophon-relay";

// ---------------------------------------------------------------- the URL

/// What a `/push` socket is, from its URL.
#[derive(Debug, PartialEq)]
pub enum Wanted {
    /// `id=`: the computer's socket.
    Computer(String),
    /// `to=`: a device's socket, to send that computer actions.
    Device(String),
}

fn computer_id(v: &str) -> bool {
    (16..=128).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn parse(uri: &str) -> Result<Wanted, &'static str> {
    let (_, query) = uri.split_once('?').unwrap_or((uri, ""));
    let (mut id, mut to) = (None, None);
    for pair in query.split('&') {
        match pair.split_once('=').unwrap_or((pair, "")) {
            ("id", v) => id = Some(v),
            ("to", v) => to = Some(v),
            _ => {}
        }
    }
    let (wanted, v): (fn(String) -> Wanted, &str) = match (id, to) {
        (Some(v), None) => (Wanted::Computer, v),
        (None, Some(v)) => (Wanted::Device, v),
        (None, None) => return Err("Missing id or to parameter"),
        _ => return Err("Give id or to, not both"),
    };
    if !computer_id(v) {
        return Err("Invalid computer id");
    }
    Ok(wanted(v.to_owned()))
}

/// The socket limits of a `/push` request, told from its first bytes (the
/// path is not known to the handshake before it reads). The frame limit
/// itself is checked on each frame; this bounds what is read at all.
pub async fn socket_config(stream: &TcpStream) -> Option<WebSocketConfig> {
    let mut start = [0u8; 9];
    let n = stream.peek(&mut start).await.ok()?;
    (&start[..n] == b"GET /push").then(|| {
        WebSocketConfig::default().max_message_size(Some(4 * MAX_FRAME)).max_frame_size(Some(4 * MAX_FRAME))
    })
}

// ---------------------------------------------------------------- sealing

/// Opens `base64(ephemeral public key (32) ‖ nonce (24) ‖ box)`.
fn open(secret: &SecretKey, sealed: &str) -> Option<Vec<u8>> {
    let raw = STANDARD.decode(sealed).ok()?;
    if raw.len() < 32 + 24 + 16 {
        return None;
    }
    let public = PublicKey::from(<[u8; 32]>::try_from(&raw[..32]).ok()?);
    let nonce = <[u8; 24]>::try_from(&raw[32..56]).ok()?;
    SalsaBox::new(&public, secret).decrypt(&nonce.into(), &raw[56..]).ok()
}

/// The gateway's secret key: `<dir>/push.key` (32 bytes), made on the first
/// start. A directory that cannot be used gives a key for this run only.
fn load_key(dir: &Path) -> SecretKey {
    let path = dir.join("push.key");
    match std::fs::read(&path) {
        Ok(bytes) => match <[u8; 32]>::try_from(bytes.as_slice()) {
            Ok(key) => return SecretKey::from(key),
            Err(_) => log::warn!("{} is not a 32-byte key; using a key for this run only", path.display()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let key: [u8; 32] = rand::random();
            match store_key(dir, &path, &key) {
                Ok(()) => log::info!("made the push gateway's key: {}", path.display()),
                Err(e) => log::warn!(
                    "cannot write {}: {e}; using a key for this run only (computers will refuse the next one)",
                    path.display()
                ),
            }
            return SecretKey::from(key);
        }
        Err(e) => log::warn!("cannot read {}: {e}; using a key for this run only", path.display()),
    }
    SecretKey::from(rand::random::<[u8; 32]>())
}

fn store_key(dir: &Path, path: &Path, key: &[u8; 32]) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(path)?;
    file.write_all(key)?;
    file.sync_all()
}

// ---------------------------------------------------------------- limits

/// At most `limit` in any minute.
struct Window {
    limit: usize,
    hits: VecDeque<Instant>,
}

impl Window {
    fn new(limit: usize) -> Self {
        Self { limit, hits: VecDeque::new() }
    }

    fn expire(&mut self, now: Instant) {
        while self.hits.front().is_some_and(|t| now.duration_since(*t) >= MINUTE) {
            self.hits.pop_front();
        }
    }

    fn allow(&mut self, now: Instant) -> bool {
        self.expire(now);
        if self.hits.len() >= self.limit {
            return false;
        }
        self.hits.push_back(now);
        true
    }
}

// ---------------------------------------------------------------- a notification

/// The sealed JSON of a `push`.
#[derive(Debug)]
struct Notification {
    token: String,
    development: bool,
    title: String,
    body: String,
    category: String,
    thread: Option<String>,
    collapse: Option<String>,
    data: Option<Value>,
}

impl Notification {
    fn parse(bytes: &[u8]) -> Option<Self> {
        let v: Value = serde_json::from_slice(bytes).ok()?;
        let required = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let optional = |k: &str| required(k).filter(|s| !s.is_empty());
        let token = required("token")?;
        // It becomes a part of the request's path.
        if token.is_empty() || token.len() > 200 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let development = match v.get("env")?.as_str()? {
            "production" => false,
            "development" => true,
            _ => return None,
        };
        let data = match v.get("data") {
            None | Some(Value::Null) => None,
            Some(d @ Value::Object(_)) => Some(d.clone()),
            Some(_) => return None,
        };
        Some(Self {
            token,
            development,
            title: required("title")?,
            body: required("body")?,
            category: required("category")?,
            thread: optional("thread"),
            collapse: optional("collapse"),
            data,
        })
    }

    fn payload(&self) -> Value {
        let mut aps = json!({
            "alert": { "title": self.title, "body": self.body },
            "sound": "default",
            "category": self.category,
        });
        if let Some(thread) = &self.thread {
            aps["thread-id"] = json!(thread);
        }
        if self.category == "PERMISSION" {
            aps["interruption-level"] = json!("time-sensitive");
        }
        let mut payload = json!({ "aps": aps });
        if let Some(data) = &self.data {
            payload["sophon"] = data.clone();
        }
        payload
    }
}

// ---------------------------------------------------------------- APNs

/// The APNs provider: the app's signing key and one connection per
/// environment (HTTP/2, kept by the client's pool).
pub struct Apns {
    key: SigningKey,
    key_id: String,
    team_id: String,
    topic: String,
    production: String,
    development: String,
    client: reqwest::Client,
    jwt: Mutex<Option<(String, Instant)>>,
}

impl Apns {
    /// From `APNS_KEY_FILE`, `APNS_KEY_ID`, `APNS_TEAM_ID`, `APNS_TOPIC`;
    /// none when they are not (all) there.
    pub fn from_env() -> Option<Self> {
        let var = |k: &str| std::env::var(k).ok().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty());
        let Some(file) = var("APNS_KEY_FILE") else {
            log::info!("push gateway: no APNS_KEY_FILE, pushes are answered NotConfigured");
            return None;
        };
        let pem = match std::fs::read_to_string(&file) {
            Ok(pem) if !pem.trim().is_empty() => pem,
            // The service passes an empty file when no key was installed.
            Ok(_) => {
                log::info!("push gateway: {file} is empty, pushes are answered NotConfigured");
                return None;
            }
            Err(e) => {
                log::warn!("push gateway: cannot read {file}: {e}; pushes are answered NotConfigured");
                return None;
            }
        };
        let (Some(key_id), Some(team_id)) = (var("APNS_KEY_ID"), var("APNS_TEAM_ID")) else {
            log::warn!("push gateway: APNS_KEY_ID and APNS_TEAM_ID are needed; pushes are answered NotConfigured");
            return None;
        };
        let topic = var("APNS_TOPIC").unwrap_or_else(|| DEFAULT_TOPIC.to_owned());
        match Self::new(&pem, &key_id, &team_id, &topic, APNS_PRODUCTION, APNS_DEVELOPMENT) {
            Ok(apns) => {
                log::info!("push gateway: APNs key {key_id} of team {team_id}, topic {topic}");
                Some(apns)
            }
            Err(e) => {
                log::warn!("push gateway: {file}: {e}; pushes are answered NotConfigured");
                None
            }
        }
    }

    /// `production` and `development` are the base URLs of the two APNs
    /// environments (Apple's, or a test's).
    fn new(pem: &str, key_id: &str, team_id: &str, topic: &str, production: &str, development: &str) -> Result<Self, String> {
        use p256::pkcs8::DecodePrivateKey;
        let key = SigningKey::from_pkcs8_pem(pem.trim()).map_err(|e| format!("not a PKCS#8 P-256 key: {e}"))?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(10 * 60))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            key,
            key_id: key_id.to_owned(),
            team_id: team_id.to_owned(),
            topic: topic.to_owned(),
            production: production.trim_end_matches('/').to_owned(),
            development: development.trim_end_matches('/').to_owned(),
            client,
            jwt: Mutex::new(None),
        })
    }

    /// The provider token, made anew when 50 minutes old.
    fn jwt(&self) -> String {
        let mut kept = self.jwt.lock().unwrap();
        if let Some((jwt, made)) = &*kept
            && made.elapsed() < JWT_LIFE
        {
            return jwt.clone();
        }
        let iat = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let part = |v: Value| URL_SAFE_NO_PAD.encode(v.to_string());
        let signed = format!(
            "{}.{}",
            part(json!({ "alg": "ES256", "kid": self.key_id })),
            part(json!({ "iss": self.team_id, "iat": iat }))
        );
        let signature: Signature = self.key.sign(signed.as_bytes());
        let jwt = format!("{signed}.{}", URL_SAFE_NO_PAD.encode(signature.to_bytes()));
        *kept = Some((jwt.clone(), Instant::now()));
        jwt
    }

    /// Sends one notification; an error is the `reason` of `pushed`.
    async fn send(&self, n: &Notification) -> Result<(), String> {
        let base = if n.development { &self.development } else { &self.production };
        let mut request = self
            .client
            .post(format!("{base}/3/device/{}", n.token))
            .header("authorization", format!("bearer {}", self.jwt()))
            .header("apns-topic", &self.topic)
            .header("apns-push-type", "alert")
            .header("apns-priority", "10")
            .header("content-type", "application/json");
        if let Some(collapse) = &n.collapse {
            request = request.header("apns-collapse-id", collapse);
        }
        let response = match request.body(n.payload().to_string()).send().await {
            Ok(r) => r,
            Err(e) => {
                log::warn!("APNs: {e}");
                return Err("Unreachable".into());
            }
        };
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let reason = response
            .bytes()
            .await
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .and_then(|v| v.get("reason").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| format!("Http{}", status.as_u16()));
        if reason == "ExpiredProviderToken" {
            *self.jwt.lock().unwrap() = None;
        }
        Err(reason)
    }
}

// ---------------------------------------------------------------- the gateway

struct Computer {
    number: u64,
    tx: Tx,
}

struct Device {
    to: String,
    tx: Tx,
}

#[derive(Default)]
struct Sockets {
    next: u64,
    /// By computer id.
    computers: HashMap<String, Computer>,
    /// By socket number.
    devices: HashMap<u64, Device>,
    /// By device token.
    tokens: HashMap<String, Window>,
}

pub struct Gateway {
    secret: SecretKey,
    /// The public key, as `hello` says it.
    key: String,
    apns: Option<Apns>,
    sockets: Mutex<Sockets>,
}

impl Gateway {
    /// With the key of `RELAY_STATE_DIR` and the APNs key of the environment.
    pub fn from_env() -> Self {
        let dir = std::env::var("RELAY_STATE_DIR").ok().filter(|d| !d.is_empty()).unwrap_or_else(|| DEFAULT_STATE_DIR.into());
        Self::new(Path::new(&dir), Apns::from_env())
    }

    pub fn new(state_dir: &Path, apns: Option<Apns>) -> Self {
        let secret = load_key(state_dir);
        let key = STANDARD.encode(secret.public_key().as_bytes());
        Self { secret, key, apns, sockets: Mutex::default() }
    }

    fn push(self: &Arc<Self>, v: &Value, window: &mut Window, tx: &Tx) {
        let r#ref = v.get("ref").cloned().unwrap_or(Value::Null);
        let refuse = |reason: &str| {
            let _ = tx.send(text(json!({ "type": "pushed", "ref": r#ref, "ok": false, "reason": reason })));
        };
        if self.apns.is_none() {
            return refuse("NotConfigured");
        }
        let now = Instant::now();
        if !window.allow(now) {
            return refuse("RateLimited");
        }
        let opened = v.get("sealed").and_then(Value::as_str).and_then(|s| open(&self.secret, s));
        let Some(notification) = opened.and_then(|bytes| Notification::parse(&bytes)) else {
            return refuse("BadRequest");
        };
        {
            let mut s = self.sockets.lock().unwrap();
            if s.tokens.len() >= 4096 {
                s.tokens.retain(|_, w| {
                    w.expire(now);
                    !w.hits.is_empty()
                });
            }
            let token = notification.token.to_ascii_lowercase();
            if !s.tokens.entry(token).or_insert_with(|| Window::new(TOKEN_PUSHES)).allow(now) {
                return refuse("RateLimited");
            }
        }
        // APNs answers in its own time; the socket goes on meanwhile.
        let (gateway, tx) = (self.clone(), tx.clone());
        tokio::spawn(async move {
            let Some(apns) = &gateway.apns else { return };
            let answer = match apns.send(&notification).await {
                Ok(()) => json!({ "type": "pushed", "ref": r#ref, "ok": true }),
                Err(reason) => json!({ "type": "pushed", "ref": r#ref, "ok": false, "reason": reason }),
            };
            let _ = tx.send(text(answer));
        });
    }

    /// A device's action, to its computer's socket.
    fn act(&self, to: &str, number: u64, v: &Value, tx: &Tx) {
        let r#ref = v.get("ref").cloned().unwrap_or(Value::Null);
        let passed = json!({ "type": "act", "from": number, "ref": r#ref, "sealed": v.get("sealed").cloned().unwrap_or(Value::Null) });
        let s = self.sockets.lock().unwrap();
        if !s.computers.get(to).is_some_and(|c| c.tx.send(text(passed)).is_ok()) {
            let _ = tx.send(text(json!({ "type": "acted", "ref": r#ref, "ok": false, "reason": "Offline" })));
        }
    }

    /// A computer's answer, back to the device's socket, without `from`.
    fn acted(&self, id: &str, mut v: Value) {
        let Some(from) = v.as_object_mut().and_then(|o| o.remove("from")).and_then(|f| f.as_u64()) else { return };
        let s = self.sockets.lock().unwrap();
        // Only to a device that addressed this computer.
        if let Some(device) = s.devices.get(&from).filter(|d| d.to == id) {
            let _ = device.tx.send(text(v));
        }
    }
}

pub async fn serve(ws: WebSocketStream<TcpStream>, wanted: Wanted, gateway: Arc<Gateway>) {
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();
    let _ = tx.send(text(json!({ "type": "hello", "key": gateway.key, "apns": gateway.apns.is_some() })));
    let number = {
        let mut s = gateway.sockets.lock().unwrap();
        s.next += 1;
        let number = s.next;
        match &wanted {
            Wanted::Computer(id) => {
                if let Some(old) = s.computers.insert(id.clone(), Computer { number, tx: tx.clone() }) {
                    let _ = old.tx.send(close(1008, "Replaced by new connection"));
                }
            }
            Wanted::Device(to) => {
                s.devices.insert(number, Device { to: to.clone(), tx: tx.clone() });
            }
        }
        number
    };
    log::info!("push: {wanted:?} connected (socket {number})");

    // Writes what the gateway sends this socket; a Close ends it.
    let writer = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let closing = matches!(msg, Message::Close(_));
            if sink.send(msg).await.is_err() || closing {
                break;
            }
        }
        let _ = sink.close().await;
    });

    let mut window = Window::new(SOCKET_PUSHES);
    loop {
        let msg = tokio::select! {
            msg = stream.next() => msg,
            // Closed from this side (replaced).
            _ = tx.closed() => break,
        };
        let frame = match msg {
            Some(Ok(Message::Text(t))) => t,
            Some(Ok(Message::Binary(b))) if b.len() > MAX_FRAME => {
                let _ = tx.send(close(1009, "Frame too large"));
                break;
            }
            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
            Some(Ok(_)) => continue,
        };
        if frame.len() > MAX_FRAME {
            let _ = tx.send(close(1009, "Frame too large"));
            break;
        }
        let Ok(v) = serde_json::from_str::<Value>(&frame) else { continue };
        match (v.get("type").and_then(Value::as_str), &wanted) {
            (Some("ping"), _) => {
                let _ = tx.send(text(json!({ "type": "pong" })));
            }
            (Some("push"), Wanted::Computer(_)) => gateway.push(&v, &mut window, &tx),
            (Some("acted"), Wanted::Computer(id)) => gateway.acted(id, v),
            (Some("act"), Wanted::Device(to)) => gateway.act(to, number, &v, &tx),
            _ => {}
        }
    }

    {
        let mut s = gateway.sockets.lock().unwrap();
        match &wanted {
            Wanted::Computer(id) => {
                if s.computers.get(id).is_some_and(|c| c.number == number) {
                    s.computers.remove(id);
                }
            }
            Wanted::Device(_) => {
                s.devices.remove(&number);
            }
        }
    }
    drop(tx);
    let _ = writer.await;
    log::info!("push: {wanted:?} disconnected (socket {number})");
}

#[cfg(test)]
mod tests;
