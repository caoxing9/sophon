//! Gemini CLI, through the Agent Client Protocol (`gemini --acp`):
//! JSON-RPC over the process's input and output, a session for the
//! conversation and a prompt request for each turn, which is answered
//! once the turn is over.
//!
//! Its sign-in is Gemini CLI's own (`~/.gemini`): the variables of its
//! `.env` (an API key, an endpoint), which the CLI does not read itself
//! in this mode, are handed to it, and the way of signing in its
//! settings name is the one asked for.

use super::{Command, Emit, Event, IDLE, Outcome, Process, Start, tool};
use serde_json::{Value, json};
use std::{collections::HashMap, io, path::PathBuf, time::Duration};
use tokio::sync::mpsc::UnboundedReceiver;

/// What a request of ours was, for when its answer comes.
enum Sent {
    Prompt,
    Other,
}

struct Gemini {
    start: Start,
    emit: Emit,
    process: Option<Process>,
    session: Option<String>,
    busy: bool,
    stopping: bool,
    restart: bool,
    outcome: Outcome,
    next: u64,
    sent: HashMap<u64, Sent>,
    /// Prompts sent while a turn was underway: each a turn of its own
    /// once the one before it is over (a turn takes nothing more).
    held: Vec<Value>,
    /// Permissions asked, by the id the client knows them by: the
    /// request to answer and the options it offered.
    asks: HashMap<String, (Value, Value)>,
    /// Each call as it stands, its updates laid over it.
    calls: HashMap<String, Value>,
    /// The message the reply's last words were of.
    message: Option<String>,
    spoke: bool,
    fresh: bool,
    retried: bool,
    /// The modes the session has, by id.
    modes: Vec<String>,
}

pub async fn run(start: Start, mut inbox: UnboundedReceiver<Command>, emit: Emit) {
    let mut gemini = Gemini {
        session: start.session.clone(),
        start,
        emit,
        process: None,
        busy: false,
        stopping: false,
        restart: false,
        outcome: Outcome::Completed,
        next: 0,
        sent: HashMap::new(),
        held: Vec::new(),
        asks: HashMap::new(),
        calls: HashMap::new(),
        message: None,
        spoke: false,
        fresh: false,
        retried: false,
        modes: Vec::new(),
    };
    loop {
        tokio::select! {
            command = inbox.recv() => {
                let Some(command) = command else { break };
                if let Err(error) = gemini.command(command).await {
                    gemini.fail(error.to_string());
                }
            }
            line = super::line(&mut gemini.process) => match line {
                Some(line) => {
                    if let Err(error) = gemini.read(&line).await {
                        gemini.fail(error.to_string());
                    }
                }
                None => gemini.stopped().await,
            },
            _ = tokio::time::sleep(IDLE), if gemini.process.is_some() && !gemini.busy => gemini.close(),
        }
    }
}

/// Gemini CLI's folder: `GEMINI_CLI_HOME` (its `.gemini` inside) if the
/// provider's settings or the host name one, else `~/.gemini`.
fn folder(settings: &Value) -> Option<PathBuf> {
    let named = settings
        .pointer("/env/GEMINI_CLI_HOME")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("GEMINI_CLI_HOME").map(PathBuf::from));
    Some(match named {
        Some(home) => home.join(".gemini"),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".gemini"),
    })
}

/// What Gemini CLI's lowest layer of settings (its system defaults, under
/// the user's and the project's) says with `AGENTS.md` among the files of
/// instructions it reads: the file every agent reads, as Codex does
/// (`GEMINI.md` alone otherwise). `existing`: what that layer says now.
/// `None`: it reads it already, or was told what to read and not this.
fn with_agents_md(mut existing: Value) -> Option<Value> {
    const AGENTS: &str = "AGENTS.md";
    let files = match &existing["context"]["fileName"] {
        Value::Null => vec![json!(AGENTS), json!("GEMINI.md")],
        Value::String(name) if name == AGENTS => return None,
        Value::String(name) => vec![json!(AGENTS), json!(name)],
        Value::Array(names) if names.iter().any(|name| name == AGENTS) => return None,
        Value::Array(names) => std::iter::once(json!(AGENTS)).chain(names.iter().cloned()).collect(),
        _ => return None,
    };
    if !existing.is_object() {
        existing = json!({});
    }
    if !existing["context"].is_object() {
        existing["context"] = json!({});
    }
    existing["context"]["fileName"] = json!(files);
    Some(existing)
}

