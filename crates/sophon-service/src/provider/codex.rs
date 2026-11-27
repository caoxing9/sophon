//! Codex, through its app server: JSON-RPC over the process's input and
//! output, a thread for the conversation and a turn for each prompt.
//!
//! Which account, endpoint and approval settings it uses is Codex's own
//! to say (`~/.codex`), unless the provider's settings name an endpoint
//! or the agent's mode asks for more or less than Codex's defaults.

use super::{Command, Emit, Event, IDLE, Outcome, Process, Start, tool};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io,
    time::Duration,
};
use tokio::sync::mpsc::UnboundedReceiver;

/// What a request of ours was, for when its answer comes.
enum Sent {
    Turn,
    /// Into the turn underway; if that turn was over, this starts one.
    Steer(Value),
    Other,
    /// Taking the thread back some turns.
    Rollback,
}

/// What Codex waits on: the request to answer, and how.
struct Ask {
    request: Value,
    /// The questions asked (each with the id its answer goes under), or
    /// none for an approval.
    questions: Option<Value>,
}

struct Codex {
    start: Start,
    emit: Emit,
    process: Option<Process>,
    thread: Option<String>,
    turn: Option<String>,
    busy: bool,
    stopping: bool,
    restart: bool,
    outcome: Outcome,
    next: u64,
    sent: HashMap<u64, Sent>,
    /// Prompts taken between a turn's start being asked for and its id
    /// being known; they go into it once it is.
    held: Vec<Value>,
    asks: HashMap<String, Ask>,
    streamed: HashSet<String>,
    spoke: bool,
    fresh: bool,
    retried: bool,
    /// The images of the turn underway, kept as files while it runs.
    images: Vec<tempfile::NamedTempFile>,
    /// The skills Codex has where the agent runs, by name: their files.
    skills: HashMap<String, String>,
}

pub async fn run(start: Start, mut inbox: UnboundedReceiver<Command>, emit: Emit) {
    let mut codex = Codex {
        thread: None,
        start,
        emit,
        process: None,
        turn: None,
        busy: false,
        stopping: false,
        restart: false,
        outcome: Outcome::Completed,
        next: 0,
        sent: HashMap::new(),
        held: Vec::new(),
        asks: HashMap::new(),
        streamed: HashSet::new(),
        spoke: false,
        fresh: false,
        retried: false,
        images: Vec::new(),
        skills: HashMap::new(),
    };
    loop {
        tokio::select! {
            command = inbox.recv() => {
                let Some(command) = command else { break };
                if let Err(error) = codex.command(command).await {
                    codex.fail(error.to_string());
                }
            }
            line = super::line(&mut codex.process) => match line {
                Some(line) => {
                    if let Err(error) = codex.read(&line).await {
                        codex.fail(error.to_string());
                    }
                }
                None => codex.stopped().await,
            },
            _ = tokio::time::sleep(IDLE), if codex.process.is_some() && !codex.busy => codex.close(),
        }
    }
}

fn status(item: &Value) -> &'static str {
    match item["status"].as_str() {
        Some("completed") => "completed",
        Some("failed") => "failed",
        Some("declined") => "canceled",
        _ => "running",
    }
}

