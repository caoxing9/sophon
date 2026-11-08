//! What the connection runs on, the same code on every platform: a
//! WebSocket, timers and a clock. Natively that is tokio and tungstenite
//! on the connection's own thread; in a browser (WebAssembly) the page's
//! WebSocket and NSH's timers, on the engine's thread.

use std::{future::Future, time::Duration};

/// A WebSocket frame, as the connection and the relay's encryption see it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Message {
    Text(String),
    Binary(Vec<u8>),
    Close(Option<CloseFrame>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

/// Why the peer closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseFrame {
    pub code: u16,
    pub reason: String,
}

/// Waits `d`, or gives `None` when `f` took longer.
pub async fn timeout<F: Future>(d: Duration, f: F) -> Option<F::Output> {
    let f = std::pin::pin!(f);
    let s = std::pin::pin!(sleep(d));
    match futures_util::future::select(f, s).await {
        futures_util::future::Either::Left((out, _)) => Some(out),
        futures_util::future::Either::Right(_) => None,
    }
}

/// Ticks every `period`, the first at `start`. Late ticks do not pile up.
pub struct Interval {
    next: Instant,
    period: Duration,
}

impl Interval {
    pub async fn tick(&mut self) {
        let now = Instant::now();
        if self.next > now {
            sleep(self.next - now).await;
        }
        self.next = (self.next + self.period).max(Instant::now());
    }
}

pub fn interval_at(start: Instant, period: Duration) -> Interval {
    Interval { next: start, period }
}

