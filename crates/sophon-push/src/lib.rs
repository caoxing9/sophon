//! Push notifications (`docs/push.md`): what a computer sends a paired
//! phone when an agent needs its owner, and what the phone sends back when
//! an action of the notification is pressed.
//!
//! Shared by the app and the service. The app keeps who wants
//! notifications (a phone registers over the bridge) and writes each
//! space's share of it for that space's service ([`ServiceFile`]); the
//! service watches its agents, says what happened ([`Note`]) and sends it
//! through the gateway ([`gateway`]), and takes the actions pressed.
//!
//! This file is the part free of sockets: sealing, the words, the records,
//! and checking an action.

pub mod gateway;

use std::path::{Path, PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::{Value, json};

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

/// A keypair to seal to (its secret, its public key): tests' and a new
/// gateway's.
pub fn keypair() -> ([u8; 32], [u8; 32]) {
    let secret = crypto_box::SecretKey::generate(&mut crypto_box::aead::OsRng);
    (secret.to_bytes(), *secret.public_key().as_bytes())
}

// ------------------------------------------------------------ who wants it

/// What a device wants to hear of: each kind can be turned off on the
/// phone. A device that names none (an older app) hears of all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Events {
    /// A turn finished.
    pub finished: bool,
    /// An approval is waited for.
    pub permission: bool,
    /// A turn failed, or the agent asks a question.
    pub attention: bool,
    /// The plan's limit is near.
    pub usage: bool,
}

impl Default for Events {
    fn default() -> Self {
        Self { finished: true, permission: true, attention: true, usage: true }
    }
}

impl Events {
    pub fn from_json(v: Option<&Value>) -> Self {
        let on = |k: &str| v.and_then(|v| v.get(k)).and_then(Value::as_bool).unwrap_or(true);
        Self { finished: on("finished"), permission: on("permission"), attention: on("attention"), usage: on("usage") }
    }

    pub fn to_json(self) -> Value {
        json!({ "finished": self.finished, "permission": self.permission, "attention": self.attention, "usage": self.usage })
    }

    /// Whether a note of `kind` is wanted.
    pub fn wants(self, kind: Kind) -> bool {
        match kind {
            Kind::Permission => self.permission,
            Kind::Question | Kind::Failed => self.attention,
            Kind::Finished => self.finished,
            Kind::Usage => self.usage,
        }
    }
}

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
    /// The space on this Mac it reaches (its service's home); `None`: the
    /// first.
    pub space: Option<String>,
    /// What it wants to hear of.
    pub events: Events,
}

impl Wants {
    fn from_json(d: &Value) -> Self {
        let s = |k: &str| d.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        Self {
            token: s("token"),
            env: s("env"),
            lang: s("lang"),
            hide: d.get("hide").and_then(Value::as_bool).unwrap_or(false),
            pairing: s("pairing"),
            space: d.get("space").and_then(Value::as_str).map(str::to_owned),
            events: Events::from_json(d.get("events")),
        }
    }

    fn to_json(&self) -> Value {
        json!({ "token": self.token, "env": self.env, "lang": self.lang, "hide": self.hide, "pairing": self.pairing, "space": self.space, "events": self.events.to_json() })
    }
}

/// The app's record, kept in `push.json` in its data folder: this
/// computer's id at the gateway, the gateways' keys as first seen (by the
/// app, for the phones' own use), and the devices that want notifications
/// (by the id each made for itself).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Record {
    pub id: String,
    pub gateways: Vec<(String, [u8; 32])>,
    pub devices: Vec<(String, Wants)>,
}

impl Record {
    pub fn from_json(v: &Value) -> Self {
        Self {
            id: v.get("id").and_then(Value::as_str).unwrap_or_default().to_owned(),
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
                .map(|(id, d)| (id.clone(), Wants::from_json(d)))
                .filter(|(_, w)| !w.token.is_empty())
                .collect(),
        }
    }

