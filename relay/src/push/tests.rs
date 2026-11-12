//! The gateway over real WebSockets on a local port, with a local HTTP/1
//! server standing in for APNs (the client's own HTTP/2 + TLS to Apple is
//! not exercised here).

use std::os::unix::fs::PermissionsExt;

use p256::{
    ecdsa::{VerifyingKey, signature::Verifier},
    pkcs8::{EncodePrivateKey, LineEnding},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

use super::*;
use crate::tests::{Ws, connect, next_close, next_json, send, silent, start, temp_dir};

const COMPUTER: &str = "computer-0123456789_A";
const OTHER: &str = "computer-9876543210_B";
const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

// ---------------------------------------------------------------- helpers

/// A request the stand-in for APNs received.
#[derive(Clone, Debug)]
struct Seen {
    line: String,
    headers: HashMap<String, String>,
    body: Value,
}

/// A local HTTP/1 server that records requests and answers as told.
struct Mock {
    base: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    answer: Arc<Mutex<(u16, String)>>,
}

impl Mock {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let answer = Arc::new(Mutex::new((200, String::new())));
        let (seen2, answer2) = (seen.clone(), answer.clone());
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let (seen, answer) = (seen2.clone(), answer2.clone());
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    loop {
                        let head = loop {
                            if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                                break at + 4;
                            }
                            let mut chunk = [0u8; 4096];
                            match stream.read(&mut chunk).await {
                                Ok(n) if n > 0 => buf.extend_from_slice(&chunk[..n]),
                                _ => return,
                            }
                        };
                        let text = String::from_utf8_lossy(&buf[..head]).into_owned();
                        let mut lines = text.split("\r\n");
                        let line = lines.next().unwrap_or_default().to_owned();
                        let headers: HashMap<String, String> = lines
                            .filter_map(|l| l.split_once(':'))
                            .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
                            .collect();
                        let length: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
                        while buf.len() < head + length {
                            let mut chunk = [0u8; 4096];
                            match stream.read(&mut chunk).await {
                                Ok(n) if n > 0 => buf.extend_from_slice(&chunk[..n]),
                                _ => return,
                            }
                        }
                        let body = serde_json::from_slice(&buf[head..head + length]).unwrap_or(Value::Null);
                        buf.drain(..head + length);
                        seen.lock().unwrap().push(Seen { line, headers, body });
                        let (status, body) = answer.lock().unwrap().clone();
                        let response = format!(
                            "HTTP/1.1 {status} Answer\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
                            body.len()
                        );
                        if stream.write_all(response.as_bytes()).await.is_err() {
                            return;
                        }
                    }
                });
            }
        });
        Self { base, seen, answer }
    }

    fn answers(&self, status: u16, body: &str) {
        *self.answer.lock().unwrap() = (status, body.to_owned());
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().unwrap().clone()
    }
}

/// A signing key as Apple gives it (a `.p8`: PKCS#8 PEM), and its public key.
fn test_p8() -> (String, VerifyingKey) {
    let key = SigningKey::from_slice(&rand::random::<[u8; 32]>()).unwrap();
    let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
    assert!(pem.starts_with("-----BEGIN PRIVATE KEY-----"));
    (pem, VerifyingKey::from(&key))
}

fn test_apns(production: &str, development: &str) -> (Apns, VerifyingKey) {
    let (pem, public) = test_p8();
    (Apns::new(&pem, "KEYID12345", "TEAMID6789", "dev.sophon.test", production, development).unwrap(), public)
}

/// Checks a provider token: its header, its claims, and its signature.
fn check_jwt(jwt: &str, public: &VerifyingKey) {
    let parts: Vec<&str> = jwt.split('.').collect();
    assert_eq!(parts.len(), 3);
    let part = |p: &str| serde_json::from_slice::<Value>(&URL_SAFE_NO_PAD.decode(p).unwrap()).unwrap();
    assert_eq!(part(parts[0]), json!({ "alg": "ES256", "kid": "KEYID12345" }));
    let claims = part(parts[1]);
    assert_eq!(claims["iss"], "TEAMID6789");
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let iat = claims["iat"].as_u64().unwrap();
    assert!(iat <= now && now - iat < 60, "iat {iat}, now {now}");
    assert_eq!(claims.as_object().unwrap().len(), 2);
    let raw = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
    assert_eq!(raw.len(), 64);
    let signature = Signature::from_slice(&raw).unwrap();
    public.verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &signature).expect("the signature verifies");
}

