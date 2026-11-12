//! Push notifications (`docs/push.md`): what the computer sends a paired
//! phone when an agent needs its owner, and what the phone sends back when
//! an action of the notification is pressed.
//!
//! This file is the part free of sockets: sealing, the notifications'
//! words, the computer's record of who wants them, and checking an action.
//! [`gateway`](super::push_gateway) talks to the gateway.

use std::path::PathBuf;

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::{Value, json};

use super::model::Agent;

/// An action is refused this long after it was made (seconds).
const ACT_LIFE: u64 = 600;

// ------------------------------------------------------------------ sealing

/// `plain` for the holder of `to`'s secret key alone:
/// `base64(ephemeral public key ‖ nonce ‖ box)`.
pub fn seal(to: &[u8; 32], plain: &[u8]) -> String {
    use crypto_box::aead::{Aead, AeadCore, OsRng};
    let own = crypto_box::SecretKey::generate(&mut OsRng);
    let b = crypto_box::SalsaBox::new(&crypto_box::PublicKey::from(*to), &own);
    let nonce = crypto_box::SalsaBox::generate_nonce(&mut OsRng);
    let mut out = own.public_key().as_bytes().to_vec();
    out.extend_from_slice(&nonce);
    // Encrypting a slice in memory does not fail.
    out.extend_from_slice(&b.encrypt(&nonce, plain).unwrap_or_default());
    B64.encode(out)
}

/// What was sealed to the public key of `secret`.
pub fn open(secret: &[u8; 32], sealed: &str) -> Option<Vec<u8>> {
    use crypto_box::aead::Aead;
    let wire = B64.decode(sealed.trim()).ok()?;
    if wire.len() < 32 + 24 {
        return None;
    }
    let from: [u8; 32] = wire[..32].try_into().ok()?;
    let b = crypto_box::SalsaBox::new(&crypto_box::PublicKey::from(from), &crypto_box::SecretKey::from(*secret));
    b.decrypt(crypto_box::Nonce::from_slice(&wire[32..56]), &wire[56..]).ok()
}

// ------------------------------------------------------------ the computer

/// A device that wants notifications: its APNs token and how it reads.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Wants {
    /// APNs' device token, hex.
    pub token: String,
    /// `production` or `development`: which of Apple's servers knows it.
    pub env: String,
    /// The device's language (`zh`, `en`).
    pub lang: String,
    /// Says only that something needs the owner, not what.
    pub hide: bool,
    /// The pairing token it authenticated with: it stays on the list while
    /// that token is good.
    pub pairing: String,
    /// The space on this Mac it reaches (its Paseo home); `None`: the first.
    pub space: Option<String>,
}

/// The computer's side, kept in `push.json`: its id at the gateway, the
/// gateways' keys as first seen, and the devices that want notifications
/// (by the id each made for itself).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Record {
    pub id: String,
    pub gateways: Vec<(String, [u8; 32])>,
    pub devices: Vec<(String, Wants)>,
}

fn record_path() -> PathBuf {
    super::data_dir().join("push.json")
}