    pub fn to_json(&self) -> Value {
        let gateways: serde_json::Map<String, Value> = self.gateways.iter().map(|(h, k)| (h.clone(), B64.encode(k).into())).collect();
        let devices: serde_json::Map<String, Value> = self.devices.iter().map(|(id, w)| (id.clone(), w.to_json())).collect();
        json!({ "id": self.id, "gateways": gateways, "devices": devices })
    }

    /// The record at `path`, with an id made now when it has none.
    pub fn load(path: &Path) -> Self {
        let mut r = read_json(path).map(|v| Self::from_json(&v)).unwrap_or_default();
        if r.id.is_empty() {
            r.id = uuid::Uuid::new_v4().simple().to_string();
            r.save(path);
        }
        r
    }

    pub fn save(&self, path: &Path) {
        write_private(path, &self.to_json());
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

    /// `device` forgot this computer: it comes off the list, when it is the
    /// one that registered with `pairing` (no device takes another off).
    /// Whether anything changed.
    pub fn forget(&mut self, device: &str, pairing: &str) -> bool {
        let before = self.devices.len();
        self.devices.retain(|(id, w)| !(id == device && w.pairing == pairing));
        before != self.devices.len()
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

    /// The spaces some device wants notifications from (`None`: the first).
    pub fn spaces(&self) -> Vec<Option<String>> {
        let mut out: Vec<Option<String>> = Vec::new();
        for (_, w) in &self.devices {
            if !out.contains(&w.space) {
                out.push(w.space.clone());
            }
        }
        out
    }

    /// The id at the gateway of the service of `space`: the computer's own
    /// for the first, a name of its own for each other (one socket per id:
    /// two services sharing one would put each other off).
    pub fn space_id(&self, space: Option<&str>) -> String {
        match space {
            None => self.id.clone(),
            Some(home) => format!("{}-{:016x}", self.id, fnv(home.as_bytes())),
        }
    }

    /// The key to seal with for `gateway`: the one first seen. `None` when
    /// the gateway now says another, which is not trusted.
    pub fn gateway_key(&mut self, gateway: &str, said: [u8; 32]) -> Option<[u8; 32]> {
        trust(&mut self.gateways, gateway, said)
    }
}

/// `said` for `gateway` among the keys first seen: kept when new, `None`
/// when it is not the one first seen.
fn trust(known: &mut Vec<(String, [u8; 32])>, gateway: &str, said: [u8; 32]) -> Option<[u8; 32]> {
    match known.iter().find(|(h, _)| h == gateway) {
        Some((_, first)) => (*first == said).then_some(said),
        None => {
            known.push((gateway.to_owned(), said));
            Some(said)
        }
    }
}

/// FNV-1a: a short, steady name for a path.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3))
}

/// What the app tells a space's service for its notifications, in
/// `push.json` in the service's home: its id at the gateway, the gateway,
/// the devices of that space, and what an action is checked with (the
/// bridge's secret and the pairing tokens good now). Kept as the user's
/// alone (0600), as the bridge's own record is.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ServiceFile {
    pub id: String,
    /// `host:port` of the relay whose server is the gateway; empty: none.
    pub gateway: String,
    pub tls: bool,
    pub devices: Vec<Wants>,
    pub secret: Option<[u8; 32]>,
    pub tokens: Vec<String>,
}

impl ServiceFile {
    pub const NAME: &'static str = "push.json";