/// Seals as the app does: a keypair for the one message.
fn seal(to: &PublicKey, plain: &[u8]) -> String {
    let ephemeral = SecretKey::from(rand::random::<[u8; 32]>());
    let nonce: [u8; 24] = rand::random();
    let boxed = SalsaBox::new(to, &ephemeral).encrypt(&nonce.into(), plain).unwrap();
    let mut raw = ephemeral.public_key().as_bytes().to_vec();
    raw.extend_from_slice(&nonce);
    raw.extend_from_slice(&boxed);
    STANDARD.encode(raw)
}

/// A socket past its `hello`, and the key the hello gave.
async fn open_socket(addr: &str, path: &str) -> (Ws, PublicKey) {
    let mut ws = connect(addr, path).await;
    let hello = next_json(&mut ws).await;
    assert_eq!(hello["type"], "hello");
    let key = <[u8; 32]>::try_from(STANDARD.decode(hello["key"].as_str().unwrap()).unwrap().as_slice()).unwrap();
    (ws, PublicKey::from(key))
}

async fn computer(addr: &str) -> (Ws, PublicKey) {
    open_socket(addr, &format!("/push?id={COMPUTER}")).await
}

async fn gateway_with(apns: Option<Apns>) -> String {
    let dir = temp_dir("push");
    let addr = start(Gateway::new(&dir, apns)).await;
    let _ = std::fs::remove_dir_all(dir);
    addr
}

fn notification(token: &str, category: &str) -> Value {
    json!({
        "token": token,
        "env": "production",
        "title": "sophon · main",
        "body": "Waiting for your approval: npm test",
        "category": category,
        "thread": "agent-1",
        "data": { "host": COMPUTER, "agent": "agent-1", "permission": "perm-1" },
    })
}

