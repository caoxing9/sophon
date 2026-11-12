//! The WebSocket connection to the local daemon.
//!
//! It runs on tokio's threads (`nsh_core::prelude::background`), owns the
//! socket and talks to the UI over two channels: `Command`s in, `Event`s out.
//! Both channel ends are `Send`; the UI side drains events inside `spawn` and
//! writes signals there. Reconnects back off from 1 s to 30 s and resume the
//! open agent.

use std::{collections::HashMap, path::PathBuf, time::Duration};

use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde_json::{Value, json};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use super::rt::{self, Message};

use super::model::{Agent, DiffFile, Item, Project, Provider, Workspace};

/// What the UI asks the daemon to do.
#[derive(Debug, Clone)]
pub enum Command {
    /// Show an agent: load its tail and stream it live.
    Open(String),
    /// Try the service again now, not after the wait between attempts
    /// (the app just started it).
    Reconnect,
    /// `behavior`: `steer` or `interrupt` while a turn runs.
    /// `message_id`: the id the host echoes the message back with, so the
    /// copy shown at once is not shown twice.
    Send { agent_id: String, text: String, behavior: Option<&'static str>, images: Vec<ImageInput>, attachments: Vec<Value>, message_id: String },
    /// Asks the host's Sophon (its bridge) for the pictures of a message
    /// this device has none of: sent from another device, or forgotten.
    /// `cwd`: the conversation's folder, where the host looks in the
    /// agent's own record of it.
    AskPictures { message_id: String, text: String, cwd: String },
    /// Asks the host's Sophon for its session-log statistics (a phone has
    /// no logs of its own).
    HostUsage,
    /// Tells the host's Sophon this device wants push notifications
    /// (`docs/push.md`): its id, Apple's token for it (empty: no longer),
    /// which of Apple's servers knows the token, its language, and
    /// whether notifications hide what they are about.
    PushRegister { device: String, token: String, env: String, lang: String, hide: bool },
    /// Uploads a file to the host for the next message; answered by
    /// [`Event::Uploaded`] with the same `id`.
    Upload { id: String, name: String, mime: String, bytes: Vec<u8> },
    Permit {
        agent_id: String,
        permission_id: String,
        allow: bool,
        action_id: Option<String>,
        /// The tool's input with the user's answers (questions).
        updated_input: Option<Value>,
    },
    Cancel(String),
    /// Creates a workspace (the directory itself, or a new worktree cut from
    /// it) and starts an agent in it with the first prompt.
    CreateAgent(CreateAgent),
    Archive(String),
    SetModel { agent_id: String, model: String },
    SetMode { agent_id: String, mode: String },
    SetThinking { agent_id: String, thinking: Option<String> },
    Rename { agent_id: String, title: String },
    ArchiveWorkspace(String),
    PinWorkspace { workspace_id: String, pinned: bool },
    RenameWorkspace { workspace_id: String, title: String },
    ClearAttention(String),
    ListCommands(String),
    /// Files and folders matching `query` under `cwd` (@ mentions).
    Suggest { cwd: String, query: String },
    /// Folders on the host matching `query` (a path, from the home folder).
    FindDirs { query: String },
    /// The page of history before `cursor`.
    LoadOlder { agent_id: String, cursor: Value },
    /// Streams the uncommitted changes of `cwd` (one checkout at a time);
    /// `None` stops.
    WatchDiff(Option<String>),
    /// Follows the terminals of a workspace (its list, live).
    WatchTerminals { cwd: String, workspace_id: String },
    CreateTerminal { cwd: String, workspace_id: String },
    /// Streams a terminal's screen: a full replay, then live output.
    AttachTerminal(String),
    DetachTerminal(String),
    TerminalInput { terminal_id: String, data: String },
    TerminalResize { terminal_id: String, rows: u16, cols: u16 },
    KillTerminal(String),
    ListDir { cwd: String, path: String },
    ReadFile { cwd: String, path: String },
    /// The whole file, any size, as bytes (for saving it here): answered by
    /// [`Event::Downloaded`] with the same `id`.
    Download { id: String, cwd: String, path: String },
    WriteFile { cwd: String, path: String, content: String, modified_at: String, revision: Option<String> },
    /// Branch, dirty state, ahead / behind, and the pull request.
    GitStatus(String),
    Git { cwd: String, action: GitAction },
    /// Sessions, archived included, newest first; `search` filters them.
    History { search: String },
    /// Brings an archived agent back.
    Unarchive(String),
    /// Connects to another daemon: `None` is this machine's.
    SwitchHost(Option<RemoteHost>),
    Schedules,
    MarkUnread(String),
    UndoArchive(String),
    Commits(String),
    Subagents(String),
    CommitFileDiff { cwd: String, sha: String, path: String },
    Usage,
    ReadProjectConfig(String),
    WriteProjectConfig { root: String, config: Value, revision: Value },
    /// `op`: `create-file`, `create-dir`, `rename`, `duplicate`, `delete`.
    FileOp { cwd: String, path: String, name: String, op: &'static str },
    ProviderFeatures { provider: String, cwd: String, model: Option<String>, mode: Option<String> },
    SetFeature { agent: String, id: String, value: Value },
    GetConfig,
    PairingOffer,
    Labels,
    /// Assigns (or removes) a label, creating it when new: (name, color).
    AssignLabel { workspace: String, name: String, color: String, assigned: bool },
    UpdateDaemon,
    RecoverWorkspace(String),
    /// A partial config, merged by the daemon.
    SetConfig(Value),
    /// A config patch of providers (`providers`, `removeProviders`): the
    /// provider list is refreshed after it.
    PatchProviders(Value),
    /// Asks every provider again what it offers.
    RefreshProviders,
    /// What the provider's binary, version and login are.
    ProviderDiagnostic(String),
    RestartDaemon,
    RecentSessions { query: String },
    ImportSession { provider: String, handle: String, cwd: String },
    Script { workspace: String, name: String, start: bool },
    Rewind { agent: String, message_id: String, mode: &'static str },
    Fork(String),
    SubagentTimeline { parent: String, id: String },
    BackgroundWork(String),
    StopBackground { agent: String, item: String, token: String },
    Branches { cwd: String, query: String },
    AddProject(String),
    CreateProjectDir { parent: String, name: String },
    CloneProject { repo: String, target: String },
    RenameProject { id: String, name: String },
    RemoveProject(String),
    /// `op`: `pause`, `resume`, `delete` or `run-once`.
    ScheduleOp { id: String, op: &'static str },
    CreateSchedule(NewSchedule),
    /// Changes a schedule: its name, prompt, cadence, folder and isolation.
    UpdateSchedule { id: String, to: NewSchedule },
    /// Asks for a schedule's runs.
    ScheduleRuns(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewSchedule {
    pub name: String,
    pub prompt: String,
    /// A cron expression (`0 9 * * 1-5`), or `every:<ms>`.
    pub cadence: String,
    /// The zone a cron expression's times are in; the service's is UTC.
    pub timezone: Option<String>,
    pub cwd: String,
    pub provider: String,
    pub model: Option<String>,
    pub worktree: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Schedule {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub cadence: String,
    /// `active` | `paused` | `completed`.
    pub status: String,
    pub next_run: Option<String>,
    pub last_run: Option<String>,
    pub target: String,
    /// The cadence as the form takes it: a cron expression, or `every:<ms>`.
    pub expression: String,
    /// The name as given; empty when it has none.
    pub given_name: String,
    /// Where a new agent starts, when the schedule starts one.
    pub cwd: String,
    pub worktree: bool,
}

/// One time a schedule ran.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScheduleRun {
    pub id: String,
    pub started: String,
    pub ended: Option<String>,
    /// `running` | `succeeded` | `failed`.
    pub status: String,
    pub agent_id: Option<String>,
    /// The workspace the run's agent worked in; archived when the run ends.
    pub workspace_id: Option<String>,
    /// What the agent answered last, when the run ended with an answer.
    pub output: Option<String>,
    pub error: Option<String>,
}

impl ScheduleRun {
    fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_owned);
        Some(Self {
            id: s("id")?,
            started: s("startedAt").or_else(|| s("scheduledFor")).unwrap_or_default(),
            ended: s("endedAt"),
            status: s("status").unwrap_or_default(),
            agent_id: s("agentId"),
            workspace_id: s("workspaceId"),
            output: s("output").map(|o| o.trim().to_owned()).filter(|o| !o.is_empty()),
            error: s("error"),
        })
    }
}

/// A cadence as the service takes it.
fn cadence_json(cadence: &str, timezone: Option<&str>) -> Value {
    match cadence.strip_prefix("every:").and_then(|ms| ms.parse::<u64>().ok()) {
        Some(ms) => json!({ "type": "every", "everyMs": ms }),
        None => {
            let mut cron = json!({ "type": "cron", "expression": cadence });
            if let Some(zone) = timezone {
                cron["timezone"] = json!(zone);
            }
            cron
        }
    }
}

impl Schedule {
    fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        let cadence = match v.pointer("/cadence/type").and_then(Value::as_str) {
            Some("every") => {
                let ms = v.pointer("/cadence/everyMs").and_then(Value::as_u64).unwrap_or(0);
                crate::cadence::interval_words(ms, crate::i18n::is_zh())
            }
            _ => crate::cadence::describe(v.pointer("/cadence/expression").and_then(Value::as_str).unwrap_or_default()),
        };
        let target = match v.pointer("/target/type").and_then(Value::as_str) {
            Some("new-agent") => {
                let cwd = v.pointer("/target/config/cwd").and_then(Value::as_str).unwrap_or_default();
                let provider = v.pointer("/target/config/provider").and_then(Value::as_str).unwrap_or_default();
                format!("New {provider} agent in {}", super::model::folder_name(cwd))
            }
            _ => "An existing agent".to_owned(),
        };
        Some(Self {
            id: s("id")?,
            name: s("name").filter(|n| !n.is_empty()).unwrap_or_else(|| "Untitled schedule".into()),
            prompt: s("prompt").unwrap_or_default(),
            cadence,
            expression: match v.pointer("/cadence/type").and_then(Value::as_str) {
                Some("every") => format!("every:{}", v.pointer("/cadence/everyMs").and_then(Value::as_u64).unwrap_or(0)),
                _ => v.pointer("/cadence/expression").and_then(Value::as_str).unwrap_or_default().to_owned(),
            },
            given_name: s("name").unwrap_or_default(),
            cwd: v.pointer("/target/config/cwd").and_then(Value::as_str).unwrap_or_default().to_owned(),
            worktree: v.pointer("/target/config/isolation").and_then(Value::as_str) == Some("worktree"),
            status: s("status").unwrap_or_default(),
            next_run: s("nextRunAt"),
            last_run: s("lastRunAt"),
            target,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GitAction {
    /// Stages everything and commits; an empty message lets the daemon write one.
    Commit(String),
    SwitchBranch(String),
    Stash,
    PopStash,
    Push,
    Pull,
    CreatePr,
    MergeIntoBase,
    /// Merges the pull request: `merge`, `squash` or `rebase`.
    MergePr(&'static str),
}

impl GitAction {
    pub fn label(&self) -> &'static str {
        match self {
            GitAction::Commit(_) => "Commit",
            GitAction::SwitchBranch(_) => "Switch branch",
            GitAction::Stash => "Stash",
            GitAction::PopStash => "Pop stash",
            GitAction::Push => "Push",
            GitAction::Pull => "Pull",
            GitAction::CreatePr => "Create pull request",
            GitAction::MergeIntoBase => "Merge into base",
            GitAction::MergePr(_) => "Merge pull request",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct GitStatus {
    pub is_git: bool,
    pub branch: Option<String>,
    pub dirty: bool,
    pub base_ref: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub has_remote: bool,
    pub managed_worktree: bool,
}

/// A provider's plan usage and its limit windows.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProviderUsage {
    pub id: String,
    pub name: String,
    pub plan: Option<String>,
    pub error: Option<String>,
    pub windows: Vec<UsageWindow>,
}

/// A limit window: what it is called, how much is used, when it resets.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UsageWindow {
    pub id: String,
    pub label: String,
    pub used: Option<f64>,
    pub resets_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecentSession {
    pub provider: String,
    pub provider_label: String,
    pub handle: String,
    pub cwd: String,
    pub title: String,
    pub last_activity: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Subagent {
    pub id: String,
    pub parent: String,
    pub title: String,
    pub subtitle: String,
    /// `running` | `completed` | `failed` | `canceled`.
    pub status: String,
}

impl Subagent {
    fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
        Some(Self {
            id: s("id")?,
            parent: s("parentAgentId").unwrap_or_default(),
            title: s("title").or_else(|| s("description")).unwrap_or_else(|| "Sub-agent".into()),
            subtitle: s("subtitle").or_else(|| s("description")).unwrap_or_default(),
            status: s("status").unwrap_or_default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BackgroundItem {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub stop_token: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Commit {
    pub sha: String,
    pub short: String,
    pub subject: String,
    pub author: String,
    pub date: String,
    pub pushed: bool,
    /// (path, additions, deletions)
    pub files: Vec<(String, u64, u64)>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PullRequest {
    pub number: Option<u64>,
    pub url: String,
    pub title: String,
    pub state: String,
    pub merged: bool,
    pub draft: bool,
    /// (name, status) of each check.
    pub checks: Vec<(String, String)>,
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq)]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
}

/// A file's content, as the daemon read it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileContent {
    pub path: String,
    /// `text` | `image` | `binary`.
    pub kind: String,
    pub text: Option<String>,
    pub modified_at: String,
    pub revision: Option<String>,
    pub size: u64,
}

/// A terminal the daemon runs for a workspace.
#[derive(Debug, Clone, PartialEq)]
pub struct TerminalInfo {
    pub id: String,
    pub name: String,
    pub title: Option<String>,
}

impl TerminalInfo {
    fn from_json(v: &Value) -> Option<Self> {
        Some(Self {
            id: v.get("id")?.as_str()?.to_owned(),
            name: v.get("name").and_then(Value::as_str).unwrap_or("Terminal").to_owned(),
            title: v.get("title").and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_owned),
        })
    }
}

/// A host switch the app asked for, taken by the connection however far its
/// current attempt has gone: a connect, a handshake, or the retries of a LAN
/// address that does not answer, which read no commands meanwhile (a host
/// out of reach held the switch away from it until it answered).
static SWITCH: std::sync::Mutex<Option<Option<RemoteHost>>> = std::sync::Mutex::new(None);

fn switch_signal() -> &'static tokio::sync::Notify {
    static SIGNAL: std::sync::OnceLock<tokio::sync::Notify> = std::sync::OnceLock::new();
    SIGNAL.get_or_init(tokio::sync::Notify::new)
}

/// A line in `connection.log` (in the app's data folder): each attempt,
/// connection and loss with its reason, for when the app sits on
/// "Connecting…". Kept under 256 KB (the older half goes).
fn trace(line: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::io::Write;
        let path = super::data_dir().join("connection.log");
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 256 * 1024) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let half = text.len() / 2;
                let cut = text[half..].find('\n').map_or(half, |i| half + i + 1);
                let _ = std::fs::write(&path, &text[cut..]);
            }
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            let _ = writeln!(f, "{now} {}", redact(line));
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = line;
}

/// `line` without a pairing link's or an address's secrets: the value of
/// each `lanKey=`, `token=`, `offer=` and `password=` becomes `***`.
fn redact(line: &str) -> String {
    let mut out = line.to_owned();
    for key in ["lanKey=", "token=", "offer=", "password="] {
        let mut from = 0;
        while let Some(i) = out[from..].find(key) {
            let start = from + i + key.len();
            let end = out[start..].find(|c: char| c == '&' || c == '/' || c.is_whitespace() || c == ';').map_or(out.len(), |j| start + j);
            out.replace_range(start..end, "***");
            from = start + 3;
        }
    }
    out
}

/// Moves the connection to `next` (`None`: this computer's service) now,
/// ending the session or attempt under way.
pub fn switch_host(next: Option<RemoteHost>) {
    trace(&format!("switch to {}", next.as_ref().map_or("this computer", |_| "a saved host")));
    // At once: a conversation opened before the task turns reads this host's copy.
    super::cache::set_host(super::cache::host_key(next.as_ref().map(|r| r.url.as_str())));
    *SWITCH.lock().unwrap_or_else(|e| e.into_inner()) = Some(next);
    switch_signal().notify_one();
}

fn take_switch() -> Option<Option<RemoteHost>> {
    SWITCH.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// A daemon on another machine, reached directly (LAN, Tailscale, a tunnel).
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteHost {
    /// `ws://host:port/ws`, `wss://…`; a bare `host:port` gets `ws://…/ws`.
    pub url: String,
    pub password: Option<String>,
}

impl RemoteHost {
    pub fn ws_url(&self) -> String {
        let u = self.url.trim();
        if u.starts_with("ws://") || u.starts_with("wss://") {
            u.to_owned()
        } else if let Some(rest) = u.strip_prefix("https://") {
            format!("wss://{}/ws", rest.trim_end_matches('/'))
        } else if let Some(rest) = u.strip_prefix("http://") {
            format!("ws://{}/ws", rest.trim_end_matches('/'))
        } else {
            format!("ws://{}/ws", u.trim_end_matches('/'))
        }
    }
}

/// A slash command or skill an agent offers.
#[derive(Debug, Clone, PartialEq)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub hint: String,
    pub skill: bool,
}

/// An image sent with a message: base64 data and its media type.
#[derive(Debug, Clone, PartialEq)]
pub struct ImageInput {
    pub name: String,
    pub mime: String,
    pub data: String,
}

fn images_json(images: &[ImageInput]) -> Value {
    Value::Array(images.iter().map(|i| json!({ "data": i.data, "mimeType": i.mime })).collect())
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateAgent {
    /// An existing workspace to start in; otherwise `path` makes one.
    pub workspace_id: Option<String>,
    pub path: String,
    pub worktree: bool,
    /// Worktrees: the branch to start from, and the new branch's name.
    pub base_branch: Option<String>,
    pub branch_name: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub thinking: Option<String>,
    pub prompt: String,
    pub images: Vec<ImageInput>,
    /// Files uploaded to the host ([`Command::Upload`]).
    pub attachments: Vec<Value>,
    /// Context carried over (a fork's chat history), as a text attachment.
    pub context: Option<Value>,
    pub features: serde_json::Map<String, Value>,
}

/// What the daemon told us, decoded for the UI.
#[derive(Debug, Clone)]
pub enum Event {
    Connecting { url: String },
    Connected { hostname: String, version: String },
    Disconnected { reason: String },
    ProjectsReset(Vec<Project>),
    ProjectUpsert(Project),
    ProjectRemove(String),
    WorkspacesReset(Vec<Workspace>),
    WorkspaceUpsert(Workspace),
    WorkspaceRemove(String),
    Providers(Vec<Provider>),
    /// The agent a `CreateAgent` started.
    Created(Agent),
    CreateFailed(String),
    AgentsReset(Vec<Agent>),
    AgentUpsert(Agent),
    AgentRemove(String),
    /// `older`: the cursor of the page before these items, when there is one.
    /// `cursor`: the epoch and last seq the page covers, so live events it
    /// already holds are not applied again.
    TimelineReset { agent_id: String, items: Vec<Item>, older: Option<Value>, cursor: Option<(String, u64)> },
    TimelineOlder { agent_id: String, items: Vec<Item>, older: Option<Value> },
    /// The conversation shown from this device's copy has everything the
    /// host has now (what came since arrived as `TimelineLive`).
    TimelineSynced { agent_id: String },
    /// The host's session-log statistics as its Sophon read them
    /// (`HostUsage`); `None` when this session cannot ask for them.
    HostUsage(Option<Value>),
    /// The pictures the host's Sophon kept for a message (`AskPictures`).
    Pictures { message_id: String, text: String, pictures: Vec<String> },
    /// Where the host takes a notification's actions (`PushRegister`'s
    /// answer): its gateway (empty: it has none) and its id there.
    PushInfo { gateway: String, tls: bool, id: String },
    Commands { agent_id: String, list: Vec<SlashCommand> },
    Diff { cwd: String, files: Vec<DiffFile>, error: Option<String>, too_large: bool },
    Terminals { cwd: String, list: Vec<TerminalInfo> },
    TerminalCreated(TerminalInfo),
    /// Output for a terminal; `reset` clears it first (a replay).
    TerminalOutput { terminal_id: String, data: String, reset: bool },
    TerminalExit(String),
    Dir { cwd: String, path: String, entries: Vec<DirEntry>, error: Option<String> },
    File { cwd: String, path: String, file: Option<FileContent>, error: Option<String> },
    /// A save finished: the new version, or why not (`conflict`: the file
    /// changed on disk since it was read).
    FileSaved { path: String, modified_at: Option<String>, revision: Option<String>, error: Option<String> },
    GitStatus { cwd: String, status: GitStatus },
    PullRequest { cwd: String, pr: Option<PullRequest> },
    /// A git action finished: `url` is a created pull request's.
    GitDone { cwd: String, action: &'static str, error: Option<String>, url: Option<String> },
    History { search: String, agents: Vec<Agent> },
    Schedules(Vec<Schedule>),
    ScheduleRuns { id: String, runs: Vec<ScheduleRun> },
    Commits { cwd: String, commits: Vec<Commit>, error: Option<String> },
    Subagents { parent: String, list: Vec<Subagent> },
    Rewound { agent: String, error: Option<String> },
    RecentSessions(Vec<RecentSession>),
    DaemonConfig(Value),
    /// A provider's diagnostic text.
    Diagnostic { provider: String, text: String },
    Usage(Vec<ProviderUsage>),
    /// A link a phone pairs with: relay, daemon id and key.
    PairingOffer { url: Option<String>, relay_enabled: bool, error: Option<String> },
    /// A file of [`Command::Upload`] is on the host: the attachment that
    /// names it in a message, or why it is not.
    Uploaded { id: String, result: Result<Value, String> },
    /// A [`Command::Download`]'s file, or why it could not be read.
    Downloaded { id: String, result: Result<Vec<u8>, String> },
    /// The label catalog: (name, color).
    Labels(Vec<(String, String)>),
    CommitFileDiff { sha: String, path: String, file: Option<DiffFile>, error: Option<String> },
    /// A project's `paseo.json`: (config, revision) or the error.
    ProjectConfig { root: String, result: Result<(Value, Value), String>, saved: bool },
    DraftFeatures { provider: String, features: Vec<super::model::Feature> },
    /// A file operation finished: the directory to list again.
    FileOpDone { dir: String, error: Option<String> },
    Forked { agent: String, context: Option<Value>, error: Option<String> },
    SubagentUpsert(Subagent),
    SubagentTimeline { parent: String, id: String, items: Vec<Item> },
    BackgroundWork { agent: String, items: Vec<BackgroundItem> },
    Branches { cwd: String, list: Vec<String> },
    Suggestions { query: String, entries: Vec<(String, bool)> },
    /// `cursor`: the event's epoch and seq; the host can deliver one more
    /// than once (a subscription and its implicit delivery, a refetch that
    /// already has it), and a chunk applied twice shows twice.
    TimelineLive { agent_id: String, item: Item, cursor: Option<(String, u64)> },
    Notice(String),
}

/// Runs forever, reconnecting as needed. Ends when the UI drops its channels.
pub async fn run(events: UnboundedSender<Event>, mut commands: UnboundedReceiver<Command>) {
    let mut open_agent: Option<String> = None;
    let mut diff_cwd: Option<String> = None;
    let mut remote: Option<RemoteHost> = None;
    // A pairing link with a LAN bridge: the LAN first, the relay when the
    // LAN does not answer (away from home), the LAN again after that.
    let mut lan_index = 0usize;
    let mut backoff = Duration::from_secs(1);
    // The host whose kept lists the UI has been given.
    let mut shown: Option<String> = None;
    loop {
        let key = super::cache::host_key(remote.as_ref().map(|r| r.url.as_str()));
        if shown.as_deref() != Some(key.as_str()) {
            super::cache::set_host(key.clone());
            shown = Some(key);
            // What this host said last time, before it answers again.
            for ev in cached_lists() {
                if events.send(ev).is_err() {
                    return;
                }
            }
        }
        let endpoint = match &remote {
            Some(r) => {
                // Where its bridge is on this network now, if it is here:
                // first, before the addresses pairing gave. Away from it
                // (nothing answers) the relay at once, not after each
                // address has timed out in turn.
                let found = if lan_index == 0 { lan_now(r).await } else { None };
                if lan_index == 0 && found.is_none() && has_relay(r) {
                    lan_index = usize::MAX;
                }
                endpoint_for(r, lan_index, found)
            }
            None => discover(),
        };
        let lan_attempt = endpoint.is_lan();
        // The address without a pairing link's key and token.
        trace(&format!("connecting to {}", endpoint.url.split('?').next().unwrap_or("")));
        if events.send(Event::Connecting { url: endpoint.url.clone() }).is_err() {
            return;
        }
        let attempt = tokio::select! {
            r = session(&endpoint, &events, &mut commands, &mut open_agent, &mut diff_cwd) => Some(r),
            _ = switch_signal().notified() => None,
        };
        let result = match attempt {
            Some(r) => r,
            // Switched while connecting (or connected): the new host now.
            None => match take_switch() {
                Some(next) => Ok(Exit::Switch(next)),
                None => continue,
            },
        };
        let reason = match result {
            Ok(Exit::UiGone) => return,
            Ok(Exit::Switch(next)) => {
                remote = next;
                open_agent = None;
                diff_cwd = None;
                backoff = Duration::from_secs(1);
                continue;
            }
            Ok(Exit::Lan) => {
                lan_index = 0;
                backoff = Duration::from_secs(1);
                continue;
            }
            Ok(Exit::Closed(reason)) => {
                backoff = Duration::from_secs(1);
                reason
            }
            Err(e) => {
                if lan_attempt {
                    // Not on that network: the next address, then the relay.
                    trace(&format!("LAN address did not answer: {e}"));
                    lan_index += 1;
                    continue;
                }
                e
            }
        };
        lan_index = 0;
        trace(&format!("lost: {reason}; next attempt in {} ms", backoff.as_millis()));
        if events.send(Event::Disconnected { reason }).is_err() {
            return;
        }
        // Keep the latest open request while waiting, so a click during an
        // outage is honoured on reconnect.
        let deadline = rt::sleep(backoff);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                _ = &mut deadline => break,
                _ = switch_signal().notified() => {
                    if let Some(next) = take_switch() {
                        remote = next;
                        open_agent = None;
                        diff_cwd = None;
                        lan_index = 0;
                        backoff = Duration::from_secs(1);
                        break;
                    }
                }
                cmd = commands.recv() => match cmd {
                    None => return,
                    Some(Command::Open(id)) => open_agent = Some(id),
                    Some(Command::WatchDiff(cwd)) => diff_cwd = cwd,
                    Some(Command::SwitchHost(next)) => {
                        remote = next;
                        open_agent = None;
                        diff_cwd = None;
                        backoff = Duration::from_secs(1);
                        break;
                    }
                    // Asked while away: no answer, said so (no notice).
                    Some(Command::HostUsage) => { let _ = events.send(Event::HostUsage(None)); }
                    Some(Command::AskPictures { .. } | Command::PushRegister { .. }) => {}
                    // Just started: try now, then again soon.
                    Some(Command::Reconnect) => {
                        backoff = Duration::from_millis(250);
                        break;
                    }
                    Some(_) => { let _ = events.send(Event::Notice("Not connected to the service".into())); }
                },
            }
        }
        backoff = (backoff * 2).min(Duration::from_secs(30));
    }
}

/// Where a saved host's session goes: its LAN bridge, its relay, or its
/// direct address.
fn endpoint_for(r: &RemoteHost, lan_index: usize, found: Option<String>) -> Endpoint {
    // Another space on this Mac: its own service, found as the first one is.
    if let Some(home) = crate::spaces::home_of(&r.url) {
        return discover_in(&home);
    }
    if let Some(link) = super::relay::PairingLink::parse(&r.url) {
        let candidate = link.lan.clone().and_then(|(addrs, key, token)| {
            let mut list: Vec<&str> = found.as_deref().into_iter().collect();
            list.extend(addrs.split(',').filter(|a| !a.is_empty() && Some(*a) != found.as_deref()));
            // Without a relay, keep cycling through the addresses.
            let i = if link.relay.is_none() && !list.is_empty() { lan_index % list.len() } else { lan_index };
            list.get(i).map(|a| (a.to_string(), key, token))
        });
        if let Some((addr, key, token)) = candidate {
            return Endpoint {
                url: format!("ws://{addr}/ws"),
                local_credential: None,
                password: r.password.clone(),
                relay: Some(super::relay::RelayOffer {
                    server_id: "bridge".into(),
                    daemon_key: key,
                    endpoint: addr,
                    use_tls: false,
                    password: None,
                }),
                bridge_token: Some(token),
                lan_probe: vec![],
                lan_key: None,
            };
        }
        if let Some(offer) = link.relay {
            // Sophon's own place at the relay (the LAN bridge's key): the
            // device authenticates to it as on the LAN.
            let bridge_token = link.lan.as_ref().filter(|(_, key, _)| *key == offer.daemon_key).map(|(_, _, token)| token.clone());
            let lan_probe = link
                .lan
                .as_ref()
                .map(|(addrs, ..)| addrs.split(',').filter(|a| !a.is_empty()).map(str::to_owned).collect())
                .unwrap_or_default();
            let lan_key = link.lan.as_ref().map(|(_, key, _)| B64.encode(key));
            return Endpoint {
                url: offer.ws_url(),
                local_credential: None,
                password: r.password.clone().or_else(|| offer.password.clone()),
                relay: Some(offer),
                bridge_token,
                lan_probe,
                lan_key,
            };
        }
    }
    Endpoint { url: r.ws_url(), local_credential: None, password: r.password.clone(), relay: None, bridge_token: None, lan_probe: vec![], lan_key: None }
}

/// A paired host's bridge on this network now, found by its key.
fn has_relay(r: &RemoteHost) -> bool {
    super::relay::PairingLink::parse(&r.url).is_some_and(|l| l.relay.is_some() && l.lan.is_some())
}

/// The address the host's bridge answers at on this network, if it is
/// here: where Bonjour finds it, else the first of pairing's addresses
/// that takes a connection. All asked at once, for under a second.
async fn lan_now(r: &RemoteHost) -> Option<String> {
    let (addrs, key, _) = super::relay::PairingLink::parse(&r.url)?.lan?;
    let addrs: Vec<String> = addrs.split(',').filter(|a| !a.is_empty()).map(str::to_owned).collect();
    let named = find_key(B64.encode(key), Duration::from_millis(900));
    let answering = async {
        let probes = addrs.iter().map(|a| async move { rt::reachable(a, Duration::from_millis(800)).await.then(|| a.clone()) });
        futures_util::future::join_all(probes).await.into_iter().flatten().next()
    };
    let (named, answering) = tokio::join!(named, answering);
    named.or(answering)
}

/// The lists this device kept of the current host, as the events its
/// answers make.
fn cached_lists() -> Vec<Event> {
    let mut out = vec![];
    for (kind, name) in LISTS {
        if let Some(payload) = super::cache::load(name) {
            out.extend(handle(kind, &payload, &mut HashMap::new(), None, &mut vec![]));
        }
    }
    out
}

/// The answers kept whole, and the names they are kept under.
const LISTS: [(&str, &str); 3] =
    [("project.list.response", "projects"), ("fetch_workspaces_response", "workspaces"), ("fetch_agents_response", "agents")];

/// A conversation's page as the host sent it: its items, the cursor of the
/// page before it (while there is one), and where it ends.
pub fn timeline_page(payload: &Value) -> (Vec<Item>, Option<Value>, Option<(String, u64)>) {
    let items = payload
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| entries.iter().filter_map(|e| e.get("item").and_then(Item::from_json)).collect())
        .unwrap_or_default();
    let older = (payload.get("hasOlder").and_then(Value::as_bool) == Some(true))
        .then(|| payload.get("startCursor").cloned())
        .flatten()
        .filter(|c| !c.is_null());
    (items, older, end_cursor(payload))
}

/// The page of `agent_id` this device kept, for showing before the host
/// answers.
pub fn cached_timeline(agent_id: &str) -> Option<(Vec<Item>, Option<Value>, Option<(String, u64)>)> {
    let page = timeline_page(&super::cache::load(&super::cache::timeline_name(agent_id))?);
    // Without its end there is no asking for what came after.
    page.2.is_some().then_some(page)
}

/// The address Bonjour finds the bridge with `key` (base64) at, on a thread
/// of its own (the lookup blocks).
#[cfg(any(target_os = "macos", target_os = "ios"))]
async fn find_key(key: String, wait: Duration) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(super::bonjour::find(&key, wait));
    });
    rx.await.ok().flatten()
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
async fn find_key(_key: String, _wait: Duration) -> Option<String> {
    None
}

enum Exit {
    UiGone,
    Closed(String),
    /// The user picked another host.
    Switch(Option<RemoteHost>),
    /// The LAN bridge answers: leave the relay for it.
    Lan,
}

/// What a request was for, so its response can be routed.
enum Pending {
    Agents,
    Agent,
    Projects,
    Workspaces,
    Providers,
    Timeline(String),
    /// What came after the page this device kept.
    After(String, u64),
    Older(String),
    Commands,
    Suggest(String),
    Diff,
    CreateTerminal,
    AttachTerminal,
    Explorer,
    Write(String),
    Git(&'static str),
    History(String),
    Schedules,
    ScheduleChange(&'static str),
    ScheduleRuns(String),
    Project,
    /// The event subscription; `true` while asking for the newer events,
    /// retried without them on a daemon that refuses them.
    Events(bool),
    FileOp(String),
    Pairing,
    /// A feature an older daemon may not have: failures stay quiet.
    Optional,
    /// A providers config patch: the list is refreshed once it is saved.
    ProviderConfig,
    /// Step 1 of `CreateAgent`: the workspace; step 2 starts the agent.
    CreateWorkspace,
    CreateAgent,
    Import,
    Upload(String),
    Download(String),
    Other(&'static str),
}

async fn session(
    endpoint: &Endpoint,
    events: &UnboundedSender<Event>,
    commands: &mut UnboundedReceiver<Command>,
    open_agent: &mut Option<String>,
    diff_cwd: &mut Option<String>,
) -> Result<Exit, String> {
    let mut diff_sub: Option<String> = None;
    // Terminal output arrives in binary frames addressed by slot.
    let mut slots: HashMap<u8, String> = HashMap::new();
    let mut utf8_tails: HashMap<String, Vec<u8>> = HashMap::new();
    // Downloads under way: their bytes so far, by request id.
    let mut downloads: HashMap<String, Vec<u8>> = HashMap::new();
    let (sink, mut rx) = rt::timeout(
        Duration::from_secs(if endpoint.is_lan() { 3 } else { 10 }),
        rt::connect(endpoint.url.as_str()),
    )
    .await
    .ok_or_else(|| "Timed out connecting to the daemon".to_owned())?
    .map_err(|e| format!("Cannot reach the Sophon service at {}: {e}", endpoint.url))?;
    let mut tx = Out { sink, seal: None };
    // Through the relay: the end-to-end handshake first, then every frame
    // is sealed for the daemon and opened from it.
    let mut unseal: Option<super::relay::Receiver> = None;
    if let Some(offer) = &endpoint.relay {
        let (sender, receiver) = relay_handshake(offer, &mut tx, &mut rx).await?;
        tx.seal = Some(sender);
        unseal = Some(receiver);
        if let Some(token) = &endpoint.bridge_token {
            send(&mut tx, &json!({ "type": "sophon_bridge_auth", "token": token, "device": crate::lan::device_name() })).await?;
        }
    }

    let mut hello = json!({
        "type": "hello",
        "clientId": client_id(),
        "clientType": "mobile",
        "protocolVersion": 1,
        "appVersion": concat!("sophon/", env!("CARGO_PKG_VERSION")),
        "capabilities": {
            "project_updates": true,
            "provider_subagents": true,
            "projected_subagent_timeline": true,
            "hello_rejection": true,
            "owned_subscriptions": true,
            "explicit_event_subscriptions": true,
            "selective_agent_timeline": true,
            "timeline_replacement_invalidation": true,
            "all_providers": true,
            "background_agents": true,
        },
    });
    if let Some(token) = &endpoint.local_credential {
        hello["auth"] = json!({ "kind": "localCredential", "token": token });
    } else if let Some(password) = endpoint.password.as_ref().filter(|p| !p.is_empty()) {
        hello["auth"] = json!({ "kind": "password", "password": password });
    }
    send(&mut tx, &hello).await?;

    let mut pending: HashMap<String, Pending> = HashMap::new();
    let mut connected = false;
    let mut ping = rt::interval_at(
        rt::Instant::now() + Duration::from_secs(10),
        Duration::from_secs(10),
    );
    let hello_deadline = rt::sleep(Duration::from_secs(15));
    tokio::pin!(hello_deadline);
    // The daemon answers every ping: a connection silent for longer than
    // this is dead (a relay that dropped it, a phone back from sleep), and
    // the session ends so the loop reconnects instead of sending into it.
    const SILENCE: Duration = Duration::from_secs(25);
    let mut last_frame = rt::Instant::now();
    let mut lan_check = rt::interval_at(
        rt::Instant::now() + Duration::from_secs(5),
        Duration::from_secs(15),
    );

    loop {
        tokio::select! {
            _ = &mut hello_deadline, if !connected => {
                return Err("The daemon did not answer the handshake".into());
            }
            _ = lan_check.tick(), if connected && (!endpoint.lan_probe.is_empty() || endpoint.lan_key.is_some()) => {
                let found = match &endpoint.lan_key {
                    Some(key) => find_key(key.clone(), Duration::from_millis(1000)).await,
                    None => None,
                };
                for addr in found.iter().chain(&endpoint.lan_probe) {
                    if rt::reachable(addr, Duration::from_millis(800)).await {
                        log::info!("the LAN bridge at {addr} answers: leaving the relay");
                        return Ok(Exit::Lan);
                    }
                }
            }
            _ = ping.tick(), if connected => {
                if last_frame.elapsed() > SILENCE {
                    return Err("The connection timed out".into());
                }
                send(&mut tx, &json!({ "type": "ping" })).await?;
            }
            cmd = commands.recv() => {
                let Some(cmd) = cmd else { return Ok(Exit::UiGone) };
                if let Command::SwitchHost(next) = cmd {
                    // Said, not dropped: the computer left behind (its
                    // bridge, its device list) hears it go at once.
                    let _ = tx.sink.send(Message::Close(None)).await;
                    return Ok(Exit::Switch(next));
                }
                if let Command::Open(id) = &cmd {
                    *open_agent = Some(id.clone());
                }
                if let Command::PushRegister { device, token, env, lang, hide } = &cmd {
                    // Only the host's Sophon (its bridge) keeps this.
                    if connected && endpoint.bridge_token.is_some() {
                        send(&mut tx, &json!({ "type": "sophon_push_register", "device": device, "token": token, "env": env, "lang": lang, "hide": hide })).await?;
                    }
                    continue;
                }
                if let Command::HostUsage = &cmd {
                    if connected && endpoint.bridge_token.is_some() {
                        send(&mut tx, &json!({ "type": "sophon_usage_request" })).await?;
                    } else if events.send(Event::HostUsage(None)).is_err() {
                        return Ok(Exit::UiGone);
                    }
                    continue;
                }
                if let Command::AskPictures { message_id, text, cwd } = &cmd {
                    // Sophon's bridge answers this, not the service: a
                    // session that goes through one asks it.
                    if connected && endpoint.bridge_token.is_some() {
                        send(&mut tx, &json!({ "type": "sophon_pictures_request", "messageId": message_id, "text": text, "cwd": cwd })).await?;
                        continue;
                    }
                    // Straight to the service on this computer: the agents'
                    // records are here to read.
                    #[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
                    if endpoint.relay.is_none() && ["127.0.0.1", "localhost", "[::1]"].iter().any(|h| endpoint.url.contains(h)) {
                        let (events, message_id, text, cwd) = (events.clone(), message_id.clone(), text.clone(), cwd.clone());
                        std::thread::spawn(move || {
                            let pictures = crate::agent_pictures::find(&cwd, &message_id, &text);
                            if !pictures.is_empty() {
                                let _ = events.send(Event::Pictures { message_id, text, pictures });
                            }
                        });
                    }
                    continue;
                }
                if let Command::WatchDiff(cwd) = &cmd {
                    if *diff_cwd == *cwd {
                        continue;
                    }
                    *diff_cwd = cwd.clone();
                    if !connected {
                        continue;
                    }
                    if let Some(sub) = diff_sub.take() {
                        send(&mut tx, &request(&mut pending, Pending::Other("stop diff"), json!({
                            "type": "unsubscribe_checkout_diff_request", "subscriptionId": sub,
                        }))).await?;
                    }
                    if let Some(cwd) = cwd {
                        send(&mut tx, &diff_request(cwd, &mut pending)).await?;
                    }
                    continue;
                }
                if !connected {
                    if let Command::Upload { id, .. } = cmd {
                        let _ = events.send(Event::Uploaded { id, result: Err("Not connected".into()) });
                    } else if let Command::Download { id, .. } = cmd {
                        let _ = events.send(Event::Downloaded { id, result: Err("Not connected".into()) });
                    }
                    continue;
                }
                if let Command::Download { id, cwd, path } = cmd {
                    // Under our id: the file's frames carry it.
                    downloads.insert(id.clone(), Vec::new());
                    pending.insert(id.clone(), Pending::Download(id.clone()));
                    send(&mut tx, &session_msg(json!({
                        "type": "file_explorer_request",
                        "cwd": cwd,
                        "path": path,
                        "mode": "file",
                        "acceptBinary": true,
                        "requestId": id,
                    }))).await?;
                    continue;
                }
                if let Command::Upload { id, name, mime, bytes } = cmd {
                    upload(&mut tx, &mut pending, id, name, mime, bytes).await?;
                    continue;
                }
                for msg in command_messages(cmd, &mut pending) {
                    send(&mut tx, &msg).await?;
                }
            }
            frame = rx.next() => {
                let frame = match frame {
                    None => return Ok(Exit::Closed("The daemon closed the connection".into())),
                    Some(Err(e)) => return Err(format!("Connection lost: {e}")),
                    Some(Ok(frame)) => frame,
                };
                last_frame = rt::Instant::now();
                let frame = match unseal.as_mut() {
                    None => frame,
                    Some(r) => match frame {
                        Message::Close(_) | Message::Ping(_) | Message::Pong(_) => frame,
                        other => match r.open(other) {
                            Ok(Some(f)) => f,
                            Ok(None) => continue,
                            Err(e) => return Err(e),
                        },
                    },
                };
                let text = match frame {
                    Message::Text(t) => t,
                    Message::Binary(bytes) if matches!(bytes.first(), Some(&(FILE_BEGIN..=FILE_END))) => {
                        if let Some((id, result)) = download_frame(&bytes, &mut downloads) {
                            pending.remove(&id);
                            if events.send(Event::Downloaded { id, result }).is_err() {
                                return Ok(Exit::UiGone);
                            }
                        }
                        continue;
                    }
                    Message::Binary(bytes) => {
                        for ev in terminal_frame(&bytes, &slots, &mut utf8_tails) {
                            if events.send(ev).is_err() {
                                return Ok(Exit::UiGone);
                            }
                        }
                        continue;
                    }
                    Message::Close(c) => {
                        let reason = c.map(|c| format!("{} ({})", c.reason, c.code))
                            .unwrap_or_else(|| "closed".into());
                        return Ok(Exit::Closed(format!("The daemon closed the connection: {reason}")));
                    }
                    // Terminal and file frames are binary; this client does not
                    // subscribe to either yet.
                    _ => continue,
                };
                let Ok(msg) = serde_json::from_str::<Value>(text.as_str()) else { continue };
                match msg.get("type").and_then(Value::as_str) {
                    Some("hello.rejected") => {
                        let reason = msg.get("reason").and_then(Value::as_str).unwrap_or("rejected");
                        return Err(format!("The daemon refused this client: {reason}"));
                    }
                    Some("session") => {}
                    Some("sophon_usage") => {
                        if events.send(Event::HostUsage(msg.get("report").cloned())).is_err() {
                            return Ok(Exit::UiGone);
                        }
                        continue;
                    }
                    Some("sophon_push_info") => {
                        let field = |k| msg.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
                        let info = Event::PushInfo { gateway: field("gateway"), tls: msg.get("tls").and_then(Value::as_bool).unwrap_or(false), id: field("id") };
                        if events.send(info).is_err() {
                            return Ok(Exit::UiGone);
                        }
                        continue;
                    }
                    Some("sophon_pictures") => {
                        let field = |k| msg.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
                        let pictures: Vec<String> = msg
                            .get("pictures")
                            .and_then(Value::as_array)
                            .map(|l| l.iter().filter_map(Value::as_str).filter(|p| p.starts_with("data:image/")).map(str::to_owned).collect())
                            .unwrap_or_default();
                        if !pictures.is_empty() && events.send(Event::Pictures { message_id: field("messageId"), text: field("text"), pictures }).is_err() {
                            return Ok(Exit::UiGone);
                        }
                        continue;
                    }
                    _ => continue,
                }
                let Some(inner) = msg.get("message") else { continue };
                let kind = inner.get("type").and_then(Value::as_str).unwrap_or_default();
                let payload = inner.get("payload").cloned().unwrap_or(Value::Null);

                if !connected {
                    if kind == "status" && payload.get("status").and_then(Value::as_str) == Some("server_info") {
                        connected = true;
                        let field = |k| payload.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
                        trace(&format!("connected to {} ({})", field("hostname"), field("version")));
                        if events.send(Event::Connected { hostname: field("hostname"), version: field("version") }).is_err() {
                            return Ok(Exit::UiGone);
                        }
                        for m in startup_messages(open_agent.as_deref(), &mut pending) {
                            send(&mut tx, &m).await?;
                        }
                        if let Some(cwd) = diff_cwd.as_deref() {
                            send(&mut tx, &diff_request(cwd, &mut pending)).await?;
                        }
                    }
                    continue;
                }

                if kind == "subscribe_terminal_response"
                    && let (Some(slot), Some(id)) = (
                        payload.get("slot").and_then(Value::as_u64),
                        payload.get("terminalId").and_then(Value::as_str),
                    )
                {
                    slots.retain(|_, t| t != id);
                    slots.insert(slot as u8, id.to_owned());
                }
                if kind == "subscribe_checkout_diff_response" {
                    diff_sub = payload.get("subscriptionId").and_then(Value::as_str).map(str::to_owned);
                }
                if let Some((_, name)) = LISTS.iter().find(|(k, _)| *k == kind) {
                    super::cache::save(name, &payload);
                }
                let mut follow_ups = Vec::new();
                for ev in handle(kind, &payload, &mut pending, open_agent.as_deref(), &mut follow_ups) {
                    if events.send(ev).is_err() {
                        return Ok(Exit::UiGone);
                    }
                }
                for m in follow_ups {
                    send(&mut tx, &m).await?;
                }
            }
        }
    }
}

fn request(pending: &mut HashMap<String, Pending>, what: Pending, mut msg: Value) -> Value {
    let id = uuid::Uuid::new_v4().to_string();
    msg["requestId"] = json!(id);
    pending.insert(id, what);
    session_msg(msg)
}

fn session_msg(message: Value) -> Value {
    json!({ "type": "session", "message": message })
}

/// A live event's place in its agent's timeline: `(epoch, seq)`.
fn seq_cursor(payload: &Value) -> Option<(String, u64)> {
    Some((payload.get("epoch")?.as_str()?.to_owned(), payload.get("seq")?.as_u64()?))
}

/// The last place a fetched page covers: `(epoch, seq)`.
fn end_cursor(payload: &Value) -> Option<(String, u64)> {
    let end = payload.get("endCursor")?;
    let epoch = end.get("epoch").or_else(|| payload.get("epoch"))?.as_str()?.to_owned();
    Some((epoch, end.get("seq")?.as_u64()?))
}

/// Items a conversation opens with, and each earlier page brings.
const TIMELINE_PAGE: u32 = 40;

/// The latest page of a conversation, whole.
fn tail_request(agent_id: &str, pending: &mut HashMap<String, Pending>) -> Value {
    request(
        pending,
        Pending::Timeline(agent_id.to_owned()),
        json!({
            "type": "fetch_agent_timeline_request",
            "agentId": agent_id,
            "direction": "tail",
            // A screenful at a time: earlier ones load as
            // the conversation scrolls up, so a long agent opens as
            // fast as a short one, through a relay too.
            "limit": TIMELINE_PAGE,
            "projection": "projected",
        }),
    )
}

fn timeline_requests(agent_id: &str, pending: &mut HashMap<String, Pending>) -> Vec<Value> {
    // With a page kept on this device (shown already), only what came
    // after it; the host answers with the latest page instead when the
    // kept one no longer joins on (`reset`).
    let kept = super::cache::load(&super::cache::timeline_name(agent_id)).and_then(|p| {
        let cursor = p.get("endCursor")?;
        let seq = cursor.get("seq")?.as_u64()?;
        cursor.get("epoch")?.as_str()?;
        Some((cursor.clone(), seq))
    });
    let first = match kept {
        Some((cursor, seq)) => request(
            pending,
            Pending::After(agent_id.to_owned(), seq),
            json!({
                "type": "fetch_agent_timeline_request",
                "agentId": agent_id,
                "direction": "after",
                "cursor": cursor,
                "limit": TIMELINE_PAGE,
                "projection": "projected",
            }),
        ),
        None => tail_request(agent_id, pending),
    };
    vec![
        first,
        request(
            pending,
            Pending::Other("timeline subscription"),
            json!({ "type": "agent.timeline.set_subscription.request", "agentIds": [agent_id] }),
        ),
    ]
}

/// Decodes a terminal frame: `[opcode][slot][payload]`. Output (0x01) and
/// replays (0x05 in one frame; 0x06 / 0x07 / 0x08 chunked) become UTF-8
/// text, keeping a split character's bytes for the next frame.
fn terminal_frame(bytes: &[u8], slots: &HashMap<u8, String>, tails: &mut HashMap<String, Vec<u8>>) -> Vec<Event> {
    let (Some(&opcode), Some(&slot)) = (bytes.first(), bytes.get(1)) else { return vec![] };
    let Some(id) = slots.get(&slot) else { return vec![] };
    let payload = &bytes[2..];
    let reset = matches!(opcode, 0x05 | 0x06);
    if !matches!(opcode, 0x01 | 0x05 | 0x06 | 0x07) {
        return vec![];
    }
    let tail = tails.entry(id.clone()).or_default();
    if reset {
        tail.clear();
    }
    if opcode == 0x06 {
        return vec![Event::TerminalOutput { terminal_id: id.clone(), data: String::new(), reset: true }];
    }
    tail.extend_from_slice(payload);
    let valid = match std::str::from_utf8(tail) {
        Ok(_) => tail.len(),
        Err(e) if e.error_len().is_none() => e.valid_up_to(),
        Err(_) => tail.len(),
    };
    let data = String::from_utf8_lossy(&tail[..valid]).into_owned();
    tail.drain(..valid);
    vec![Event::TerminalOutput { terminal_id: id.clone(), data, reset }]
}

fn diff_request(cwd: &str, pending: &mut HashMap<String, Pending>) -> Value {
    request(
        pending,
        Pending::Diff,
        json!({ "type": "subscribe_checkout_diff_request", "cwd": cwd, "compare": { "mode": "uncommitted" } }),
    )
}

const BASE_EVENTS: &[&str] =
    &["project.update", "agent_permission_request", "agent_permission_resolved", "agent_attention_required"];
// Events 0.15 (the service Sophon ships) knows: one it does not makes it
// refuse the whole list.
const NEWER_EVENTS: &[&str] = &["providers_snapshot_update", "agent.provider_subagents.update"];

fn events_request(pending: &mut HashMap<String, Pending>, full: bool) -> Value {
    let mut events: Vec<&str> = BASE_EVENTS.to_vec();
    if full {
        events.extend_from_slice(NEWER_EVENTS);
    }
    request(pending, Pending::Events(full), json!({ "type": "session.events.set_subscription.request", "events": events }))
}

fn startup_messages(open_agent: Option<&str>, pending: &mut HashMap<String, Pending>) -> Vec<Value> {
    let mut out = vec![
        events_request(pending, true),
        request(pending, Pending::Projects, json!({ "type": "project.list.request" })),
        request(
            pending,
            Pending::Workspaces,
            json!({ "type": "fetch_workspaces_request", "page": { "limit": 200 }, "subscribe": {} }),
        ),
        request(
            pending,
            Pending::Agents,
            json!({ "type": "fetch_agents_request", "page": { "limit": 200 }, "subscribe": {} }),
        ),
        request(pending, Pending::Providers, json!({ "type": "get_providers_snapshot_request" })),
        request(pending, Pending::Optional, json!({ "type": "workspace.label.list.request" })),
    ];
    if let Some(id) = open_agent {
        out.extend(timeline_requests(id, pending));
    }
    out
}

/// Asks the providers again (an added instance, a changed login), quietly
/// on a daemon without it.
fn refresh_providers(pending: &mut HashMap<String, Pending>) -> Value {
    request(pending, Pending::Optional, json!({ "type": "refresh_providers_snapshot_request" }))
}

fn command_messages(cmd: Command, pending: &mut HashMap<String, Pending>) -> Vec<Value> {
    match cmd {
        Command::Open(id) => timeline_requests(&id, pending),
        // Sent by the session itself, with its binary frames.
        Command::Upload { .. } | Command::Download { .. } | Command::Reconnect | Command::AskPictures { .. } | Command::HostUsage | Command::PushRegister { .. } => vec![],
        Command::Send { agent_id, text, behavior, images, attachments, message_id } => {
            let mut msg = json!({
                "type": "send_agent_message_request",
                "agentId": agent_id,
                "text": text,
                "messageId": message_id,
            });
            if let Some(b) = behavior {
                msg["activeTurnBehavior"] = json!(b);
            }
            if !images.is_empty() {
                msg["images"] = images_json(&images);
            }
            if !attachments.is_empty() {
                msg["attachments"] = Value::Array(attachments);
            }
            vec![request(pending, Pending::Other("send message"), msg)]
        }
        Command::ClearAttention(workspace_id) => vec![request(
            pending,
            Pending::Other("clear attention"),
            json!({ "type": "workspace.clear_attention.request", "workspaceId": workspace_id }),
        )],
        Command::ListCommands(agent_id) => vec![request(
            pending,
            Pending::Commands,
            json!({ "type": "list_commands_request", "agentId": agent_id }),
        )],
        Command::FindDirs { query } => vec![request(
            pending,
            Pending::Suggest(query.clone()),
            json!({
                "type": "directory_suggestions_request",
                "query": query,
                "includeFiles": false,
                "includeDirectories": true,
                "limit": 40,
            }),
        )],
        Command::Suggest { cwd, query } => vec![request(
            pending,
            Pending::Suggest(query.clone()),
            json!({
                "type": "directory_suggestions_request",
                "query": query,
                "cwd": cwd,
                "includeFiles": true,
                "includeDirectories": true,
                "limit": 12,
            }),
        )],
        // Handled by the session, which owns the subscription.
        Command::WatchDiff(_) | Command::SwitchHost(_) => vec![],
        Command::WatchTerminals { cwd, workspace_id } => vec![session_msg(json!({
            "type": "subscribe_terminals_request",
            "requestId": uuid::Uuid::new_v4().to_string(),
            "cwd": cwd,
            "workspaceId": workspace_id,
        }))],
        Command::CreateTerminal { cwd, workspace_id } => vec![request(
            pending,
            Pending::CreateTerminal,
            json!({ "type": "create_terminal_request", "cwd": cwd, "workspaceId": workspace_id }),
        )],
        Command::AttachTerminal(id) => vec![request(
            pending,
            Pending::AttachTerminal,
            json!({
                "type": "subscribe_terminal_request",
                "terminalId": id,
                "restore": { "mode": "full-snapshot", "scrollbackLines": 2000 },
            }),
        )],
        Command::DetachTerminal(id) => {
            vec![session_msg(json!({ "type": "unsubscribe_terminal_request", "terminalId": id }))]
        }
        Command::TerminalInput { terminal_id, data } => vec![session_msg(json!({
            "type": "terminal_input",
            "terminalId": terminal_id,
            "message": { "type": "input", "data": data },
        }))],
        Command::TerminalResize { terminal_id, rows, cols } => vec![session_msg(json!({
            "type": "terminal_input",
            "terminalId": terminal_id,
            "message": { "type": "resize", "rows": rows, "cols": cols, "intent": "claim" },
        }))],
        Command::ListDir { cwd, path } => vec![request(
            pending,
            Pending::Explorer,
            json!({ "type": "file_explorer_request", "cwd": cwd, "path": path, "mode": "list" }),
        )],
        Command::ReadFile { cwd, path } => vec![request(
            pending,
            Pending::Explorer,
            json!({ "type": "file_explorer_request", "cwd": cwd, "path": path, "mode": "file", "maxBytes": 2_000_000 }),
        )],
        Command::WriteFile { cwd, path, content, modified_at, revision } => {
            let mut msg = json!({
                "type": "fs.file.write.request",
                "cwd": cwd,
                "path": path.clone(),
                "content": content,
                "expectedModifiedAt": modified_at,
            });
            if let Some(r) = revision {
                msg["expectedRevision"] = json!(r);
            }
            vec![request(pending, Pending::Write(path), msg)]
        }
        Command::GitStatus(cwd) => vec![
            request(pending, Pending::Other("read git status"), json!({ "type": "checkout_status_request", "cwd": cwd })),
            request(pending, Pending::Optional, json!({ "type": "checkout_pr_status_request", "cwd": cwd })),
        ],
        Command::Git { cwd, action } => {
            let label = action.label();
            let msg = match action {
                GitAction::Commit(message) => {
                    let mut m = json!({ "type": "checkout_commit_request", "cwd": cwd, "addAll": true });
                    if !message.trim().is_empty() {
                        m["message"] = json!(message);
                    }
                    m
                }
                GitAction::Push => json!({ "type": "checkout_push_request", "cwd": cwd }),
                GitAction::SwitchBranch(b) => json!({ "type": "checkout_switch_branch_request", "cwd": cwd, "branch": b }),
                GitAction::Stash => json!({ "type": "stash_save_request", "cwd": cwd }),
                GitAction::MergePr(m) => json!({ "type": "checkout_pr_merge_request", "cwd": cwd, "mergeMethod": m }),
                GitAction::PopStash => json!({ "type": "stash_pop_request", "cwd": cwd, "stashIndex": 0 }),
                GitAction::Pull => json!({ "type": "checkout_pull_request", "cwd": cwd }),
                GitAction::CreatePr => json!({ "type": "checkout_pr_create_request", "cwd": cwd }),
                GitAction::MergeIntoBase => json!({ "type": "checkout_merge_request", "cwd": cwd, "strategy": "merge" }),
            };
            vec![request(pending, Pending::Git(label), msg)]
        }
        Command::History { search } => {
            let mut msg = json!({
                "type": "fetch_agent_history_request",
                "filter": { "includeArchived": true },
                "sort": [{ "key": "updated_at", "direction": "desc" }],
                "page": { "limit": 150 },
            });
            if !search.trim().is_empty() {
                msg["search"] = json!(search.trim());
            }
            vec![request(pending, Pending::History(search), msg)]
        }
        Command::ReadProjectConfig(root) => vec![request(
            pending,
            Pending::Other("read project settings"),
            json!({ "type": "read_project_config_request", "repoRoot": root }),
        )],
        Command::WriteProjectConfig { root, config, revision } => vec![request(
            pending,
            Pending::Other("save project settings"),
            json!({ "type": "write_project_config_request", "repoRoot": root, "config": config, "expectedRevision": revision }),
        )],
        Command::CommitFileDiff { cwd, sha, path } => vec![request(
            pending,
            Pending::Other("load the commit diff"),
            json!({ "type": "checkout.commits.file_diff.request", "cwd": cwd, "sha": sha, "path": path }),
        )],
        Command::Usage => vec![request(pending, Pending::Optional, json!({ "type": "provider.usage.list.request" }))],
        Command::FileOp { cwd, path, name, op } => {
            let msg = match op {
                "create-file" | "create-dir" => json!({
                    "type": "fs.entry.create.request", "cwd": cwd, "parentPath": path, "name": name,
                    "kind": if op == "create-dir" { "directory" } else { "file" },
                }),
                "rename" => json!({ "type": "fs.entry.rename.request", "cwd": cwd, "path": path, "name": name }),
                "duplicate" => json!({ "type": "fs.entry.duplicate.request", "cwd": cwd, "path": path }),
                _ => json!({ "type": "fs.entry.delete.request", "cwd": cwd, "path": path }),
            };
            vec![request(pending, Pending::FileOp(path.clone()), msg)]
        }
        Command::ProviderFeatures { provider, cwd, model, mode } => {
            let mut draft = json!({ "provider": provider, "cwd": cwd });
            if let Some(m) = model {
                draft["model"] = json!(m);
            }
            if let Some(m) = mode {
                draft["modeId"] = json!(m);
            }
            vec![request(pending, Pending::Optional, json!({ "type": "list_provider_features_request", "draftConfig": draft }))]
        }
        Command::SetFeature { agent, id, value } => vec![request(
            pending,
            Pending::Other("change the setting"),
            json!({ "type": "set_agent_feature_request", "agentId": agent, "featureId": id, "value": value }),
        )],
        Command::PairingOffer => vec![request(pending, Pending::Pairing, json!({ "type": "daemon.get_pairing_offer.request" }))],
        Command::Labels => vec![request(pending, Pending::Optional, json!({ "type": "workspace.label.list.request" }))],
        Command::AssignLabel { workspace, name, color, assigned } => vec![request(
            pending,
            Pending::Other("label the workspace"),
            json!({
                "type": "workspace.label.assignment.set.request",
                "workspaceId": workspace,
                "label": { "name": name, "color": color },
                "assigned": assigned,
            }),
        )],
        Command::UpdateDaemon => vec![request(pending, Pending::Other("update the daemon"), json!({ "type": "daemon.update.request" }))],
        Command::RecoverWorkspace(id) => vec![request(
            pending,
            Pending::Other("recover the workspace"),
            json!({ "type": "workspace.recovery.restore.request", "workspaceId": id }),
        )],
        Command::GetConfig => vec![request(pending, Pending::Optional, json!({ "type": "get_daemon_config_request" }))],
        Command::SetConfig(patch) => vec![request(
            pending,
            Pending::Other("save host settings"),
            json!({ "type": "set_daemon_config_request", "config": patch }),
        )],
        Command::PatchProviders(patch) => {
            vec![request(pending, Pending::ProviderConfig, json!({ "type": "set_daemon_config_request", "config": patch }))]
        }
        Command::RefreshProviders => vec![refresh_providers(pending)],
        Command::ProviderDiagnostic(provider) => vec![request(
            pending,
            Pending::Optional,
            json!({ "type": "provider_diagnostic_request", "provider": provider }),
        )],
        Command::RestartDaemon => vec![request(
            pending,
            Pending::Other("restart the daemon"),
            json!({ "type": "restart_server_request", "reason": "Restarted from Sophon" }),
        )],
        Command::RecentSessions { query } => {
            let mut msg = json!({ "type": "fetch_recent_provider_sessions_request", "limit": 200 });
            if !query.trim().is_empty() {
                msg["query"] = json!(query.trim());
            }
            vec![request(pending, Pending::Optional, msg)]
        }
        Command::ImportSession { provider, handle, cwd } => vec![request(
            pending,
            Pending::Import,
            json!({ "type": "import_agent_request", "providerId": provider, "providerHandleId": handle, "cwd": cwd }),
        )],
        Command::Script { workspace, name, start } => vec![request(
            pending,
            Pending::Other(if start { "start the script" } else { "stop the script" }),
            json!({
                "type": if start { "workspace.script.start.request" } else { "workspace.script.stop.request" },
                "workspaceId": workspace,
                "scriptName": name,
            }),
        )],
        Command::Rewind { agent, message_id, mode } => vec![request(
            pending,
            Pending::Other("rewind"),
            json!({ "type": "agent.rewind.request", "agentId": agent, "messageId": message_id, "mode": mode }),
        )],
        Command::Fork(agent) => vec![request(
            pending,
            Pending::Other("fork"),
            json!({ "type": "agent.fork_context.request", "agentId": agent }),
        )],
        Command::Subagents(parent) => vec![request(
            pending,
            Pending::Optional,
            json!({ "type": "agent.provider_subagents.list.request", "parentAgentId": parent }),
        )],
        Command::SubagentTimeline { parent, id } => vec![request(
            pending,
            Pending::Other("load the sub-agent"),
            json!({
                "type": "agent.provider_subagents.timeline.get.request",
                "parentAgentId": parent,
                "subagentId": id,
                "direction": "tail",
                "limit": 300,
            }),
        )],
        Command::BackgroundWork(agent) => vec![request(
            pending,
            Pending::Optional,
            json!({ "type": "agent.background_work.list.request", "agentId": agent }),
        )],
        Command::StopBackground { agent, item, token } => vec![request(
            pending,
            Pending::Other("stop background work"),
            json!({ "type": "agent.background_work.stop.request", "agentId": agent, "itemId": item, "stopToken": token }),
        )],
        Command::Commits(cwd) => {
            vec![request(pending, Pending::Optional, json!({ "type": "checkout.commits.list.request", "cwd": cwd }))]
        }
        Command::Branches { cwd, query } => vec![request(
            pending,
            Pending::Optional,
            json!({ "type": "branch_suggestions_request", "cwd": cwd, "query": query, "limit": 60 }),
        )],
        Command::AddProject(cwd) => {
            vec![request(pending, Pending::Project, json!({ "type": "project.add.request", "cwd": cwd }))]
        }
        Command::CreateProjectDir { parent, name } => vec![request(
            pending,
            Pending::Project,
            json!({ "type": "project.create_directory.request", "parentPath": parent, "name": name }),
        )],
        Command::CloneProject { repo, target } => vec![request(
            pending,
            Pending::Project,
            json!({ "type": "project.github.clone.request", "repo": repo, "targetDirectory": target }),
        )],
        Command::RenameProject { id, name } => vec![request(
            pending,
            Pending::Other("rename the project"),
            json!({ "type": "project.rename.request", "projectId": id, "customName": name }),
        )],
        Command::RemoveProject(id) => vec![request(
            pending,
            Pending::Other("remove the project"),
            json!({ "type": "project.remove.request", "projectId": id }),
        )],
        Command::MarkUnread(id) => vec![request(
            pending,
            Pending::Other("mark unread"),
            json!({ "type": "workspace.mark_unread.request", "workspaceId": id }),
        )],
        Command::UndoArchive(id) => vec![request(
            pending,
            Pending::Other("restore the workspace"),
            json!({ "type": "workspace.archive.undo.request", "workspaceId": id }),
        )],
        Command::Schedules => vec![request(pending, Pending::Schedules, json!({ "type": "schedule/list" }))],
        Command::ScheduleOp { id, op } => vec![request(
            pending,
            Pending::ScheduleChange(op),
            json!({ "type": format!("schedule/{op}"), "scheduleId": id }),
        )],
        Command::CreateSchedule(n) => {
            let cadence = cadence_json(&n.cadence, n.timezone.as_deref());
            let mut config = json!({
                "provider": n.provider,
                "cwd": n.cwd,
                "isolation": if n.worktree { "worktree" } else { "local" },
            });
            if let Some(m) = n.model {
                config["model"] = json!(m);
            }
            let mut msg = json!({
                "type": "schedule/create",
                "prompt": n.prompt,
                "cadence": cadence,
                "target": { "type": "new-agent", "config": config },
            });
            if !n.name.trim().is_empty() {
                msg["name"] = json!(n.name.trim());
            }
            vec![request(pending, Pending::ScheduleChange("create"), msg)]
        }
        Command::UpdateSchedule { id, to } => {
            let name = to.name.trim();
            let mut msg = json!({
                "type": "schedule/update",
                "scheduleId": id,
                "name": if name.is_empty() { Value::Null } else { json!(name) },
                "prompt": to.prompt,
                "cadence": cadence_json(&to.cadence, to.timezone.as_deref()),
            });
            // Only a schedule that starts an agent has a folder to change.
            if !to.cwd.is_empty() {
                msg["newAgentConfig"] = json!({ "cwd": to.cwd, "isolation": if to.worktree { "worktree" } else { "local" } });
            }
            vec![request(pending, Pending::ScheduleChange("update"), msg)]
        }
        Command::ScheduleRuns(id) => {
            vec![request(pending, Pending::ScheduleRuns(id.clone()), json!({ "type": "schedule/logs", "scheduleId": id }))]
        }
        Command::Unarchive(agent_id) => vec![request(
            pending,
            Pending::Other("restore the agent"),
            json!({ "type": "refresh_agent_request", "agentId": agent_id }),
        )],
        Command::KillTerminal(id) => vec![request(
            pending,
            Pending::Other("close terminal"),
            json!({ "type": "kill_terminal_request", "terminalId": id }),
        )],
        Command::LoadOlder { agent_id, cursor } => vec![request(
            pending,
            Pending::Older(agent_id.clone()),
            json!({
                "type": "fetch_agent_timeline_request",
                "agentId": agent_id,
                "direction": "before",
                "cursor": cursor,
                "limit": TIMELINE_PAGE,
                "projection": "projected",
            }),
        )],
        // The permission's own id goes in `requestId`; the daemon answers with
        // `agent_permission_resolved`, not a correlated response.
        Command::Permit { agent_id, permission_id, allow, action_id, updated_input } => {
            let mut response = json!({ "behavior": if allow { "allow" } else { "deny" } });
            if let Some(action) = action_id {
                response["selectedActionId"] = json!(action);
            }
            if let Some(input) = updated_input {
                response["updatedInput"] = input;
            }
            if !allow {
                response["message"] = json!("Dismissed by user");
            }
            vec![session_msg(json!({
                "type": "agent_permission_response",
                "agentId": agent_id,
                "requestId": permission_id,
                "response": response,
            }))]
        }
        Command::Cancel(agent_id) => vec![request(
            pending,
            Pending::Other("stop agent"),
            json!({ "type": "cancel_agent_request", "agentId": agent_id }),
        )],
        Command::Archive(agent_id) => vec![request(
            pending,
            Pending::Other("archive agent"),
            json!({ "type": "archive_agent_request", "agentId": agent_id }),
        )],
        Command::CreateAgent(create) => match create.workspace_id.clone() {
            Some(workspace_id) => vec![create_agent_message(&create, &workspace_id, pending)],
            None => {
                let source = if create.worktree {
                    let mut src = json!({ "kind": "worktree", "cwd": create.path, "action": "branch-off" });
                    if let Some(b) = create.base_branch.as_ref().filter(|b| !b.is_empty()) {
                        src["baseBranch"] = json!(b);
                    }
                    if let Some(n) = create.branch_name.as_ref().filter(|n| !n.trim().is_empty()) {
                        src["branchName"] = json!(n.trim());
                    }
                    src
                } else {
                    json!({ "kind": "directory", "path": create.path })
                };
                // One request makes the workspace and its first agent; the
                // agent's cwd is the source, which the daemon maps into a new
                // worktree.
                let msg = json!({
                    "type": "workspace.create.request",
                    "idempotencyKey": uuid::Uuid::new_v4().to_string(),
                    "source": source,
                    "agent": agent_input(&create, &create.path),
                });
                vec![request(pending, Pending::CreateWorkspace, msg)]
            }
        },
        Command::SetModel { agent_id, model } => vec![request(
            pending,
            Pending::Other("change model"),
            json!({ "type": "set_agent_model_request", "agentId": agent_id, "modelId": model }),
        )],
        Command::SetMode { agent_id, mode } => vec![request(
            pending,
            Pending::Other("change mode"),
            json!({ "type": "set_agent_mode_request", "agentId": agent_id, "modeId": mode }),
        )],
        Command::SetThinking { agent_id, thinking } => vec![request(
            pending,
            Pending::Other("change thinking"),
            json!({ "type": "set_agent_thinking_request", "agentId": agent_id, "thinkingOptionId": thinking }),
        )],
        Command::Rename { agent_id, title } => vec![request(
            pending,
            Pending::Other("rename agent"),
            json!({ "type": "update_agent_request", "agentId": agent_id, "name": title }),
        )],
        Command::ArchiveWorkspace(workspace_id) => vec![request(
            pending,
            Pending::Other("archive workspace"),
            json!({ "type": "archive_workspace_request", "workspaceId": workspace_id }),
        )],
        Command::PinWorkspace { workspace_id, pinned } => vec![request(
            pending,
            Pending::Other("pin workspace"),
            json!({ "type": "workspace.pin.set.request", "workspaceId": workspace_id, "pinned": pinned }),
        )],
        Command::RenameWorkspace { workspace_id, title } => vec![request(
            pending,
            Pending::Other("rename workspace"),
            json!({ "type": "workspace.title.set.request", "workspaceId": workspace_id, "title": title }),
        )],
    }
}

/// The agent part of a create request: provider, model, mode, thinking and
/// the first prompt.
fn agent_input(create: &CreateAgent, cwd: &str) -> Value {
    let mut config = json!({ "provider": create.provider, "cwd": cwd });
    if let Some(model) = &create.model {
        config["model"] = json!(model);
    }
    if let Some(mode) = &create.mode {
        config["modeId"] = json!(mode);
    }
    if let Some(thinking) = &create.thinking {
        config["thinkingOptionId"] = json!(thinking);
    }
    if !create.features.is_empty() {
        config["featureValues"] = Value::Object(create.features.clone());
    }
    let mut agent = json!({ "config": config, "labels": {} });
    if !create.images.is_empty() {
        agent["images"] = images_json(&create.images);
    }
    let attachments: Vec<Value> = create.context.iter().chain(&create.attachments).cloned().collect();
    if !attachments.is_empty() {
        agent["attachments"] = Value::Array(attachments);
    }
    if !create.prompt.trim().is_empty() {
        agent["initialPrompt"] = json!(create.prompt);
        agent["clientMessageId"] = json!(uuid::Uuid::new_v4().to_string());
    }
    agent
}

/// A new agent in an existing workspace.
fn create_agent_message(create: &CreateAgent, workspace_id: &str, pending: &mut HashMap<String, Pending>) -> Value {
    let mut msg = agent_input(create, &create.path);
    msg["type"] = json!("create_agent_request");
    msg["workspaceId"] = json!(workspace_id);
    request(pending, Pending::CreateAgent, msg)
}

fn list_of<T>(payload: &Value, key: &str, decode: impl Fn(&Value) -> Option<T>) -> Vec<T> {
    payload.get(key).and_then(Value::as_array).map(|l| l.iter().filter_map(decode).collect()).unwrap_or_default()
}

fn agents_of(payload: &Value) -> Vec<Agent> {
    payload
        .get("entries")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|e| Agent::from_json(e.get("agent").unwrap_or(e)))
                .collect()
        })
        .unwrap_or_default()
}

/// Decodes one session message into UI events; `follow_ups` collects requests
/// it implies (a refetch after a turn ends or the history is replaced).
fn handle(
    kind: &str,
    payload: &Value,
    pending: &mut HashMap<String, Pending>,
    open_agent: Option<&str>,
    follow_ups: &mut Vec<Value>,
) -> Vec<Event> {
    let request_id = payload.get("requestId").and_then(Value::as_str);
    let agent_id = payload.get("agentId").and_then(Value::as_str).map(str::to_owned);
    let refetch_agent = |id: &str, pending: &mut HashMap<String, Pending>, follow_ups: &mut Vec<Value>| {
        follow_ups.push(request(pending, Pending::Agent, json!({ "type": "fetch_agent_request", "agentId": id })));
    };

    match kind {
        "rpc_error" => {
            let what = request_id.and_then(|id| pending.remove(id));
            match what {
                Some(Pending::Events(true)) => {
                    follow_ups.push(events_request(pending, false));
                    return vec![];
                }
                Some(Pending::Optional) => return vec![],
                // The page showed the change already: show the config as it is.
                Some(Pending::ProviderConfig) => {
                    follow_ups.push(request(pending, Pending::Optional, json!({ "type": "get_daemon_config_request" })));
                }
                _ => {}
            }
            // A host that cannot say what came after: the latest page whole.
            if let Some(Pending::After(id, _)) = &what {
                follow_ups.push(tail_request(id, pending));
                return vec![];
            }
            let error = payload.get("error").and_then(Value::as_str).unwrap_or("request failed");
            let label = match what {
                Some(Pending::CreateWorkspace | Pending::CreateAgent | Pending::Import) => {
                    return vec![Event::CreateFailed(error.to_owned())];
                }
                Some(Pending::Other(label)) => label,
                Some(Pending::Timeline(_) | Pending::Older(_) | Pending::After(..)) => "load history",
                Some(Pending::Commands) => "load commands",
                Some(Pending::Diff) => "load changes",
                Some(Pending::CreateTerminal) => "open a terminal",
                Some(Pending::AttachTerminal) => "attach the terminal",
                Some(Pending::Explorer) => "read files",
                Some(Pending::History(_)) => "load history",
                Some(Pending::Schedules | Pending::ScheduleChange(_) | Pending::ScheduleRuns(_)) => "update schedules",
                Some(Pending::Project) => "add the project",
                Some(Pending::Events(_)) => "subscribe to events",
                Some(Pending::FileOp(_)) => "change the file",
                Some(Pending::Upload(id)) => {
                    return vec![Event::Uploaded { id, result: Err(error.to_owned()) }];
                }
                Some(Pending::Download(id)) => {
                    return vec![Event::Downloaded { id, result: Err(error.to_owned()) }];
                }
                Some(Pending::Pairing) => {
                    return vec![Event::PairingOffer { url: None, relay_enabled: false, error: Some(error.to_owned()) }];
                }
                Some(Pending::Optional) => "load",
                Some(Pending::Git(action)) => return vec![Event::GitDone { cwd: String::new(), action, error: Some(error.to_owned()), url: None }],
                Some(Pending::Write(_)) => "save the file",
                Some(Pending::Suggest(_)) => "search files",
                Some(Pending::Agents | Pending::Agent) => "load agents",
                Some(Pending::Projects | Pending::Workspaces) => "load projects",
                Some(Pending::Providers) => "load providers",
                Some(Pending::ProviderConfig) => "save agent settings",
                None => "request",
            };
            vec![Event::Notice(format!("Could not {label}: {error}"))]
        }
        "project.list.response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::ProjectsReset(list_of(payload, "projects", Project::from_json))]
        }
        "project.update" => match payload.get("kind").and_then(Value::as_str) {
            Some("upsert") => payload.get("project").and_then(Project::from_json).map(Event::ProjectUpsert).into_iter().collect(),
            Some("remove") => payload
                .get("projectId")
                .and_then(Value::as_str)
                .map(|id| Event::ProjectRemove(id.to_owned()))
                .into_iter()
                .collect(),
            _ => vec![],
        },
        "fetch_workspaces_response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::WorkspacesReset(list_of(payload, "entries", Workspace::from_json))]
        }
        "workspace_update" => match payload.get("kind").and_then(Value::as_str) {
            Some("upsert") => payload
                .get("workspace")
                .and_then(Workspace::from_json)
                .map(Event::WorkspaceUpsert)
                .into_iter()
                .collect(),
            Some("remove") => payload
                .get("id")
                .and_then(Value::as_str)
                .map(|id| Event::WorkspaceRemove(id.to_owned()))
                .into_iter()
                .collect(),
            _ => vec![],
        },
        "get_providers_snapshot_response" | "providers_snapshot_update" => {
            request_id.and_then(|id| pending.remove(id));
            if payload.get("notModified").and_then(Value::as_bool) == Some(true) {
                return vec![];
            }
            vec![Event::Providers(list_of(payload, "entries", Provider::from_json))]
        }
        "workspace.create.response" => {
            if !matches!(request_id.and_then(|id| pending.remove(id)), Some(Pending::CreateWorkspace)) {
                return vec![];
            }
            if let Some(error) = payload.get("error").and_then(Value::as_str) {
                return vec![Event::CreateFailed(error.to_owned())];
            }
            let mut out = vec![];
            if let Some(workspace) = payload.get("workspace").and_then(Workspace::from_json) {
                out.push(Event::WorkspaceUpsert(workspace));
            }
            match payload.get("agent").and_then(Agent::from_json) {
                Some(agent) => {
                    out.push(Event::AgentUpsert(agent.clone()));
                    out.push(Event::Created(agent));
                }
                None => out.push(Event::CreateFailed("The workspace was created but its agent did not start".into())),
            }
            out
        }
        // Legacy create replies arrive as status payloads carrying the requestId.
        "status" => {
            let status = payload.get("status").and_then(Value::as_str);
            if !matches!(status, Some("agent_created" | "agent_resumed" | "agent_create_failed")) {
                return vec![];
            }
            if !matches!(request_id.and_then(|id| pending.remove(id)), Some(Pending::CreateAgent | Pending::Import)) {
                return vec![];
            }
            match (status, payload.get("agent").and_then(Agent::from_json)) {
                (Some("agent_created" | "agent_resumed"), Some(agent)) => vec![Event::AgentUpsert(agent.clone()), Event::Created(agent)],
                _ => vec![Event::CreateFailed(
                    payload.get("error").and_then(Value::as_str).unwrap_or("Could not start the agent").to_owned(),
                )],
            }
        }
        "fetch_agents_response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::AgentsReset(agents_of(payload))]
        }
        "fetch_agent_response" => {
            request_id.and_then(|id| pending.remove(id));
            payload.get("agent").and_then(Agent::from_json).map(Event::AgentUpsert).into_iter().collect()
        }
        "agent_update" => match payload.get("kind").and_then(Value::as_str) {
            Some("upsert") => payload.get("agent").and_then(Agent::from_json).map(Event::AgentUpsert).into_iter().collect(),
            Some("remove") => agent_id.map(Event::AgentRemove).into_iter().collect(),
            _ => vec![],
        },
        "fetch_agent_timeline_response" => {
            let (agent_id, older_page, after) = match request_id.and_then(|id| pending.remove(id)) {
                Some(Pending::Timeline(id)) => (id, false, None),
                Some(Pending::Older(id)) => (id, true, None),
                Some(Pending::After(id, from)) => (id, false, Some(from)),
                _ => return vec![],
            };
            let flag = |k: &str| payload.get(k).and_then(Value::as_bool) == Some(true);
            if let Some(from) = after.filter(|_| !flag("reset")) {
                // What came since the page this device kept, applied as
                // live events are (each with its place, so one the
                // subscription brought too is not shown twice). More than
                // a page of it, a row that began before the kept page
                // ended (it would repeat its start), or no answer: the
                // latest page whole instead.
                let rows = payload.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
                let seq = |row: &Value, k: &str| row.get(k).and_then(Value::as_u64);
                let joins = rows.iter().all(|r| seq(r, "seqStart").is_some_and(|s| s > from));
                let epoch = payload.get("epoch").and_then(Value::as_str);
                if payload.get("error").is_some_and(|e| !e.is_null()) || flag("hasNewer") || !joins || epoch.is_none() {
                    follow_ups.push(tail_request(&agent_id, pending));
                    return vec![];
                }
                let epoch = epoch.unwrap_or_default();
                let mut out: Vec<Event> = rows
                    .iter()
                    .filter_map(|row| {
                        let item = row.get("item").and_then(Item::from_json)?;
                        let end = seq(row, "seqEnd").or_else(|| seq(row, "seq"))?;
                        Some(Event::TimelineLive { agent_id: agent_id.clone(), item, cursor: Some((epoch.to_owned(), end)) })
                    })
                    .collect();
                out.push(Event::TimelineSynced { agent_id: agent_id.clone() });
                if let Some(agent) = payload.get("agent").and_then(Agent::from_json) {
                    out.push(Event::AgentUpsert(agent));
                }
                return out;
            }
            if let Some(error) = payload.get("error").and_then(Value::as_str) {
                if older_page {
                    return vec![Event::Notice(format!("Could not load earlier messages: {error}"))];
                }
                // Shown in place of the history, so the view never waits on it.
                let message = format!("Could not load history: {error}");
                return vec![Event::TimelineReset { agent_id, items: vec![Item::Error { message }], older: None, cursor: None }];
            }
            let (items, older, cursor) = timeline_page(payload);
            let mut out: Vec<Event> = vec![if older_page {
                Event::TimelineOlder { agent_id: agent_id.clone(), items, older }
            } else {
                // The latest page: what the next opening shows first.
                super::cache::save(&super::cache::timeline_name(&agent_id), payload);
                Event::TimelineReset { agent_id: agent_id.clone(), items, older, cursor }
            }];
            if let Some(agent) = payload.get("agent").and_then(Agent::from_json) {
                out.push(Event::AgentUpsert(agent));
            }
            out
        }
        "subscribe_checkout_diff_response" | "checkout_diff_update" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::Diff {
                cwd: payload.get("cwd").and_then(Value::as_str).unwrap_or_default().to_owned(),
                files: list_of(payload, "files", DiffFile::from_json),
                error: payload.pointer("/error/message").and_then(Value::as_str).map(str::to_owned),
                too_large: payload.get("diffTooLarge").and_then(Value::as_bool).unwrap_or(false),
            }]
        }
        "file_explorer_response" => {
            // A download that failed: its file never comes.
            if let Some(Pending::Download(id)) = request_id.and_then(|id| pending.remove(id)) {
                let error = payload.get("error").and_then(Value::as_str).unwrap_or("The file could not be read");
                return vec![Event::Downloaded { id, result: Err(error.to_owned()) }];
            }
            let cwd = payload.get("cwd").and_then(Value::as_str).unwrap_or_default().to_owned();
            let path = payload.get("path").and_then(Value::as_str).unwrap_or_default().to_owned();
            let error = payload.get("error").and_then(Value::as_str).map(str::to_owned);
            if payload.get("mode").and_then(Value::as_str) == Some("file") {
                let file = payload.get("file").filter(|f| !f.is_null()).map(|f| {
                    let s = |k: &str| f.get(k).and_then(Value::as_str).map(str::to_owned);
                    FileContent {
                        path: s("path").unwrap_or_default(),
                        kind: s("kind").unwrap_or_default(),
                        text: (s("encoding").as_deref() == Some("utf-8")).then(|| s("content")).flatten(),
                        modified_at: s("modifiedAt").unwrap_or_default(),
                        revision: s("revision"),
                        size: f.get("size").and_then(Value::as_u64).unwrap_or(0),
                    }
                });
                return vec![Event::File { cwd, path, file, error }];
            }
            let entries = payload
                .pointer("/directory/entries")
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .filter_map(|e| {
                            Some(DirEntry {
                                name: e.get("name")?.as_str()?.to_owned(),
                                path: e.get("path")?.as_str()?.to_owned(),
                                is_dir: e.get("kind").and_then(Value::as_str) == Some("directory"),
                                size: e.get("size").and_then(Value::as_u64).unwrap_or(0),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            vec![Event::Dir { cwd, path, entries, error }]
        }
        "checkout_status_response" | "checkout_status_update" => {
            request_id.and_then(|id| pending.remove(id));
            let p = payload;
            let n = |k: &str| p.get(k).and_then(Value::as_u64);
            let s = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_owned);
            vec![Event::GitStatus {
                cwd: s("cwd").unwrap_or_default(),
                status: GitStatus {
                    is_git: p.get("isGit").and_then(Value::as_bool).unwrap_or(false),
                    branch: s("currentBranch"),
                    dirty: p.get("isDirty").and_then(Value::as_bool).unwrap_or(false),
                    base_ref: s("baseRef"),
                    ahead: n("aheadOfOrigin").or_else(|| p.pointer("/aheadBehind/ahead").and_then(Value::as_u64)).unwrap_or(0),
                    behind: n("behindOfOrigin").or_else(|| p.pointer("/aheadBehind/behind").and_then(Value::as_u64)).unwrap_or(0),
                    has_remote: p.get("hasRemote").and_then(Value::as_bool).unwrap_or(false),
                    managed_worktree: p.get("isPaseoOwnedWorktree").and_then(Value::as_bool).unwrap_or(false),
                },
            }]
        }
        "checkout_pr_status_response" => {
            request_id.and_then(|id| pending.remove(id));
            let pr = payload.get("status").filter(|s| !s.is_null()).map(|st| {
                let s = |k: &str| st.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
                PullRequest {
                    number: st.get("number").and_then(Value::as_u64),
                    url: s("url"),
                    title: s("title"),
                    state: s("state"),
                    merged: st.get("isMerged").and_then(Value::as_bool).unwrap_or(false),
                    draft: st.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
                    checks: list_of(st, "checks", |c| {
                        Some((c.get("name")?.as_str()?.to_owned(), c.get("status").and_then(Value::as_str).unwrap_or_default().to_owned()))
                    }),
                }
            });
            vec![Event::PullRequest { cwd: payload.get("cwd").and_then(Value::as_str).unwrap_or_default().to_owned(), pr }]
        }
        "checkout_commit_response" | "checkout_push_response" | "checkout_pull_response" | "checkout_pr_create_response"
        | "checkout_merge_response" | "checkout_switch_branch_response" | "stash_save_response" | "stash_pop_response"
        | "checkout_pr_merge_response" => {
            let Some(Pending::Git(action)) = request_id.and_then(|id| pending.remove(id)) else { return vec![] };
            let error = payload
                .get("error")
                .filter(|e| !e.is_null())
                .map(|e| e.get("message").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| e.to_string()));
            vec![Event::GitDone {
                cwd: payload.get("cwd").and_then(Value::as_str).unwrap_or_default().to_owned(),
                action,
                error,
                url: payload.get("url").and_then(Value::as_str).map(str::to_owned),
            }]
        }
        k if k.starts_with("project.") && k.ends_with(".response") && k != "project.list.response" => {
            let mine = matches!(request_id.and_then(|id| pending.remove(id)), Some(Pending::Project));
            if let Some(e) = payload.get("error").and_then(Value::as_str) {
                return vec![Event::Notice(e.to_owned())];
            }
            match payload.get("project").and_then(Project::from_json) {
                Some(p) if mine => {
                    let name = p.name.clone();
                    vec![Event::ProjectUpsert(p), Event::Notice(format!("Added {name}"))]
                }
                Some(p) => vec![Event::ProjectUpsert(p)],
                None => vec![],
            }
        }
        k if k.starts_with("fs.entry.") && k.ends_with(".response") => {
            let Some(Pending::FileOp(path)) = request_id.and_then(|id| pending.remove(id)) else { return vec![] };
            let dir = payload
                .get("parentPath")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| path.rsplit_once('/').map(|(d, _)| d.to_owned()).unwrap_or_else(|| ".".into()));
            vec![Event::FileOpDone { dir, error: payload.get("error").and_then(Value::as_str).map(str::to_owned) }]
        }
        "read_project_config_response" | "write_project_config_response" => {
            request_id.and_then(|id| pending.remove(id));
            let root = payload.get("repoRoot").and_then(Value::as_str).unwrap_or_default().to_owned();
            let result = if payload.get("ok").and_then(Value::as_bool) == Some(true) {
                Ok((
                    payload.get("config").cloned().filter(|c| !c.is_null()).unwrap_or_else(|| json!({})),
                    payload.get("revision").cloned().unwrap_or(Value::Null),
                ))
            } else {
                Err(payload
                    .pointer("/error/message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .unwrap_or_else(|| payload.get("error").map(|e| e.to_string()).unwrap_or_default()))
            };
            vec![Event::ProjectConfig { root, result, saved: kind.starts_with("write") }]
        }
        "checkout.commits.file_diff.response" => {
            request_id.and_then(|id| pending.remove(id));
            let s = |k: &str| payload.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            vec![Event::CommitFileDiff {
                sha: s("sha"),
                path: s("path"),
                file: payload.get("file").filter(|f| !f.is_null()).and_then(DiffFile::from_json),
                error: payload.pointer("/error/message").and_then(Value::as_str).map(str::to_owned),
            }]
        }
        "provider.usage.list.response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::Usage(list_of(payload, "providers", |p| {
                let s = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_owned);
                let name = s("displayName").or_else(|| s("providerId"))?;
                Some(ProviderUsage {
                    id: s("providerId").unwrap_or_else(|| name.clone()),
                    name,
                    plan: s("planLabel"),
                    error: s("error"),
                    windows: list_of(p, "windows", |w| {
                        let label = w.get("label")?.as_str()?.to_owned();
                        Some(UsageWindow {
                            id: w.get("id").and_then(Value::as_str).map_or_else(|| label.clone(), str::to_owned),
                            label,
                            used: w.get("usedPct").and_then(Value::as_f64)
                                .or_else(|| w.get("remainingPct").and_then(Value::as_f64).map(|r| 100.0 - r)),
                            resets_at: w.get("resetsAt").and_then(Value::as_str).map(str::to_owned),
                        })
                    }),
                })
            }))]
        }
        "list_provider_features_response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::DraftFeatures {
                provider: payload.get("provider").and_then(Value::as_str).unwrap_or_default().to_owned(),
                features: list_of(payload, "features", super::model::Feature::from_json),
            }]
        }
        "file.upload.response" => {
            let Some(Pending::Upload(id)) = request_id.and_then(|id| pending.remove(id)) else { return vec![] };
            let result = match payload.get("file").filter(|f| f.is_object()) {
                Some(file) => Ok(file.clone()),
                None => Err(payload.get("error").and_then(Value::as_str).unwrap_or("Upload failed").to_owned()),
            };
            vec![Event::Uploaded { id, result }]
        }
        "daemon.get_pairing_offer.response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::PairingOffer {
                url: payload.get("url").and_then(Value::as_str).map(str::to_owned),
                relay_enabled: payload.get("relayEnabled").and_then(Value::as_bool).unwrap_or(false),
                error: None,
            }]
        }
        "workspace.label.list.response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::Labels(list_of(payload, "labels", |l| {
                Some((l.get("name")?.as_str()?.to_owned(), l.get("color").and_then(Value::as_str).unwrap_or("violet").to_owned()))
            }))]
        }
        "workspace.label.update" | "workspace.label.assignment.set.response" => {
            request_id.and_then(|id| pending.remove(id));
            follow_ups.push(request(pending, Pending::Optional, json!({ "type": "workspace.label.list.request" })));
            vec![]
        }
        "daemon.update.progress" | "daemon.update.response" => {
            request_id.and_then(|id| pending.remove(id));
            let msg = payload
                .get("message")
                .or_else(|| payload.get("status"))
                .or_else(|| payload.get("phase"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            match (payload.get("error").and_then(Value::as_str), msg) {
                (Some(e), _) => vec![Event::Notice(format!("Update failed: {e}"))],
                (None, Some(m)) => vec![Event::Notice(format!("Daemon update: {m}"))],
                _ => vec![],
            }
        }
        "workspace.recovery.restore.response" => {
            request_id.and_then(|id| pending.remove(id));
            match payload.get("error").and_then(Value::as_str) {
                Some(e) => vec![Event::Notice(format!("Could not recover: {e}"))],
                None => vec![Event::Notice("Workspace recovered".into())],
            }
        }
        "get_daemon_config_response" | "set_daemon_config_response" => {
            if let Some(Pending::ProviderConfig) = request_id.and_then(|id| pending.remove(id)) {
                follow_ups.push(refresh_providers(pending));
            }
            payload.get("config").cloned().map(Event::DaemonConfig).into_iter().collect()
        }
        // The refresh is done: the list as it is now.
        "refresh_providers_snapshot_response" => {
            request_id.and_then(|id| pending.remove(id));
            follow_ups.push(request(pending, Pending::Providers, json!({ "type": "get_providers_snapshot_request" })));
            vec![]
        }
        "provider_diagnostic_response" => {
            request_id.and_then(|id| pending.remove(id));
            let s = |k: &str| payload.get(k).and_then(Value::as_str).map(str::to_owned);
            match (s("provider"), s("diagnostic")) {
                (Some(provider), Some(text)) => vec![Event::Diagnostic { provider, text }],
                _ => vec![],
            }
        }
        "fetch_recent_provider_sessions_response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::RecentSessions(list_of(payload, "entries", |e| {
                let s = |k: &str| e.get(k).and_then(Value::as_str).map(str::to_owned);
                Some(RecentSession {
                    provider: s("providerId")?,
                    provider_label: s("providerLabel").unwrap_or_default(),
                    handle: s("providerHandleId")?,
                    cwd: s("cwd").unwrap_or_default(),
                    title: s("title").or_else(|| s("lastPromptPreview")).or_else(|| s("firstPromptPreview")).unwrap_or_else(|| "Untitled session".into()),
                    last_activity: s("lastActivityAt").unwrap_or_default(),
                })
            }))]
        }
        "agent.rewind.response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::Rewound {
                agent: agent_id.unwrap_or_default(),
                error: payload.get("error").and_then(Value::as_str).map(str::to_owned),
            }]
        }
        "agent.fork_context.response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::Forked {
                agent: agent_id.unwrap_or_default(),
                context: payload.get("attachment").filter(|a| !a.is_null()).cloned(),
                error: payload.get("error").and_then(Value::as_str).map(str::to_owned),
            }]
        }
        "agent.provider_subagents.list.response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::Subagents {
                parent: payload.get("parentAgentId").and_then(Value::as_str).unwrap_or_default().to_owned(),
                list: list_of(payload, "subagents", Subagent::from_json),
            }]
        }
        "agent.provider_subagents.update" => match payload.get("kind").and_then(Value::as_str) {
            Some("upsert") => payload.get("subagent").and_then(Subagent::from_json).map(Event::SubagentUpsert).into_iter().collect(),
            _ => vec![],
        },
        "agent.provider_subagents.timeline.get.response" => {
            request_id.and_then(|id| pending.remove(id));
            let s = |k: &str| payload.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
            // `rows` from a service that projects the sub-agent's timeline
            // (what this client asks for), `entries` from one before that.
            let items = ["rows", "entries"]
                .iter()
                .find_map(|k| payload.get(*k).and_then(Value::as_array))
                .map(|e| e.iter().filter_map(|e| e.get("item").and_then(Item::from_json)).collect())
                .unwrap_or_default();
            vec![Event::SubagentTimeline { parent: s("parentAgentId"), id: s("subagentId"), items }]
        }
        "agent.background_work.list.response" | "agent.background_work.update" => {
            request_id.and_then(|id| pending.remove(id));
            let items = list_of(payload, "items", |i| {
                let s = |k: &str| i.get(k).and_then(Value::as_str).map(str::to_owned);
                Some(BackgroundItem {
                    id: s("id")?,
                    kind: s("kind").unwrap_or_default(),
                    label: s("command").or_else(|| s("description")).unwrap_or_else(|| "Background task".into()),
                    stop_token: (i.get("canStop").and_then(Value::as_bool) != Some(false)).then(|| s("stopToken")).flatten(),
                })
            });
            vec![Event::BackgroundWork { agent: payload.get("agentId").and_then(Value::as_str).unwrap_or_default().to_owned(), items }]
        }
        "checkout.commits.list.response" => {
            request_id.and_then(|id| pending.remove(id));
            let commits = list_of(payload, "commits", |c| {
                let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
                Some(Commit {
                    sha: s("sha"),
                    short: s("shortSha"),
                    subject: s("subject"),
                    author: s("authorName"),
                    date: s("authorDate"),
                    pushed: c.get("isOnRemote").and_then(Value::as_bool).unwrap_or(false),
                    files: list_of(c, "files", |f| {
                        Some((
                            f.get("path")?.as_str()?.to_owned(),
                            f.get("additions").and_then(Value::as_u64).unwrap_or(0),
                            f.get("deletions").and_then(Value::as_u64).unwrap_or(0),
                        ))
                    }),
                })
            });
            vec![Event::Commits {
                cwd: payload.get("cwd").and_then(Value::as_str).unwrap_or_default().to_owned(),
                commits,
                error: payload.pointer("/error/message").and_then(Value::as_str).map(str::to_owned),
            }]
        }
        "branch_suggestions_response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::Branches {
                cwd: payload.get("cwd").and_then(Value::as_str).unwrap_or_default().to_owned(),
                list: list_of(payload, "branches", |b| b.as_str().map(str::to_owned)),
            }]
        }
        "schedule/list/response" => {
            request_id.and_then(|id| pending.remove(id));
            if let Some(e) = payload.get("error").and_then(Value::as_str) {
                return vec![Event::Notice(format!("Could not load schedules: {e}"))];
            }
            vec![Event::Schedules(list_of(payload, "schedules", Schedule::from_json))]
        }
        "schedule/logs/response" => {
            let Some(Pending::ScheduleRuns(id)) = request_id.and_then(|id| pending.remove(id)) else { return vec![] };
            if let Some(e) = payload.get("error").and_then(Value::as_str) {
                return vec![Event::Notice(format!("Could not load the schedule's runs: {e}"))];
            }
            vec![Event::ScheduleRuns { id, runs: list_of(payload, "runs", ScheduleRun::from_json) }]
        }
        k if k.starts_with("schedule/") && k.ends_with("/response") => {
            let what = match request_id.and_then(|id| pending.remove(id)) {
                Some(Pending::ScheduleChange(op)) => op,
                _ => return vec![],
            };
            follow_ups.push(request(pending, Pending::Schedules, json!({ "type": "schedule/list" })));
            match payload.get("error").and_then(Value::as_str) {
                Some(e) => vec![Event::Notice(format!("Schedule {what} failed: {e}"))],
                None if what == "run-once" => vec![Event::Notice("Schedule started".into())],
                None => vec![],
            }
        }
        "fetch_agent_history_response" => {
            let Some(Pending::History(search)) = request_id.and_then(|id| pending.remove(id)) else { return vec![] };
            vec![Event::History { search, agents: agents_of(payload) }]
        }
        "fs.file.write.response" => {
            let Some(Pending::Write(path)) = request_id.and_then(|id| pending.remove(id)) else { return vec![] };
            let result = payload.get("result").cloned().unwrap_or(Value::Null);
            let s = |k: &str| result.get(k).and_then(Value::as_str).map(str::to_owned);
            match s("status").as_deref() {
                Some("written") => vec![Event::FileSaved { path, modified_at: s("modifiedAt"), revision: s("revision"), error: None }],
                Some("conflict") => vec![Event::FileSaved {
                    path,
                    modified_at: None,
                    revision: None,
                    error: Some("The file changed on disk since you opened it. Reopen it to see the latest version".into()),
                }],
                _ => vec![Event::FileSaved { path, modified_at: None, revision: None, error: s("error").or(Some("Save failed".into())) }],
            }
        }
        "terminals_changed" | "list_terminals_response" => {
            request_id.and_then(|id| pending.remove(id));
            vec![Event::Terminals {
                cwd: payload.get("cwd").and_then(Value::as_str).unwrap_or_default().to_owned(),
                list: list_of(payload, "terminals", TerminalInfo::from_json),
            }]
        }
        "create_terminal_response" => {
            request_id.and_then(|id| pending.remove(id));
            if let Some(error) = payload.get("error").and_then(Value::as_str) {
                return vec![Event::Notice(format!("Could not open a terminal: {error}"))];
            }
            payload.get("terminal").and_then(TerminalInfo::from_json).map(Event::TerminalCreated).into_iter().collect()
        }
        "subscribe_terminal_response" => {
            request_id.and_then(|id| pending.remove(id));
            match payload.get("error").and_then(Value::as_str) {
                Some(error) => vec![Event::Notice(format!("Could not attach the terminal: {error}"))],
                None => vec![],
            }
        }
        "terminal_stream_exit" => payload
            .get("terminalId")
            .and_then(Value::as_str)
            .map(|id| Event::TerminalExit(id.to_owned()))
            .into_iter()
            .collect(),
        "list_commands_response" => {
            request_id.and_then(|id| pending.remove(id));
            let agent_id = agent_id.unwrap_or_default();
            vec![Event::Commands {
                agent_id,
                list: list_of(payload, "commands", |c| {
                    Some(SlashCommand {
                        name: c.get("name")?.as_str()?.to_owned(),
                        description: c.get("description").and_then(Value::as_str).unwrap_or_default().to_owned(),
                        hint: c.get("argumentHint").and_then(Value::as_str).unwrap_or_default().to_owned(),
                        skill: c.get("kind").and_then(Value::as_str) == Some("skill"),
                    })
                }),
            }]
        }
        "directory_suggestions_response" => {
            let Some(Pending::Suggest(query)) = request_id.and_then(|id| pending.remove(id)) else {
                return vec![];
            };
            let mut entries: Vec<(String, bool)> = list_of(payload, "entries", |e| {
                Some((e.get("path")?.as_str()?.to_owned(), e.get("kind").and_then(Value::as_str) == Some("directory")))
            });
            if entries.is_empty() {
                entries = list_of(payload, "directories", |d| Some((d.as_str()?.to_owned(), true)));
            }
            vec![Event::Suggestions { query, entries }]
        }
        "agent_stream" => {
            let Some(agent_id) = agent_id else { return vec![] };
            let event = payload.get("event").cloned().unwrap_or(Value::Null);
            match event.get("type").and_then(Value::as_str) {
                Some("timeline") => event
                    .get("item")
                    .and_then(Item::from_json)
                    .map(|item| Event::TimelineLive { agent_id, item, cursor: seq_cursor(payload) })
                    .into_iter()
                    .collect(),
                // A finished turn: replace the delta-built rows with the
                // daemon's authoritative projection.
                Some("turn_completed" | "turn_failed" | "turn_canceled") => {
                    if open_agent == Some(agent_id.as_str()) {
                        follow_ups.extend(timeline_requests(&agent_id, pending).into_iter().take(1));
                    }
                    refetch_agent(&agent_id, pending, follow_ups);
                    vec![]
                }
                Some("permission_requested" | "permission_resolved" | "turn_started") => {
                    refetch_agent(&agent_id, pending, follow_ups);
                    vec![]
                }
                _ => vec![],
            }
        }
        "agent.timeline.replacement" => {
            if let Some(id) = agent_id.filter(|id| open_agent == Some(id.as_str())) {
                follow_ups.extend(timeline_requests(&id, pending).into_iter().take(1));
            }
            vec![]
        }
        "agent_permission_request" | "agent_permission_resolved" | "agent_attention_required" => {
            if let Some(id) = agent_id {
                refetch_agent(&id, pending, follow_ups);
            }
            vec![]
        }
        "send_agent_message_response" | "cancel_agent_response" => {
            request_id.and_then(|id| pending.remove(id));
            let mut out = vec![];
            if let Some(error) = payload.get("error").and_then(Value::as_str) {
                out.push(Event::Notice(error.to_owned()));
            }
            if let Some(agent) = payload.get("agent").and_then(Agent::from_json) {
                out.push(Event::AgentUpsert(agent));
            }
            out
        }
        _ => {
            if kind.ends_with("_response") || kind.ends_with(".response") {
                request_id.and_then(|id| pending.remove(id));
            }
            vec![]
        }
    }
}