    pub fn from_json(v: &Value) -> Self {
        Self {
            id: v.get("id").and_then(Value::as_str).unwrap_or_default().to_owned(),
            gateway: v.get("gateway").and_then(Value::as_str).unwrap_or_default().to_owned(),
            tls: v.get("tls").and_then(Value::as_bool).unwrap_or(false),
            devices: v.get("devices").and_then(Value::as_array).into_iter().flatten().map(Wants::from_json).filter(|w| !w.token.is_empty()).collect(),
            secret: v.get("secret").and_then(Value::as_str).and_then(|k| B64.decode(k).ok()).and_then(|k| k.try_into().ok()),
            tokens: v.get("tokens").and_then(Value::as_array).into_iter().flatten().filter_map(|t| t.as_str().map(str::to_owned)).collect(),
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "gateway": self.gateway,
            "tls": self.tls,
            "devices": self.devices.iter().map(Wants::to_json).collect::<Vec<_>>(),
            "secret": self.secret.map(|k| B64.encode(k)),
            "tokens": self.tokens,
        })
    }

    pub fn path(home: &Path) -> PathBuf {
        home.join(Self::NAME)
    }

    pub fn load(home: &Path) -> Self {
        read_json(&Self::path(home)).map(|v| Self::from_json(&v)).unwrap_or_default()
    }

    /// Writes it when it says something new (or takes it away when there
    /// is nobody to tell). Whether the file changed.
    pub fn store(&self, home: &Path) -> bool {
        let path = Self::path(home);
        if self.devices.is_empty() {
            return std::fs::remove_file(&path).is_ok();
        }
        if read_json(&path).as_ref() == Some(&self.to_json()) {
            return false;
        }
        write_private(&path, &self.to_json());
        true
    }
}

/// The gateways' keys as a service first saw them, in its home
/// (`push-gateways.json`): one that says another later is not trusted.
pub fn trust_gateway(home: &Path, gateway: &str, said: [u8; 32]) -> Option<[u8; 32]> {
    let path = home.join("push-gateways.json");
    let mut known: Vec<(String, [u8; 32])> = read_json(&path)
        .and_then(|v| v.as_object().cloned())
        .into_iter()
        .flatten()
        .filter_map(|(h, k)| Some((h, B64.decode(k.as_str()?).ok()?.try_into().ok()?)))
        .collect();
    let before = known.len();
    let key = trust(&mut known, gateway, said);
    if known.len() != before {
        let map: serde_json::Map<String, Value> = known.iter().map(|(h, k)| (h.clone(), B64.encode(k).into())).collect();
        write_private(&path, &Value::Object(map));
    }
    key
}

/// The devices a service took off its list when they said they forgot
/// the computer, by their ids with the pairing token each came with
/// (`push-forgotten.json` in its home): the app takes them off its record
/// and revokes their pairings, then clears the list.
pub fn forgotten(home: &Path) -> Vec<(String, String)> {
    read_json(&home.join("push-forgotten.json"))
        .and_then(|v| v.as_array().cloned())
        .into_iter()
        .flatten()
        .filter_map(|e| Some((e.get("device")?.as_str()?.to_owned(), e.get("pairing")?.as_str()?.to_owned())))
        .collect()
}

pub fn note_forgotten(home: &Path, device: &str, pairing: &str) {
    let mut all = forgotten(home);
    all.push((device.to_owned(), pairing.to_owned()));
    let v: Vec<Value> = all.iter().map(|(d, p)| json!({ "device": d, "pairing": p })).collect();
    write_private(&home.join("push-forgotten.json"), &Value::Array(v));
}

pub fn clear_forgotten(home: &Path) {
    let _ = std::fs::remove_file(home.join("push-forgotten.json"));
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// `value` at `path`, for the user alone.
fn write_private(path: &Path, value: &Value) {
    let _ = path.parent().map(std::fs::create_dir_all);
    if std::fs::write(path, value.to_string()).is_ok() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
        }
    }
}

// ------------------------------------------------------------ what to say

/// What happened, as a notification says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// It waits for an approval.
    Permission,
    /// It asks a question.
    Question,
    /// It finished its turn.
    Finished,
    /// Its turn failed.
    Failed,
    /// The plan's limit is near.
    Usage,
}

impl Kind {
    /// The notification's category: what the phone offers on it (allow and
    /// deny on an approval; a reply on the rest; nothing on a limit).
    fn category(self) -> &'static str {
        match self {
            Kind::Permission => "PERMISSION",
            Kind::Question | Kind::Failed => "ATTENTION",
            Kind::Finished => "FINISHED",
            Kind::Usage => "USAGE",
        }
    }
}