async fn push(ws: &mut Ws, key: &PublicKey, r#ref: u64, sealed: &Value) {
    send(ws, json!({ "type": "push", "ref": r#ref, "sealed": seal(key, sealed.to_string().as_bytes()) })).await;
}

// ---------------------------------------------------------------- the key

#[test]
fn the_key_is_made_then_loaded_again() {
    let root = temp_dir("key");
    let dir = root.join("state").join("sophon-relay");
    let first = Gateway::new(&dir, None);
    let path = dir.join("push.key");
    assert_eq!(std::fs::read(&path).unwrap().len(), 32);
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    assert_eq!(STANDARD.decode(&first.key).unwrap().len(), 32);

    let second = Gateway::new(&dir, None);
    assert_eq!(first.key, second.key);
    assert_eq!(first.secret.to_bytes(), second.secret.to_bytes());
    assert_eq!(std::fs::read(&path).unwrap(), first.secret.to_bytes());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_directory_that_cannot_be_written_gives_a_key_for_the_run() {
    let root = temp_dir("nokey");
    let file = root.join("a-file");
    std::fs::write(&file, b"x").unwrap();
    // A directory cannot be made under a file.
    let dir = file.join("state");
    let first = Gateway::new(&dir, None);
    let second = Gateway::new(&dir, None);
    assert_eq!(STANDARD.decode(&first.key).unwrap().len(), 32);
    assert_ne!(first.key, second.key);
    assert!(!dir.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_damaged_key_file_is_left_alone() {
    let dir = temp_dir("badkey");
    std::fs::write(dir.join("push.key"), b"too short").unwrap();
    let gateway = Gateway::new(&dir, None);
    assert_eq!(STANDARD.decode(&gateway.key).unwrap().len(), 32);
    assert_eq!(std::fs::read(dir.join("push.key")).unwrap(), b"too short");
    let _ = std::fs::remove_dir_all(dir);
}

// ---------------------------------------------------------------- hello, ping

#[tokio::test]
async fn hello_carries_the_key_and_whether_apns_is_there() {
    let dir = temp_dir("hello");
    let gateway = Gateway::new(&dir, None);
    let key = gateway.key.clone();
    let addr = start(gateway).await;
    for path in [format!("/push?id={COMPUTER}"), format!("/push?to={COMPUTER}")] {
        let mut ws = connect(&addr, &path).await;
        assert_eq!(next_json(&mut ws).await, json!({ "type": "hello", "key": key, "apns": false }));
        send(&mut ws, json!({ "type": "ping" })).await;
        assert_eq!(next_json(&mut ws).await, json!({ "type": "pong" }));
    }
    // It is the public key of the secret key in the state directory.
    let secret = <[u8; 32]>::try_from(std::fs::read(dir.join("push.key")).unwrap().as_slice()).unwrap();
    assert_eq!(SecretKey::from(secret).public_key().as_bytes().to_vec(), STANDARD.decode(&key).unwrap());
    let _ = std::fs::remove_dir_all(dir);

    let (apns, _) = test_apns("http://127.0.0.1:9", "http://127.0.0.1:9");
    let addr = gateway_with(Some(apns)).await;
    let mut ws = connect(&addr, &format!("/push?id={COMPUTER}")).await;
    assert_eq!(next_json(&mut ws).await["apns"], true);
}

// ---------------------------------------------------------------- push

#[tokio::test]
async fn a_sealed_push_becomes_the_apns_request() {
    let (production, development) = (Mock::start().await, Mock::start().await);
    let (apns, public) = test_apns(&production.base, &development.base);
    let addr = gateway_with(Some(apns)).await;
    let (mut ws, key) = computer(&addr).await;

    // An approval, to production, replacing an earlier one.
    let mut permission = notification(TOKEN, "PERMISSION");
    permission["collapse"] = json!("agent-1-perm");
    push(&mut ws, &key, 7, &permission).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 7, "ok": true }));

    let seen = production.seen();
    assert_eq!(seen.len(), 1);
    assert!(development.seen().is_empty());
    let request = &seen[0];
    assert_eq!(request.line, format!("POST /3/device/{TOKEN} HTTP/1.1"));
    assert_eq!(request.headers["apns-topic"], "dev.sophon.test");
    assert_eq!(request.headers["apns-push-type"], "alert");
    assert_eq!(request.headers["apns-priority"], "10");
    assert_eq!(request.headers["apns-collapse-id"], "agent-1-perm");
    let jwt = request.headers["authorization"].strip_prefix("bearer ").expect("a bearer token");
    check_jwt(jwt, &public);
    assert_eq!(
        request.body,
        json!({
            "aps": {
                "alert": { "title": "sophon · main", "body": "Waiting for your approval: npm test" },
                "sound": "default",
                "category": "PERMISSION",
                "thread-id": "agent-1",
                "interruption-level": "time-sensitive",
            },
            "sophon": { "host": COMPUTER, "agent": "agent-1", "permission": "perm-1" },
        })
    );

    // A finished turn, to the sandbox: not time-sensitive, nothing to replace.
    let mut finished = notification(TOKEN, "FINISHED");
    finished["env"] = json!("development");
    finished["data"] = json!({ "host": COMPUTER, "agent": "agent-1" });
    push(&mut ws, &key, 8, &finished).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 8, "ok": true }));

    assert_eq!(production.seen().len(), 1);
    let seen = development.seen();
    assert_eq!(seen.len(), 1);
    let request = &seen[0];
    assert_eq!(request.line, format!("POST /3/device/{TOKEN} HTTP/1.1"));
    assert_eq!(request.headers["apns-topic"], "dev.sophon.test");
    assert_eq!(request.headers["apns-push-type"], "alert");
    assert!(!request.headers.contains_key("apns-collapse-id"));
    // The provider token is the one made before.
    assert_eq!(request.headers["authorization"], format!("bearer {jwt}"));
    assert_eq!(
        request.body,
        json!({
            "aps": {
                "alert": { "title": "sophon · main", "body": "Waiting for your approval: npm test" },
                "sound": "default",
                "category": "FINISHED",
                "thread-id": "agent-1",
            },
            "sophon": { "host": COMPUTER, "agent": "agent-1" },
        })
    );

    for category in ["ATTENTION", "USAGE"] {
        push(&mut ws, &key, 9, &notification(TOKEN, category)).await;
        assert_eq!(next_json(&mut ws).await["ok"], true);
        let body = production.seen().last().unwrap().body.clone();
        assert_eq!(body["aps"]["category"], category);
        assert!(body["aps"].get("interruption-level").is_none());
    }
}

