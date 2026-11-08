//! Paseo's relay, end to end encrypted (`docs/relay-endpoint-security.md`):
//! the relay forwards frames between a client and a daemon it cannot read.
//!
//! - Pairing gives the daemon's id, its static X25519 key and the relay:
//!   `https://app.paseo.sh/#offer=<base64url JSON>` or
//!   `relay://host:port/<serverId>?key=…&ssl=true&password=…`.
//! - The handshake is `Noise_NK_25519_ChaChaPoly_SHA256` with the prologue
//!   `paseo-relay-e2ee/1`; message 1 rides in an `e2ee_hello`, message 2 in
//!   the daemon's `e2ee_ready`. The payload is `[0x01 version, flags]`, bit 0
//!   of the flags is "binary ciphertext".
//! - Each application frame is one record of chunks, each
//!   `u16 BE length || Noise(kind, flags, u32 BE total, data)`; a text record
//!   travels as base64 text, a binary one as binary.

use std::sync::Arc;

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::{Value, json};
use super::rt::Message;

const PROLOGUE: &[u8] = b"paseo-relay-e2ee/1";
const PATTERN: &str = "Noise_NK_25519_ChaChaPoly_SHA256";
const NOISE_MAX: usize = 65535;
const TAG: usize = 16;
const HEADER: usize = 6;
const CHUNK_DATA: usize = NOISE_MAX - TAG - HEADER;
const KIND_TEXT: u8 = 0x00;
const KIND_BINARY: u8 = 0x01;
const FIRST: u8 = 0x01;
const FINAL: u8 = 0x02;
pub const DEFAULT_RELAY: &str = "relay.paseo.sh:443";

/// What pairing hands a client.
#[derive(Debug, Clone, PartialEq)]
pub struct RelayOffer {
    pub server_id: String,
    pub daemon_key: [u8; 32],
    /// `host:port`.
    pub endpoint: String,
    pub use_tls: bool,
    pub password: Option<String>,
}

impl RelayOffer {
    /// Reads a pairing link: the `#offer=` URL or a `relay://` URI.
    pub fn parse(input: &str) -> Option<Self> {
        let s = input.trim();
        if let Some(i) = s.find("#offer=") {
            let enc = s[i + 7..].trim().replace('-', "+").replace('_', "/");
            let padded = format!("{enc}{}", "=".repeat((4 - enc.len() % 4) % 4));
            let json: Value = serde_json::from_slice(&B64.decode(padded).ok()?).ok()?;
            let endpoint = json.pointer("/relay/endpoint")?.as_str()?.to_owned();
            return Some(Self {
                server_id: json.get("serverId")?.as_str()?.to_owned(),
                daemon_key: key(json.get("daemonPublicKeyB64")?.as_str()?)?,
                use_tls: json.pointer("/relay/useTls").and_then(Value::as_bool).unwrap_or_else(|| endpoint.ends_with(":443")),
                endpoint,
                password: None,
            });
        }
        let rest = s.strip_prefix("relay://")?;
        let (authority, tail) = rest.split_once('/')?;
        let (id, query) = tail.split_once('?').unwrap_or((tail, ""));
        let param = |name: &str| {
            query.split('&').find_map(|kv| {
                let (k, v) = kv.split_once('=')?;
                (k == name).then(|| percent_decode(v))
            })
        };
        Some(Self {
            server_id: percent_decode(id),
            daemon_key: key(&param("key")?)?,
            endpoint: authority.to_owned(),
            use_tls: param("ssl").as_deref() == Some("true") || authority.ends_with(":443"),
            password: param("password").filter(|p| !p.is_empty()),
        })
    }

    /// The relay socket the client side of this daemon's session uses.
    pub fn ws_url(&self) -> String {
        let scheme = if self.use_tls { "wss" } else { "ws" };
        format!("{scheme}://{}/ws?serverId={}&role=client&v=2", self.endpoint, self.server_id)
    }
}