impl Record {
    pub fn from_json(v: &Value) -> Self {
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        Self {
            id: s(v, "id"),
            gateways: v
                .get("gateways")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(host, key)| Some((host.clone(), B64.decode(key.as_str()?).ok()?.try_into().ok()?)))
                .collect(),
            devices: v
                .get("devices")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .map(|(id, d)| {
                    let wants = Wants {
                        token: s(d, "token"),
                        env: s(d, "env"),
                        lang: s(d, "lang"),
                        hide: d.get("hide").and_then(Value::as_bool).unwrap_or(false),
                        pairing: s(d, "pairing"),
                        space: d.get("space").and_then(Value::as_str).map(str::to_owned),
                    };
                    (id.clone(), wants)
                })
                .filter(|(_, w)| !w.token.is_empty())
                .collect(),
        }
    }

    pub fn to_json(&self) -> Value {
        let gateways: serde_json::Map<String, Value> = self.gateways.iter().map(|(h, k)| (h.clone(), B64.encode(k).into())).collect();
        let devices: serde_json::Map<String, Value> =
            self.devices
                .iter()
                .map(|(id, w)| (id.clone(), json!({ "token": w.token, "env": w.env, "lang": w.lang, "hide": w.hide, "pairing": w.pairing, "space": w.space })))
                .collect();
        json!({ "id": self.id, "gateways": gateways, "devices": devices })
    }

    /// The record on disk, with an id made now when it has none.
    pub fn load() -> Self {
        let mut r = std::fs::read_to_string(record_path()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()).map(|v| Self::from_json(&v)).unwrap_or_default();
        if r.id.is_empty() {
            r.id = uuid::Uuid::new_v4().simple().to_string();
            r.save();
        }
        r
    }

    pub fn save(&self) {
        // Tests keep their records in memory: never the user's file.
        if cfg!(test) {
            return;
        }
        let path = record_path();
        let _ = path.parent().map(std::fs::create_dir_all);
        if std::fs::write(&path, self.to_json().to_string()).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
    }

    /// What `device` said it wants: an empty token takes it off the list.
    /// Whether anything changed.
    pub fn register(&mut self, device: &str, wants: Wants) -> bool {
        let before = self.devices.clone();
        self.devices.retain(|(id, _)| id != device);
        if !wants.token.is_empty() {
            self.devices.push((device.to_owned(), wants));
        }
        before != self.devices
    }

    /// Only the devices whose pairing token is still one of `good` stay.
    /// Whether anything changed.
    pub fn keep_only(&mut self, good: &[String]) -> bool {
        let before = self.devices.len();
        self.devices.retain(|(_, w)| good.contains(&w.pairing));
        before != self.devices.len()
    }

    /// The devices of `space` that want notifications.
    pub fn wanting(&self, space: Option<&str>) -> Vec<Wants> {
        self.devices.iter().filter(|(_, w)| w.space.as_deref() == space).map(|(_, w)| w.clone()).collect()
    }

    /// The key to seal with for `gateway`: the one first seen. `None` when
    /// the gateway now says another, which is not trusted.
    pub fn gateway_key(&mut self, gateway: &str, said: [u8; 32]) -> Option<[u8; 32]> {
        match self.gateways.iter().find(|(h, _)| h == gateway) {
            Some((_, known)) => (*known == said).then_some(said),
            None => {
                self.gateways.push((gateway.to_owned(), said));
                Some(said)
            }
        }
    }
}

// ------------------------------------------------------------ what to say

/// Why an agent is worth a notification.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Reason {
    /// It waits for an approval.
    Permission,
    /// It finished its turn.
    Finished,
    /// It needs its owner otherwise (an error, a question).
    Attention,
}

/// Why `next` is worth a notification after `before`, if it is: it began
/// to need its owner, or stopped running.
pub fn reason(before: Option<&Agent>, next: &Agent) -> Option<Reason> {
    let needed = |a: &Agent| a.requires_attention || !a.pending.is_empty();
    let newly = needed(next) && !before.is_some_and(needed);
    let asks_again = !next.pending.is_empty() && before.is_some_and(|b| b.pending.first().map(|p| &p.id) != next.pending.first().map(|p| &p.id));
    let finished = before.is_some_and(|b| b.is_running()) && !next.is_running();
    if (newly || asks_again) && !next.pending.is_empty() {
        Some(Reason::Permission)
    } else if finished {
        Some(Reason::Finished)
    } else if newly {
        Some(Reason::Attention)
    } else {
        None
    }
}

fn zh(lang: &str) -> bool {
    lang.starts_with("zh")
}

/// The push for `agent` to a device that `wants` it (`docs/push.md`, the
/// sealed JSON), from the computer called `host`.
pub fn agent_push(host: &str, agent: &Agent, why: Reason, wants: &Wants) -> Value {
    let zh = zh(&wants.lang);
    let (category, body) = match why {
        Reason::Permission => {
            let what = agent.pending.first().map(|p| p.title.clone()).unwrap_or_default();
            let body = match (wants.hide || what.is_empty(), zh) {
                (true, true) => "等待你的确认".to_owned(),
                (true, false) => "Waiting for your approval".to_owned(),
                (false, true) => format!("等待你的确认：{what}"),
                (false, false) => format!("Waiting for your approval: {what}"),
            };
            ("PERMISSION", body)
        }
        Reason::Finished => ("FINISHED", if zh { "已完成这一轮" } else { "Finished its turn" }.to_owned()),
        Reason::Attention => ("ATTENTION", if zh { "需要你看一下" } else { "Needs your attention" }.to_owned()),
    };
    let title = if wants.hide { "Sophon".to_owned() } else { agent.title.clone() };
    let mut data = json!({ "host": host, "agent": agent.id });
    if why == Reason::Permission
        && let Some(p) = agent.pending.first()
    {
        data["permission"] = p.id.clone().into();
    }
    json!({
        "token": wants.token,
        "env": wants.env,
        "title": title,
        "body": clip(&body, 240),
        "category": category,
        "thread": agent.id,
        // A later word about the same agent takes the earlier one's place.
        "collapse": format!("agent-{}", clip(&agent.id, 48)),
        "data": data,
    })
}