#[tokio::test]
async fn the_reason_of_apns_is_the_reason_of_pushed() {
    let mock = Mock::start().await;
    // Nothing listens on the discard port: the development environment is unreachable.
    let (apns, _) = test_apns(&mock.base, "http://127.0.0.1:9");
    let addr = gateway_with(Some(apns)).await;
    let (mut ws, key) = computer(&addr).await;

    mock.answers(400, r#"{"reason":"BadDeviceToken"}"#);
    push(&mut ws, &key, 1, &notification(TOKEN, "FINISHED")).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 1, "ok": false, "reason": "BadDeviceToken" }));

    mock.answers(410, r#"{"reason":"Unregistered","timestamp":1794450000000}"#);
    push(&mut ws, &key, 2, &notification(TOKEN, "FINISHED")).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 2, "ok": false, "reason": "Unregistered" }));

    mock.answers(503, "");
    push(&mut ws, &key, 3, &notification(TOKEN, "FINISHED")).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 3, "ok": false, "reason": "Http503" }));

    let mut unreachable = notification(TOKEN, "FINISHED");
    unreachable["env"] = json!("development");
    push(&mut ws, &key, 4, &unreachable).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 4, "ok": false, "reason": "Unreachable" }));
}

#[tokio::test]
async fn without_an_apns_key_every_push_is_not_configured() {
    let addr = gateway_with(None).await;
    let (mut ws, key) = computer(&addr).await;
    push(&mut ws, &key, 7, &notification(TOKEN, "PERMISSION")).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 7, "ok": false, "reason": "NotConfigured" }));
    send(&mut ws, json!({ "type": "push", "ref": 8, "sealed": "not sealed" })).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 8, "ok": false, "reason": "NotConfigured" }));
}

#[tokio::test]
async fn what_cannot_be_opened_or_read_is_a_bad_request() {
    let mock = Mock::start().await;
    let (apns, _) = test_apns(&mock.base, &mock.base);
    let addr = gateway_with(Some(apns)).await;
    let (mut ws, key) = computer(&addr).await;
    let bad = json!({ "type": "pushed", "ref": 1, "ok": false, "reason": "BadRequest" });

    send(&mut ws, json!({ "type": "push", "ref": 1 })).await;
    assert_eq!(next_json(&mut ws).await, bad);
    send(&mut ws, json!({ "type": "push", "ref": 1, "sealed": "not base64 !" })).await;
    assert_eq!(next_json(&mut ws).await, bad);
    send(&mut ws, json!({ "type": "push", "ref": 1, "sealed": STANDARD.encode([0u8; 40]) })).await;
    assert_eq!(next_json(&mut ws).await, bad);

    // Sealed to another key.
    let another = SecretKey::from(rand::random::<[u8; 32]>()).public_key();
    push(&mut ws, &another, 1, &notification(TOKEN, "FINISHED")).await;
    assert_eq!(next_json(&mut ws).await, bad);

    // Sealed well, changed on the way.
    let mut raw = STANDARD.decode(seal(&key, notification(TOKEN, "FINISHED").to_string().as_bytes())).unwrap();
    *raw.last_mut().unwrap() ^= 1;
    send(&mut ws, json!({ "type": "push", "ref": 1, "sealed": STANDARD.encode(raw) })).await;
    assert_eq!(next_json(&mut ws).await, bad);

    // Opened, but not a notification.
    send(&mut ws, json!({ "type": "push", "ref": 1, "sealed": seal(&key, b"not json") })).await;
    assert_eq!(next_json(&mut ws).await, bad);
    push(&mut ws, &key, 1, &notification("../../3/device/abc", "FINISHED")).await;
    assert_eq!(next_json(&mut ws).await, bad);
    push(&mut ws, &key, 1, &notification("", "FINISHED")).await;
    assert_eq!(next_json(&mut ws).await, bad);
    let mut wrong_env = notification(TOKEN, "FINISHED");
    wrong_env["env"] = json!("staging");
    push(&mut ws, &key, 1, &wrong_env).await;
    assert_eq!(next_json(&mut ws).await, bad);
    let mut no_title = notification(TOKEN, "FINISHED");
    no_title.as_object_mut().unwrap().remove("title");
    push(&mut ws, &key, 1, &no_title).await;
    assert_eq!(next_json(&mut ws).await, bad);

    assert!(mock.seen().is_empty());

    // A device's socket sends no pushes.
    let (mut device, key) = open_socket(&addr, &format!("/push?to={COMPUTER}")).await;
    push(&mut device, &key, 1, &notification(TOKEN, "FINISHED")).await;
    assert!(silent(&mut device).await);
    assert!(mock.seen().is_empty());
}

// ---------------------------------------------------------------- limits