/// A file of Gemini CLI's system defaults that reads `AGENTS.md`, for
/// `GEMINI_CLI_SYSTEM_DEFAULTS_PATH`: the host's own defaults (if any)
/// with it added, written where the service keeps such things. `None`
/// when that variable is named already or nothing needs adding.
fn system_defaults(settings: &Value, env: &[(&str, &str)]) -> Option<PathBuf> {
    let named = |name: &str| {
        env.iter()
            .find(|(own, _)| *own == name)
            .map(|(_, value)| value.to_string())
            .or_else(|| settings.pointer(&format!("/env/{name}")).and_then(Value::as_str).map(str::to_owned))
            .or_else(|| std::env::var(name).ok())
            .filter(|value| !value.is_empty())
    };
    if named("GEMINI_CLI_SYSTEM_DEFAULTS_PATH").is_some() {
        return None;
    }
    // Where Gemini CLI looks for them: beside its system settings.
    let system = named("GEMINI_CLI_SYSTEM_SETTINGS_PATH").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(if cfg!(target_os = "macos") {
            "/Library/Application Support/GeminiCli/settings.json"
        } else if cfg!(windows) {
            "C:\\ProgramData\\gemini-cli\\settings.json"
        } else {
            "/etc/gemini-cli/settings.json"
        })
    });
    let existing = system
        .parent()
        .and_then(|dir| std::fs::read_to_string(dir.join("system-defaults.json")).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(json!({}));
    let defaults = with_agents_md(existing)?;
    let path = std::env::temp_dir().join("sophon").join("gemini-system-defaults.json");
    std::fs::create_dir_all(path.parent()?).ok()?;
    std::fs::write(&path, defaults.to_string()).ok()?;
    Some(path)
}

/// The variables of a `.env` file: `NAME=value` lines, a value in quotes
/// without them; comments and blank lines are not.
fn dotenv(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| {
            let line = line.strip_prefix("export ").unwrap_or(line);
            let (name, value) = line.split_once('=')?;
            let name = name.trim();
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            (!name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
                .then(|| (name.to_owned(), value.to_owned()))
        })
        .collect()
}