fn key(b64: &str) -> Option<[u8; 32]> {
    B64.decode(b64.trim()).ok()?.try_into().ok()
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() + 0 && i + 2 <= b.len() - 1 + 1 {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&b[i + 1..(i + 3).min(b.len())]).unwrap_or(""), 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The initiator's half of the handshake, before the daemon answers.
pub struct Handshake {
    state: snow::HandshakeState,
    hello: String,
    /// The legacy box key: daemons older than the protected mode (Paseo
    /// < 0.13) answer the legacy hello only.
    legacy: crypto_box::SalsaBox,
}

impl Handshake {
    pub fn start(offer: &RelayOffer) -> Result<Self, String> {
        let params = PATTERN.parse().map_err(|e| format!("{e:?}"))?;
        let builder = snow::Builder::new(params);
        // A separate ephemeral for the legacy field, which only old daemons read.
        let legacy_pair = builder.generate_keypair().map_err(|e| e.to_string())?;
        let legacy_secret: [u8; 32] = legacy_pair.private.clone().try_into().map_err(|_| "bad key".to_owned())?;
        let legacy = crypto_box::SalsaBox::new(
            &crypto_box::PublicKey::from(offer.daemon_key),
            &crypto_box::SecretKey::from(legacy_secret),
        );
        let mut state = snow::Builder::new(PATTERN.parse().map_err(|e| format!("{e:?}"))?)
            .prologue(PROLOGUE)
            .remote_public_key(&offer.daemon_key)
            .build_initiator()
            .map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 1024];
        let n = state.write_message(&[0x01, 0x01], &mut buf).map_err(|e| e.to_string())?;
        let hello = json!({
            "type": "e2ee_hello",
            "key": B64.encode(&legacy_pair.public),
            "capabilities": { "binaryCiphertext": true },
            "protected": { "v": 1, "message": B64.encode(&buf[..n]) },
        })
        .to_string();
        Ok(Self { state, hello, legacy })
    }

    /// Sent until the daemon is ready (the relay may have no daemon yet).
    pub fn hello(&self) -> &str {
        &self.hello
    }

    /// Reads the daemon's `e2ee_ready`; `Ok(None)` for unrelated traffic.
    /// Unrelated traffic hands the handshake back with an empty error.
    pub fn finish(mut self, text: &str) -> Result<Option<(Sender, Receiver)>, (Self, String)> {
        let ready = serde_json::from_str::<Value>(text).ok().filter(|v| v.get("type").and_then(Value::as_str) == Some("e2ee_ready"));
        let Some(v) = ready else { return Err((self, String::new())) };
        let Some(msg) = v.pointer("/protected/message").and_then(Value::as_str).and_then(|m| B64.decode(m).ok()) else {
            // An older daemon: the legacy box channel (no replay protection).
            let binary = v.pointer("/capabilities/binaryCiphertext").and_then(Value::as_bool) == Some(true);
            let b = Arc::new(self.legacy);
            return Ok(Some((
                Sender { mode: Mode::Legacy(b.clone()), nonce: 0, binary },
                Receiver { mode: Mode::Legacy(b), nonce: 0, binary },
            )));
        };
        let mut payload = vec![0u8; 64];
        let n = match self.state.read_message(&msg, &mut payload) {
            Ok(n) => n,
            Err(e) => return Err((self, format!("Relay handshake failed: {e}"))),
        };
        if n != 2 || payload[0] != 0x01 || payload[1] & !0x01 != 0 {
            return Err((self, "Relay handshake: unsupported parameters".into()));
        }
        let binary = payload[1] & 0x01 != 0;
        let transport = match self.state.into_stateless_transport_mode() {
            Ok(t) => Arc::new(t),
            Err(e) => return Err((Self::dead(), format!("Relay handshake failed: {e}"))),
        };
        Ok(Some((
            Sender { mode: Mode::Protected(transport.clone()), nonce: 0, binary },
            Receiver { mode: Mode::Protected(transport), nonce: 0, binary },
        )))
    }

    fn dead() -> Self {
        // Only used to carry an error after the state was consumed.
        let state = snow::Builder::new(PATTERN.parse().expect("pattern"))
            .local_private_key(&[1u8; 32])
            .remote_public_key(&[9u8; 32])
            .build_initiator()
            .expect("state");
        let legacy = crypto_box::SalsaBox::new(&crypto_box::PublicKey::from([9u8; 32]), &crypto_box::SecretKey::from([1u8; 32]));
        Self { state, hello: String::new(), legacy }
    }
}

/// The negotiated construction.
#[derive(Clone)]
enum Mode {
    Protected(Arc<snow::StatelessTransportState>),
    Legacy(Arc<crypto_box::SalsaBox>),
}