#[test]
fn a_window_lets_its_limit_through_each_minute() {
    let start = Instant::now();
    let mut window = Window::new(3);
    assert!(window.allow(start));
    assert!(window.allow(start + Duration::from_secs(10)));
    assert!(window.allow(start + Duration::from_secs(20)));
    assert!(!window.allow(start + Duration::from_secs(30)));
    assert!(!window.allow(start + Duration::from_secs(59)));
    // The first is a minute old: one more.
    assert!(window.allow(start + Duration::from_secs(60)));
    assert!(!window.allow(start + Duration::from_secs(69)));
    assert!(window.allow(start + Duration::from_secs(70)));
    assert!(window.allow(start + Duration::from_secs(200)));
}

#[tokio::test]
async fn a_socket_sends_sixty_pushes_a_minute() {
    let mock = Mock::start().await;
    let (apns, _) = test_apns(&mock.base, &mock.base);
    let addr = gateway_with(Some(apns)).await;
    let (mut ws, key) = computer(&addr).await;

    // Sixty pushes, to tokens of their own (so only the socket's limit counts).
    for i in 0..60u64 {
        push(&mut ws, &key, i, &notification(&format!("{i:064x}"), "FINISHED")).await;
    }
    push(&mut ws, &key, 60, &notification(&format!("{:064x}", 60), "FINISHED")).await;
    push(&mut ws, &key, 61, &notification(&format!("{:064x}", 61), "FINISHED")).await;
    let mut answers = HashMap::new();
    for _ in 0..62 {
        let v = next_json(&mut ws).await;
        assert_eq!(v["type"], "pushed");
        answers.insert(v["ref"].as_u64().unwrap(), v);
    }
    for i in 0..60u64 {
        assert_eq!(answers[&i]["ok"], true, "push {i}");
    }
    for i in [60, 61] {
        assert_eq!(answers[&i], json!({ "type": "pushed", "ref": i, "ok": false, "reason": "RateLimited" }));
    }
    assert_eq!(mock.seen().len(), 60);

    // Another socket has its own count.
    let (mut other, key) = open_socket(&addr, &format!("/push?id={OTHER}")).await;
    push(&mut other, &key, 1, &notification(&format!("{:064x}", 99), "FINISHED")).await;
    assert_eq!(next_json(&mut other).await, json!({ "type": "pushed", "ref": 1, "ok": true }));
}

#[tokio::test]
async fn a_device_token_receives_thirty_pushes_a_minute() {
    let mock = Mock::start().await;
    let (apns, _) = test_apns(&mock.base, &mock.base);
    let addr = gateway_with(Some(apns)).await;
    let (mut ws, key) = computer(&addr).await;

    for i in 0..30u64 {
        push(&mut ws, &key, i, &notification(TOKEN, "FINISHED")).await;
    }
    // The 31st, the same token written in capitals.
    push(&mut ws, &key, 30, &notification(&TOKEN.to_uppercase(), "FINISHED")).await;
    let mut answers = HashMap::new();
    for _ in 0..31 {
        let v = next_json(&mut ws).await;
        answers.insert(v["ref"].as_u64().unwrap(), v);
    }
    for i in 0..30u64 {
        assert_eq!(answers[&i], json!({ "type": "pushed", "ref": i, "ok": true }));
    }
    assert_eq!(answers[&30], json!({ "type": "pushed", "ref": 30, "ok": false, "reason": "RateLimited" }));
    assert_eq!(mock.seen().len(), 30);

    // The limit is the token's, whichever socket sends: another computer is refused too.
    let (mut other, other_key) = open_socket(&addr, &format!("/push?id={OTHER}")).await;
    push(&mut other, &other_key, 1, &notification(TOKEN, "FINISHED")).await;
    assert_eq!(next_json(&mut other).await["reason"], "RateLimited");
    assert_eq!(mock.seen().len(), 30);

    // Another token goes through.
    push(&mut ws, &key, 31, &notification(&format!("{:064x}", 5), "FINISHED")).await;
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pushed", "ref": 31, "ok": true }));
    assert_eq!(mock.seen().len(), 31);
}

