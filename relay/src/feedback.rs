//! `/feedback` (WebSocket, JSON text frames): an issue reported from
//! Sophon by someone with no GitHub account. The relay files it in the
//! repository with its own token, so the app keeps none.
//!
//! The app sends one frame and gets one back, then the socket closes:
//!
//! ```json
//! { "type": "report", "title": "bug: …", "body": "## What happened …", "label": "bug" }
//! { "type": "filed", "ok": true, "url": "https://github.com/…/issues/12" }
//! { "type": "filed", "ok": false, "reason": "RateLimited" }
//! ```
//!
//! `label` is one of `bug`, `enhancement`, `question`. Reasons:
//! `NotConfigured` (no token here), `BadRequest`, `RateLimited`,
//! `Unreachable` (GitHub did not take it).
//!
//! Configuration (environment): `FEEDBACK_GITHUB_TOKEN` (a fine-grained
//! token with Issues: write on the repository alone), `FEEDBACK_REPO`
//! (default `caoxing9/sophon`). Without a token every report is answered
//! `NotConfigured`.

use std::{
    collections::{HashMap, VecDeque},
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

/// Reports from one address in an hour.
const PER_ADDRESS: usize = 5;
/// Reports from everyone in an hour.
const IN_ALL: usize = 60;
const HOUR: Duration = Duration::from_secs(60 * 60);
/// How long the app has to send its report.
const WAIT: Duration = Duration::from_secs(30);
const TITLE_MAX: usize = 200;
const BODY_MAX: usize = 12_000;
const LABELS: [&str; 3] = ["bug", "enhancement", "question"];

/// A report, checked.
#[derive(Debug, PartialEq)]
pub struct Report {
    pub title: String,
    pub body: String,
    pub label: String,
}

/// The report in a frame, if it is one worth filing.
pub fn report_of(frame: &str) -> Result<Report, &'static str> {
    let v: Value = serde_json::from_str(frame).map_err(|_| "BadRequest")?;
    if v.get("type").and_then(Value::as_str) != Some("report") {
        return Err("BadRequest");
    }
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().trim().to_owned();
    let (title, body, label) = (s("title"), s("body"), s("label"));
    if title.is_empty() || body.is_empty() || title.chars().count() > TITLE_MAX || body.chars().count() > BODY_MAX || !LABELS.contains(&label.as_str()) {
        return Err("BadRequest");
    }
    Ok(Report { title, body, label })
}

/// At most `limit` in any hour.
#[derive(Default)]
struct Hour(VecDeque<Instant>);

impl Hour {
    fn allow(&mut self, limit: usize, now: Instant) -> bool {
        while self.0.front().is_some_and(|t| now.duration_since(*t) >= HOUR) {
            self.0.pop_front();
        }
        if self.0.len() >= limit {
            return false;
        }
        self.0.push_back(now);
        true
    }
}

struct GitHub {
    /// `https://api.github.com`, or a test's.
    api: String,
    repo: String,
    token: String,
    client: reqwest::Client,
}

/// Where reports are filed, and how many have come.
pub struct Desk {
    github: Option<GitHub>,
    from: Mutex<HashMap<IpAddr, Hour>>,
    all: Mutex<Hour>,
}

impl Desk {
    pub fn from_env() -> Self {
        let token = std::env::var("FEEDBACK_GITHUB_TOKEN").ok().filter(|t| !t.trim().is_empty());
        let repo = std::env::var("FEEDBACK_REPO").ok().filter(|r| !r.trim().is_empty()).unwrap_or_else(|| "caoxing9/sophon".into());
        if token.is_none() {
            log::info!("feedback: no FEEDBACK_GITHUB_TOKEN: reports are answered NotConfigured");
        }
        Self::new(token.map(|t| ("https://api.github.com".to_owned(), repo, t)))
    }

    /// `github`: the API's base URL, the repository (`owner/name`) and the
    /// token; `None`: not configured.
    pub fn new(github: Option<(String, String, String)>) -> Self {
        let github = github.and_then(|(api, repo, token)| {
            let client = reqwest::Client::builder().timeout(Duration::from_secs(20)).build().ok()?;
            Some(GitHub { api: api.trim_end_matches('/').to_owned(), repo, token: token.trim().to_owned(), client })
        });
        Self { github, from: Mutex::new(HashMap::new()), all: Mutex::new(Hour::default()) }
    }