/// The socket's sending half, sealing frames when the session goes through
/// the relay.
struct Out {
    sink: rt::WsTx,
    seal: Option<super::relay::Sender>,
}

/// File transfer: the request, then the bytes in binary frames
/// (begin with the metadata, chunks, end), all under the request's id.
async fn upload(
    tx: &mut Out,
    pending: &mut HashMap<String, Pending>,
    id: String,
    name: String,
    mime: String,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let request_id = uuid::Uuid::new_v4().to_string();
    let modified_at = "1970-01-01T00:00:00.000Z";
    pending.insert(request_id.clone(), Pending::Upload(id));
    send(tx, &session_msg(json!({
        "type": "file.upload.request",
        "fileName": name,
        "mimeType": mime,
        "size": bytes.len(),
        "modifiedAt": modified_at,
        "requestId": request_id,
    })))
    .await?;
    for frame in file_frames(&request_id, &name, &mime, modified_at, &bytes) {
        send_frame(tx, Message::Binary(frame)).await?;
    }
    Ok(())
}

/// A frame of a download's file: begin (its metadata), a chunk of its
/// bytes, end. The whole file once its end comes, with its request id; a
/// frame of no download we asked for is dropped.
fn download_frame(frame: &[u8], downloads: &mut HashMap<String, Vec<u8>>) -> Option<(String, Result<Vec<u8>, String>)> {
    let (&op, rest) = frame.split_first()?;
    let (&len, rest) = rest.split_first()?;
    let id = std::str::from_utf8(rest.get(..len as usize)?).ok()?.to_owned();
    let body = &rest[len as usize..];
    let bytes = downloads.get_mut(&id)?;
    match op {
        FILE_BEGIN => {
            // The size, to hold the file in one allocation.
            let meta = body.get(2..).and_then(|m| serde_json::from_slice::<Value>(m).ok());
            if let Some(size) = meta.and_then(|m| m.get("size").and_then(Value::as_u64)) {
                bytes.reserve(size as usize);
            }
            None
        }
        FILE_CHUNK => {
            bytes.extend_from_slice(body);
            None
        }
        FILE_END => downloads.remove(&id).map(|b| (id, Ok(b))),
        _ => None,
    }
}