#[tokio::test]
async fn a_frame_over_16_kb_is_refused() {
    let addr = gateway_with(None).await;
    let (mut ws, _) = computer(&addr).await;

    // Exactly 16 KB passes.
    let mut frame = json!({ "type": "ping", "pad": "" }).to_string();
    let pad = "x".repeat(MAX_FRAME - frame.len());
    frame = json!({ "type": "ping", "pad": pad }).to_string();
    assert_eq!(frame.len(), MAX_FRAME);
    ws.send(Message::Text(frame.into())).await.unwrap();
    assert_eq!(next_json(&mut ws).await, json!({ "type": "pong" }));

    // One byte more closes the socket.
    let frame = json!({ "type": "ping", "pad": format!("{pad}x") }).to_string();
    assert_eq!(frame.len(), MAX_FRAME + 1);
    ws.send(Message::Text(frame.into())).await.unwrap();
    assert_eq!(next_close(&mut ws).await, 1009);

    // So does a much larger one (past what the socket reads at all).
    let (mut device, _) = open_socket(&addr, &format!("/push?to={COMPUTER}")).await;
    let huge = json!({ "type": "act", "ref": 1, "sealed": "x".repeat(200 * 1024) }).to_string();
    let _ = device.send(Message::Text(huge.into())).await;
    let ended = loop {
        match tokio::time::timeout(Duration::from_secs(10), device.next()).await.expect("the socket ends") {
            Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break true,
            Some(Ok(Message::Text(t))) => panic!("answered an oversized frame: {t}"),
            Some(Ok(_)) => {}
        }
    };
    assert!(ended);
}

// ---------------------------------------------------------------- actions

#[tokio::test]
async fn an_action_goes_to_the_computer_and_its_answer_comes_back() {
    let addr = gateway_with(None).await;
    let (mut mac, _) = computer(&addr).await;
    let (mut phone, _) = open_socket(&addr, &format!("/push?to={COMPUTER}")).await;
    let (mut watch, _) = open_socket(&addr, &format!("/push?to={COMPUTER}")).await;

    send(&mut phone, json!({ "type": "act", "ref": 1, "sealed": "c2VhbGVkIHRvIHRoZSBjb21wdXRlcg==" })).await;
    let act = next_json(&mut mac).await;
    let from = act["from"].as_u64().expect("a socket number");
    assert_eq!(act, json!({ "type": "act", "from": from, "ref": 1, "sealed": "c2VhbGVkIHRvIHRoZSBjb21wdXRlcg==" }));

    send(&mut watch, json!({ "type": "act", "ref": 1, "sealed": "d2F0Y2g=" })).await;
    let second = next_json(&mut mac).await;
    let from_watch = second["from"].as_u64().unwrap();
    assert_ne!(from, from_watch);
    assert_eq!(second["sealed"], "d2F0Y2g=");

    // Each answer reaches the socket that asked, without `from`.
    send(&mut mac, json!({ "type": "acted", "from": from_watch, "ref": 1, "ok": false, "reason": "Expired" })).await;
    send(&mut mac, json!({ "type": "acted", "from": from, "ref": 1, "ok": true })).await;
    assert_eq!(next_json(&mut phone).await, json!({ "type": "acted", "ref": 1, "ok": true }));
    assert_eq!(next_json(&mut watch).await, json!({ "type": "acted", "ref": 1, "ok": false, "reason": "Expired" }));

    // Another computer cannot answer for this one's devices.
    let (mut stranger, _) = open_socket(&addr, &format!("/push?id={OTHER}")).await;
    send(&mut stranger, json!({ "type": "acted", "from": from, "ref": 2, "ok": true })).await;
    // Nor is an answer to a socket that is not there a trouble.
    send(&mut mac, json!({ "type": "acted", "from": 999_999, "ref": 2, "ok": true })).await;
    assert!(silent(&mut phone).await);
    assert!(silent(&mut stranger).await);
}

#[tokio::test]
async fn an_action_for_a_computer_that_is_away_is_offline() {
    let addr = gateway_with(None).await;
    let (mut phone, _) = open_socket(&addr, &format!("/push?to={COMPUTER}")).await;
    send(&mut phone, json!({ "type": "act", "ref": 5, "sealed": "eA==" })).await;
    assert_eq!(next_json(&mut phone).await, json!({ "type": "acted", "ref": 5, "ok": false, "reason": "Offline" }));

    // Another computer being there changes nothing.
    let (_other, _) = open_socket(&addr, &format!("/push?id={OTHER}")).await;
    send(&mut phone, json!({ "type": "act", "ref": 6, "sealed": "eA==" })).await;
    assert_eq!(next_json(&mut phone).await["reason"], "Offline");

    // The computer comes, then leaves.
    let (mut mac, _) = computer(&addr).await;
    send(&mut phone, json!({ "type": "act", "ref": 7, "sealed": "eA==" })).await;
    assert_eq!(next_json(&mut mac).await["ref"], 7);
    mac.close(None).await.unwrap();
    while let Some(Ok(_)) = mac.next().await {}
    tokio::time::sleep(Duration::from_millis(100)).await;
    send(&mut phone, json!({ "type": "act", "ref": 8, "sealed": "eA==" })).await;
    assert_eq!(next_json(&mut phone).await, json!({ "type": "acted", "ref": 8, "ok": false, "reason": "Offline" }));
}

