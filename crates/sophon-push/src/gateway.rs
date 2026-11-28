//! The sockets of push notifications (`docs/push.md`): a computer's
//! standing one at the gateway, which carries its pushes out and devices'
//! actions in, and a device's short one that delivers an action.

use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use super::{Act, check_act, gateway_url, now, seal};

/// Where a computer's socket goes: the relay's server (`host:port`, over
/// TLS or not) and the computer's id there.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub endpoint: String,
    pub tls: bool,
    pub id: String,
}

/// A computer's socket at the gateway, for as long as it runs. Each push
/// `rx` gives (the JSON `docs/push.md` seals) goes out sealed to the
/// gateway's key; one given while there is no gateway is dropped: it would
/// come stale. Read again at each connection: `target`; `trust` keeps a
/// gateway's key as first seen and refuses another; `secret` and `tokens`
/// check an action (the bridge's key, the paired devices' tokens now), and
/// `on_act` hears each that passes, with the token it came with.
pub async fn computer_loop(
    target: impl Fn() -> Option<Target>,
    trust: impl Fn(&str, [u8; 32]) -> Option<[u8; 32]>,
    secret: impl Fn() -> Option<[u8; 32]>,
    tokens: impl Fn() -> Vec<String>,
    mut rx: mpsc::UnboundedReceiver<Value>,
    on_act: impl Fn(&str, Act),
) {
    let mut wait = Duration::from_secs(5);
    loop {
        let Some(t) = target() else {
            // No gateway: what was asked meanwhile is dropped.
            while rx.try_recv().is_ok() {}
            tokio::time::sleep(Duration::from_secs(15)).await;
            continue;
        };
        let Some(url) = gateway_url(&t.endpoint, t.tls, &format!("id={}", t.id)) else {
            while rx.try_recv().is_ok() {}
            tokio::time::sleep(Duration::from_secs(15)).await;
            continue;
        };
        match tokio::time::timeout(Duration::from_secs(15), tokio_tungstenite::connect_async(&url)).await {
            Ok(Ok((ws, _))) => {
                wait = Duration::from_secs(5);
                let ended = serve(ws, &t, &mut rx, &trust, &secret, &tokens, &on_act, &target).await;
                log::info!("push gateway {}: {ended}", t.endpoint);
            }
            Ok(Err(e)) => log::info!("push gateway {}: {e}", t.endpoint),
            Err(_) => log::info!("push gateway {}: timed out", t.endpoint),
        }
        while rx.try_recv().is_ok() {}
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(120));
    }
}