const FILE_BEGIN: u8 = 0x10;
const FILE_CHUNK: u8 = 0x11;
const FILE_END: u8 = 0x12;
const FILE_CHUNK_BYTES: usize = 128 * 1024;

fn file_frames(request_id: &str, name: &str, mime: &str, modified_at: &str, bytes: &[u8]) -> Vec<Vec<u8>> {
    let id = request_id.as_bytes();
    let head = |op: u8| {
        let mut f = vec![op, id.len() as u8];
        f.extend_from_slice(id);
        f
    };
    let meta = json!({
        "mime": mime,
        "size": bytes.len(),
        "encoding": "binary",
        "modifiedAt": modified_at,
        "fileName": name,
    })
    .to_string();
    let mut begin = head(FILE_BEGIN);
    begin.extend_from_slice(&(meta.len() as u16).to_be_bytes());
    begin.extend_from_slice(meta.as_bytes());
    let mut frames = vec![begin];
    for chunk in bytes.chunks(FILE_CHUNK_BYTES) {
        let mut f = head(FILE_CHUNK);
        f.extend_from_slice(chunk);
        frames.push(f);
    }
    frames.push(head(FILE_END));
    frames
}

async fn send(tx: &mut Out, msg: &Value) -> Result<(), String> {
    send_frame(tx, Message::Text(msg.to_string())).await
}