    /// Whether a report from `from` is taken now.
    fn allow(&self, from: Option<IpAddr>, now: Instant) -> bool {
        if let Some(ip) = from {
            let mut map = self.from.lock().unwrap();
            // Addresses quiet for an hour are forgotten.
            map.retain(|_, h| h.0.back().is_some_and(|t| now.duration_since(*t) < HOUR));
            if !map.entry(ip).or_default().allow(PER_ADDRESS, now) {
                return false;
            }
        }
        self.all.lock().unwrap().allow(IN_ALL, now)
    }

    /// Files `report`: the issue's page.
    async fn file(&self, report: &Report) -> Result<String, &'static str> {
        let gh = self.github.as_ref().ok_or("NotConfigured")?;
        let body = format!("{}\n\n_Filed by Sophon's relay for someone without a GitHub account._\n", report.body);
        let payload = json!({ "title": report.title, "body": body, "labels": [report.label] });
        let answer = gh
            .client
            .post(format!("{}/repos/{}/issues", gh.api, gh.repo))
            .header("authorization", format!("Bearer {}", gh.token))
            .header("accept", "application/vnd.github+json")
            .header("x-github-api-version", "2022-11-28")
            .header("user-agent", "sophon-relay")
            .header("content-type", "application/json")
            .body(payload.to_string())
            .send()
            .await
            .map_err(|e| {
                log::warn!("feedback: GitHub unreachable: {e}");
                "Unreachable"
            })?;
        let status = answer.status();
        let bytes = answer.bytes().await.map_err(|_| "Unreachable")?;
        if !status.is_success() {
            log::warn!("feedback: GitHub answered {status}: {}", String::from_utf8_lossy(&bytes).chars().take(300).collect::<String>());
            return Err("Unreachable");
        }
        let v: Value = serde_json::from_slice(&bytes).map_err(|_| "Unreachable")?;
        v.get("html_url").and_then(Value::as_str).map(str::to_owned).ok_or("Unreachable")
    }
}