/// An agent's news for a notification.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub kind: Kind,
    pub agent: String,
    /// The conversation's name.
    pub title: String,
    /// The project it is in.
    pub project: String,
    /// What it is about: the approval's command, the question, the reply's
    /// first words, the error.
    pub detail: String,
    /// The approval waited for, for the notification's actions.
    pub permission: Option<String>,
}

fn zh(lang: &str) -> bool {
    lang.starts_with("zh")
}

/// The words of a note's state: "Waiting for your approval", "Done"…
fn state_words(kind: Kind, zh: bool) -> &'static str {
    match (kind, zh) {
        (Kind::Permission, true) => "等待你的确认",
        (Kind::Permission, false) => "Waiting for your approval",
        (Kind::Question, true) => "在等你回答",
        (Kind::Question, false) => "Waiting for your answer",
        (Kind::Finished, true) => "已完成",
        (Kind::Finished, false) => "Done",
        (Kind::Failed, true) => "出错了",
        (Kind::Failed, false) => "Failed",
        (Kind::Usage, true) => "额度",
        (Kind::Usage, false) => "Plan limit",
    }
}

/// The push of `note` to a device that `wants` it (`docs/push.md`, the
/// sealed JSON), from the computer called `host` at the gateway. The title
/// is the conversation; the body its state, the project and what it is
/// about ("Done · sophon\nThe tests pass now…"). Hidden: the state alone.
pub fn note_push(host: &str, note: &Note, wants: &Wants) -> Value {
    let zh = zh(&wants.lang);
    let state = state_words(note.kind, zh);
    let body = if wants.hide {
        state.to_owned()
    } else {
        let head = if note.project.is_empty() { state.to_owned() } else { format!("{state} · {}", note.project) };
        match (note.kind, clip(&note.detail, 200)) {
            (_, d) if d.is_empty() => head,
            // An approval and a question say what, on the same line.
            (Kind::Permission | Kind::Question, d) => format!("{state}{}{d}{}", if zh { "：" } else { ": " }, if note.project.is_empty() { String::new() } else { format!(" · {}", note.project) }),
            (_, d) => format!("{head}\n{d}"),
        }
    };
    let title = if wants.hide || note.title.trim().is_empty() { "Sophon".to_owned() } else { clip(&note.title, 80) };
    let mut data = json!({ "host": host, "agent": note.agent });
    if let Some(p) = &note.permission {
        data["permission"] = p.clone().into();
    }
    json!({
        "token": wants.token,
        "env": wants.env,
        "title": title,
        "body": clip_lines(&body, 240),
        "category": note.kind.category(),
        "thread": note.agent,
        // A later word about the same agent takes the earlier one's place.
        "collapse": format!("agent-{}", clip(&note.agent, 48)),
        "data": data,
    })
}