async fn send_frame(tx: &mut Out, mut frame: Message) -> Result<(), String> {
    if let Some(seal) = tx.seal.as_mut() {
        frame = seal.seal(frame)?;
    }
    tx.sink.send(frame).await.map_err(|e| format!("Send failed: {e}"))
}

/// Sends the hello every second until the daemon answers (the relay may
/// not have the daemon's side yet), then opens the channel.
async fn relay_handshake(
    offer: &super::relay::RelayOffer,
    tx: &mut Out,
    rx: &mut rt::WsRx,
) -> Result<(super::relay::Sender, super::relay::Receiver), String> {
    let mut hs = super::relay::Handshake::start(offer)?;
    let hello = hs.hello().to_owned();
    let mut retry = rt::interval(Duration::from_secs(1));
    let deadline = rt::sleep(Duration::from_secs(20));
    tokio::pin!(deadline);
    loop {
        tokio::select! {
            _ = &mut deadline => return Err("The daemon did not answer through the relay; is it running with the relay on?".into()),
            _ = retry.tick() => {
                tx.sink.send(Message::Text(hello.clone())).await.map_err(|e| format!("Relay send failed: {e}"))?;
            }
            frame = rx.next() => match frame {
                None => return Err("The relay closed the connection".into()),
                Some(Err(e)) => return Err(format!("Relay connection lost: {e}")),
                Some(Ok(Message::Text(t))) => match hs.finish(t.as_str()) {
                    Ok(Some(pair)) => return Ok(pair),
                    Ok(None) => unreachable!("finish consumes the handshake only on success"),
                    Err((back, e)) if e.is_empty() => hs = back,
                    Err((_, e)) => return Err(e),
                },
                Some(Ok(Message::Close(c))) => {
                    return Err(format!("The relay closed the connection: {}", c.map(|c| c.reason).unwrap_or_default()));
                }
                Some(Ok(_)) => {}
            },
        }
    }
}