/// The push that the plan's limit is near: `label` (`Claude · Weekly`) at
/// `percent`, running out at `ends` when the pace says so.
pub fn usage_push(host: &str, label: &str, percent: u32, ends: Option<&str>, wants: &Wants) -> Value {
    let zh = zh(&wants.lang);
    let mut body = if zh { format!("{label} 已用 {percent}%") } else { format!("{label} is at {percent}%") };
    if let Some(ends) = ends {
        body.push_str(&if zh { format!("，按当前速度约 {ends} 用完") } else { format!(": at this pace it runs out around {ends}") });
    }
    json!({
        "token": wants.token,
        "env": wants.env,
        "title": if zh { "套餐额度" } else { "Plan limit" },
        "body": body,
        "category": "USAGE",
        "thread": "usage",
        "collapse": format!("usage-{}", clip(label, 40)),
        "data": { "host": host },
    })
}

/// The mark a limit at `percent` has reached: 95, 80, or 0 below them. A
/// push says each once as the use climbs past it.
pub fn limit_mark(percent: f64) -> u32 {
    if percent >= 95.0 {
        95
    } else if percent >= 80.0 {
        80
    } else {
        0
    }
}

/// Whether a limit now at `mark` is news after `told`, the mark known
/// before: a higher one. The first reading (`None`) is not: the app just
/// started, and what it finds was said already or is old.
pub fn limit_news(told: Option<u32>, mark: u32) -> bool {
    told.is_some_and(|told| mark > told)
}

/// `text` in at most `max` characters, with an ellipsis when cut.
fn clip(text: &str, max: usize) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= max {
        return one_line;
    }
    one_line.chars().take(max.saturating_sub(1)).chain(['…']).collect()
}

// ----------------------------------------------------------------- actions

/// What a pressed action asks of the computer.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Permission { agent: String, permission: String, allow: bool },
    Reply { agent: String, text: String },
}

/// The action a device sealed to the computer (the sealed JSON of
/// `docs/push.md`), for [`check_act`].
pub fn act_json(token: &str, at: u64, act: &Act) -> Value {
    match act {
        Act::Permission { agent, permission, allow } => json!({ "token": token, "at": at, "kind": "permission", "agent": agent, "permission": permission, "allow": allow }),
        Act::Reply { agent, text } => json!({ "token": token, "at": at, "kind": "reply", "agent": agent, "text": text }),
    }
}

/// The action in `sealed`, opened with the bridge's `secret`, when one of
/// `tokens` (the paired devices') made it within the last ten minutes of
/// `now` (seconds). The token it came with, too.
pub fn check_act(secret: &[u8; 32], sealed: &str, tokens: &[String], now: u64) -> Result<(String, Act), &'static str> {
    let plain = open(secret, sealed).ok_or("not for this computer")?;
    let v: Value = serde_json::from_slice(&plain).map_err(|_| "not an action")?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let token = s("token");
    if token.is_empty() || !tokens.contains(&token) {
        return Err("not a paired device");
    }
    let at = v.get("at").and_then(Value::as_u64).unwrap_or(0);
    if now.abs_diff(at) > ACT_LIFE {
        return Err("too old");
    }
    let act = match s("kind").as_str() {
        "permission" => Act::Permission { agent: s("agent"), permission: s("permission"), allow: v.get("allow").and_then(Value::as_bool).unwrap_or(false) },
        "reply" => Act::Reply { agent: s("agent"), text: s("text") },
        _ => return Err("unknown action"),
    };
    let complete = match &act {
        Act::Permission { agent, permission, .. } => !agent.is_empty() && !permission.is_empty(),
        Act::Reply { agent, text } => !agent.is_empty() && !text.trim().is_empty(),
    };
    if !complete {
        return Err("incomplete action");
    }
    Ok((token, act))
}