#[allow(clippy::too_many_arguments)]
async fn serve<S>(
    ws: tokio_tungstenite::WebSocketStream<S>,
    t: &Target,
    rx: &mut mpsc::UnboundedReceiver<Value>,
    trust: &impl Fn(&str, [u8; 32]) -> Option<[u8; 32]>,
    secret: &impl Fn() -> Option<[u8; 32]>,
    tokens: &impl Fn() -> Vec<String>,
    on_act: &impl Fn(&str, Act),
    target: &impl Fn() -> Option<Target>,
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
    let Some(key) = trust(&t.endpoint, said) else { return "its key changed: not trusted".into() };
    if hello.get("apns").and_then(Value::as_bool) == Some(false) {
        log::warn!("push gateway {} has no APNs key: notifications will not arrive", t.endpoint);
    }
    let mut next_ref: u64 = 0;
    let mut ping = tokio::time::interval(Duration::from_secs(25));
    loop {
        tokio::select! {
            push = rx.recv() => {
                let Some(push) = push else { return "closed".into() };
                next_ref += 1;
                let frame = json!({ "type": "push", "ref": next_ref, "sealed": seal(&key, push.to_string().as_bytes()) });
                if sink.send(Message::Text(frame.to_string().into())).await.is_err() {
                    return "lost".into();
                }
            }
            _ = ping.tick() => {
                // Another gateway, or another id, since: start again there.
                if target().as_ref() != Some(t) {
                    return "moved".into();
                }
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
                        let checked = secret().ok_or("no bridge").and_then(|s| check_act(&s, sealed, &tokens(), now()));
                        let mut answer = json!({ "type": "acted", "from": v.get("from"), "ref": v.get("ref"), "ok": checked.is_ok() });
                        match checked {
                            Ok((token, act)) => on_act(&token, act),
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
    let url = gateway_url(gateway, tls, &format!("to={id}")).ok_or("no gateway")?;
    let (ws, _) = tokio_tungstenite::connect_async(&url).await.map_err(|e| e.to_string())?;
    let (mut sink, mut stream) = ws.split();
    let sealed = seal(key, super::act_json(token, now(), act).to_string().as_bytes());
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
    use crate::{act_json, keypair, open};
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
        let (gw_secret, gw_public) = keypair();
        let (bridge_secret, bridge_public) = keypair();
        let (addr, mut seen, say) = gateway(gw_public).await;
        let target = Target { endpoint: addr.clone(), tls: false, id: "mac1".into() };
        let (tx, rx) = mpsc::unbounded_channel::<Value>();
        let (acts_tx, mut acts) = mpsc::unbounded_channel::<Act>();
        let trusted = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let kept = trusted.clone();
        let serving = tokio::spawn(async move {
            computer_loop(
                move || Some(target.clone()),
                move |gw, key| {
                    kept.lock().unwrap().push((gw.to_owned(), key));
                    Some(key)
                },
                move || Some(bridge_secret),
                || vec!["tok".to_owned()],
                rx,
                move |_, act| {
                    let _ = acts_tx.send(act);
                },
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(300)).await;

        // A push goes out sealed: only the gateway reads it.
        tx.send(json!({ "token": "ab12", "title": "t", "body": "npm test" })).unwrap();
        let frame = tokio::time::timeout(Duration::from_secs(5), seen.recv()).await.unwrap().unwrap();
        assert_eq!((frame["type"].as_str(), frame["ref"].as_u64()), (Some("push"), Some(1)));
        assert!(!frame.to_string().contains("npm test"));
        let plain: Value = serde_json::from_slice(&open(&gw_secret, frame["sealed"].as_str().unwrap()).unwrap()).unwrap();
        assert_eq!(plain["body"], "npm test");
        assert_eq!(*trusted.lock().unwrap(), vec![(addr.clone(), gw_public)]);

        // An action from a paired device is heard and acknowledged.
        let act = Act::Permission { agent: "a1".into(), permission: "p1".into(), allow: true };
        let sealed = seal(&bridge_public, act_json("tok", now(), &act).to_string().as_bytes());
        say.send(json!({ "type": "act", "from": 9, "ref": 1, "sealed": sealed })).unwrap();
        assert_eq!(tokio::time::timeout(Duration::from_secs(5), acts.recv()).await.unwrap(), Some(act.clone()));
        let acted = tokio::time::timeout(Duration::from_secs(5), seen.recv()).await.unwrap().unwrap();
        assert_eq!(acted, json!({ "type": "acted", "from": 9, "ref": 1, "ok": true }));

        // One from a stranger is refused, and not heard.
        let forged = seal(&bridge_public, act_json("other", now(), &act).to_string().as_bytes());
        say.send(json!({ "type": "act", "from": 9, "ref": 2, "sealed": forged })).unwrap();
        let refused = tokio::time::timeout(Duration::from_secs(5), seen.recv()).await.unwrap().unwrap();
        assert_eq!((refused["ok"].as_bool(), refused["reason"].as_str()), (Some(false), Some("not a paired device")));
        assert!(acts.try_recv().is_err());
        serving.abort();
    }

    /// Against a real `sophon-relay` (its `/push`), with no APNs key:
    /// `PUSH_GATEWAY=127.0.0.1:port cargo test -p sophon-push live -- --ignored`.
    /// The computer's socket and a device's action meet through it.
    #[tokio::test]
    #[ignore]
    async fn the_computer_and_a_device_meet_at_the_gateway_live() {
        let gateway = std::env::var("PUSH_GATEWAY").expect("PUSH_GATEWAY: a relay to test against");
        let (bridge_secret, bridge_public) = keypair();
        let id = uuid::Uuid::new_v4().simple().to_string();
        let target = Target { endpoint: gateway.clone(), tls: false, id: id.clone() };
        let (_tx, rx) = mpsc::unbounded_channel::<Value>();
        let (acts_tx, mut acts) = mpsc::unbounded_channel::<Act>();
        let serving = tokio::spawn(async move {
            computer_loop(move || Some(target.clone()), |_, k| Some(k), move || Some(bridge_secret), || vec!["tok".to_owned()], rx, move |_, act| {
                let _ = acts_tx.send(act);
            })
            .await
        });
        tokio::time::sleep(Duration::from_millis(500)).await;
        let act = Act::Reply { agent: "a1".into(), text: "run the tests".into() };
        deliver(&gateway, false, &id, &bridge_public, "tok", &act).await.expect("the computer took the action");
        assert_eq!(tokio::time::timeout(Duration::from_secs(5), acts.recv()).await.unwrap(), Some(act.clone()));
        assert_eq!(deliver(&gateway, false, &id, &bridge_public, "other", &act).await, Err("not a paired device".to_owned()));
        assert_eq!(deliver(&gateway, false, &uuid::Uuid::new_v4().simple().to_string(), &bridge_public, "tok", &act).await, Err("Offline".to_owned()));
        serving.abort();
    }
}
