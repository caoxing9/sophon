//! The sockets of push notifications (`docs/push.md`): the computer's
//! standing one at the gateway, which carries its pushes out and devices'
//! actions in, and a device's short one that delivers an action.

use std::{sync::Mutex, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use super::push::{self, Act, Record};

/// Pushes waiting for the computer's socket, and the record it reads.
struct Outbox {
    tx: mpsc::UnboundedSender<Value>,
    record: Record,
}

static OUTBOX: Mutex<Option<Outbox>> = Mutex::new(None);

/// The computer's record of who wants notifications (`push.json`), once
/// [`computer_loop`] runs.
pub fn record() -> Option<Record> {
    OUTBOX.lock().ok()?.as_ref().map(|o| o.record.clone())
}

/// Changes the record and keeps it. Whether `change` said anything changed.
pub fn update_record(change: impl FnOnce(&mut Record) -> bool) -> bool {
    let Ok(mut guard) = OUTBOX.lock() else { return false };
    let Some(outbox) = guard.as_mut() else { return false };
    let changed = change(&mut outbox.record);
    if changed {
        outbox.record.save();
    }
    changed
}

/// Sends `push` (the JSON `docs/push.md` seals) when the gateway is there;
/// one that cannot go now is not kept for later: it would come stale.
pub fn send(push: Value) {
    if let Ok(guard) = OUTBOX.lock()
        && let Some(outbox) = guard.as_ref()
    {
        let _ = outbox.tx.send(push);
    }
}

/// The gateway this computer uses: its relay's server, if it has one of
/// its own (`host:port` and whether over TLS).
pub fn target() -> Option<(String, bool)> {
    let endpoint = super::connection::relay_endpoint()?;
    let tls = super::connection::relay_use_tls().unwrap_or_else(|| endpoint.ends_with(":443"));
    push::gateway_url(&endpoint, tls, "").map(|_| (endpoint, tls))
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The computer's socket at the gateway, for as long as the app runs:
/// `on_act` hears each action a paired device sent (checked already).
/// `secret` and `tokens` are the bridge's key and its devices' tokens now.
pub async fn computer_loop(secret: impl Fn() -> Option<[u8; 32]>, tokens: impl Fn() -> Vec<String>, on_act: impl Fn(Act)) {
    let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
    if let Ok(mut guard) = OUTBOX.lock() {
        *guard = Some(Outbox { tx, record: Record::load() });
    }
    let mut wait = Duration::from_secs(5);
    loop {
        // Read each time: the relay can be set while the app runs.
        let Some((endpoint, tls)) = target() else {
            // No gateway: what was asked meanwhile is dropped.
            while rx.try_recv().is_ok() {}
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        let id = record().map(|r| r.id).unwrap_or_default();
        let Some(url) = push::gateway_url(&endpoint, tls, &format!("id={id}")) else { continue };
        match tokio_tungstenite::connect_async(&url).await {
            Ok((ws, _)) => {
                wait = Duration::from_secs(5);
                let ended = serve_computer(ws, &endpoint, &mut rx, &secret, &tokens, &on_act).await;
                log::info!("push gateway {endpoint}: {ended}");
            }
            Err(e) => log::info!("push gateway {endpoint}: {e}"),
        }
        while rx.try_recv().is_ok() {}
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(120));
    }
}

async fn serve_computer<S>(
    ws: tokio_tungstenite::WebSocketStream<S>,
    endpoint: &str,
    rx: &mut mpsc::UnboundedReceiver<Value>,
    secret: &impl Fn() -> Option<[u8; 32]>,
    tokens: &impl Fn() -> Vec<String>,
    on_act: &impl Fn(Act),
) -> String
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let (mut sink, mut stream) = ws.split();
    // The gateway says its key first.
    let hello = match tokio::time::timeout(Duration::from_secs(15), stream.next()).await {
        Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str::<Value>(t.as_str()).unwrap_or(Value::Null),
        _ => return "no hello".into(),
    };
    let said: Option<[u8; 32]> = hello.get("key").and_then(Value::as_str).and_then(|k| B64.decode(k).ok()).and_then(|k| k.try_into().ok());
    let Some(said) = said.filter(|_| hello.get("type").and_then(Value::as_str) == Some("hello")) else { return "no key in its hello".into() };
    let mut key = None;
    update_record(|r| {
        let before = r.gateways.len();
        key = r.gateway_key(endpoint, said);
        r.gateways.len() != before
    });
    let Some(key) = key else { return "its key changed: not trusted (remove it from push.json to trust the new one)".into() };
    if hello.get("apns").and_then(Value::as_bool) == Some(false) {
        log::warn!("push gateway {endpoint} has no APNs key: notifications will not arrive");
    }
    let mut next_ref: u64 = 0;
    let mut ping = tokio::time::interval(Duration::from_secs(25));
    loop {
        tokio::select! {
            push = rx.recv() => {
                let Some(push) = push else { return "closed".into() };
                next_ref += 1;
                let frame = json!({ "type": "push", "ref": next_ref, "sealed": push::seal(&key, push.to_string().as_bytes()) });
                if sink.send(Message::Text(frame.to_string().into())).await.is_err() {
                    return "lost".into();
                }
            }
            _ = ping.tick() => {
                if sink.send(Message::Text(json!({ "type": "ping" }).to_string().into())).await.is_err() {
                    return "lost".into();
                }
            }
            msg = stream.next() => {
                let text = match msg {
                    Some(Ok(Message::Text(t))) => t,
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => return "lost".into(),
                    Some(Ok(_)) => continue,
                };
                let Ok(v) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                match v.get("type").and_then(Value::as_str) {
                    Some("pushed") if v.get("ok").and_then(Value::as_bool) == Some(false) => {
                        log::warn!("push refused: {}", v.get("reason").and_then(Value::as_str).unwrap_or("?"));
                    }
                    Some("act") => {
                        let sealed = v.get("sealed").and_then(Value::as_str).unwrap_or_default();
                        let checked = secret().ok_or("no bridge").and_then(|s| push::check_act(&s, sealed, &tokens(), now()));
                        let mut answer = json!({ "type": "acted", "from": v.get("from"), "ref": v.get("ref"), "ok": checked.is_ok() });
                        match checked {
                            Ok((_, act)) => on_act(act),
                            Err(why) => {
                                log::info!("push action refused: {why}");
                                answer["reason"] = why.into();
                            }
                        }
                        if sink.send(Message::Text(answer.to_string().into())).await.is_err() {
                            return "lost".into();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// A device delivers `act` to the computer `id` through `gateway`
/// (`host:port`): sealed to the bridge's `key`, signed with the device's
/// pairing `token`. `Ok` once the computer took it.
pub async fn deliver(gateway: &str, tls: bool, id: &str, key: &[u8; 32], token: &str, act: &Act) -> Result<(), String> {
    let url = push::gateway_url(gateway, tls, &format!("to={id}")).ok_or("no gateway")?;
    let (ws, _) = tokio_tungstenite::connect_async(&url).await.map_err(|e| e.to_string())?;
    let (mut sink, mut stream) = ws.split();
    let sealed = push::seal(key, push::act_json(token, now(), act).to_string().as_bytes());
    sink.send(Message::Text(json!({ "type": "act", "ref": 1, "sealed": sealed }).to_string().into())).await.map_err(|e| e.to_string())?;
    while let Some(Ok(msg)) = stream.next().await {
        let Message::Text(t) = msg else { continue };
        let Ok(v) = serde_json::from_str::<Value>(t.as_str()) else { continue };
        if v.get("type").and_then(Value::as_str) == Some("acted") {
            let _ = sink.close().await;
            return match v.get("ok").and_then(Value::as_bool) {
                Some(true) => Ok(()),
                _ => Err(v.get("reason").and_then(Value::as_str).unwrap_or("refused").to_owned()),
            };
        }
    }
    Err("the gateway hung up".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// The least of a gateway: says hello with `key`, and hands the test
    /// each frame the computer sends and a way to send it frames.
    async fn gateway(key: [u8; 32]) -> (String, mpsc::UnboundedReceiver<Value>, mpsc::UnboundedSender<Value>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let (seen_tx, seen_rx) = mpsc::unbounded_channel();
        let (say_tx, mut say_rx) = mpsc::unbounded_channel::<Value>();
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            ws.send(Message::Text(json!({ "type": "hello", "key": B64.encode(key), "apns": true }).to_string().into())).await.unwrap();
            loop {
                tokio::select! {
                    said = say_rx.recv() => {
                        let Some(said) = said else { break };
                        ws.send(Message::Text(said.to_string().into())).await.unwrap();
                    }
                    msg = ws.next() => {
                        let Some(Ok(Message::Text(t))) = msg else { break };
                        let v: Value = serde_json::from_str(t.as_str()).unwrap();
                        if v["type"] != "ping" {
                            let _ = seen_tx.send(v);
                        }
                    }
                }
            }
        });
        (addr, seen_rx, say_tx)
    }

    #[tokio::test]
    async fn the_computer_seals_its_pushes_to_the_gateway_and_hears_actions() {
        let (gw_secret, gw_public) = crate::daemon::relay::generate_key();
        let (bridge_secret, bridge_public) = crate::daemon::relay::generate_key();
        let (addr, mut seen, say) = gateway(gw_public).await;

        let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
        *OUTBOX.lock().unwrap() = Some(Outbox { tx: tx.clone(), record: Record { id: "mac1".into(), ..Default::default() } });
        let (acts_tx, mut acts) = mpsc::unbounded_channel::<Act>();
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/push?id=mac1")).await.unwrap();
        let endpoint = addr.clone();
        let serving = tokio::spawn(async move {
            serve_computer(ws, &endpoint, &mut rx, &move || Some(bridge_secret), &|| vec!["tok".to_owned()], &move |act| {
                let _ = acts_tx.send(act);
            })
            .await
        });

        // A push goes out sealed: only the gateway reads it.
        send(json!({ "token": "ab12", "title": "t", "body": "npm test" }));
        let frame = tokio::time::timeout(Duration::from_secs(5), seen.recv()).await.unwrap().unwrap();
        assert_eq!((frame["type"].as_str(), frame["ref"].as_u64()), (Some("push"), Some(1)));
        assert!(!frame.to_string().contains("npm test"));
        let plain: Value = serde_json::from_slice(&push::open(&gw_secret, frame["sealed"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!(plain["body"], "npm test");
        // The gateway's key is kept as first seen.
        assert_eq!(record().unwrap().gateways, vec![(addr.clone(), gw_public)]);

        // An action from a paired device is heard and acknowledged.
        let act = Act::Permission { agent: "a1".into(), permission: "p1".into(), allow: true };
        let sealed = push::seal(&bridge_public, push::act_json("tok", now(), &act).to_string().as_bytes());
        say.send(json!({ "type": "act", "from": 9, "ref": 1, "sealed": sealed })).unwrap();
        assert_eq!(tokio::time::timeout(Duration::from_secs(5), acts.recv()).await.unwrap(), Some(act.clone()));
        let acted = tokio::time::timeout(Duration::from_secs(5), seen.recv()).await.unwrap().unwrap();
        assert_eq!(acted, json!({ "type": "acted", "from": 9, "ref": 1, "ok": true }));

        // One from a stranger is refused, and not heard.
        let forged = push::seal(&bridge_public, push::act_json("other", now(), &act).to_string().as_bytes());
        say.send(json!({ "type": "act", "from": 9, "ref": 2, "sealed": forged })).unwrap();
        let refused = tokio::time::timeout(Duration::from_secs(5), seen.recv()).await.unwrap().unwrap();
        assert_eq!((refused["ok"].as_bool(), refused["reason"].as_str()), (Some(false), Some("not a paired device")));
        assert!(acts.try_recv().is_err());

        serving.abort();
        *OUTBOX.lock().unwrap() = None;
    }
}