/// One report on a socket from `from`: taken, filed, answered.
pub async fn serve(ws: WebSocketStream<TcpStream>, from: Option<IpAddr>, desk: Arc<Desk>) {
    let (mut sink, mut stream) = ws.split();
    let frame = loop {
        match tokio::time::timeout(WAIT, stream.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => break t.to_string(),
            Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => continue,
            _ => return,
        }
    };
    let filed = match report_of(&frame) {
        Err(why) => Err(why),
        Ok(_) if !desk.allow(from, Instant::now()) => Err("RateLimited"),
        Ok(report) => desk.file(&report).await,
    };
    let answer = match &filed {
        Ok(url) => {
            log::info!("feedback: filed {url}");
            json!({ "type": "filed", "ok": true, "url": url })
        }
        Err(why) => json!({ "type": "filed", "ok": false, "reason": why }),
    };
    let _ = sink.send(Message::Text(answer.to_string().into())).await;
    let _ = sink.close().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_is_filed_only_when_it_is_whole_and_short() {
        let frame = |title: &str, body: &str, label: &str| json!({ "type": "report", "title": title, "body": body, "label": label }).to_string();
        assert_eq!(
            report_of(&frame(" bug: it hangs ", "## What happened\n\nIt hangs", "bug")),
            Ok(Report { title: "bug: it hangs".into(), body: "## What happened\n\nIt hangs".into(), label: "bug".into() })
        );
        assert_eq!(report_of(&frame("", "x", "bug")), Err("BadRequest"));
        assert_eq!(report_of(&frame("t", "  ", "bug")), Err("BadRequest"));
        assert_eq!(report_of(&frame("t", "x", "wontfix")), Err("BadRequest"));
        assert_eq!(report_of(&frame(&"t".repeat(TITLE_MAX + 1), "x", "bug")), Err("BadRequest"));
        assert_eq!(report_of(&frame("t", &"x".repeat(BODY_MAX + 1), "bug")), Err("BadRequest"));
        assert_eq!(report_of(r#"{"type":"push"}"#), Err("BadRequest"));
        assert_eq!(report_of("not json"), Err("BadRequest"));
    }

    #[test]
    fn an_address_files_five_an_hour() {
        let desk = Desk::new(None);
        let (a, b): (IpAddr, IpAddr) = ("10.0.0.1".parse().unwrap(), "10.0.0.2".parse().unwrap());
        let now = Instant::now();
        for _ in 0..PER_ADDRESS {
            assert!(desk.allow(Some(a), now));
        }
        assert!(!desk.allow(Some(a), now), "the sixth in the hour");
        assert!(desk.allow(Some(b), now), "another address");
        assert!(desk.allow(Some(a), now + HOUR), "an hour later");
    }

    /// A stand-in for GitHub's API: one request at a time, recorded,
    /// answered `status` with `body`.
    async fn github(status: u16, body: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Vec::new()));
        let kept = seen.clone();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = Vec::new();
                let mut chunk = [0u8; 8192];
                // Head and body: until the body is as long as it says.
                loop {
                    let Ok(n) = stream.read(&mut chunk).await else { break };
                    if n == 0 {
                        break;
                    }
                    buf.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&buf).to_string();
                    if let Some(at) = text.find("\r\n\r\n") {
                        let length = text[..at]
                            .lines()
                            .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                            .unwrap_or(0);
                        if buf.len() >= at + 4 + length {
                            break;
                        }
                    }
                }
                kept.lock().unwrap().push(String::from_utf8_lossy(&buf).to_string());
                let reply = format!("HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(reply.as_bytes()).await;
            }
        });
        (base, seen)
    }

    #[tokio::test]
    async fn a_report_becomes_an_issue_with_the_relay_s_token() {
        use crate::tests::{connect, next_json, send, start_with, temp_dir};
        let (api, seen) = github(201, r#"{"html_url":"https://github.com/o/r/issues/7"}"#).await;
        let dir = temp_dir("feedback");
        let desk = Desk::new(Some((api, "o/r".into(), "secret-token".into())));
        let addr = start_with(crate::push::Gateway::new(&dir, None), desk).await;

        let mut ws = connect(&addr, "/feedback").await;
        send(&mut ws, json!({ "type": "report", "title": "bug: it hangs", "body": "## What happened\n\nIt hangs", "label": "bug" })).await;
        assert_eq!(next_json(&mut ws).await, json!({ "type": "filed", "ok": true, "url": "https://github.com/o/r/issues/7" }));
        let request = seen.lock().unwrap()[0].clone();
        assert!(request.starts_with("POST /repos/o/r/issues "), "{request}");
        assert!(request.to_ascii_lowercase().contains("authorization: bearer secret-token"));
        let body: Value = serde_json::from_str(&request[request.find("\r\n\r\n").unwrap() + 4..]).unwrap();
        assert_eq!((body["title"].as_str(), body["labels"].clone()), (Some("bug: it hangs"), json!(["bug"])));
        assert!(body["body"].as_str().unwrap().starts_with("## What happened\n\nIt hangs"));

        // Not a report: nothing goes to GitHub.
        let mut ws = connect(&addr, "/feedback").await;
        send(&mut ws, json!({ "type": "report", "title": "", "body": "x", "label": "bug" })).await;
        assert_eq!(next_json(&mut ws).await, json!({ "type": "filed", "ok": false, "reason": "BadRequest" }));
        assert_eq!(seen.lock().unwrap().len(), 1);

        // GitHub refuses: said, not hidden.
        let (api, _) = github(403, r#"{"message":"Resource not accessible"}"#).await;
        let addr = start_with(crate::push::Gateway::new(&dir, None), Desk::new(Some((api, "o/r".into(), "t".into())))).await;
        let mut ws = connect(&addr, "/feedback").await;
        send(&mut ws, json!({ "type": "report", "title": "t", "body": "b", "label": "question" })).await;
        assert_eq!(next_json(&mut ws).await["reason"], "Unreachable");

        // No token: not configured.
        let addr = start_with(crate::push::Gateway::new(&dir, None), Desk::new(None)).await;
        let mut ws = connect(&addr, "/feedback").await;
        send(&mut ws, json!({ "type": "report", "title": "t", "body": "b", "label": "bug" })).await;
        assert_eq!(next_json(&mut ws).await["reason"], "NotConfigured");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn everyone_files_sixty_an_hour() {
        let desk = Desk::new(None);
        let now = Instant::now();
        for i in 0..IN_ALL {
            let ip: IpAddr = format!("10.0.{}.{}", i / 200, i % 200 + 1).parse().unwrap();
            assert!(desk.allow(Some(ip), now));
        }
        assert!(!desk.allow(Some("10.9.9.9".parse().unwrap()), now));
    }
}