pub fn interval(period: Duration) -> Interval {
    interval_at(Instant::now(), period)
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use futures_util::{
        SinkExt, StreamExt,
        stream::{SplitSink, SplitStream},
    };
    use tokio_tungstenite::{
        MaybeTlsStream, WebSocketStream,
        tungstenite::{self, protocol::CloseFrame as WsClose},
    };

    use super::{CloseFrame, Message};

    pub use std::time::Instant;
    pub use tokio::time::sleep;

    type Ws = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;

    pub struct WsTx(SplitSink<Ws, tungstenite::Message>);
    pub struct WsRx(SplitStream<Ws>);

    pub async fn connect(url: &str) -> Result<(WsTx, WsRx), String> {
        let (ws, _) = tokio_tungstenite::connect_async(url).await.map_err(|e| e.to_string())?;
        let (tx, rx) = ws.split();
        Ok((WsTx(tx), WsRx(rx)))
    }

    impl WsTx {
        pub async fn send(&mut self, m: Message) -> Result<(), String> {
            self.0.send(to_ws(m)).await.map_err(|e| e.to_string())
        }
    }

    impl WsRx {
        pub async fn next(&mut self) -> Option<Result<Message, String>> {
            self.0.next().await.map(|r| r.map(from_ws).map_err(|e| e.to_string()))
        }
    }

    /// Whether something listens at `addr` (`host:port`).
    pub async fn reachable(addr: &str, within: std::time::Duration) -> bool {
        matches!(tokio::time::timeout(within, tokio::net::TcpStream::connect(addr)).await, Ok(Ok(_)))
    }

    pub fn to_ws(m: Message) -> tungstenite::Message {
        match m {
            Message::Text(t) => tungstenite::Message::Text(t.into()),
            Message::Binary(b) => tungstenite::Message::Binary(b.into()),
            Message::Ping(b) => tungstenite::Message::Ping(b.into()),
            Message::Pong(b) => tungstenite::Message::Pong(b.into()),
            Message::Close(c) => tungstenite::Message::Close(c.map(|c| WsClose { code: c.code.into(), reason: c.reason.into() })),
        }
    }

    pub fn from_ws(m: tungstenite::Message) -> Message {
        match m {
            tungstenite::Message::Text(t) => Message::Text(t.as_str().to_owned()),
            tungstenite::Message::Binary(b) => Message::Binary(b.to_vec()),
            tungstenite::Message::Ping(b) => Message::Ping(b.to_vec()),
            tungstenite::Message::Pong(b) => Message::Pong(b.to_vec()),
            tungstenite::Message::Close(c) => {
                Message::Close(c.map(|c| CloseFrame { code: u16::from(c.code), reason: c.reason.to_string() }))
            }
            tungstenite::Message::Frame(_) => Message::Pong(vec![]),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::time::Duration;

    use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
    use wasm_bindgen::{JsCast, closure::Closure};
    use web_sys::{BinaryType, CloseEvent, Event, MessageEvent, WebSocket};

    use super::{CloseFrame, Message};

    pub use nsh_core::time::Instant;

    pub async fn sleep(d: Duration) {
        nsh_core::element::timer(d).await;
    }

    /// The page's socket and the callbacks that feed it to [`WsRx`]; the
    /// socket closes when the sending half goes.
    pub struct WsTx {
        ws: WebSocket,
        _callbacks: [Closure<dyn FnMut(web_sys::Event)>; 3],
    }

    pub struct WsRx(UnboundedReceiver<Result<Message, String>>);

    impl Drop for WsTx {
        fn drop(&mut self) {
            self.ws.set_onmessage(None);
            self.ws.set_onclose(None);
            self.ws.set_onerror(None);
            self.ws.set_onopen(None);
            let _ = self.ws.close();
        }
    }

    pub async fn connect(url: &str) -> Result<(WsTx, WsRx), String> {
        let ws = WebSocket::new(url).map_err(|e| format!("{e:?}"))?;
        ws.set_binary_type(BinaryType::Arraybuffer);
        let (tx, mut rx) = unbounded_channel::<Result<Message, String>>();
        let (opened_tx, mut opened) = unbounded_channel::<bool>();

        let on_message = {
            let tx = tx.clone();
            Closure::<dyn FnMut(Event)>::new(move |e: Event| {
                let Ok(e) = e.dyn_into::<MessageEvent>() else { return };
                let data = e.data();
                let m = if let Some(text) = data.as_string() {
                    Message::Text(text)
                } else if let Ok(buf) = data.dyn_into::<js_sys::ArrayBuffer>() {
                    Message::Binary(js_sys::Uint8Array::new(&buf).to_vec())
                } else {
                    return;
                };
                let _ = tx.send(Ok(m));
            })
        };
        let on_close = {
            let tx = tx.clone();
            let opened_tx = opened_tx.clone();
            Closure::<dyn FnMut(Event)>::new(move |e: Event| {
                let _ = opened_tx.send(false);
                let frame = e.dyn_into::<CloseEvent>().ok().map(|c| CloseFrame { code: c.code(), reason: c.reason() });
                let _ = tx.send(Ok(Message::Close(frame)));
            })
        };
        let on_open = Closure::<dyn FnMut(Event)>::new(move |_: Event| {
            let _ = opened_tx.send(true);
        });
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        let socket = WsTx { ws, _callbacks: [on_message, on_close, on_open] };
        // The browser says no more than that it failed; the close follows.
        match opened.recv().await {
            Some(true) => {}
            _ => {
                // Drain the close the failure queued.
                while rx.try_recv().is_ok() {}
                return Err("the connection failed".into());
            }
        }
        Ok((socket, WsRx(rx)))
    }

    impl WsTx {
        pub async fn send(&mut self, m: Message) -> Result<(), String> {
            let sent = match m {
                Message::Text(t) => self.ws.send_with_str(&t),
                Message::Binary(b) => self.ws.send_with_u8_array(&b),
                Message::Close(c) => match c {
                    Some(c) => self.ws.close_with_code_and_reason(c.code, &c.reason),
                    None => self.ws.close(),
                },
                // The browser answers pings itself.
                Message::Ping(_) | Message::Pong(_) => Ok(()),
            };
            sent.map_err(|e| format!("{e:?}"))
        }
    }

    impl WsRx {
        pub async fn next(&mut self) -> Option<Result<Message, String>> {
            match self.0.recv().await {
                // A close ends the stream after it.
                Some(Ok(Message::Close(c))) => {
                    self.0.close();
                    Some(Ok(Message::Close(c)))
                }
                other => other,
            }
        }
    }

    /// A page cannot open raw connections: nothing answers.
    pub async fn reachable(_addr: &str, _within: Duration) -> bool {
        false
    }
}