#[tokio::test]
async fn a_newer_computer_socket_replaces_the_older() {
    let addr = gateway_with(None).await;
    let (mut old, _) = computer(&addr).await;
    let (mut new, _) = computer(&addr).await;
    assert_eq!(next_close(&mut old).await, 1008);

    let (mut phone, _) = open_socket(&addr, &format!("/push?to={COMPUTER}")).await;
    send(&mut phone, json!({ "type": "act", "ref": 1, "sealed": "eA==" })).await;
    let act = next_json(&mut new).await;
    assert_eq!(act["type"], "act");

    // The older one going away does not take the newer one with it.
    drop(old);
    tokio::time::sleep(Duration::from_millis(200)).await;
    send(&mut phone, json!({ "type": "act", "ref": 2, "sealed": "eA==" })).await;
    assert_eq!(next_json(&mut new).await["ref"], 2);
    send(&mut new, json!({ "type": "acted", "from": act["from"], "ref": 2, "ok": true })).await;
    assert_eq!(next_json(&mut phone).await, json!({ "type": "acted", "ref": 2, "ok": true }));
}

// ---------------------------------------------------------------- the provider token

#[test]
fn the_provider_token_is_signed_and_kept_fifty_minutes() {
    let (apns, public) = test_apns("http://127.0.0.1:9", "http://127.0.0.1:9");
    let jwt = apns.jwt();
    check_jwt(&jwt, &public);
    // Another key does not verify it.
    let (_, another) = test_p8();
    let (signed, signature) = jwt.rsplit_once('.').unwrap();
    let signature = Signature::from_slice(&URL_SAFE_NO_PAD.decode(signature).unwrap()).unwrap();
    assert!(another.verify(signed.as_bytes(), &signature).is_err());

    // Kept: the same token, made at the same moment.
    let made = apns.jwt.lock().unwrap().as_ref().unwrap().1;
    assert_eq!(apns.jwt(), jwt);
    assert_eq!(apns.jwt.lock().unwrap().as_ref().unwrap().1, made);

    // 49 minutes old: kept. 51 minutes old: made anew.
    let Some(aged) = Instant::now().checked_sub(Duration::from_secs(49 * 60)) else { return };
    apns.jwt.lock().unwrap().as_mut().unwrap().1 = aged;
    apns.jwt();
    assert_eq!(apns.jwt.lock().unwrap().as_ref().unwrap().1, aged);
    let Some(old) = Instant::now().checked_sub(Duration::from_secs(51 * 60)) else { return };
    *apns.jwt.lock().unwrap() = Some(("stale".into(), old));
    let fresh = apns.jwt();
    assert_ne!(fresh, "stale");
    check_jwt(&fresh, &public);
    assert!(apns.jwt.lock().unwrap().as_ref().unwrap().1 > old);
}

#[test]
fn a_file_that_is_not_a_p8_is_refused() {
    assert!(Apns::new("not a key", "K", "T", "topic", "http://a", "http://b").is_err());
}

// ---------------------------------------------------------------- the URL

#[test]
fn the_url_says_which_socket() {
    assert_eq!(parse(&format!("/push?id={COMPUTER}")), Ok(Wanted::Computer(COMPUTER.into())));
    assert_eq!(parse(&format!("/push?to={COMPUTER}&x=1")), Ok(Wanted::Device(COMPUTER.into())));
    assert!(parse("/push").is_err());
    assert!(parse("/push?id=").is_err());
    assert!(parse("/push?id=fifteen-chars-0").is_err());
    assert!(parse("/push?id=sixteen-chars-01").is_ok());
    assert!(parse("/push?id=has%20space-0123456789").is_err());
    assert!(parse(&format!("/push?id={COMPUTER}&to={COMPUTER}")).is_err());
}
