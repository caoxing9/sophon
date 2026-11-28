//! A gateway that prints each push opened instead of sending it to Apple:
//! for trying the service's notifications on one machine.
//!
//! `cargo run -p sophon-push --example fake_gateway -- 127.0.0.1:18444`,
//! then name `127.0.0.1:18444` as the gateway in a service's `push.json`.

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::Message;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:18444".into());
    let (secret, public) = sophon_push::keypair();
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind");
    println!("fake gateway on {addr}");
    loop {
        let Ok((stream, _)) = listener.accept().await else { continue };
        tokio::spawn(async move {
            let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let _ = ws.send(Message::Text(json!({ "type": "hello", "key": B64.encode(public), "apns": true }).to_string().into())).await;
            println!("a computer connected");
            while let Some(Ok(Message::Text(t))) = ws.next().await {
                let v: Value = serde_json::from_str(t.as_str()).unwrap_or(Value::Null);
                match v["type"].as_str() {
                    Some("push") => {
                        let plain = v["sealed"].as_str().and_then(|s| sophon_push::open(&secret, s)).unwrap_or_default();
                        println!("PUSH {}", String::from_utf8_lossy(&plain));
                        let _ = ws.send(Message::Text(json!({ "type": "pushed", "ref": v["ref"], "ok": true }).to_string().into())).await;
                    }
                    Some("ping") => {
                        let _ = ws.send(Message::Text(json!({ "type": "pong" }).to_string().into())).await;
                    }
                    _ => {}
                }
            }
            println!("a computer left");
        });
    }
}