impl Mode {
    pub fn protected(&self) -> bool {
        matches!(self, Mode::Protected(_))
    }
}

fn legacy_seal(b: &crypto_box::SalsaBox, plain: &[u8]) -> Result<Vec<u8>, String> {
    use crypto_box::aead::{Aead, AeadCore, OsRng};
    let nonce = crypto_box::SalsaBox::generate_nonce(&mut OsRng);
    let ct = b.encrypt(&nonce, plain).map_err(|_| "Encryption failed".to_owned())?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(out)
}

fn legacy_open(b: &crypto_box::SalsaBox, wire: &[u8]) -> Result<Vec<u8>, String> {
    use crypto_box::aead::Aead;
    if wire.len() < 24 {
        return Err("Relay ciphertext too short".into());
    }
    let nonce = crypto_box::Nonce::from_slice(&wire[..24]);
    b.decrypt(nonce, &wire[24..]).map_err(|_| "Relay message failed authentication".to_owned())
}

/// Encrypts application frames (the initiator sends with the first cipher).
pub struct Sender {
    mode: Mode,
    nonce: u64,
    binary: bool,
}

impl Sender {
    /// Whether the channel is the protected (Noise) one.
    pub fn protected(&self) -> bool {
        self.mode.protected()
    }

    pub fn seal(&mut self, frame: Message) -> Result<Message, String> {
        let (kind, data): (u8, Vec<u8>) = match frame {
            Message::Text(t) => (KIND_TEXT, t.as_str().as_bytes().to_vec()),
            Message::Binary(b) => (KIND_BINARY, b.to_vec()),
            other => return Ok(other),
        };
        let transport = match &self.mode {
            Mode::Legacy(b) => {
                let wire = legacy_seal(b, &data)?;
                return Ok(if self.binary && kind == KIND_BINARY {
                    Message::Binary(wire.into())
                } else {
                    Message::Text(B64.encode(wire).into())
                });
            }
            Mode::Protected(t) => t.clone(),
        };
        let chunks = data.len().div_ceil(CHUNK_DATA).max(1);
        let mut wire = Vec::with_capacity(data.len() + chunks * (2 + HEADER + TAG));
        let mut buf = vec![0u8; NOISE_MAX];
        for i in 0..chunks {
            let part = &data[(i * CHUNK_DATA).min(data.len())..((i + 1) * CHUNK_DATA).min(data.len())];
            let mut plain = Vec::with_capacity(HEADER + part.len());
            plain.push(kind);
            plain.push(if i == 0 { FIRST } else { 0 } | if i == chunks - 1 { FINAL } else { 0 });
            plain.extend_from_slice(&(data.len() as u32).to_be_bytes());
            plain.extend_from_slice(part);
            let n = transport.write_message(self.nonce, &plain, &mut buf).map_err(|e| e.to_string())?;
            self.nonce += 1;
            wire.extend_from_slice(&(n as u16).to_be_bytes());
            wire.extend_from_slice(&buf[..n]);
        }
        Ok(if self.binary && kind == KIND_BINARY {
            Message::Binary(wire.into())
        } else {
            Message::Text(B64.encode(wire).into())
        })
    }
}

/// Decrypts the daemon's records.
pub struct Receiver {
    mode: Mode,
    nonce: u64,
    binary: bool,
}