/// The timeline's rows for one of Codex's items, as it stands.
fn rows(item: &Value) -> Vec<Value> {
    let id = item["id"].as_str().unwrap_or_default();
    match item["type"].as_str() {
        Some("commandExecution") => vec![tool(
            id,
            "shell",
            status(item),
            json!({"type":"shell","command":item["command"],"cwd":item["cwd"],
                "output":item["aggregatedOutput"],"exitCode":item["exitCode"]}),
            None,
        )],
        // A row a file, each with its own diff.
        Some("fileChange") => item["changes"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .map(|(index, change)| {
                let detail = if change["kind"]["type"] == "add" {
                    json!({"type":"write","filePath":change["path"],"content":change["diff"]})
                } else {
                    json!({"type":"edit","filePath":change["path"],"unifiedDiff":change["diff"]})
                };
                tool(
                    &format!("{id}:{index}"),
                    "apply_patch",
                    status(item),
                    detail,
                    None,
                )
            })
            .collect(),
        Some("mcpToolCall") => vec![tool(
            id,
            &format!(
                "{}.{}",
                item["server"].as_str().unwrap_or_default(),
                item["tool"].as_str().unwrap_or_default()
            ),
            status(item),
            json!({"type":"unknown","input":item["arguments"],
                "output":if item["error"].is_null() { &item["result"] } else { &item["error"] }}),
            item["error"]["message"].as_str(),
        )],
        Some("webSearch") => vec![tool(
            id,
            "web_search",
            status(item),
            json!({"type":"search","query":item["query"],"toolName":"web_search"}),
            None,
        )],
        Some("collabAgentToolCall") => vec![tool(
            id,
            "agent",
            status(item),
            json!({"type":"sub_agent","description":item["prompt"],"log":""}),
            None,
        )],
        _ => vec![],
    }
}

/// Codex's own commands Sophon offers when `/` is typed: the ones its
/// app server does here (`compact`, `review`) and the ones Sophon does
/// with its own screens (`slash::native` in the app). The rest of
/// Codex's are its terminal's.
const COMMANDS: [(&str, &str); 8] = [
    ("compact", "Summarize the conversation to free up context"),
    ("review", "Review the uncommitted changes, or what follows the command"),
    ("model", "Choose the model and its reasoning"),
    ("permissions", "Choose what Codex may do without asking"),
    ("new", "Start a new conversation"),
    ("resume", "Open an earlier conversation"),
    ("status", "Show the model, the mode and the computer"),
    ("memory", "Edit the project's AGENTS.md"),
];

/// The skills in an answer to `skills/list`, enabled ones: each as a
/// command is listed (`name`, `description`, `kind`) with its `path`.
fn skills_of(result: &Value) -> Vec<Value> {
    result["data"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|entry| entry["skills"].as_array().into_iter().flatten())
        .filter(|skill| skill["enabled"] != false)
        .filter_map(|skill| {
            let description = skill["interface"]["shortDescription"]
                .as_str()
                .or(skill["shortDescription"].as_str())
                .or(skill["description"].as_str())
                .unwrap_or_default();
            Some(json!({"name":skill["name"].as_str()?,"description":description,
                "kind":"skill","path":skill["path"].as_str()?}))
        })
        .collect()
}

/// A message that starts with a command: its name and what follows it.
fn slash(text: &str) -> Option<(&str, &str)> {
    let rest = text.trim_start().strip_prefix('/')?;
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let name = &rest[..end];
    (!name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_:.".contains(&b)))
    .then(|| (name, rest[end..].trim()))
}

/// The commands Codex has in `cwd`: those Sophon does for it and its
/// skills, asked of an app server started for the question.
pub async fn commands(settings: &Value, cwd: &str) -> io::Result<Value> {
    let mut codex = Codex::asking(settings, cwd)?;
    let asking = async {
        codex
            .call(
                "initialize",
                json!({"clientInfo":{"name":"sophon","title":"Sophon","version":env!("CARGO_PKG_VERSION")},
                    "capabilities":null}),
            )
            .await?;
        codex.send(json!({"method":"initialized"})).await?;
        codex.call("skills/list", json!({"cwds":[cwd]})).await
    };
    // Without its skills, the commands Sophon does are still there.
    let skills = tokio::time::timeout(Duration::from_secs(20), asking)
        .await
        .ok()
        .and_then(Result::ok)
        .map(|result| skills_of(&result))
        .unwrap_or_default();
    let mut list: Vec<Value> = COMMANDS
        .iter()
        .map(|(name, description)| json!({"name":name,"description":description}))
        .collect();
    list.extend(skills);
    Ok(json!(list))
}

/// An app server for a question about the provider's account, started
/// in its own folder (`CODEX_HOME`, made if it is not there: Codex refuses
/// one that is not) and said hello to.
async fn about_account(settings: &Value) -> io::Result<Codex> {
    if let Some(home) = settings["env"]["CODEX_HOME"]
        .as_str()
        .filter(|home| !home.is_empty())
    {
        std::fs::create_dir_all(home)?;
    }
    let cwd = std::env::temp_dir().display().to_string();
    let mut codex = Codex::asking(settings, &cwd)?;
    codex
        .call(
            "initialize",
            json!({"clientInfo":{"name":"sophon","title":"Sophon","version":env!("CARGO_PKG_VERSION")},
                "capabilities":null}),
        )
        .await?;
    codex.send(json!({"method":"initialized"})).await?;
    Ok(codex)
}

/// What Codex says of the account it is signed in with (`account/read`,
/// as T3 Code reads it), in the shape the Agents settings read a sign-in
/// in (`claude auth status --json`'s): signed in or not, how, and with
/// which ChatGPT account and plan. `None` when it does not say.
pub async fn account(settings: &Value) -> Option<Value> {
    let asking = async {
        let mut codex = about_account(settings).await?;
        codex
            .call("account/read", json!({"refreshToken":false}))
            .await
    };
    let read = tokio::time::timeout(Duration::from_secs(15), asking)
        .await
        .ok()?
        .ok()?;
    Some(account_status(&read))
}

/// [`account`]'s answer, from `account/read`'s.
fn account_status(read: &Value) -> Value {
    let account = &read["account"];
    match account["type"].as_str() {
        Some("chatgpt") => json!({"loggedIn":true,"authMethod":"chatgpt",
            "email":account["email"],"planTier":plan(account["planType"].as_str())}),
        Some("apiKey") => json!({"loggedIn":true,"authMethod":"api_key"}),
        Some(other) => json!({"loggedIn":true,"authMethod":other}),
        None => json!({"loggedIn":false}),
    }
}

/// A ChatGPT plan's name, as T3 Code names it (`codexPlanLabel`).
fn plan(kind: Option<&str>) -> Option<&'static str> {
    Some(match kind? {
        "free" => "ChatGPT Free",
        "go" => "ChatGPT Go",
        "plus" => "ChatGPT Plus",
        "pro" => "ChatGPT Pro 200",
        "prolite" => "ChatGPT Pro 100",
        "promax" => "ChatGPT Pro 500",
        "self_serve_business_prolite" => "ChatGPT Business Premium",
        "team" | "self_serve_business_usage_based" | "business" => "ChatGPT Business",
        "ent26" | "enterprise_cbp_automation" | "enterprise_cbp_usage_based" | "enterprise" => {
            "ChatGPT Enterprise"
        }
        "edu" | "edu_plus" | "edu_pro" => "ChatGPT Edu",
        _ => "ChatGPT",
    })
}

/// Starts a sign-in to ChatGPT by Codex's own (`account/login/start`, as
/// T3 Code starts it): in a browser on the host (`device` false: Codex
/// waits for it on the host's localhost), or with a code typed on any
/// device (`device`). The answer says where to go (`authUrl`, or
/// `verificationUrl` and `userCode`); the sign-in ends with the future,
/// which keeps Codex running until it says how it went (dropped, Codex
/// ends and the sign-in with it).
pub async fn login(
    settings: &Value,
    device: bool,
) -> io::Result<(
    Value,
    impl std::future::Future<Output = Result<(), String>> + use<>,
)> {
    let mut codex = about_account(settings).await?;
    let kind = if device {
        "chatgptDeviceCode"
    } else {
        "chatgpt"
    };
    let started = tokio::time::timeout(
        Duration::from_secs(30),
        codex.call("account/login/start", json!({"type":kind})),
    )
    .await
    .map_err(|_| io::Error::other("Codex did not start the sign-in"))??;
    let login = started["loginId"].as_str().unwrap_or_default().to_owned();
    let done = async move {
        loop {
            let Some(line) = super::line(&mut codex.process).await else {
                return Err("Codex stopped before the sign-in ended".to_owned());
            };
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let params = &message["params"];
            let ours = params["loginId"]
                .as_str()
                .is_none_or(|id| login.is_empty() || id == login);
            if message["method"] == "account/login/completed" && ours {
                return if params["success"] == true {
                    Ok(())
                } else {
                    Err(params["error"]
                        .as_str()
                        .unwrap_or("The sign-in did not finish")
                        .to_owned())
                };
            }
        }
    };
    Ok((started, done))
}

impl Codex {
    /// An app server in `cwd` for a question, with no conversation.
    fn asking(settings: &Value, cwd: &str) -> io::Result<Self> {
        let start = Start {
            family: "codex",
            cwd: cwd.to_owned(),
            settings: settings.clone(),
            session: None,
            model: None,
            thinking: None,
            mode: String::new(),
            cost: 0.0,
            env: Vec::new(),
            plugins: Vec::new(),
            record: None,
        };
        let process = Process::spawn("codex", settings, cwd, &["app-server".to_owned()], &[])?;
        Ok(Codex {
            thread: None,
            start,
            emit: Box::new(|_| {}),
            process: Some(process),
            turn: None,
            busy: false,
            stopping: false,
            restart: false,
            outcome: Outcome::Completed,
            next: 0,
            sent: HashMap::new(),
            held: Vec::new(),
            asks: HashMap::new(),
            streamed: HashSet::new(),
            spoke: false,
            fresh: false,
            retried: false,
            images: Vec::new(),
            skills: HashMap::new(),
        })
    }

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
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        Ok(id)
    }

    /// A request answered before anything else is done: the answer, or
    /// the error Codex gave.
    async fn call(&mut self, method: &str, params: Value) -> io::Result<Value> {
        let id = self.request(method, params, Sent::Other).await?;
        self.sent.remove(&id);
        loop {
            let line = super::line(&mut self.process)
                .await
                .ok_or_else(|| io::Error::other("Codex stopped while starting"))?;
            let Ok(message) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if message["id"] != id || !message["method"].is_null() {
                continue;
            }
            if let Some(error) = message["error"]["message"].as_str() {
                return Err(io::Error::other(error.to_owned()));
            }
            return Ok(message["result"].clone());
        }
    }

    fn close(&mut self) {
        self.process = None;
        self.thread = None;
        self.turn = None;
        self.sent.clear();
        self.held.clear();
        self.streamed.clear();
        self.restart = false;
    }

    /// Starts the app server and opens the conversation's thread in it.
    async fn open(&mut self) -> io::Result<()> {
        if self.process.is_some() {
            return Ok(());
        }
        let settings = &self.start.settings;
        let mut args = Vec::new();
        if let Some(endpoint) = settings
            .pointer("/env/OPENAI_BASE_URL")
            .and_then(Value::as_str)
        {
            if !settings
                .pointer("/env/OPENAI_API_KEY")
                .and_then(Value::as_str)
                .is_some_and(|key| !key.is_empty())
            {
                return Err(io::Error::other(
                    "A custom Codex endpoint needs its own API key",
                ));
            }
            for option in [
                "model_provider=\"sophon\"".to_owned(),
                "model_providers.sophon.name=\"Sophon\"".to_owned(),
                format!("model_providers.sophon.base_url={}", json!(endpoint)),
                "model_providers.sophon.env_key=\"OPENAI_API_KEY\"".to_owned(),
                "model_providers.sophon.requires_openai_auth=false".to_owned(),
                "model_providers.sophon.wire_api=\"responses\"".to_owned(),
            ] {
                args.extend(["-c".to_owned(), option]);
            }
        }
        args.push("app-server".to_owned());
        let env: Vec<_> = self
            .start
            .env
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        self.process = Some(Process::spawn(
            "codex",
            settings,
            &self.start.cwd,
            &args,
            &env,
        )?);
        let opening = async {
            self.call(
                "initialize",
                json!({"clientInfo":{"name":"sophon","title":"Sophon","version":env!("CARGO_PKG_VERSION")},
                    "capabilities":null}),
            )
            .await?;
            self.send(json!({"method":"initialized"})).await?;
            let mut params = json!({"cwd":self.start.cwd,"model":self.start.model});
            // The default mode is Codex's own settings, said by saying
            // nothing; the others say what they are.
            match self.start.mode.as_str() {
                "read-only" => {
                    params["approvalPolicy"] = json!("on-request");
                    params["sandbox"] = json!("read-only");
                }
                "full-access" => {
                    params["approvalPolicy"] = json!("never");
                    params["sandbox"] = json!("danger-full-access");
                }
                _ => {}
            }
            let opened = match self.start.session.clone() {
                Some(thread) => {
                    params["threadId"] = json!(thread);
                    self.call("thread/resume", params).await?
                }
                None => self.call("thread/start", params).await?,
            };
            opened["thread"]["id"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| io::Error::other("Codex opened no thread"))
        };
        let thread = tokio::time::timeout(Duration::from_secs(45), opening)
            .await
            .map_err(|_| io::Error::other("Codex did not start in time"))??;
        if self.start.session.as_deref() != Some(thread.as_str()) {
            self.start.session = Some(thread.clone());
            (self.emit)(Event::Session(thread.clone()));
        }
        self.thread = Some(thread);
        // The skills a `/name` message may call on; none if Codex cannot say.
        let cwd = self.start.cwd.clone();
        if let Ok(Ok(result)) = tokio::time::timeout(
            Duration::from_secs(10),
            self.call("skills/list", json!({"cwds":[cwd]})),
        )
        .await
        {
            self.skills = skills_of(&result)
                .into_iter()
                .filter_map(|skill| {
                    Some((skill["name"].as_str()?.to_owned(), skill["path"].as_str()?.to_owned()))
                })
                .collect();
        }
        Ok(())
    }

    fn begin(&mut self) {
        self.busy = true;
        self.stopping = false;
        self.outcome = Outcome::Completed;
        self.retried = false;
        (self.emit)(Event::Busy);
    }

    fn idle(&mut self) {
        if !self.busy {
            return;
        }
        self.busy = false;
        self.turn = None;
        self.spoke = false;
        self.images.clear();
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
            self.outcome = if self.stopping {
                Outcome::Canceled
            } else {
                Outcome::Failed(format!("Codex stopped ({why})"))
            };
            self.idle();
        }
    }

    async fn turn(&mut self, input: Value) -> io::Result<()> {
        if !self.busy {
            self.begin();
        }
        let params = json!({"threadId":self.thread,"input":input,
            "model":self.start.model,"effort":self.start.thinking});
        self.request("turn/start", params, Sent::Turn)
            .await
            .map(drop)
    }

    async fn steer(&mut self, input: Value) -> io::Result<()> {
        let params = json!({"threadId":self.thread,"input":input,"expectedTurnId":self.turn});
        self.request("turn/steer", params, Sent::Steer(input))
            .await
            .map(drop)
    }

    async fn command(&mut self, command: Command) -> io::Result<()> {
        match command {
            Command::Prompt { text, images, .. } => {
                self.open().await?;
                // Codex's own commands, done by its app server: the
                // terminal's, which the model would only read as words.
                match slash(&text) {
                    Some(("compact", "")) if !self.busy => {
                        self.begin();
                        let params = json!({"threadId":self.thread});
                        return self
                            .request("thread/compact/start", params, Sent::Turn)
                            .await
                            .map(drop);
                    }
                    Some(("review", what)) if !self.busy => {
                        self.begin();
                        let target = if what.is_empty() {
                            json!({"type":"uncommittedChanges"})
                        } else {
                            json!({"type":"custom","instructions":what})
                        };
                        let params = json!({"threadId":self.thread,"target":target});
                        return self.request("review/start", params, Sent::Turn).await.map(drop);
                    }
                    _ => {}
                }
                // A skill, called as Codex's terminal calls one: `$name`,
                // with its file.
                let skill = slash(&text).and_then(|(name, what)| {
                    let path = self.skills.get(name)?;
                    let text = if what.is_empty() { format!("${name}") } else { format!("${name} {what}") };
                    Some((text, json!({"type":"skill","name":name,"path":path})))
                });
                let mut input = match skill {
                    Some((text, skill)) => vec![json!({"type":"text","text":text,"text_elements":[]}), skill],
                    None => vec![json!({"type":"text","text":text,"text_elements":[]})],
                };
                for image in &images {
                    let (_, extension, data) = super::image(image)?;
                    let mut file = tempfile::Builder::new().suffix(extension).tempfile()?;
                    io::Write::write_all(
                        &mut file,
                        &base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data)
                            .map_err(io::Error::other)?,
                    )?;
                    input.push(json!({"type":"localImage","path":file.path()}));
                    self.images.push(file);
                }
                match (self.busy, self.turn.is_some()) {
                    (true, true) => self.steer(json!(input)).await,
                    (true, false) => {
                        self.held.push(json!(input));
                        Ok(())
                    }
                    _ => self.turn(json!(input)).await,
                }
            }
            Command::Interrupt => {
                if !self.busy {
                    return Ok(());
                }
                self.stopping = true;
                self.interrupt().await
            }
            Command::Answer { id, response } => {
                let Some(ask) = self.asks.remove(&id) else {
                    return Ok(());
                };
                let allowed = response["behavior"] == "allow";
                let result = match &ask.questions {
                    // Allowed always is allowed for as long as this
                    // thread's process lasts, which is as far as Codex
                    // takes an approval's word.
                    None => {
                        json!({"decision":match (allowed, response["selectedActionId"].as_str()) {
                            (false, _) => "decline",
                            (true, Some("allow_always")) => "acceptForSession",
                            (true, _) => "accept",
                        }})
                    }
                    Some(questions) => {
                        let given = &response["updatedInput"]["answers"];
                        let mut answers = serde_json::Map::new();
                        for question in questions
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter(|_| allowed)
                        {
                            let answer = [&question["header"], &question["question"]]
                                .into_iter()
                                .filter_map(Value::as_str)
                                .find_map(|key| given.get(key));
                            if let (Some(id), Some(answer)) = (question["id"].as_str(), answer) {
                                answers.insert(id.to_owned(), json!({"answers":[answer]}));
                            }
                        }
                        json!({"answers":answers})
                    }
                };
                (self.emit)(Event::Settled(id));
                self.send(json!({"id":ask.request,"result":result})).await
            }
            // What a thread may do is set as it is opened.
            Command::Mode(mode) => {
                self.start.mode = mode.clone();
                (self.emit)(Event::Mode(mode));
                if self.busy {
                    self.restart = true;
                } else {
                    self.close();
                }
                Ok(())
            }
            Command::Model(model) => {
                self.start.model = model;
                Ok(())
            }
            Command::StopTask(_) => Ok(()),
            Command::Rewind { turns, files, .. } => {
                if self.busy || files {
                    let why = if files {
                        "Codex keeps no checkpoints of files to go back to"
                    } else {
                        "The agent is working: stop it first"
                    };
                    (self.emit)(Event::Rewound(Err(why.into())));
                    return Ok(());
                }
                self.open().await?;
                let params = json!({"threadId":self.thread,"numTurns":turns});
                self.request("thread/rollback", params, Sent::Rollback)
                    .await
                    .map(drop)
            }
            Command::Thinking(thinking) => {
                self.start.thinking = thinking;
                Ok(())
            }
        }
    }

    /// Stops the turn underway, if its id is known yet (if not, it is
    /// stopped as soon as it is).
    async fn interrupt(&mut self) -> io::Result<()> {
        if self.turn.is_none() {
            return Ok(());
        }
        let params = json!({"threadId":self.thread,"turnId":self.turn});
        self.request("turn/interrupt", params, Sent::Other)
            .await
            .map(drop)
    }

    /// The turn underway is `id`: what waited for that goes ahead.
    async fn running(&mut self, id: &str) -> io::Result<()> {
        if self.turn.as_deref() == Some(id) {
            return Ok(());
        }
        self.turn = Some(id.to_owned());
        if self.stopping {
            return self.interrupt().await;
        }
        for input in std::mem::take(&mut self.held) {
            self.steer(input).await?;
        }
        Ok(())
    }

    async fn read(&mut self, line: &str) -> io::Result<()> {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        let params = &message["params"];
        match (message["method"].as_str(), message["id"].is_null()) {
            (Some(method), false) => self.asked(method, &message["id"], params).await,
            (Some(method), true) => self.notified(method, params).await,
            (None, _) => {
                let Some(sent) = message["id"].as_u64().and_then(|id| self.sent.remove(&id)) else {
                    return Ok(());
                };
                let error = message["error"]["message"].as_str();
                match (sent, error) {
                    (Sent::Turn, Some(error)) => {
                        self.outcome = Outcome::Failed(error.to_owned());
                        self.idle();
                        Ok(())
                    }
                    (Sent::Turn, None) => match message["result"]["turn"]["id"].as_str() {
                        Some(id) => self.running(id).await,
                        None => Ok(()),
                    },
                    (Sent::Steer(input), Some(_)) => self.turn(input).await,
                    (Sent::Rollback, error) => {
                        (self.emit)(Event::Rewound(
                            error.map_or(Ok(()), |error| Err(error.to_owned())),
                        ));
                        Ok(())
                    }
                    _ => Ok(()),
                }
            }
        }
    }

    /// A request of Codex's: something to approve or answer, shown as a
    /// permission; anything else is refused, so Codex does not wait.
    async fn asked(&mut self, method: &str, request: &Value, params: &Value) -> io::Result<()> {
        let (name, kind, input, detail, questions) = match method {
            "item/commandExecution/requestApproval" => (
                "shell",
                "tool",
                json!({"command":params["command"],"cwd":params["cwd"],"reason":params["reason"]}),
                json!({"type":"shell","command":params["command"],"cwd":params["cwd"]}),
                None,
            ),
            "item/fileChange/requestApproval" => (
                "apply_patch",
                "tool",
                json!({"description":params["reason"].as_str().unwrap_or("Apply the proposed file changes")}),
                Value::Null,
                None,
            ),
            "item/tool/requestUserInput" => {
                let questions: Vec<_> = params["questions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|question| {
                        json!({"id":question["id"],"header":question["header"],"question":question["question"],
                            "options":question["options"].as_array().cloned().unwrap_or_default(),"multiSelect":false})
                    })
                    .collect();
                (
                    "ask_user",
                    "question",
                    json!({"questions":questions}),
                    Value::Null,
                    Some(json!(questions)),
                )
            }
            _ => {
                return self
                    .send(json!({"id":request,"error":{"code":-32601,"message":"Not supported by this host"}}))
                    .await;
            }
        };
        let id = crate::state::id();
        let actions = if questions.is_none() {
            json!([{"id":"allow","label":"Allow","behavior":"allow"},
                {"id":"allow_always","label":"Always allow","behavior":"allow"},
                {"id":"deny","label":"Deny","behavior":"deny"}])
        } else {
            json!([])
        };
        (self.emit)(Event::Asked(
            json!({"id":id,"provider":"codex","name":name,"kind":kind,
            "input":input,"detail":detail,"actions":actions}),
        ));
        self.asks.insert(
            id,
            Ask {
                request: request.clone(),
                questions,
            },
        );
        Ok(())
    }

    /// What happened in a thread of an agent this one started: its
    /// messages and calls, whole, for that agent's own timeline. Its
    /// turns starting and ending are not this conversation's.
    fn started(&mut self, thread: &str, method: &str, params: &Value) {
        // How it stands is how its own turns go, not how the call
        // that started it went: that call is done once it has started.
        let standing = match method {
            "turn/started" => Some("running"),
            "turn/completed" => Some(match params["turn"]["status"].as_str() {
                Some("failed") => "failed",
                Some("interrupted") => "canceled",
                _ => "completed",
            }),
            _ => None,
        };
        if let Some(standing) = standing {
            return (self.emit)(Event::Subagent(json!({"id":thread,"status":standing})));
        }
        let item = &params["item"];
        let done = method == "item/completed";
        if !done && method != "item/started" {
            return;
        }
        let mut items = match item["type"].as_str() {
            Some("agentMessage") if done => vec![json!({"type":"assistant_message",
                "text":format!("{}\n\n", item["text"].as_str().unwrap_or_default())})],
            Some("reasoning") if done => {
                let summary: Vec<_> = item["summary"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                vec![json!({"type":"reasoning","text":summary.join("\n\n")})]
            }
            _ => rows(item),
        };
        items.retain(|item| {
            item["text"]
                .as_str()
                .is_none_or(|text| !text.trim().is_empty())
        });
        for item in items {
            (self.emit)(Event::Sub {
                id: thread.to_owned(),
                item,
            });
        }
    }

    async fn notified(&mut self, method: &str, params: &Value) -> io::Result<()> {
        if let Some(thread) = params["threadId"].as_str()
            && self.thread.as_deref().is_some_and(|own| own != thread)
        {
            self.started(thread, method, params);
            return Ok(());
        }
        let item = &params["item"];
        let kind = item["type"].as_str();
        // A call that starts agents: each is known by its thread.
        if kind == Some("collabAgentToolCall")
            && matches!(method, "item/started" | "item/completed")
        {
            for thread in item["receiverThreadIds"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let title: String = item["prompt"]
                    .as_str()
                    .unwrap_or_default()
                    .lines()
                    .next()
                    .unwrap_or_default()
                    .chars()
                    .take(80)
                    .collect();
                (self.emit)(Event::Subagent(json!({"id":thread,
                    "title":if title.is_empty() { Value::Null } else { json!(title) }})));
            }
        }
        match method {
            "turn/started" => {
                if !self.busy {
                    self.begin();
                }
                if let Some(id) = params["turn"]["id"].as_str() {
                    self.running(id).await?;
                }
            }
            "turn/completed" => {
                let turn = &params["turn"];
                match turn["status"].as_str() {
                    Some("interrupted") => self.outcome = Outcome::Canceled,
                    Some("failed") => {
                        if !matches!(self.outcome, Outcome::Failed(_)) {
                            self.outcome = Outcome::Failed(
                                turn["error"]["message"]
                                    .as_str()
                                    .unwrap_or("Codex could not finish this turn")
                                    .to_owned(),
                            );
                        }
                    }
                    _ => self.outcome = Outcome::Completed,
                }
                self.idle();
            }
            "error" => {
                let said = params["error"]["message"]
                    .as_str()
                    .unwrap_or("Codex reported an error");
                if params["willRetry"] != true {
                    self.outcome = Outcome::Failed(said.to_owned());
                } else if !self.retried {
                    self.retried = true;
                    self.item(json!({"type":"notification","message":format!("{said} Codex is retrying.")}));
                }
            }
            "item/started" if kind == Some("agentMessage") => self.fresh = true,
            "item/agentMessage/delta" => {
                if let Some(id) = params["itemId"].as_str() {
                    self.streamed.insert(id.to_owned());
                }
                self.reply(params["delta"].as_str().unwrap_or_default());
            }
            "item/reasoning/summaryTextDelta" => {
                if let Some(id) = params["itemId"].as_str() {
                    self.streamed.insert(id.to_owned());
                }
                if let Some(text) = params["delta"].as_str().filter(|text| !text.is_empty()) {
                    self.item(json!({"type":"reasoning","text":text}));
                }
            }
            "item/reasoning/summaryPartAdded"
                if params["summaryIndex"]
                    .as_u64()
                    .is_some_and(|index| index > 0) =>
            {
                self.item(json!({"type":"reasoning","text":"\n\n"}));
            }
            "item/started" | "item/completed" => {
                let streamed = item["id"]
                    .as_str()
                    .is_some_and(|id| self.streamed.contains(id));
                match kind {
                    Some("agentMessage") if !streamed => {
                        self.fresh = true;
                        self.reply(item["text"].as_str().unwrap_or_default());
                    }
                    Some("reasoning") if !streamed && method == "item/completed" => {
                        let summary: Vec<_> = item["summary"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect();
                        if !summary.is_empty() {
                            self.item(json!({"type":"reasoning","text":summary.join("\n\n")}));
                        }
                    }
                    // A review's findings, as Codex's terminal shows them.
                    Some("exitedReviewMode") if method == "item/completed" => {
                        self.fresh = true;
                        self.reply(item["review"].as_str().unwrap_or_default());
                    }
                    Some("enteredReviewMode") => {}
                    Some("contextCompaction") if method == "item/completed" => {
                        self.item(json!({"type":"compaction","status":"completed"}));
                    }
                    _ => {
                        for row in rows(item) {
                            self.item(row);
                        }
                    }
                }
            }
            "turn/plan/updated" => {
                let items: Vec<_> = params["plan"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|step| json!({"text":step["step"],"completed":step["status"] == "completed"}))
                    .collect();
                self.item(json!({"type":"todo","items":items}));
            }
            "thread/tokenUsage/updated" => {
                let usage = &params["tokenUsage"];
                (self.emit)(Event::Usage(
                    json!({"contextWindowUsedTokens":usage["last"]["totalTokens"],
                    "contextWindowMaxTokens":usage["modelContextWindow"]}),
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_account_reads_as_the_agents_settings_read_a_sign_in() {
        let chatgpt =
            json!({"account":{"type":"chatgpt","email":"me@example.com","planType":"pro"}});
        assert_eq!(
            account_status(&chatgpt),
            json!({"loggedIn":true,"authMethod":"chatgpt","email":"me@example.com","planTier":"ChatGPT Pro 200"})
        );
        assert_eq!(
            account_status(&json!({"account":{"type":"apiKey"}})),
            json!({"loggedIn":true,"authMethod":"api_key"})
        );
        assert_eq!(
            account_status(&json!({"account":null,"requiresOpenaiAuth":true})),
            json!({"loggedIn":false})
        );
        // A plan it does not know is still ChatGPT's.
        assert_eq!(
            account_status(
                &json!({"account":{"type":"chatgpt","email":null,"planType":"new-plan"}})
            )["planTier"],
            "ChatGPT"
        );
    }

    #[test]
    fn an_item_is_rows_by_what_it_did() {
        let ran = rows(
            &json!({"type":"commandExecution","id":"c","command":"ls","cwd":"/w",
            "status":"completed","aggregatedOutput":"a\n","exitCode":0}),
        );
        assert_eq!(ran[0]["status"], "completed");
        assert_eq!(
            ran[0]["detail"],
            json!({"type":"shell","command":"ls","cwd":"/w","output":"a\n","exitCode":0})
        );
        let patched = rows(
            &json!({"type":"fileChange","id":"p","status":"inProgress","changes":[
            {"path":"/w/a","kind":{"type":"add"},"diff":"hello\n"},
            {"path":"/w/b","kind":{"type":"update","move_path":null},"diff":"@@ -1 +1 @@\n-a\n+b\n"}]}),
        );
        assert_eq!(patched.len(), 2);
        assert_eq!(
            (
                patched[0]["callId"].clone(),
                patched[0]["detail"]["type"].clone()
            ),
            (json!("p:0"), json!("write"))
        );
        assert_eq!(patched[1]["detail"]["unifiedDiff"], "@@ -1 +1 @@\n-a\n+b\n");
        assert!(rows(&json!({"type":"userMessage","id":"u"})).is_empty());
    }
}