/// One push for several agents that finished (or failed) at once: "3
/// agents finished", their names under it. A single one is its own.
pub fn notes_push(host: &str, notes: &[Note], wants: &Wants) -> Option<Value> {
    match notes {
        [] => None,
        [one] => Some(note_push(host, one, wants)),
        many => {
            let zh = zh(&wants.lang);
            let failed = many.iter().filter(|n| n.kind == Kind::Failed).count();
            let title = match (zh, failed) {
                (true, 0) => format!("{} 个 agent 已完成", many.len()),
                (true, _) => format!("{} 个 agent 有了结果", many.len()),
                (false, 0) => format!("{} agents finished", many.len()),
                (false, _) => format!("{} agents are done", many.len()),
            };
            let body = if wants.hide {
                if zh { "打开 Sophon 查看" } else { "Open Sophon to see them" }.to_owned()
            } else {
                many.iter()
                    .map(|n| {
                        let mark = if n.kind == Kind::Failed { if zh { "（出错）" } else { " (failed)" } } else { "" };
                        format!("{}{mark}", clip(&n.title, 60))
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Some(json!({
                "token": wants.token,
                "env": wants.env,
                "title": title,
                "body": clip_lines(&body, 240),
                "category": if failed > 0 { "ATTENTION" } else { "FINISHED" },
                "thread": "finished",
                "collapse": "finished",
                "data": { "host": host },
            }))
        }
    }
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
        "category": Kind::Usage.category(),
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

/// `text` on one line in at most `max` characters, with an ellipsis when cut.
pub fn clip(text: &str, max: usize) -> String {
    let one_line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    cut(&one_line, max)
}

/// `text` with its lines kept (each squeezed), in at most `max` characters.
fn clip_lines(text: &str, max: usize) -> String {
    let lines: Vec<String> = text.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|l| !l.is_empty()).collect();
    cut(&lines.join("\n"), max)
}

fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    text.chars().take(max.saturating_sub(1)).chain(['…']).collect()
}

/// A reply's first words for a notification: its text without Markdown's
/// marks (headings, emphasis, code fences, list bullets, links' addresses).
pub fn excerpt(markdown: &str) -> String {
    let mut out = Vec::new();
    let mut headings = Vec::new();
    let mut fence = false;
    for line in markdown.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            fence = !fence;
            continue;
        }
        if fence || t.is_empty() || t.chars().all(|c| matches!(c, '-' | '*' | '_' | '=' | '|' | ':' | ' ')) {
            continue;
        }
        // A heading ("## Done") is a label, not what was said: kept only
        // when there is nothing else.
        if t.starts_with('#') {
            headings.push(unmark(t.trim_start_matches('#').trim()));
            continue;
        }
        let t = t.trim_start_matches(['>', ' ']);
        let t = t.strip_prefix("- ").or_else(|| t.strip_prefix("* ")).unwrap_or(t);
        out.push(unmark(t));
        if out.iter().map(|l: &String| l.chars().count()).sum::<usize>() > 240 {
            break;
        }
    }
    if out.is_empty() { headings.join(" ") } else { out.join(" ") }
}

/// A line without emphasis marks and with links as their words.
fn unmark(line: &str) -> String {
    let mut s = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' | '`' => {}
            '_' if s.ends_with(' ') || s.is_empty() || chars.peek().is_none_or(|n| *n == ' ' || *n == '_') => {}
            // `[words](address)`: the words.
            ']' if chars.peek() == Some(&'(') => {
                for n in chars.by_ref() {
                    if n == ')' {
                        break;
                    }
                }
            }
            '[' => {}
            c => s.push(c),
        }
    }
    s
}

// ----------------------------------------------------------------- actions

/// What a pressed action asks of the computer.
#[derive(Clone, Debug, PartialEq)]
pub enum Act {
    Permission { agent: String, permission: String, allow: bool },
    Reply { agent: String, text: String },
    /// The device forgot this computer: its push registration (by the id
    /// it made for itself) goes, and so does its own pairing.
    Forget { device: String },
}

/// The action a device sealed to the computer (the sealed JSON of
/// `docs/push.md`), for [`check_act`].
pub fn act_json(token: &str, at: u64, act: &Act) -> Value {
    match act {
        Act::Permission { agent, permission, allow } => json!({ "token": token, "at": at, "kind": "permission", "agent": agent, "permission": permission, "allow": allow }),
        Act::Reply { agent, text } => json!({ "token": token, "at": at, "kind": "reply", "agent": agent, "text": text }),
        Act::Forget { device } => json!({ "token": token, "at": at, "kind": "forget", "device": device }),
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
        "forget" => Act::Forget { device: s("device") },
        _ => return Err("unknown action"),
    };
    let complete = match &act {
        Act::Permission { agent, permission, .. } => !agent.is_empty() && !permission.is_empty(),
        Act::Reply { agent, text } => !agent.is_empty() && !text.trim().is_empty(),
        Act::Forget { device } => !device.is_empty(),
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

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

#[cfg(test)]
mod tests;