/// The gateway of a relay endpoint (`host:port`): the same server, over
/// TLS when the relay is. Paseo's own relay has none.
pub fn gateway_url(endpoint: &str, tls: bool, query: &str) -> Option<String> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.starts_with("relay.paseo.sh") {
        return None;
    }
    Some(format!("{}://{endpoint}/push?{query}", if tls { "wss" } else { "ws" }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::model::Permission;

    fn keys() -> ([u8; 32], [u8; 32]) {
        crate::daemon::relay::generate_key()
    }

    fn agent(id: &str, running: bool, pending: &[(&str, &str)], attention: bool) -> Agent {
        Agent {
            id: id.into(),
            title: "sophon · main".into(),
            status: if running { "running" } else { "idle" }.into(),
            requires_attention: attention,
            pending: pending.iter().map(|(id, title)| Permission { id: (*id).into(), title: (*title).into(), ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn what_is_sealed_opens_only_with_the_receiver_s_secret() {
        let (secret, public) = keys();
        let sealed = seal(&public, "你好 hello".as_bytes());
        assert_eq!(open(&secret, &sealed).as_deref(), Some("你好 hello".as_bytes()));
        assert_eq!(open(&keys().0, &sealed), None);
        assert_eq!(open(&secret, "short"), None);
        // Each seal is its own.
        assert_ne!(sealed, seal(&public, "你好 hello".as_bytes()));
    }

    #[test]
    fn an_agent_is_worth_a_notification_when_it_starts_to_need_its_owner() {
        let running = agent("a", true, &[], false);
        assert_eq!(reason(Some(&running), &agent("a", true, &[("p1", "npm test")], false)), Some(Reason::Permission));
        assert_eq!(reason(Some(&running), &agent("a", false, &[], false)), Some(Reason::Finished));
        assert_eq!(reason(Some(&running), &agent("a", true, &[], true)), Some(Reason::Attention));
        // Nothing new: nothing said twice.
        let asking = agent("a", true, &[("p1", "npm test")], false);
        assert_eq!(reason(Some(&asking), &asking.clone()), None);
        assert_eq!(reason(Some(&running), &running.clone()), None);
        // Another approval after the first is another notification.
        assert_eq!(reason(Some(&asking), &agent("a", true, &[("p2", "git push")], false)), Some(Reason::Permission));
        // An agent first heard of while idle did not just finish.
        assert_eq!(reason(None, &agent("a", false, &[], false)), None);
    }

    #[test]
    fn a_permission_s_push_names_it_in_the_device_s_language() {
        let asking = agent("a1", true, &[("p1", "npm  test\n--watch")], false);
        let wants = Wants { token: "ab12".into(), env: "production".into(), lang: "zh-Hans".into(), ..Default::default() };
        let p = agent_push("mac1", &asking, Reason::Permission, &wants);
        assert_eq!(p["title"], "sophon · main");
        assert_eq!(p["body"], "等待你的确认：npm test --watch");
        assert_eq!(p["category"], "PERMISSION");
        assert_eq!(p["data"], json!({ "host": "mac1", "agent": "a1", "permission": "p1" }));
        assert_eq!((p["token"].as_str(), p["env"].as_str(), p["thread"].as_str()), (Some("ab12"), Some("production"), Some("a1")));
        let en = agent_push("mac1", &asking, Reason::Finished, &Wants { lang: "en".into(), ..wants.clone() });
        assert_eq!((en["body"].as_str(), en["category"].as_str()), (Some("Finished its turn"), Some("FINISHED")));
        assert!(en["data"].get("permission").is_none());
    }

    #[test]
    fn a_hidden_push_says_nothing_of_the_work() {
        let asking = agent("a1", true, &[("p1", "rm -rf build")], false);
        let p = agent_push("mac1", &asking, Reason::Permission, &Wants { token: "t".into(), hide: true, ..Default::default() });
        assert_eq!((p["title"].as_str(), p["body"].as_str()), (Some("Sophon"), Some("Waiting for your approval")));
        // The action still finds its permission.
        assert_eq!(p["data"]["permission"], "p1");
    }

    #[test]
    fn a_long_body_is_cut() {
        let long = "x".repeat(600);
        let p = agent_push("m", &agent("a", true, &[("p", &long)], false), Reason::Permission, &Wants { token: "t".into(), ..Default::default() });
        let body = p["body"].as_str().unwrap();
        assert_eq!(body.chars().count(), 240);
        assert!(body.ends_with('…'));
    }

    #[test]
    fn the_limit_s_push_says_when_it_runs_out() {
        let wants = Wants { token: "t".into(), lang: "zh".into(), ..Default::default() };
        let p = usage_push("m", "Claude · Weekly", 80, Some("11-16 11:09"), &wants);
        assert_eq!(p["body"], "Claude · Weekly 已用 80%，按当前速度约 11-16 11:09 用完");
        assert_eq!(usage_push("m", "Codex · Weekly", 90, None, &Wants::default())["body"], "Codex · Weekly is at 90%");
    }

    #[test]
    fn a_limit_is_news_once_as_it_passes_each_mark() {
        assert_eq!((limit_mark(12.0), limit_mark(80.0), limit_mark(94.9), limit_mark(95.0), limit_mark(100.0)), (0, 80, 80, 95, 95));
        // Found high when the app starts: not said again.
        assert!(!limit_news(None, 80));
        assert!(limit_news(Some(0), 80));
        assert!(!limit_news(Some(80), 80));
        assert!(limit_news(Some(80), 95));
        // The window reset: back below, and news again later.
        assert!(!limit_news(Some(95), 0));
        assert!(limit_news(Some(0), 95));
    }

    #[test]
    fn an_action_counts_only_from_a_paired_device_and_while_fresh() {
        let (secret, public) = keys();
        let tokens = vec!["tok-phone".to_owned()];
        let act = Act::Permission { agent: "a1".into(), permission: "p1".into(), allow: true };
        let sealed = |token: &str, at: u64, act: &Act| seal(&public, act_json(token, at, act).to_string().as_bytes());
        assert_eq!(check_act(&secret, &sealed("tok-phone", 1000, &act), &tokens, 1100), Ok(("tok-phone".into(), act.clone())));
        assert_eq!(check_act(&secret, &sealed("tok-other", 1000, &act), &tokens, 1100), Err("not a paired device"));
        assert_eq!(check_act(&secret, &sealed("tok-phone", 1000, &act), &tokens, 1000 + 601), Err("too old"));
        assert_eq!(check_act(&keys().0, &sealed("tok-phone", 1000, &act), &tokens, 1000), Err("not for this computer"));
        let reply = Act::Reply { agent: "a1".into(), text: "run the tests".into() };
        assert_eq!(check_act(&secret, &sealed("tok-phone", 1000, &reply), &tokens, 1000), Ok(("tok-phone".into(), reply)));
        let empty = Act::Reply { agent: "a1".into(), text: "  ".into() };
        assert_eq!(check_act(&secret, &sealed("tok-phone", 1000, &empty), &tokens, 1000), Err("incomplete action"));
    }

    #[test]
    fn the_record_keeps_who_wants_notifications_and_the_gateway_s_first_key() {
        let mut r = Record { id: "mac1".into(), ..Default::default() };
        let wants = Wants { token: "ab".into(), env: "development".into(), lang: "zh".into(), hide: true, pairing: "tok".into(), space: Some("/s/lin".into()) };
        assert!(r.register("dev1", wants.clone()));
        assert!(!r.register("dev1", wants.clone()));
        assert_eq!(Record::from_json(&r.to_json()), r);
        // No token: no longer wanted. A device no longer paired goes too.
        assert!(r.register("dev1", Wants::default()));
        assert!(r.devices.is_empty());
        r.register("dev2", wants.clone());
        assert_eq!((r.wanting(Some("/s/lin")), r.wanting(None)), (vec![wants], vec![]));
        assert!(!r.keep_only(&["tok".to_owned()]));
        assert!(r.keep_only(&["another".to_owned()]));
        assert!(r.devices.is_empty());

        let (first, other) = ([1u8; 32], [2u8; 32]);
        assert_eq!(r.gateway_key("1.2.3.4:8443", first), Some(first));
        assert_eq!(r.gateway_key("1.2.3.4:8443", first), Some(first));
        assert_eq!(r.gateway_key("1.2.3.4:8443", other), None);
        assert_eq!(Record::from_json(&r.to_json()).gateways, vec![("1.2.3.4:8443".to_owned(), first)]);
    }

    #[test]
    fn the_gateway_is_the_relay_s_server() {
        assert_eq!(gateway_url("1.2.3.4:8443", false, "id=m").as_deref(), Some("ws://1.2.3.4:8443/push?id=m"));
        assert_eq!(gateway_url("relay.me:443", true, "to=m").as_deref(), Some("wss://relay.me:443/push?to=m"));
        assert_eq!(gateway_url("relay.paseo.sh:443", true, "id=m"), None);
        assert_eq!(gateway_url("", false, "id=m"), None);
    }
}