// ---------------------------------------------------------------- discovery

/// Where the daemon listens and the credential it accepts from this machine.
pub struct Endpoint {
    pub url: String,
    pub local_credential: Option<String>,
    /// The daemon password, for a remote host that requires one.
    pub password: Option<String>,
    /// Set when the session goes through the relay (or Sophon's LAN
    /// bridge, which speaks the same encryption).
    pub relay: Option<super::relay::RelayOffer>,
    /// The LAN bridge's pairing token, sent first through the channel.
    pub bridge_token: Option<String>,
    /// Through the relay: the LAN bridge's addresses, probed now and then so
    /// the session moves to the LAN as soon as it answers (back home, or
    /// once iOS grants local network access, whose prompt fails the first
    /// attempt).
    pub lan_probe: Vec<String>,
    /// The bridge's key (base64), to find it on the LAN by Bonjour.
    pub lan_key: Option<String>,
}

impl Endpoint {
    /// A LAN bridge's address (tried briefly; the relay comes next).
    pub fn is_lan(&self) -> bool {
        self.relay.as_ref().is_some_and(|r| r.server_id == "bridge")
    }
}

/// The relay this Mac's service registers with (`daemon.relay.endpoint` in
/// its config file; the service exposes no call to change it): `None` is
/// the default relay.
pub fn relay_endpoint() -> Option<String> {
    let text = std::fs::read_to_string(daemon_home().join("config.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v.pointer("/daemon/relay/endpoint").and_then(Value::as_str).map(str::to_owned).filter(|s| !s.is_empty())
}

/// Whether the relay the first space sets is reached over TLS, when it says.
pub fn relay_use_tls() -> Option<bool> {
    let text = std::fs::read_to_string(daemon_home().join("config.json")).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v.pointer("/daemon/relay/useTls").and_then(Value::as_bool)
}

/// Sets the relay (`host:port`, TLS when the port is 443; empty: the default),
/// keeping the rest of the file. The service reads it when it starts.
pub fn set_relay_endpoint(endpoint: &str) -> Result<(), String> {
    let path = daemon_home().join("config.json");
    let mut v: Value = std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| json!({}));
    let endpoint = endpoint.trim().trim_start_matches("https://").trim_start_matches("wss://").trim_end_matches('/');
    if !v.get("daemon").is_some_and(Value::is_object) {
        v["daemon"] = json!({});
    }
    if !v["daemon"].get("relay").is_some_and(Value::is_object) {
        v["daemon"]["relay"] = json!({ "enabled": true });
    }
    let relay = v["daemon"]["relay"].as_object_mut().ok_or("bad config")?;
    if endpoint.is_empty() {
        relay.remove("endpoint");
        relay.remove("useTls");
    } else {
        let endpoint = if endpoint.contains(':') { endpoint.to_owned() } else { format!("{endpoint}:443") };
        relay.insert("useTls".into(), json!(endpoint.ends_with(":443")));
        relay.insert("endpoint".into(), json!(endpoint));
    }
    let text = serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| e.to_string())
}