impl Receiver {
    /// `Ok(None)`: not a record (a repeated `e2ee_ready`). Any failure closes
    /// the channel; nothing partial is returned.
    pub fn open(&mut self, frame: Message) -> Result<Option<Message>, String> {
        let (wire, was_binary) = match frame {
            Message::Binary(b) => (b.to_vec(), true),
            Message::Text(t) => {
                let s = t.as_str();
                if s.trim_start().starts_with('{') {
                    return Ok(None);
                }
                (B64.decode(s).map_err(|_| "Relay record is not base64".to_owned())?, false)
            }
            other => return Ok(Some(other)),
        };
        let transport = match &self.mode {
            Mode::Legacy(b) => {
                let plain = legacy_open(b, &wire)?;
                // With binary ciphertext a binary frame carries binary; else the
                // plaintext is text unless it is not UTF-8.
                return Ok(Some(if self.binary && was_binary {
                    Message::Binary(plain.into())
                } else {
                    match String::from_utf8(plain) {
                        Ok(t) => Message::Text(t.into()),
                        Err(e) => Message::Binary(e.into_bytes().into()),
                    }
                }));
            }
            Mode::Protected(t) => t.clone(),
        };
        let mut at = 0;
        let mut kind = None;
        let mut total = 0usize;
        let mut data = Vec::new();
        let mut buf = vec![0u8; NOISE_MAX];
        let mut done = false;
        while at < wire.len() {
            if done || at + 2 > wire.len() {
                return Err("Malformed relay record".into());
            }
            let len = u16::from_be_bytes([wire[at], wire[at + 1]]) as usize;
            at += 2;
            if at + len > wire.len() {
                return Err("Truncated relay record".into());
            }
            let n = transport.read_message(self.nonce, &wire[at..at + len], &mut buf).map_err(|_| "Relay record failed authentication".to_owned())?;
            self.nonce += 1;
            at += len;
            if n < HEADER {
                return Err("Malformed relay chunk".into());
            }
            let (k, flags) = (buf[0], buf[1]);
            let len_total = u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]) as usize;
            if kind.is_none() {
                if flags & FIRST == 0 {
                    return Err("Relay record without a first chunk".into());
                }
                kind = Some(k);
                total = len_total;
            } else if Some(k) != kind || len_total != total {
                return Err("Inconsistent relay record".into());
            }
            data.extend_from_slice(&buf[HEADER..n]);
            done = flags & FINAL != 0;
        }
        if !done || data.len() != total {
            return Err("Incomplete relay record".into());
        }
        Ok(Some(match kind {
            Some(KIND_BINARY) => Message::Binary(data.into()),
            _ => Message::Text(String::from_utf8(data).map_err(|_| "Relay text is not UTF-8".to_owned())?.into()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn daemon_side(offer_key_private: &[u8], hello: &str) -> (String, snow::StatelessTransportState) {
        let v: Value = serde_json::from_str(hello).unwrap();
        let msg1 = B64.decode(v.pointer("/protected/message").unwrap().as_str().unwrap()).unwrap();
        let mut resp = snow::Builder::new(PATTERN.parse().unwrap())
            .prologue(PROLOGUE)
            .local_private_key(offer_key_private)
            .build_responder()
            .unwrap();
        let mut payload = [0u8; 16];
        let n = resp.read_message(&msg1, &mut payload).unwrap();
        assert_eq!(&payload[..n], &[0x01, 0x01]);
        let mut buf = [0u8; 256];
        let m = resp.write_message(&[0x01, 0x01], &mut buf).unwrap();
        let ready = json!({ "type": "e2ee_ready", "protected": { "v": 1, "message": B64.encode(&buf[..m]) } }).to_string();
        (ready, resp.into_stateless_transport_mode().unwrap())
    }

    #[test]
    fn handshake_and_records_round_trip() {
        let kp = snow::Builder::new(PATTERN.parse().unwrap()).generate_keypair().unwrap();
        let offer = RelayOffer {
            server_id: "srv".into(),
            daemon_key: kp.public.clone().try_into().unwrap(),
            endpoint: DEFAULT_RELAY.into(),
            use_tls: true,
            password: None,
        };
        let hs = Handshake::start(&offer).unwrap();
        let (ready, daemon) = daemon_side(&kp.private, hs.hello());
        let (mut tx, mut rx) = hs.finish(&ready).ok().unwrap().unwrap();

        // Client → daemon, a large text frame split over chunks.
        let big = "x".repeat(150_000);
        let sealed = tx.seal(Message::Text(big.clone().into())).unwrap();
        let Message::Text(b64) = sealed else { panic!() };
        let wire = B64.decode(b64.as_str()).unwrap();
        let (mut at, mut nonce, mut got) = (0, 0u64, Vec::new());
        let mut buf = vec![0u8; NOISE_MAX];
        while at < wire.len() {
            let len = u16::from_be_bytes([wire[at], wire[at + 1]]) as usize;
            let n = daemon.read_message(nonce, &wire[at + 2..at + 2 + len], &mut buf).unwrap();
            got.extend_from_slice(&buf[HEADER..n]);
            nonce += 1;
            at += 2 + len;
        }
        assert_eq!(got.len(), 150_000);

        // Daemon → client, one binary chunk.
        let mut plain = vec![KIND_BINARY, FIRST | FINAL];
        plain.extend_from_slice(&3u32.to_be_bytes());
        plain.extend_from_slice(&[7, 8, 9]);
        let n = daemon.write_message(0, &plain, &mut buf).unwrap();
        let mut rec = (n as u16).to_be_bytes().to_vec();
        rec.extend_from_slice(&buf[..n]);
        let out = rx.open(Message::Binary(rec.into())).unwrap().unwrap();
        assert_eq!(out, Message::Binary(vec![7, 8, 9].into()));
    }

    #[test]
    fn legacy_box_round_trip() {
        use crypto_box::{PublicKey, SalsaBox, SecretKey};
        let daemon_secret = [5u8; 32];
        let daemon_public = SecretKey::from(daemon_secret).public_key();
        let offer = RelayOffer {
            server_id: "srv".into(),
            daemon_key: *daemon_public.as_bytes(),
            endpoint: DEFAULT_RELAY.into(),
            use_tls: true,
            password: None,
        };
        let hs = Handshake::start(&offer).unwrap();
        let client_pub: [u8; 32] = {
            let v: Value = serde_json::from_str(hs.hello()).unwrap();
            B64.decode(v["key"].as_str().unwrap()).unwrap().try_into().unwrap()
        };
        let ready = json!({ "type": "e2ee_ready", "capabilities": { "binaryCiphertext": true } }).to_string();
        let (mut tx, mut rx) = hs.finish(&ready).ok().unwrap().unwrap();
        assert!(!tx.protected());
        let daemon = SalsaBox::new(&PublicKey::from(client_pub), &SecretKey::from(daemon_secret));
        let Message::Text(sealed) = tx.seal(Message::Text("hello daemon".into())).unwrap() else { panic!() };
        assert_eq!(legacy_open(&daemon, &B64.decode(sealed.as_str()).unwrap()).unwrap(), b"hello daemon");
        let back = legacy_seal(&daemon, b"{\"type\":\"pong\"}").unwrap();
        assert_eq!(rx.open(Message::Text(B64.encode(back).into())).unwrap().unwrap(), Message::Text("{\"type\":\"pong\"}".into()));
    }

    #[test]
    fn bridge_handshake_and_pairing_links() {
        let (private, public) = generate_key();
        let offer = RelayOffer { server_id: "bridge".into(), daemon_key: public, endpoint: "10.0.0.2:47000".into(), use_tls: false, password: None };
        let hs = Handshake::start(&offer).unwrap();
        let (ready, mut btx, mut brx) = respond(&private, hs.hello()).unwrap();
        let (mut ctx, mut crx) = hs.finish(&ready).ok().unwrap().unwrap();
        let sealed = ctx.seal(Message::Text("ping".into())).unwrap();
        assert_eq!(brx.open(sealed).unwrap().unwrap(), Message::Text("ping".into()));
        let back = btx.seal(Message::Binary(vec![1, 2].into())).unwrap();
        assert_eq!(crx.open(back).unwrap().unwrap(), Message::Binary(vec![1, 2].into()));

        let key_b64 = B64.encode([3u8; 32]);
        let json = json!({ "v": 2, "serverId": "srv_1", "daemonPublicKeyB64": key_b64, "relay": { "endpoint": "relay.paseo.sh:443", "useTls": true } });
        let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.to_string());
        let link = PairingLink::format(Some(("192.168.1.5:47000", &public, "tok")), Some(&format!("https://app.paseo.sh/#offer={enc}")));
        let parsed = PairingLink::parse(&link).unwrap();
        assert_eq!(parsed.lan.as_ref().unwrap().0, "192.168.1.5:47000");
        assert_eq!(parsed.lan.as_ref().unwrap().1, public);
        assert_eq!(parsed.relay.unwrap().server_id, "srv_1");
    }

    #[test]
    fn parses_pairing_links() {
        let key = B64.encode([3u8; 32]);
        let json = json!({ "v": 2, "serverId": "srv_1", "daemonPublicKeyB64": key, "relay": { "endpoint": "relay.paseo.sh:443", "useTls": true } });
        let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.to_string());
        let o = RelayOffer::parse(&format!("https://app.paseo.sh/#offer={enc}")).unwrap();
        assert_eq!(o.server_id, "srv_1");
        assert_eq!(o.ws_url(), "wss://relay.paseo.sh:443/ws?serverId=srv_1&role=client&v=2");
        let r = RelayOffer::parse(&format!("relay://relay.example.com:443/srv_2?key={}&ssl=true&password=pw", key.replace('+', "%2B").replace('/', "%2F").replace('=', "%3D"))).unwrap();
        assert_eq!((r.server_id.as_str(), r.password.as_deref(), r.use_tls), ("srv_2", Some("pw"), true));
    }
}


/// The responder's half: Sophon's LAN bridge answers phones itself, with
/// its own static key.
pub fn respond(local_private: &[u8; 32], hello_text: &str) -> Result<(String, Sender, Receiver), String> {
    let v: Value = serde_json::from_str(hello_text).map_err(|_| "not a hello".to_owned())?;
    if v.get("type").and_then(Value::as_str) != Some("e2ee_hello") {
        return Err("not a hello".into());
    }
    let msg1 = v
        .pointer("/protected/message")
        .and_then(Value::as_str)
        .and_then(|m| B64.decode(m).ok())
        .ok_or("the bridge requires protected encryption")?;
    let mut state = snow::Builder::new(PATTERN.parse().map_err(|e| format!("{e:?}"))?)
        .prologue(PROLOGUE)
        .local_private_key(local_private)
        .build_responder()
        .map_err(|e| e.to_string())?;
    let mut payload = [0u8; 16];
    let n = state.read_message(&msg1, &mut payload).map_err(|e| e.to_string())?;
    if n != 2 || payload[0] != 0x01 {
        return Err("unsupported parameters".into());
    }
    let binary = payload[1] & 0x01 != 0;
    let mut buf = [0u8; 256];
    let m = state.write_message(&[0x01, if binary { 0x01 } else { 0x00 }], &mut buf).map_err(|e| e.to_string())?;
    let ready = json!({ "type": "e2ee_ready", "protected": { "v": 1, "message": B64.encode(&buf[..m]) } }).to_string();
    let t = Arc::new(state.into_stateless_transport_mode().map_err(|e| e.to_string())?);
    Ok((
        ready,
        Sender { mode: Mode::Protected(t.clone()), nonce: 0, binary },
        Receiver { mode: Mode::Protected(t), nonce: 0, binary },
    ))
}

/// A fresh X25519 key pair: (private, public).
pub fn generate_key() -> ([u8; 32], [u8; 32]) {
    let kp = snow::Builder::new(PATTERN.parse().expect("pattern")).generate_keypair().expect("keypair");
    (kp.private.try_into().expect("32"), kp.public.try_into().expect("32"))
}

/// A Sophon pairing link: the LAN bridge (address, key, token) and the
/// daemon's relay offer, so a phone tries the LAN first and the relay after.
#[derive(Debug, Clone, PartialEq)]
pub struct PairingLink {
    pub lan: Option<(String, [u8; 32], String)>,
    pub relay: Option<RelayOffer>,
}

impl PairingLink {
    pub fn parse(input: &str) -> Option<Self> {
        let s = input.trim();
        let relay = RelayOffer::parse(s);
        let lan = s.strip_prefix("sophon://pair").or_else(|| s.strip_prefix("victor://pair")).and_then(|rest| {
            let query = rest.trim_start_matches('/').strip_prefix('?')?;
            let query = query.split('#').next().unwrap_or_default();
            let param = |name: &str| {
                query.split('&').find_map(|kv| {
                    let (k, v) = kv.split_once('=')?;
                    (k == name).then(|| percent_decode(v))
                })
            };
            Some((param("lan")?, key(&param("lanKey")?)?, param("token")?))
        });
        (lan.is_some() || relay.is_some()).then_some(Self { lan, relay })
    }

    pub fn format(lan: Option<(&str, &[u8; 32], &str)>, relay_link: Option<&str>) -> String {
        let mut s = "sophon://pair".to_owned();
        if let Some((addr, key, token)) = lan {
            let enc = |v: &str| v.replace('%', "%25").replace('+', "%2B").replace('/', "%2F").replace('=', "%3D");
            s.push_str(&format!("?lan={addr}&lanKey={}&token={token}", enc(&B64.encode(key))));
        }
        if let Some(fragment) = relay_link.and_then(|r| r.find("#offer=").map(|i| &r[i..])) {
            s.push_str(fragment);
        }
        s
    }
}