/// The way of signing in Gemini CLI's settings name, else the one its
/// variables point to; `None` to take the first it offers.
fn sign_in(settings: &Value, env: &[(String, String)]) -> Option<String> {
    let chosen = folder(settings)
        .and_then(|dir| std::fs::read(dir.join("settings.json")).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|kept| {
            kept.pointer("/security/auth/selectedType")
                .or_else(|| kept.get("selectedAuthType"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    chosen.or_else(|| {
        let has = |name: &str| {
            env.iter()
                .any(|(key, value)| key == name && !value.is_empty())
                || std::env::var(name).is_ok_and(|value| !value.is_empty())
        };
        if has("GEMINI_API_KEY") {
            Some("gemini-api-key".to_owned())
        } else if has("GOOGLE_API_KEY") || has("GOOGLE_CLOUD_PROJECT") {
            Some("vertex-ai".to_owned())
        } else {
            None
        }
    })
}

fn status(update: &Value) -> &'static str {
    match update["status"].as_str() {
        Some("completed") => "completed",
        Some("failed") => "failed",
        Some("cancelled") => "canceled",
        _ => "running",
    }
}

/// The words of a call's content blocks, one after another.
fn content_text(call: &Value) -> String {
    call["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block["content"]["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

/// A call as the timeline shows it: by what kind of thing it does, with
/// what it was given and what came of it.
fn row(id: &str, call: &Value) -> Value {
    let input = &call["rawInput"];
    let title = call["title"].as_str().unwrap_or_default();
    let path = call["locations"][0]["path"]
        .as_str()
        .or_else(|| input["file_path"].as_str())
        .or_else(|| input["absolute_path"].as_str())
        .or_else(|| input["path"].as_str());
    let output = content_text(call);
    let diff = call["content"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|block| block["type"] == "diff");
    let (name, detail) = match call["kind"].as_str() {
        Some("execute") => (
            "shell",
            json!({"type":"shell","command":input["command"].as_str().unwrap_or(title),
                "cwd":input["dir_path"].as_str().or(input["directory"].as_str()),
                "output":if output.is_empty() { call["rawOutput"].clone() } else { json!(output) }}),
        ),
        Some("read") => (
            "read",
            json!({"type":"read","filePath":path.unwrap_or(title)}),
        ),
        Some("edit" | "delete" | "move") => match diff {
            Some(diff) if diff["oldText"].is_null() => (
                "write",
                json!({"type":"write","filePath":diff["path"],"content":diff["newText"]}),
            ),
            Some(diff) => (
                "edit",
                json!({"type":"edit","filePath":diff["path"],"oldString":diff["oldText"],"newString":diff["newText"]}),
            ),
            None => (
                "edit",
                json!({"type":"edit","filePath":path.unwrap_or(title)}),
            ),
        },
        Some("search") => (
            "search",
            json!({"type":"search","query":input["pattern"].as_str().or(input["query"].as_str()).unwrap_or(title)}),
        ),
        Some("fetch") => (
            "fetch",
            json!({"type":"fetch","url":input["url"].as_str().or(input["prompt"].as_str()).unwrap_or(title)}),
        ),
        _ => (
            "tool",
            json!({"type":"unknown","input":input,
                "output":if output.is_empty() { call["rawOutput"].clone() } else { json!(output) }}),
        ),
    };
    // Named by what Gemini called it, where it said.
    let name = if title.is_empty() { name } else { title };
    let failed = call["status"] == "failed";
    tool(
        id,
        name,
        status(call),
        detail,
        failed
            .then_some(output.as_str())
            .filter(|said| !said.is_empty()),
    )
}

impl Gemini {
    fn item(&mut self, item: Value) {
        self.spoke = false;
        (self.emit)(Event::Item(item));
    }

    fn reply(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let text = if self.fresh && self.spoke {
            format!("\n\n{text}")
        } else {
            text.to_owned()
        };
        self.fresh = false;
        (self.emit)(Event::Item(json!({"type":"assistant_message","text":text})));
        self.spoke = true;
    }

    async fn send(&mut self, message: Value) -> io::Result<()> {
        match &mut self.process {
            Some(process) => process.send(&message).await,
            None => Ok(()),
        }
    }

    async fn request(&mut self, method: &str, params: Value, sent: Sent) -> io::Result<u64> {
        self.next += 1;
        let id = self.next;
        self.sent.insert(id, sent);
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        Ok(id)
    }

    /// A request answered before anything else is done: the answer, or
    /// the error Gemini gave (its code and words). What comes meanwhile
    /// (a loaded session's history said again) is not this
    /// conversation's news: it has it already.
    async fn call(
        &mut self,
        method: &str,
        params: Value,
    ) -> io::Result<Result<Value, (i64, String)>> {
        let id = self.request(method, params, Sent::Other).await?;
        self.sent.remove(&id);
        loop {
            let line = super::line(&mut self.process)
                .await
                .ok_or_else(|| io::Error::other("Gemini CLI stopped while starting"))?;
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if message["id"] != id || !message["method"].is_null() {
                // A request of its own meanwhile is refused, so it does
                // not wait.
                if !message["method"].is_null() && !message["id"].is_null() {
                    self.send(json!({"jsonrpc":"2.0","id":message["id"],
                        "error":{"code":-32601,"message":"Not supported by this host"}}))
                        .await?;
                }
                continue;
            }
            return Ok(match message.get("error") {
                Some(error) if !error.is_null() => Err((
                    error["code"].as_i64().unwrap_or(0),
                    error["message"]
                        .as_str()
                        .unwrap_or("Gemini CLI refused")
                        .to_owned(),
                )),
                _ => Ok(message["result"].clone()),
            });
        }
    }

    fn close(&mut self) {
        self.process = None;
        self.sent.clear();
        self.calls.clear();
        self.restart = false;
    }

    /// Starts Gemini CLI and opens the conversation's session in it:
    /// the one it had, else a new one.
    async fn open(&mut self) -> io::Result<()> {
        if self.process.is_some() {
            return Ok(());
        }
        let settings = self.start.settings.clone();
        let mut args = vec!["--acp".to_owned()];
        if let Some(model) = self.start.model.clone().filter(|model| !model.is_empty()) {
            args.extend(["--model".to_owned(), model]);
        }
        // Its `.env`, under what the host and the agent's run already say.
        let kept = folder(&settings)
            .and_then(|dir| std::fs::read_to_string(dir.join(".env")).ok())
            .map(|text| dotenv(&text))
            .unwrap_or_default();
        let given: Vec<(String, String)> = kept
            .into_iter()
            .filter(|(name, _)| std::env::var_os(name).is_none())
            .filter(|(name, _)| !self.start.env.iter().any(|(own, _)| own == name))
            .collect();
        let way = sign_in(&settings, &given);
        let mut env: Vec<_> = given
            .iter()
            .chain(self.start.env.iter())
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        // The project's `AGENTS.md`, read as every other agent reads it.
        let defaults = system_defaults(&settings, &env);
        let defaults = defaults.as_deref().and_then(|path| path.to_str());
        if let Some(path) = defaults {
            env.push(("GEMINI_CLI_SYSTEM_DEFAULTS_PATH", path));
        }
        self.process = Some(Process::spawn(
            "gemini",
            &settings,
            &self.start.cwd,
            &args,
            &env,
        )?);
        let cwd = self.start.cwd.clone();
        let wanted = self.session.clone();
        let opening = async {
            let started = self
                .call(
                    "initialize",
                    json!({"protocolVersion":1,
                        "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},
                        "clientInfo":{"name":"sophon","title":"Sophon","version":env!("CARGO_PKG_VERSION")}}),
                )
                .await?
                .map_err(|(_, said)| io::Error::other(said))?;
            let offered: Vec<String> = started["authMethods"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|method| method["id"].as_str().map(str::to_owned))
                .collect();
            let loads = started["agentCapabilities"]["loadSession"] == true;
            let mut signed = false;
            loop {
                let opened = match wanted.as_deref().filter(|_| loads) {
                    Some(session) => {
                        let loaded = self
                            .call(
                                "session/load",
                                json!({"sessionId":session,"cwd":cwd,"mcpServers":[]}),
                            )
                            .await?;
                        // A session Gemini no longer has: a new one, which
                        // goes on from here without what was said.
                        match loaded {
                            Ok(result) => Ok((session.to_owned(), result)),
                            Err((code, said)) if code != -32000 => {
                                eprintln!(
                                    "gemini: session {session} could not be loaded ({said}): a new one"
                                );
                                self.call("session/new", json!({"cwd":cwd,"mcpServers":[]}))
                                    .await?
                                    .map(|result| {
                                        (
                                            result["sessionId"]
                                                .as_str()
                                                .unwrap_or_default()
                                                .to_owned(),
                                            result,
                                        )
                                    })
                            }
                            Err(error) => Err(error),
                        }
                    }
                    None => self
                        .call("session/new", json!({"cwd":cwd,"mcpServers":[]}))
                        .await?
                        .map(|result| {
                            (
                                result["sessionId"].as_str().unwrap_or_default().to_owned(),
                                result,
                            )
                        }),
                };
                match opened {
                    Ok(opened) => return Ok(opened),
                    // Not signed in yet: once, the way its settings name.
                    Err((-32000, _)) if !signed && !offered.is_empty() => {
                        signed = true;
                        let method = way
                            .clone()
                            .filter(|way| offered.contains(way))
                            .unwrap_or_else(|| offered[0].clone());
                        self.call("authenticate", json!({"methodId":method}))
                            .await?
                            .map_err(|(_, said)| io::Error::other(said))?;
                    }
                    Err((_, said)) => return Err(io::Error::other(said)),
                }
            }
        };
        let (session, opened) = tokio::time::timeout(Duration::from_secs(60), opening)
            .await
            .map_err(|_| io::Error::other("Gemini CLI did not start in time"))??;
        if session.is_empty() {
            return Err(io::Error::other("Gemini CLI opened no session"));
        }
        self.modes = opened["modes"]["availableModes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|mode| mode["id"].as_str().map(str::to_owned))
            .collect();
        if self.start.session.as_deref() != Some(session.as_str()) {
            self.start.session = Some(session.clone());
            (self.emit)(Event::Session(session.clone()));
        }
        self.session = Some(session);
        // The mode it is in is the conversation's.
        let current = opened["modes"]["currentModeId"]
            .as_str()
            .unwrap_or_default();
        if !self.start.mode.is_empty() && current != self.start.mode {
            self.set_mode().await?;
        }
        Ok(())
    }

    async fn set_mode(&mut self) -> io::Result<()> {
        if self.session.is_none() || !self.modes.contains(&self.start.mode) {
            return Ok(());
        }
        let params = json!({"sessionId":self.session,"modeId":self.start.mode});
        self.request("session/set_mode", params, Sent::Other)
            .await
            .map(drop)
    }

    fn begin(&mut self) {
        self.busy = true;
        self.stopping = false;
        self.outcome = Outcome::Completed;
        self.retried = false;
        self.message = None;
        (self.emit)(Event::Busy);
    }

    fn idle(&mut self) {
        if !self.busy {
            return;
        }
        self.busy = false;
        self.spoke = false;
        for id in std::mem::take(&mut self.asks).into_keys() {
            (self.emit)(Event::Settled(id));
        }
        (self.emit)(Event::Idle(std::mem::replace(
            &mut self.outcome,
            Outcome::Completed,
        )));
        if self.restart {
            self.close();
        }
    }

    fn fail(&mut self, error: String) {
        self.close();
        self.held.clear();
        self.busy = true;
        self.outcome = Outcome::Failed(error);
        self.idle();
    }

    async fn stopped(&mut self) {
        let Some(process) = self.process.take() else {
            return;
        };
        let why = process.ended().await;
        self.close();
        if self.busy {
            self.held.clear();
            self.outcome = if self.stopping {
                Outcome::Canceled
            } else {
                Outcome::Failed(format!("Gemini CLI stopped ({why})"))
            };
            self.idle();
        }
    }

    /// A turn of its own for `prompt`.
    async fn prompt(&mut self, prompt: Value) -> io::Result<()> {
        self.open().await?;
        self.begin();
        let params = json!({"sessionId":self.session,"prompt":prompt});
        self.request("session/prompt", params, Sent::Prompt)
            .await
            .map(drop)
    }

    async fn command(&mut self, command: Command) -> io::Result<()> {
        match command {
            Command::Prompt { text, images, .. } => {
                let mut prompt = vec![json!({"type":"text","text":text})];
                for image in &images {
                    let (mime, _, data) = super::image(image)?;
                    prompt.push(json!({"type":"image","data":data,"mimeType":mime}));
                }
                if self.busy {
                    self.held.push(json!(prompt));
                    return Ok(());
                }
                self.prompt(json!(prompt)).await
            }
            Command::Interrupt => {
                if !self.busy {
                    return Ok(());
                }
                self.stopping = true;
                self.held.clear();
                // What it waits on is answered as cancelled, as the
                // protocol asks of a client that cancels.
                for (id, (request, _)) in std::mem::take(&mut self.asks) {
                    (self.emit)(Event::Settled(id));
                    self.send(json!({"jsonrpc":"2.0","id":request,
                        "result":{"outcome":{"outcome":"cancelled"}}}))
                        .await?;
                }
                let session = self.session.clone();
                self.send(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":session}}))
                    .await
            }
            Command::Answer { id, response } => {
                let Some((request, options)) = self.asks.remove(&id) else {
                    return Ok(());
                };
                let allowed = response["behavior"] == "allow";
                let wanted = match (allowed, response["selectedActionId"].as_str()) {
                    (false, _) => ["reject_once", "reject_always"],
                    (true, Some("allow_always")) => ["allow_always", "allow_once"],
                    (true, _) => ["allow_once", "allow_always"],
                };
                let options: Vec<&Value> = options.as_array().into_iter().flatten().collect();
                let chosen = wanted
                    .iter()
                    .find_map(|kind| options.iter().find(|option| option["kind"] == *kind))
                    .and_then(|option| option["optionId"].as_str());
                let outcome = match chosen {
                    Some(option) => json!({"outcome":"selected","optionId":option}),
                    None => json!({"outcome":"cancelled"}),
                };
                (self.emit)(Event::Settled(id));
                self.send(json!({"jsonrpc":"2.0","id":request,"result":{"outcome":outcome}}))
                    .await
            }
            Command::Mode(mode) => {
                self.start.mode = mode.clone();
                (self.emit)(Event::Mode(mode));
                if self.process.is_some() {
                    self.set_mode().await?;
                }
                Ok(())
            }
            // Gemini CLI takes its model as it starts: the next turn starts
            // it again on the session it has.
            Command::Model(model) => {
                self.start.model = model;
                if self.busy {
                    self.restart = true;
                } else {
                    self.close();
                }
                Ok(())
            }
            Command::Thinking(thinking) => {
                self.start.thinking = thinking;
                Ok(())
            }
            Command::StopTask(_) => Ok(()),
            Command::Rewind { .. } => {
                (self.emit)(Event::Rewound(Err(
                    "Gemini CLI cannot go back in a conversation from here".into(),
                )));
                Ok(())
            }
        }
    }

    async fn read(&mut self, line: &str) -> io::Result<()> {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        let params = &message["params"];
        match (message["method"].as_str(), message["id"].is_null()) {
            (Some(method), false) => self.asked(method, &message["id"], params).await,
            (Some("session/update"), true) => {
                self.update(&params["update"]);
                Ok(())
            }
            (Some(_), true) => Ok(()),
            (None, _) => {
                let Some(sent) = message["id"].as_u64().and_then(|id| self.sent.remove(&id)) else {
                    return Ok(());
                };
                let Sent::Prompt = sent else { return Ok(()) };
                let error = message["error"]["message"].as_str();
                self.outcome = match (error, message["result"]["stopReason"].as_str()) {
                    (Some(error), _) => Outcome::Failed(error.to_owned()),
                    (None, Some("cancelled")) => Outcome::Canceled,
                    (None, Some("refusal")) => Outcome::Failed("Gemini refused to go on".into()),
                    _ if self.stopping => Outcome::Canceled,
                    _ => std::mem::replace(&mut self.outcome, Outcome::Completed),
                };
                self.idle();
                // The next of those sent meanwhile, as a turn of its own.
                if !self.held.is_empty() && self.process.is_some() {
                    let next = self.held.remove(0);
                    self.prompt(next).await?;
                }
                Ok(())
            }
        }
    }

    /// A request of Gemini's: a permission, shown as one; anything else is
    /// refused (this host offers it no files or terminals of its own:
    /// it uses its own), so Gemini does not wait.
    async fn asked(&mut self, method: &str, request: &Value, params: &Value) -> io::Result<()> {
        if method != "session/request_permission" {
            return self
                .send(json!({"jsonrpc":"2.0","id":request,
                    "error":{"code":-32601,"message":"Not supported by this host"}}))
                .await;
        }
        let call = &params["toolCall"];
        let id = call["toolCallId"].as_str().unwrap_or_default();
        // What is known of the call: its update laid over it.
        let mut known = self.calls.get(id).cloned().unwrap_or_else(|| json!({}));
        if let (Some(known), Some(update)) = (known.as_object_mut(), call.as_object()) {
            for (key, value) in update {
                if !value.is_null() {
                    known.insert(key.clone(), value.clone());
                }
            }
        }
        let shown = row(id, &known);
        let input = &known["rawInput"];
        let asked = crate::state::id();
        let always = params["options"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|option| option["kind"] == "allow_always");
        let mut actions = vec![json!({"id":"allow","label":"Allow","behavior":"allow"})];
        if always {
            actions.push(json!({"id":"allow_always","label":"Always allow","behavior":"allow"}));
        }
        actions.push(json!({"id":"deny","label":"Deny","behavior":"deny"}));
        (self.emit)(Event::Asked(
            json!({"id":asked,"provider":"gemini","name":shown["name"],
            "kind":"tool","input":if input.is_object() { input.clone() } else { json!({"description":known["title"]}) },
            "detail":shown["detail"],"actions":actions}),
        ));
        self.asks
            .insert(asked, (request.clone(), params["options"].clone()));
        Ok(())
    }

    fn update(&mut self, update: &Value) {
        match update["sessionUpdate"].as_str() {
            Some("agent_message_chunk") => {
                let message = update["messageId"].as_str().map(str::to_owned);
                if message.is_some() && message != self.message {
                    self.fresh = true;
                    self.message = message;
                }
                if let Some(text) = update["content"]["text"].as_str() {
                    self.reply(text);
                }
            }
            Some("agent_thought_chunk") => {
                let Some(text) = update["content"]["text"]
                    .as_str()
                    .filter(|text| !text.is_empty())
                else {
                    return;
                };
                // Its retries are said as thoughts: said once a turn, as a
                // retry, not as its thinking.
                if text.starts_with("[Retry ") {
                    if !self.retried {
                        self.retried = true;
                        let why = text.split_once("] ").map_or(text, |(_, rest)| rest);
                        self.item(json!({"type":"notification",
                            "message":format!("The model endpoint is not answering. Gemini CLI is retrying: {why}")}));
                    }
                    return;
                }
                self.fresh = true;
                self.item(json!({"type":"reasoning","text":text}));
            }
            Some(kind @ ("tool_call" | "tool_call_update")) => {
                let Some(id) = update["toolCallId"].as_str().map(str::to_owned) else {
                    return;
                };
                let call = self.calls.entry(id.clone()).or_insert_with(|| json!({}));
                if kind == "tool_call" {
                    *call = json!({});
                }
                if let (Some(call), Some(update)) = (call.as_object_mut(), update.as_object()) {
                    for (key, value) in update {
                        if !value.is_null() && key != "sessionUpdate" {
                            call.insert(key.clone(), value.clone());
                        }
                    }
                }
                let shown = row(&id, &self.calls[&id]);
                self.fresh = true;
                self.item(shown);
            }
            Some("plan") => {
                let items: Vec<_> = update["entries"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|entry| json!({"text":entry["content"],"completed":entry["status"] == "completed"}))
                    .collect();
                self.item(json!({"type":"todo","items":items}));
            }
            Some("current_mode_update") => {
                if let Some(mode) = update["currentModeId"].as_str() {
                    self.start.mode = mode.to_owned();
                    (self.emit)(Event::Mode(mode.to_owned()));
                }
            }
            Some("usage_update") => {
                (self.emit)(Event::Usage(
                    json!({"contextWindowUsedTokens":update["used"],"contextWindowMaxTokens":update["size"]}),
                ));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agents_md_is_added_to_what_gemini_reads_unless_it_is_there() {
        assert_eq!(
            with_agents_md(json!({})),
            Some(json!({"context":{"fileName":["AGENTS.md","GEMINI.md"]}}))
        );
        assert_eq!(
            with_agents_md(json!({"ui":{"theme":"x"},"context":{"fileName":"RULES.md"}})),
            Some(json!({"ui":{"theme":"x"},"context":{"fileName":["AGENTS.md","RULES.md"]}}))
        );
        assert_eq!(with_agents_md(json!({"context":{"fileName":["GEMINI.md","AGENTS.md"]}})), None);
        assert_eq!(with_agents_md(json!({"context":{"fileName":"AGENTS.md"}})), None);
    }

    #[test]
    fn a_dotenv_file_is_its_variables() {
        let text = "# keys\nGEMINI_API_KEY=\"k-1\"\nexport GOOGLE_GEMINI_BASE_URL=https://g.example\n\nbad line\nEMPTY=\n";
        assert_eq!(
            dotenv(text),
            [
                ("GEMINI_API_KEY".into(), "k-1".into()),
                ("GOOGLE_GEMINI_BASE_URL".into(), "https://g.example".into()),
                ("EMPTY".into(), String::new()),
            ]
        );
    }

    #[test]
    fn a_call_is_a_row_by_what_it_does() {
        let ran = row(
            "c1",
            &json!({"title":"ls -la","kind":"execute","status":"completed","rawInput":{"command":"ls -la"},
                "content":[{"type":"content","content":{"type":"text","text":"a\nb"}}]}),
        );
        assert_eq!(ran["status"], "completed");
        assert_eq!(ran["detail"]["type"], "shell");
        assert_eq!(ran["detail"]["command"], "ls -la");
        assert_eq!(ran["detail"]["output"], "a\nb");
        let wrote = row(
            "c2",
            &json!({"title":"hello.txt","kind":"edit","status":"pending",
                "content":[{"type":"diff","path":"/w/hello.txt","oldText":null,"newText":"hi"}]}),
        );
        assert_eq!(
            (&wrote["status"], &wrote["detail"]),
            (
                &json!("running"),
                &json!({"type":"write","filePath":"/w/hello.txt","content":"hi"})
            )
        );
        let edited = row(
            "c3",
            &json!({"kind":"edit","status":"failed","content":[{"type":"diff","path":"/w/a","oldText":"x","newText":"y"},
                {"type":"content","content":{"type":"text","text":"no such file"}}]}),
        );
        assert_eq!(edited["detail"]["oldString"], "x");
        assert_eq!(edited["error"], "no such file");
        let read = row(
            "c4",
            &json!({"kind":"read","locations":[{"path":"/w/a.rs"}]}),
        );
        assert_eq!(read["detail"], json!({"type":"read","filePath":"/w/a.rs"}));
    }
}