fn daemon_home() -> PathBuf {
    if let Some(home) = std::env::var_os("PASEO_HOME") {
        return PathBuf::from(home);
    }
    home_dir().join(".paseo")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// Reads the live address from `paseo.pid` (falling back to `config.json`
/// and then the default port) and the local credential the daemon rewrites
/// on every start. The credential is only offered to the address `paseo.pid`
/// names, as the daemon's CLI does.
pub fn discover() -> Endpoint {
    discover_in(&daemon_home())
}

/// The service of the Paseo home `home` on this machine (a space's).
pub fn discover_in(home: &std::path::Path) -> Endpoint {
    let read_json = |name: &str| {
        std::fs::read_to_string(home.join(name)).ok().and_then(|s| serde_json::from_str::<Value>(&s).ok())
    };
    let pid_listen = read_json("paseo.pid")
        .and_then(|v| v.get("listen").and_then(Value::as_str).map(str::to_owned))
        .filter(|l| !l.starts_with('/') && !l.starts_with("unix:"));
    let listen = std::env::var("PASEO_LISTEN")
        .ok()
        .or_else(|| pid_listen.clone())
        .or_else(|| {
            read_json("config.json").and_then(|v| v.pointer("/daemon/listen").and_then(Value::as_str).map(str::to_owned))
        })
        .unwrap_or_else(|| "127.0.0.1:6767".into());
    let local_credential = (pid_listen.as_deref() == Some(listen.as_str()))
        .then(|| std::fs::read_to_string(home.join("local-credential")).ok())
        .flatten()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    Endpoint { url: format!("ws://{listen}/ws"), local_credential, password: None, relay: None, bridge_token: None, lan_probe: vec![], lan_key: None }
}

/// A stable id, so a reconnect within the daemon's grace period resumes the
/// same logical session.
fn client_id() -> String {
    if let Some(id) = super::read_file("client-id") {
        let id = id.trim();
        if !id.is_empty() {
            return id.to_owned();
        }
    }
    let id = format!("cid_{}", uuid::Uuid::new_v4().simple());
    let _ = super::write_file("client-id", &id);
    id
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_frames_follow_the_protocol() {
        let bytes = vec![7u8; FILE_CHUNK_BYTES + 3];
        let frames = file_frames("r1", "a.txt", "text/plain", "t", &bytes);
        assert_eq!(frames.len(), 4);
        let begin = &frames[0];
        assert_eq!(&begin[..4], &[FILE_BEGIN, 2, b'r', b'1']);
        let len = u16::from_be_bytes([begin[4], begin[5]]) as usize;
        let meta: Value = serde_json::from_slice(&begin[6..]).unwrap();
        assert_eq!(len, begin.len() - 6);
        assert_eq!(meta["size"], json!(bytes.len()));
        assert_eq!(meta["fileName"], json!("a.txt"));
        assert_eq!(frames[1].len(), 4 + FILE_CHUNK_BYTES);
        assert_eq!(&frames[2][..4], &[FILE_CHUNK, 2, b'r', b'1']);
        assert_eq!(frames[2].len(), 4 + 3);
        assert_eq!(frames[3], vec![FILE_END, 2, b'r', b'1']);
    }

    use super::*;

    #[test]
    fn a_download_is_the_file_its_frames_carry_under_its_id() {
        let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
        let mut downloads = HashMap::from([("d1".to_owned(), Vec::new())]);
        // A frame of a download not asked for is dropped.
        let stray = file_frames("other", "x.bin", "application/octet-stream", "t", &bytes);
        assert!(stray.iter().all(|f| download_frame(f, &mut downloads).is_none()));
        let frames = file_frames("d1", "x.bin", "application/octet-stream", "t", &bytes);
        assert!(frames.len() > 3, "a file of several chunks");
        let (last, rest) = frames.split_last().unwrap();
        assert!(rest.iter().all(|f| download_frame(f, &mut downloads).is_none()));
        assert_eq!(download_frame(last, &mut downloads), Some(("d1".to_owned(), Ok(bytes))));
        assert!(downloads.is_empty());
    }

    /// A providers patch is saved, then the providers are asked again and
    /// listed; nothing is sent to a daemon here.
    #[test]
    fn a_providers_patch_refreshes_the_list() {
        let mut pending = HashMap::new();
        let patch = json!({ "providers": { "copilot": { "enabled": false } } });
        let sent = command_messages(Command::PatchProviders(patch.clone()), &mut pending);
        let msg = &sent[0]["message"];
        assert_eq!(msg["type"], "set_daemon_config_request");
        assert_eq!(msg["config"], patch);

        let mut follow_ups = vec![];
        let id = msg["requestId"].as_str().unwrap();
        let events = handle("set_daemon_config_response", &json!({ "requestId": id, "config": {} }), &mut pending, None, &mut follow_ups);
        assert!(matches!(&events[..], [Event::DaemonConfig(_)]));
        let refresh = &follow_ups.pop().unwrap()["message"];
        assert_eq!(refresh["type"], "refresh_providers_snapshot_request");

        let id = refresh["requestId"].as_str().unwrap();
        handle("refresh_providers_snapshot_response", &json!({ "requestId": id, "acknowledged": true }), &mut pending, None, &mut follow_ups);
        assert_eq!(follow_ups.pop().unwrap()["message"]["type"], "get_providers_snapshot_request");

        // A plain settings patch does not refresh providers.
        let sent = command_messages(Command::SetConfig(json!({ "autoArchiveAfterMerge": true })), &mut pending);
        let id = sent[0]["message"]["requestId"].as_str().unwrap().to_owned();
        handle("set_daemon_config_response", &json!({ "requestId": id, "config": {} }), &mut pending, None, &mut follow_ups);
        assert!(follow_ups.is_empty());

        // A refused patch reloads the config the page shows.
        let sent = command_messages(Command::PatchProviders(json!({ "removeProviders": ["x"] })), &mut pending);
        let id = sent[0]["message"]["requestId"].as_str().unwrap().to_owned();
        let events = handle("rpc_error", &json!({ "requestId": id, "error": "nope" }), &mut pending, None, &mut follow_ups);
        assert!(matches!(&events[..], [Event::Notice(_)]));
        assert_eq!(follow_ups.pop().unwrap()["message"]["type"], "get_daemon_config_request");
    }

    #[test]
    fn a_diagnostic_comes_back_as_its_text() {
        let mut pending = HashMap::new();
        let sent = command_messages(Command::ProviderDiagnostic("claude".into()), &mut pending);
        let msg = &sent[0]["message"];
        assert_eq!(msg["type"], "provider_diagnostic_request");
        assert_eq!(msg["provider"], "claude");
        let payload = json!({ "requestId": msg["requestId"], "provider": "claude", "diagnostic": "Claude Code\n  Version: 1.0" });
        let events = handle("provider_diagnostic_response", &payload, &mut pending, None, &mut vec![]);
        assert!(matches!(&events[..], [Event::Diagnostic { provider, text }] if provider == "claude" && text.contains("1.0")));
        assert!(pending.is_empty());
    }

    #[test]
    fn terminal_output_keeps_split_characters() {
        let slots = HashMap::from([(3u8, "t1".to_owned())]);
        let mut tails = HashMap::new();
        let bytes = "你".as_bytes();
        let first = terminal_frame(&[&[0x01, 3], &bytes[..2]].concat(), &slots, &mut tails);
        let second = terminal_frame(&[&[0x01, 3], &bytes[2..]].concat(), &slots, &mut tails);
        assert!(matches!(&first[0], Event::TerminalOutput { data, .. } if data.is_empty()));
        assert!(matches!(&second[0], Event::TerminalOutput { data, .. } if data == "你"));
    }

    /// Through the real relay to a running daemon:
    /// `SOPHON_RELAY_LINK='https://app.paseo.sh/#offer=…' cargo test relay_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn relay_live() {
        let link = std::env::var("SOPHON_RELAY_LINK").expect("SOPHON_RELAY_LINK");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            cmd_tx.send(Command::SwitchHost(Some(RemoteHost { url: link, password: None }))).unwrap();
            tokio::spawn(run(ev_tx, cmd_rx));
            let mut connected_relay = false;
            let deadline = tokio::time::sleep(Duration::from_secs(40));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    _ = &mut deadline => panic!("no answer through the relay"),
                    ev = ev_rx.recv() => match ev.unwrap() {
                        Event::Connecting { url } => {
                            println!("connecting: {url}");
                            connected_relay = url.contains("role=client");
                        }
                        Event::Connected { hostname, version } => println!("connected through relay={connected_relay}: {hostname} {version}"),
                        Event::AgentsReset(a) if connected_relay => {
                            println!("agents through the relay: {}", a.len());
                            break;
                        }
                        Event::Disconnected { reason } => println!("disconnected: {reason}"),
                        _ => {}
                    },
                }
            }
            drop(cmd_tx);
        });
    }

    /// The LAN bridge end to end, against the local daemon:
    /// `cargo test bridge_live -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn download_live() {
        // The local daemon; downloads a binary file larger than a read
        // shows (2 MB), then one that is not there.
        let dir = std::env::temp_dir().join(format!("sophon-download-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let bytes: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        std::fs::write(dir.join("big.bin"), &bytes).unwrap();
        let cwd = dir.to_string_lossy().into_owned();
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            let deadline = tokio::time::sleep(Duration::from_secs(20));
            tokio::pin!(deadline);
            let mut answers = 0;
            loop {
                tokio::select! {
                    _ = &mut deadline => panic!("no download answer"),
                    ev = ev_rx.recv() => match ev.unwrap() {
                        Event::Connected { .. } => {
                            cmd_tx.send(Command::Download { id: "d1".into(), cwd: cwd.clone(), path: "big.bin".into() }).unwrap();
                            cmd_tx.send(Command::Download { id: "d2".into(), cwd: cwd.clone(), path: "missing.bin".into() }).unwrap();
                        }
                        Event::Downloaded { id, result } => {
                            match id.as_str() {
                                "d1" => assert!(result.as_ref().is_ok_and(|b| *b == bytes), "the whole file"),
                                "d2" => assert!(result.is_err(), "a missing file is an error: {result:?}"),
                                other => panic!("unknown download {other}"),
                            }
                            println!("{id}: {}", match &result { Ok(b) => format!("{} bytes", b.len()), Err(e) => e.clone() });
                            answers += 1;
                            if answers == 2 {
                                break;
                            }
                        }
                        _ => {}
                    },
                }
            }
        });
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A file of a workspace by its path inside it, as the explorer asks:
    /// `SOPHON_TEST_DOWNLOAD=<cwd>|<path>|<size>`. Read only.
    #[test]
    #[ignore]
    fn download_workspace_file_live() {
        let Ok(spec) = std::env::var("SOPHON_TEST_DOWNLOAD") else { return };
        let mut parts = spec.splitn(3, '|');
        let (cwd, path, size) = (parts.next().unwrap().to_owned(), parts.next().unwrap().to_owned(), parts.next().unwrap().parse::<usize>().unwrap());
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            let deadline = tokio::time::sleep(Duration::from_secs(20));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    _ = &mut deadline => panic!("no download answer"),
                    ev = ev_rx.recv() => match ev.unwrap() {
                        Event::Connected { .. } => cmd_tx.send(Command::Download { id: "f".into(), cwd: cwd.clone(), path: path.clone() }).unwrap(),
                        Event::Downloaded { result, .. } => {
                            println!("downloaded: {:?}", result.as_ref().map(Vec::len));
                            assert_eq!(result.map(|b| b.len()), Ok(size));
                            return;
                        }
                        _ => {}
                    },
                }
            }
        });
    }

    #[test]
    fn what_came_after_the_kept_page_is_applied_as_live_rows() {
        let mut pending = HashMap::new();
        pending.insert("r1".to_owned(), Pending::After("a1".into(), 7));
        let mut follow_ups = vec![];
        let payload = json!({
            "requestId": "r1", "agentId": "a1", "epoch": "e", "reset": false, "hasNewer": false,
            "entries": [{ "seqStart": 8, "seqEnd": 9, "item": { "type": "assistant_message", "text": "hi" } }],
        });
        let events = handle("fetch_agent_timeline_response", &payload, &mut pending, Some("a1"), &mut follow_ups);
        assert!(follow_ups.is_empty());
        assert!(matches!(
            &events[..],
            [Event::TimelineLive { cursor: Some((e, 9)), item: Item::Assistant { text }, .. }, Event::TimelineSynced { agent_id }]
                if e == "e" && text == "hi" && agent_id == "a1"
        ));
    }

    #[test]
    fn a_kept_page_that_does_not_join_on_asks_for_the_latest_page_whole() {
        // More since than a page holds, and a row that began before the
        // kept page ended.
        for payload in [
            json!({ "requestId": "r1", "epoch": "e", "hasNewer": true, "entries": [] }),
            json!({ "requestId": "r1", "epoch": "e", "entries": [{ "seqStart": 6, "seqEnd": 9, "item": { "type": "assistant_message", "text": "hi" } }] }),
        ] {
            let mut pending = HashMap::new();
            pending.insert("r1".to_owned(), Pending::After("a1".into(), 7));
            let mut follow_ups = vec![];
            let events = handle("fetch_agent_timeline_response", &payload, &mut pending, Some("a1"), &mut follow_ups);
            assert!(events.is_empty());
            assert_eq!(follow_ups.len(), 1);
            assert_eq!(follow_ups[0]["message"]["direction"], "tail");
            assert!(pending.values().any(|p| matches!(p, Pending::Timeline(id) if id == "a1")));
        }
        // The host started the conversation's record anew: its answer is the latest page.
        let mut pending = HashMap::new();
        pending.insert("r1".to_owned(), Pending::After("a1".into(), 7));
        let payload = json!({ "requestId": "r1", "epoch": "e2", "reset": true, "entries": [{ "item": { "type": "assistant_message", "text": "new" } }] });
        let events = handle("fetch_agent_timeline_response", &payload, &mut pending, Some("a1"), &mut vec![]);
        assert!(matches!(&events[0], Event::TimelineReset { items, .. } if items.len() == 1));
    }

    #[test]
    fn a_trace_line_keeps_no_secrets() {
        assert_eq!(
            redact("lost: ws://sophon://pair?lan=10.0.0.1:1&lanKey=abc&token=xyz/ws: refused; https://app.x/#offer=eyJ password=p"),
            "lost: ws://sophon://pair?lan=10.0.0.1:1&lanKey=***&token=***/ws: refused; https://app.x/#offer=*** password=***"
        );
    }

    #[test]
    #[ignore]
    fn switching_away_from_an_unreachable_host_live() {
        // A LAN-only pairing link to nowhere, then this Mac's service: the
        // switch must not wait for the unreachable host to answer.
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (_cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            // A well-formed LAN-only link to an address that does not answer.
            let key = { use base64::Engine as _; base64::engine::general_purpose::STANDARD.encode([7u8; 32]) };
            let key = key.replace('+', "%2B").replace('/', "%2F").replace('=', "%3D");
            let link = format!("sophon://pair?lan=10.255.255.1:62584&lanKey={key}&token=t");
            assert!(super::super::relay::PairingLink::parse(&link).is_some_and(|l| l.lan.is_some()), "the link parses");
            // The first connection (this Mac's), then away to the unreachable host.
            loop {
                if let Some(Event::Connected { .. }) = ev_rx.recv().await { break; }
            }
            switch_host(Some(RemoteHost { url: link, password: None }));
            tokio::time::sleep(Duration::from_secs(2)).await;
            let asked = std::time::Instant::now();
            switch_host(None);
            let back = tokio::time::timeout(Duration::from_secs(3), async {
                loop {
                    if let Some(Event::Connected { .. }) = ev_rx.recv().await { break; }
                }
            })
            .await;
            println!("back on this Mac after {} ms", asked.elapsed().as_millis());
            assert!(back.is_ok(), "the switch waited on the unreachable host");
        });
    }

    #[test]
    #[ignore]
    fn agent_instance_add_and_remove_live() {
        // The local daemon: an API-key instance (a fake key, never used)
        // goes into its config, shows as a provider, then goes again.
        use super::super::agents::{Base, KeyInstance, key_instance_patch, remove_patch};
        const ID: &str = "sophon-live-test";
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            loop {
                if let Some(Event::Connected { .. }) = ev_rx.recv().await { break; }
            }
            let k = KeyInstance {
                label: "Sophon live test".into(),
                base: Base::Claude,
                base_url: "https://example.invalid/anthropic".into(),
                key: "not-a-key".into(),
                models: vec![("test-model".into(), "Test Model".into())],
                extra_env: vec![],
                claude_dir: None,
            };
            let t0 = std::time::Instant::now();
            cmd_tx.send(Command::SetConfig(key_instance_patch(ID, &k))).unwrap();
            cmd_tx.send(Command::RefreshProviders).unwrap();
            let shown = tokio::time::timeout(Duration::from_secs(30), async {
                loop {
                    if let Some(Event::Providers(list)) = ev_rx.recv().await {
                        if let Some(p) = list.iter().find(|p| p.id == ID) {
                            return (p.label.clone(), p.models.len(), p.status.clone());
                        }
                    }
                }
            })
            .await;
            println!("added: {shown:?} after {} ms", t0.elapsed().as_millis());
            let t1 = std::time::Instant::now();
            cmd_tx.send(Command::SetConfig(remove_patch(ID))).unwrap();
            cmd_tx.send(Command::RefreshProviders).unwrap();
            let gone = tokio::time::timeout(Duration::from_secs(30), async {
                loop {
                    if let Some(Event::Providers(list)) = ev_rx.recv().await {
                        if !list.iter().any(|p| p.id == ID) { return; }
                    }
                }
            })
            .await;
            println!("removed: {} after {} ms", gone.is_ok(), t1.elapsed().as_millis());
            assert!(shown.is_ok(), "the instance never showed");
            assert!(gone.is_ok(), "the instance never went");
        });
    }

    #[test]
    #[ignore]
    fn an_agent_runs_from_its_desktop_app_live() {
        // A service of its own with no `claude` or `codex` on its PATH,
        // never the user's: PASEO_LISTEN names it. Each goes from
        // unavailable to ready on the CLI its desktop app carries.
        let listen = std::env::var("PASEO_LISTEN").expect("PASEO_LISTEN: a test service");
        assert!(!listen.ends_with(":6767"), "not the user's own service");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            async fn status(id: &str, want: &str, ev_rx: &mut tokio::sync::mpsc::UnboundedReceiver<Event>) -> Option<usize> {
                let wait = async {
                    loop {
                        if let Some(Event::Providers(list)) = ev_rx.recv().await {
                            if let Some(p) = list.iter().find(|p| p.id == id && p.status == want) {
                                return p.models.len();
                            }
                        }
                    }
                };
                tokio::time::timeout(Duration::from_secs(60), wait).await.ok()
            }
            for id in ["claude", "codex"] {
                let bin = crate::bundled_cli::find(id).expect("the desktop app's CLI");
                cmd_tx.send(Command::RefreshProviders).unwrap();
                assert!(status(id, "unavailable", &mut ev_rx).await.is_some(), "{id} was not unavailable to begin with");
                cmd_tx.send(Command::PatchProviders(crate::bundled_cli::use_patch(id, &bin))).unwrap();
                cmd_tx.send(Command::RefreshProviders).unwrap();
                let models = status(id, "ready", &mut ev_rx).await.expect("never became ready");
                println!("{id} ready on its app's CLI, {models} models");
                assert!(models > 0);
            }
        });
    }

    #[test]
    #[ignore]
    fn agent_enable_toggle_live() {
        // The local daemon: `pi` (off here) on, then off again.
        use super::super::agents::enable_patch;
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            loop {
                if let Some(Event::Connected { .. }) = ev_rx.recv().await { break; }
            }
            for on in [true, false] {
                cmd_tx.send(Command::SetConfig(enable_patch("pi", on))).unwrap();
                cmd_tx.send(Command::RefreshProviders).unwrap();
                let seen = tokio::time::timeout(Duration::from_secs(30), async {
                    loop {
                        if let Some(Event::Providers(list)) = ev_rx.recv().await {
                            if list.iter().any(|p| p.id == "pi" && p.enabled == on) { return; }
                        }
                    }
                })
                .await;
                println!("pi enabled={on}: {}", seen.is_ok());
                assert!(seen.is_ok());
            }
        });
    }

    #[test]
    #[ignore]
    fn daemon_config_shape_live() {
        // The local daemon's config as it answers it: which keys it has.
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            loop {
                if let Some(Event::Connected { .. }) = ev_rx.recv().await { break; }
            }
            cmd_tx.send(Command::GetConfig).unwrap();
            let got = tokio::time::timeout(Duration::from_secs(10), async {
                loop {
                    if let Some(Event::DaemonConfig(c)) = ev_rx.recv().await { return c; }
                }
            })
            .await;
            match got {
                Ok(c) => {
                    for p in ["/mcp/injectIntoAgents", "/autoArchiveAfterMerge", "/idleRuntimeCleanup", "/resumeOnStartup", "/usage/enabled", "/relay/enabled", "/appendSystemPrompt"] {
                        println!("{p}: {}", c.pointer(p).map_or("MISSING".to_owned(), |v| v.to_string().chars().take(40).collect()));
                    }
                    println!("top keys: {:?}", c.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()));
                }
                Err(_) => println!("no config answer"),
            }
        });
    }

    #[test]
    #[ignore]
    fn host_flag_toggle_live() {
        // The local daemon: Archive after merge on, then off; each answer
        // carries the new value.
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            loop {
                if let Some(Event::Connected { .. }) = ev_rx.recv().await { break; }
            }
            for on in [true, false] {
                cmd_tx.send(Command::SetConfig(json!({ "autoArchiveAfterMerge": on }))).unwrap();
                let got = tokio::time::timeout(Duration::from_secs(10), async {
                    loop {
                        if let Some(Event::DaemonConfig(c)) = ev_rx.recv().await { return c; }
                    }
                })
                .await
                .expect("an answer");
                println!("set {on}: answered {:?}", got.get("autoArchiveAfterMerge"));
                assert_eq!(got.get("autoArchiveAfterMerge"), Some(&json!(on)));
            }
        });
    }

    #[test]
    #[ignore]
    fn upload_live() {
        // The local daemon; uploads a file and prints the attachment.
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            tokio::spawn(run(ev_tx, cmd_rx));
            let deadline = tokio::time::sleep(Duration::from_secs(20));
            tokio::pin!(deadline);
            let bytes: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
            loop {
                tokio::select! {
                    _ = &mut deadline => panic!("no upload answer"),
                    ev = ev_rx.recv() => match ev.unwrap() {
                        Event::Connected { .. } => {
                            cmd_tx.send(Command::Upload { id: "u1".into(), name: "sophon-test.bin".into(), mime: "application/octet-stream".into(), bytes: bytes.clone() }).unwrap();
                        }
                        Event::Uploaded { id, result } => {
                            println!("uploaded {id}: {result:?}");
                            let file = result.expect("upload");
                            assert_eq!(file["size"], json!(bytes.len()));
                            assert_eq!(std::fs::read(file["path"].as_str().unwrap()).unwrap(), bytes);
                            break;
                        }
                        _ => {}
                    },
                }
            }
        });
    }

    #[test]
    #[ignore]
    fn bridge_live() {
        let (info, _events) = super::super::bridge::start().expect("bridge");
        let link = super::super::relay::PairingLink::format(Some((&info.addr, &info.key, &info.token)), None);
        println!("pairing link: {link}");
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(async move {
            let (ev_tx, mut ev_rx) = tokio::sync::mpsc::unbounded_channel();
            let (cmd_tx, cmd_rx) = tokio::sync::mpsc::unbounded_channel();
            cmd_tx.send(Command::SwitchHost(Some(RemoteHost { url: link, password: None }))).unwrap();
            tokio::spawn(run(ev_tx, cmd_rx));
            let mut via_lan = false;
            let deadline = tokio::time::sleep(Duration::from_secs(20));
            tokio::pin!(deadline);
            loop {
                tokio::select! {
                    _ = &mut deadline => panic!("no answer through the LAN bridge"),

                    ev = ev_rx.recv() => match ev.unwrap() {
                        Event::Connecting { url } => { println!("connecting: {url}"); via_lan = !url.contains("127.0.0.1"); }
                        Event::Connected { hostname, .. } => println!("connected via LAN bridge={via_lan}: {hostname}"),
                        Event::AgentsReset(a) if via_lan => { println!("agents through the bridge: {}", a.len()); break; }
                        Event::Disconnected { reason } => println!("disconnected: {reason}"),
                        _ => {}
                    },
                }
            }
            drop(cmd_tx);
        });
    }

    #[test]
    fn remote_host_urls() {
        let h = |u: &str| RemoteHost { url: u.into(), password: None }.ws_url();
        assert_eq!(h("100.64.0.2:6767"), "ws://100.64.0.2:6767/ws");
        assert_eq!(h("https://daemon.example.com"), "wss://daemon.example.com/ws");
        assert_eq!(h("wss://x.y/ws"), "wss://x.y/ws");
    }

    #[test]
    fn stream_delta_becomes_live_event() {
        let mut pending = HashMap::new();
        let mut follow = vec![];
        let events = handle(
            "agent_stream",
            &json!({
                "agentId": "a1", "timestamp": "t",
                "event": {"type": "timeline", "provider": "claude", "item": {"type": "assistant_message", "text": "hi"}}
            }),
            &mut pending,
            Some("a1"),
            &mut follow,
        );
        assert!(matches!(&events[..], [Event::TimelineLive { agent_id, item: Item::Assistant { text }, .. }] if agent_id == "a1" && text == "hi"));
        assert!(follow.is_empty());
    }

    #[test]
    fn turn_end_refetches_open_timeline() {
        let mut pending = HashMap::new();
        let mut follow = vec![];
        handle(
            "agent_stream",
            &json!({"agentId": "a1", "timestamp": "t", "event": {"type": "turn_completed", "provider": "claude"}}),
            &mut pending,
            Some("a1"),
            &mut follow,
        );
        let types: Vec<_> = follow.iter().map(|m| m["message"]["type"].as_str().unwrap().to_owned()).collect();
        assert_eq!(types, ["fetch_agent_timeline_request", "fetch_agent_request"]);
    }

    #[test]
    fn timeline_response_routes_to_its_agent() {
        let mut pending = HashMap::new();
        let msgs = timeline_requests("a1", &mut pending);
        let rid = msgs[0]["message"]["requestId"].as_str().unwrap().to_owned();
        let events = handle(
            "fetch_agent_timeline_response",
            &json!({"requestId": rid, "entries": [{"item": {"type": "user_message", "text": "go"}}], "error": null}),
            &mut pending,
            Some("a1"),
            &mut vec![],
        );
        assert!(matches!(&events[0], Event::TimelineReset { agent_id, items, .. } if agent_id == "a1" && items.len() == 1));
    }

    #[test]
    fn permission_answer_uses_the_permission_id() {
        let msgs = command_messages(
            Command::Permit {
                agent_id: "a1".into(),
                permission_id: "p9".into(),
                allow: false,
                action_id: None,
                updated_input: None,
            },
            &mut HashMap::new(),
        );
        assert_eq!(msgs[0]["message"]["requestId"], "p9");
        assert_eq!(msgs[0]["message"]["response"]["behavior"], "deny");
    }
}
