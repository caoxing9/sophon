//! Claude Code, as a program drives it: `claude -p` reading and writing
//! a stream of JSON, with the control requests that go both ways on it
//! (it asks whether a tool may run; it is told to stop, to change mode
//! or model).
//!
//! One process serves a conversation's turns one after another, and a
//! message sent while a turn runs goes into that turn. Which account
//! and endpoint it uses is Claude Code's own to say (its login, its
//! settings files), unless the provider's settings name another.

use super::{Command, Emit, Event, IDLE, Outcome, Process, Start, tool};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io,
};
use tokio::sync::mpsc::UnboundedReceiver;

/// A tool's output is kept to this much: the agent has the whole of it,
/// and a timeline row is for reading.
const KEPT: usize = 60_000;

/// A permission the CLI waits on: its control request, and what it was
/// about to run.
struct Ask {
    request: String,
    tool: String,
    input: Value,
    /// The rules Claude Code offers to keep, so as not to ask again.
    suggestions: Value,
}

/// A call begun and not answered: its tool and input, and the agent
/// that made it if that is one this conversation's agent started (by
/// the call that started it).
struct Call {
    name: String,
    input: Value,
    within: Option<String>,
}

struct Claude {
    start: Start,
    emit: Emit,
    process: Option<Process>,
    /// A turn is underway.
    busy: bool,
    /// The CLI says when it starts and stops working, so its word is
    /// taken over the end of one result.
    states: bool,
    /// The turn underway was told to stop.
    stopping: bool,
    /// How the turn underway has gone, by its last result.
    outcome: Outcome,
    /// What was set needs a new process, once this turn is over.
    restart: bool,
    calls: HashMap<String, Call>,
    /// Agents this one started whose last item was text: their next
    /// message starts a new paragraph.
    wordy: HashSet<String>,
    /// The calls that started agents, and those of them Claude Code
    /// runs in the background: such a call is answered as soon as its
    /// agent has started, and the agent says later how it ended.
    agents: HashSet<String>,
    background: HashSet<String>,
    /// A rewind of files asked of Claude Code and not answered: the
    /// request, the message, and whether the conversation goes back too.
    rewinding: Option<(String, String, bool)>,
    /// The request that asks Claude Code what it is (its commands).
    introduced: Option<String>,
    /// Messages whose text came as it was written.
    streamed: HashSet<String>,
    asks: HashMap<String, Ask>,
    /// The last item sent was reply text, and a block of it has begun
    /// with nothing sent yet: the next text starts a new paragraph.
    spoke: bool,
    fresh: bool,
    /// A notice about the endpoint not answering went out this turn.
    retried: bool,
    /// Claude Code is compacting, and that was said.
    compacting: bool,
    requests: u64,
    /// The model answering, the tokens its last request held, and what
    /// this process has cost so far.
    model: String,
    context: u64,
    spent: f64,
}

pub async fn run(start: Start, mut inbox: UnboundedReceiver<Command>, emit: Emit) {
    let mut claude = Claude {
        start,
        emit,
        process: None,
        busy: false,
        states: false,
        stopping: false,
        outcome: Outcome::Completed,
        restart: false,
        calls: HashMap::new(),
        wordy: HashSet::new(),
        agents: HashSet::new(),
        background: HashSet::new(),
        introduced: None,
        rewinding: None,
        streamed: HashSet::new(),
        asks: HashMap::new(),
        spoke: false,
        fresh: false,
        retried: false,
        compacting: false,
        requests: 0,
        model: String::new(),
        context: 0,
        spent: 0.0,
    };
    loop {
        tokio::select! {
            command = inbox.recv() => {
                let Some(command) = command else { break };
                if let Err(error) = claude.command(command).await {
                    claude.fail(error.to_string());
                }
            }
            line = super::line(&mut claude.process) => match line {
                Some(line) => {
                    if let Err(error) = claude.read(&line).await {
                        claude.fail(error.to_string());
                    }
                }
                None => claude.stopped().await,
            },
            _ = tokio::time::sleep(IDLE), if claude.process.is_some() && !claude.busy => claude.close(),
        }
    }
}

fn clip(text: &str) -> String {
    if text.len() <= KEPT {
        return text.to_owned();
    }
    format!("{}…", &text[..text.floor_char_boundary(KEPT)])
}

/// The text of a tool's result: a string, or the text blocks of a list.
pub(crate) fn said(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

/// A call as the timeline shows it, by what the tool does.
pub(crate) fn detail(name: &str, input: &Value) -> Value {
    let field = |key: &str| input[key].clone();
    match name {
        "Bash" => json!({"type":"shell","command":field("command")}),
        "Read" => {
            json!({"type":"read","filePath":field("file_path"),"offset":field("offset"),"limit":field("limit")})
        }
        "Edit" | "MultiEdit" => json!({"type":"edit","filePath":field("file_path"),
            "oldString":field("old_string"),"newString":field("new_string")}),
        "NotebookEdit" => {
            json!({"type":"edit","filePath":field("notebook_path"),"newString":field("new_source")})
        }
        "Write" => json!({"type":"write","filePath":field("file_path"),
            "content":clip(input["content"].as_str().unwrap_or_default())}),
        "Grep" => json!({"type":"search","query":field("pattern"),"toolName":"grep"}),
        "Glob" => json!({"type":"search","query":field("pattern"),"toolName":"glob"}),
        "WebSearch" => json!({"type":"search","query":field("query"),"toolName":"web_search"}),
        "WebFetch" => json!({"type":"fetch","url":field("url"),"prompt":field("prompt")}),
        "Task" | "Agent" => json!({"type":"sub_agent","subAgentType":field("subagent_type"),
            "description":field("description"),"log":""}),
        "ExitPlanMode" => json!({"type":"plan","text":field("plan")}),
        _ => json!({"type":"unknown","input":input}),
    }
}

/// The hunks Claude Code reports for an edit, as a unified diff.
fn patch(hunks: &Value) -> Option<String> {
    let hunks = hunks.as_array().filter(|hunks| !hunks.is_empty())?;
    let mut diff = String::new();
    for hunk in hunks {
        let number = |key: &str| hunk[key].as_u64().unwrap_or(0);
        diff.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            number("oldStart"),
            number("oldLines"),
            number("newStart"),
            number("newLines")
        ));
        for line in hunk["lines"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            diff.push_str(line);
            diff.push('\n');
        }
    }
    Some(clip(&diff))
}

/// `detail` with what the call came back with: `text` as the model got
/// it, `result` as Claude Code describes it to a program.
pub(crate) fn answered(mut detail: Value, text: &str, result: &Value, failed: bool) -> Value {
    match detail["type"].as_str() {
        Some("shell") => {
            let output = match (result["stdout"].as_str(), result["stderr"].as_str()) {
                (Some(out), Some(err)) if !err.is_empty() && !out.is_empty() => {
                    format!("{out}\n{err}")
                }
                (Some(out), Some(err)) => format!("{out}{err}"),
                _ => text.to_owned(),
            };
            detail["output"] = json!(clip(if output.is_empty() { text } else { &output }));
            if failed
                && let Some(code) = text
                    .strip_prefix("Exit code ")
                    .and_then(|rest| rest.split_whitespace().next())
                    .and_then(|code| code.parse::<i64>().ok())
            {
                detail["exitCode"] = json!(code);
            }
        }
        Some("read") => {
            detail["content"] = json!(clip(
                result
                    .pointer("/file/content")
                    .and_then(Value::as_str)
                    .unwrap_or(text)
            ));
        }
        Some("edit") => {
            if let Some(diff) = patch(&result["structuredPatch"]) {
                detail["unifiedDiff"] = json!(diff);
            }
        }
        Some("search") => {
            detail["content"] = json!(clip(text));
            if result["filenames"].is_array() {
                detail["filePaths"] = result["filenames"].clone();
            }
        }
        Some("fetch") => detail["result"] = json!(clip(result["result"].as_str().unwrap_or(text))),
        Some("unknown") => detail["output"] = json!(clip(text)),
        _ => {}
    }
    detail
}

/// The answers a client gave to `AskUserQuestion`, under the questions
/// themselves: a client files each under its question's header, and
/// Claude Code reads them by the question's text.
fn answers(mut input: Value) -> Value {
    let Some(given) = input["answers"].as_object().cloned() else {
        return input;
    };
    let mut filed = serde_json::Map::new();
    for question in input["questions"].as_array().into_iter().flatten() {
        let Some(text) = question["question"].as_str() else {
            continue;
        };
        let header = question["header"]
            .as_str()
            .filter(|header| !header.is_empty())
            .unwrap_or(text);
        if let Some(answer) = given.get(text).or_else(|| given.get(header)) {
            filed.insert(text.to_owned(), answer.clone());
        }
    }
    input["answers"] = Value::Object(filed);
    input
}

impl Claude {
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

    async fn control(&mut self, request: Value) -> io::Result<()> {
        self.requests += 1;
        let id = format!("sophon-{}", self.requests);
        self.send(json!({"type":"control_request","request_id":id,"request":request}))
            .await
    }

    /// Takes the conversation back to before `message`: a new session
    /// whose record is this one's up to that message, which Claude Code
    /// resumes in its place. The old record stays as it is.
    fn cut(&mut self, message: &str) -> Result<(), String> {
        let session = self
            .start
            .session
            .clone()
            .ok_or("This conversation has not started")?;
        self.close();
        let fresh = crate::history::cut(&self.start.settings, &session, message)?;
        self.start.session = Some(fresh.clone());
        (self.emit)(Event::Session(fresh));
        Ok(())
    }

    /// Lets the process go; what it cost stays counted.
    fn close(&mut self) {
        self.process = None;
        self.start.cost += self.spent;
        self.spent = 0.0;
        self.calls.clear();
        self.agents.clear();
        self.background.clear();
        self.wordy.clear();
        self.streamed.clear();
        self.restart = false;
    }

    async fn open(&mut self) -> io::Result<()> {
        if self.process.is_some() {
            return Ok(());
        }
        let mut args: Vec<String> = [
            "-p",
            "--input-format",
            "stream-json",
            "--output-format",
            "stream-json",
            "--verbose",
            "--include-partial-messages",
            "--permission-prompt-tool",
            "stdio",
            "--allow-dangerously-skip-permissions",
            "--permission-mode",
        ]
        .map(str::to_owned)
        .into();
        args.push(self.start.mode.clone());
        for (flag, value) in [
            ("--model", &self.start.model),
            ("--effort", &self.start.thinking),
            ("--resume", &self.start.session),
        ] {
            if let Some(value) = value {
                args.extend([flag.to_owned(), value.clone()]);
            }
        }
        for folder in &self.start.plugins {
            args.extend(["--plugin-dir".to_owned(), folder.clone()]);
        }
        for tool in self.start.settings["disallowedTools"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let Some(tool) = tool.as_str() {
                args.extend(["--disallowedTools".to_owned(), tool.to_owned()]);
            }
        }
        let mut env = vec![
            ("CLAUDE_CODE_EMIT_SESSION_STATE_EVENTS", "1"),
            ("CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING", "true"),
        ];
        env.extend(
            self.start
                .env
                .iter()
                .map(|(name, value)| (name.as_str(), value.as_str())),
        );
        self.process = Some(Process::spawn(
            "claude",
            &self.start.settings,
            &self.start.cwd,
            &args,
            &env,
        )?);
        self.states = false;
        self.introduced = Some(format!("sophon-{}", self.requests + 1));
        self.control(json!({"subtype":"initialize"})).await
    }

    fn begin(&mut self) {
        self.busy = true;
        self.stopping = false;
        self.outcome = Outcome::Completed;
        self.retried = false;
        (self.emit)(Event::Busy);
    }

    /// The turn underway is over (nothing, if none is).
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

    /// Something of ours went wrong (the CLI could not be started or
    /// written to): the process goes, and the turn with it.
    fn fail(&mut self, error: String) {
        self.close();
        self.busy = true;
        self.outcome = Outcome::Failed(error);
        self.idle();
    }

    /// The process ended by itself.
    async fn stopped(&mut self) {
        let Some(process) = self.process.take() else {
            return;
        };
        let why = process.ended().await;
        self.close();
        if self.busy {
            if !self.stopping {
                self.outcome = Outcome::Failed(format!("Claude Code stopped ({why})"));
            } else {
                self.outcome = Outcome::Canceled;
            }
            self.idle();
        }
    }

    async fn command(&mut self, command: Command) -> io::Result<()> {
        match command {
            Command::Prompt { id, text, images } => {
                let mut content = Vec::new();
                for image in &images {
                    let (mime, _, data) = super::image(image)?;
                    content.push(json!({"type":"image","source":{"type":"base64","media_type":mime,"data":data}}));
                }
                let content = if content.is_empty() {
                    json!(text)
                } else {
                    content.push(json!({"type":"text","text":text}));
                    json!(content)
                };
                self.open().await?;
                if !self.busy {
                    self.begin();
                }
                let mut message = json!({"type":"user","message":{"role":"user","content":content},
                    "parent_tool_use_id":null,"session_id":self.start.session.clone().unwrap_or_default()});
                // Recorded by Claude Code under the message's own id,
                // so a rewind can name it.
                if uuid::Uuid::parse_str(&id).is_ok() {
                    message["uuid"] = json!(id);
                }
                self.send(message).await
            }
            Command::Interrupt => {
                if !self.busy {
                    return Ok(());
                }
                self.stopping = true;
                self.control(json!({"subtype":"interrupt"})).await
            }
            Command::Answer { id, response } => {
                let Some(ask) = self.asks.remove(&id) else {
                    return Ok(());
                };
                let reply = if response["behavior"] == "allow" {
                    let input = response
                        .get("updatedInput")
                        .filter(|input| input.is_object())
                        .cloned()
                        .unwrap_or(ask.input);
                    let mut reply = json!({"behavior":"allow",
                        "updatedInput":if ask.tool == "AskUserQuestion" { answers(input) } else { input }});
                    if response["selectedActionId"] == "allow_always" {
                        reply["updatedPermissions"] = ask.suggestions;
                    }
                    reply
                } else {
                    json!({"behavior":"deny",
                        "message":response["message"].as_str().unwrap_or("The user denied this request.")})
                };
                (self.emit)(Event::Settled(id));
                self.send(json!({"type":"control_response",
                    "response":{"subtype":"success","request_id":ask.request,"response":reply}}))
                    .await
            }
            Command::Mode(mode) => {
                self.start.mode = mode.clone();
                (self.emit)(Event::Mode(mode.clone()));
                self.control(json!({"subtype":"set_permission_mode","mode":mode}))
                    .await
            }
            Command::Model(model) => {
                self.start.model = model.clone();
                self.control(json!({"subtype":"set_model","model":model}))
                    .await
            }
            // An effort is the process's for its life.
            Command::StopTask(task) => {
                self.control(json!({"subtype":"stop_task","task_id":task}))
                    .await
            }
            Command::Rewind {
                message,
                files,
                conversation,
                ..
            } => {
                if self.busy {
                    (self.emit)(Event::Rewound(Err(
                        "The agent is working: stop it first".into()
                    )));
                    return Ok(());
                }
                // The files first, by a process that has the session as
                // it is; then the conversation, cut where the message is.
                if files {
                    self.open().await?;
                    self.rewinding = Some((
                        format!("sophon-{}", self.requests + 1),
                        message.clone(),
                        conversation,
                    ));
                    return self
                        .control(json!({"subtype":"rewind_files","user_message_id":message}))
                        .await;
                }
                let cut = self.cut(&message);
                (self.emit)(Event::Rewound(cut));
                Ok(())
            }
            Command::Thinking(thinking) => {
                self.start.thinking = thinking;
                if self.busy {
                    self.restart = true;
                } else {
                    self.close();
                }
                Ok(())
            }
        }
    }

    fn mode(&mut self, mode: &Value) {
        if let Some(mode) = mode.as_str().filter(|mode| *mode != self.start.mode) {
            self.start.mode = mode.to_owned();
            (self.emit)(Event::Mode(mode.to_owned()));
        }
    }

    async fn read(&mut self, line: &str) -> io::Result<()> {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return Ok(());
        };
        // What an agent it started says is that agent's, not this
        // conversation's: here only the call that started it is shown,
        // and what it does goes to a timeline of its own.
        let within = event["parent_tool_use_id"].as_str().map(str::to_owned);
        let own = within.is_none();
        match event["type"].as_str() {
            Some("system") => self.system(&event),
            Some("stream_event") if own => self.streaming(&event["event"]),
            Some("assistant") => self.said(&event, within),
            Some("user") if event["isReplay"] != true => self.returned(&event),
            Some("result") => self.result(&event),
            Some("control_request") => {
                let id = event["request_id"].as_str().unwrap_or_default().to_owned();
                if event["request"]["subtype"] == "can_use_tool" {
                    self.ask(id, &event["request"]);
                } else {
                    self.send(json!({"type":"control_response",
                        "response":{"subtype":"error","request_id":id,"error":"Not supported by this host"}}))
                        .await?;
                }
            }
            Some("control_cancel_request") => {
                let asked: Vec<_> = self
                    .asks
                    .iter()
                    .filter(|(_, ask)| event["request_id"] == ask.request.as_str())
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in asked {
                    self.asks.remove(&id);
                    (self.emit)(Event::Settled(id));
                }
            }
            Some("control_response") => {
                let response = &event["response"];
                let ours = response["request_id"].as_str();
                if let Some((_, message, conversation)) = self
                    .rewinding
                    .take_if(|(request, _, _)| Some(request.as_str()) == ours)
                {
                    let done = match response["error"].as_str() {
                        Some(error) => Err(error.to_owned()),
                        None if conversation => self.cut(&message),
                        None => Ok(()),
                    };
                    (self.emit)(Event::Rewound(done));
                } else if let Some(error) = response["error"].as_str() {
                    eprintln!("claude: control request refused: {error}");
                } else if self.introduced.as_deref() == response["request_id"].as_str()
                    && response["response"]["commands"].is_array()
                {
                    (self.emit)(Event::Commands(response["response"]["commands"].clone()));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn system(&mut self, event: &Value) {
        match event["subtype"].as_str() {
            Some("init") => {
                if let Some(session) = event["session_id"].as_str()
                    && self.start.session.as_deref() != Some(session)
                {
                    self.start.session = Some(session.to_owned());
                    (self.emit)(Event::Session(session.to_owned()));
                }
                self.mode(&event["permissionMode"]);
            }
            Some("status") => {
                self.mode(&event["permissionMode"]);
                // Said once: Claude Code says it is at it again and again
                // while one compaction lasts.
                let compacting = event["status"] == "compacting";
                if compacting && !self.compacting {
                    self.item(json!({"type":"compaction","status":"loading"}));
                }
                self.compacting = compacting;
            }
            // An agent it started, as Claude Code keeps track of it.
            Some("task_started") => {
                if let Some(id) = event["tool_use_id"]
                    .as_str()
                    .filter(|id| self.agents.contains(*id))
                {
                    if event["is_backgrounded"] == true {
                        self.background.insert(id.to_owned());
                    }
                    (self.emit)(Event::Subagent(json!({"id":id,"status":"running",
                        "title":event["description"],"subtitle":event["subagent_type"]})));
                }
            }
            Some("task_notification") => {
                if let Some(id) = event["tool_use_id"]
                    .as_str()
                    .filter(|id| self.agents.contains(*id))
                {
                    self.background.remove(id);
                    let status = match event["status"].as_str() {
                        Some("failed") => "failed",
                        Some("stopped" | "killed") => "canceled",
                        _ => "completed",
                    };
                    (self.emit)(Event::Subagent(json!({"id":id,"status":status})));
                }
            }
            Some("background_tasks_changed") => {
                let tasks: Vec<Value> = event["tasks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|task| {
                        Some(
                            json!({"id":task["task_id"].as_str()?,"kind":task["task_type"],
                            "description":task["description"],"command":task["command"]}),
                        )
                    })
                    .collect();
                (self.emit)(Event::Background(json!(tasks)));
            }
            Some("compact_boundary") => {
                self.compacting = false;
                self.item(json!({"type":"compaction","status":"completed"}))
            }
            Some("session_state_changed") => {
                self.states = true;
                match event["state"].as_str() {
                    Some("running") if !self.busy => self.begin(),
                    Some("idle") => self.idle(),
                    _ => {}
                }
            }
            // Said once a turn: Claude Code goes on trying for minutes,
            // and nothing else would show why nothing comes.
            Some("api_retry") if !self.retried => {
                self.retried = true;
                let why = match event["error_status"].as_u64() {
                    Some(status) => format!("HTTP {status}"),
                    None => event["error"].as_str().unwrap_or("no answer").to_owned(),
                };
                self.item(json!({"type":"notification",
                    "message":format!("The model endpoint is not answering ({why}). Claude Code is retrying.")}));
            }
            _ => {}
        }
    }

    fn streaming(&mut self, event: &Value) {
        match event["type"].as_str() {
            Some("message_start") => {
                if let Some(id) = event["message"]["id"].as_str() {
                    self.streamed.insert(id.to_owned());
                }
            }
            Some("content_block_start") if event["content_block"]["type"] == "text" => {
                self.fresh = true
            }
            Some("content_block_delta") => match event["delta"]["type"].as_str() {
                Some("text_delta") => {
                    self.reply(event["delta"]["text"].as_str().unwrap_or_default())
                }
                Some("thinking_delta") => {
                    if let Some(text) = event["delta"]["thinking"]
                        .as_str()
                        .filter(|text| !text.is_empty())
                    {
                        self.item(json!({"type":"reasoning","text":text}));
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }

    /// A message of the agent's, whole: its calls, and its text if that
    /// did not come as it was written.
    fn said(&mut self, event: &Value, within: Option<String>) {
        let message = &event["message"];
        // What Claude Code writes in the model's place when a request
        // fails; the turn's result says the same as its error.
        let stand_in = message["model"] == "<synthetic>" || !event["error"].is_null();
        let streamed = message["id"]
            .as_str()
            .is_some_and(|id| self.streamed.contains(id));
        if !stand_in && within.is_none() {
            let usage = &message["usage"];
            let tokens: u64 = [
                "input_tokens",
                "cache_creation_input_tokens",
                "cache_read_input_tokens",
                "output_tokens",
            ]
            .iter()
            .filter_map(|key| usage[*key].as_u64())
            .sum();
            if tokens > 0 {
                self.context = tokens;
            }
            if let Some(model) = message["model"].as_str() {
                self.model = model.to_owned();
            }
        }
        for block in message["content"].as_array().into_iter().flatten() {
            let text = |key: &str| block[key].as_str().filter(|text| !text.is_empty());
            match (block["type"].as_str(), &within) {
                (Some("tool_use"), _) => self.called(block, within.clone()),
                (Some("text"), None) if !streamed && !stand_in => {
                    self.fresh = true;
                    self.reply(block["text"].as_str().unwrap_or_default());
                }
                (Some("thinking"), None) if !streamed => {
                    if let Some(text) = text("thinking") {
                        self.item(json!({"type":"reasoning","text":text}));
                    }
                }
                // An agent it started: its messages come whole.
                (Some("text"), Some(agent)) if !stand_in => {
                    if let Some(text) = text("text") {
                        let text = if self.wordy.contains(agent) {
                            format!("\n\n{text}")
                        } else {
                            text.to_owned()
                        };
                        self.within(
                            Some(agent.clone()),
                            json!({"type":"assistant_message","text":text}),
                        );
                        self.wordy.insert(agent.clone());
                    }
                }
                (Some("thinking"), Some(agent)) => {
                    if let Some(text) = text("thinking") {
                        self.within(Some(agent.clone()), json!({"type":"reasoning","text":text}));
                    }
                }
                _ => {}
            }
        }
    }

    /// An item of this conversation, or of the agent `within` it.
    fn within(&mut self, within: Option<String>, item: Value) {
        match within {
            None => self.item(item),
            Some(id) => {
                self.wordy.remove(&id);
                (self.emit)(Event::Sub { id, item });
            }
        }
    }

    fn called(&mut self, block: &Value, within: Option<String>) {
        let (Some(id), Some(name)) = (block["id"].as_str(), block["name"].as_str()) else {
            return;
        };
        let input = &block["input"];
        if name == "TodoWrite" {
            let items: Vec<_> = input["todos"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|todo| json!({"text":todo["content"],"completed":todo["status"] == "completed"}))
                .collect();
            return self.within(within, json!({"type":"todo","items":items}));
        }
        // A call that starts an agent: that agent is known by the call.
        if matches!(name, "Task" | "Agent") {
            self.agents.insert(id.to_owned());
            (self.emit)(Event::Subagent(json!({"id":id,"status":"running",
                "title":input["description"],"subtitle":input["subagent_type"]})));
        }
        self.calls.insert(
            id.to_owned(),
            Call {
                name: name.to_owned(),
                input: input.clone(),
                within: within.clone(),
            },
        );
        self.within(within, tool(id, name, "running", detail(name, input), None));
    }

    /// What the agent's calls came back with.
    fn returned(&mut self, event: &Value) {
        for block in event["message"]["content"].as_array().into_iter().flatten() {
            let Some(Call {
                name,
                input,
                within,
            }) = block["tool_use_id"]
                .as_str()
                .and_then(|id| self.calls.remove(id))
            else {
                continue;
            };
            let text = said(&block["content"]);
            let failed = block["is_error"] == true;
            let detail = answered(
                detail(&name, &input),
                &text,
                &event["tool_use_result"],
                failed,
            );
            let id = block["tool_use_id"].as_str().unwrap_or_default();
            let item = match (failed, self.stopping) {
                (true, true) => tool(id, &name, "canceled", detail, None),
                (true, false) => tool(id, &name, "failed", detail, Some(&clip(&text))),
                _ => tool(id, &name, "completed", detail, None),
            };
            // The call's answer is its agent's end, unless that agent
            // goes on in the background.
            if self.agents.contains(id) && (failed || !self.background.contains(id)) {
                (self.emit)(Event::Subagent(json!({"id":id,"status":item["status"]})));
            }
            self.within(within, item);
        }
    }

    fn result(&mut self, event: &Value) {
        let failed = event["is_error"] == true;
        self.outcome = match (failed, self.stopping) {
            (true, true) => Outcome::Canceled,
            (true, false) => Outcome::Failed(
                event["result"]
                    .as_str()
                    .filter(|text| !text.is_empty())
                    .map(str::to_owned)
                    .or_else(|| {
                        let errors: Vec<_> = event["errors"]
                            .as_array()?
                            .iter()
                            .filter_map(Value::as_str)
                            .collect();
                        (!errors.is_empty()).then(|| errors.join("\n"))
                    })
                    .unwrap_or_else(|| "Claude Code could not finish this turn".to_owned()),
            ),
            _ => Outcome::Completed,
        };
        self.stopping = false;
        if let Some(cost) = event["total_cost_usd"].as_f64() {
            self.spent = cost;
        }
        let models = event["modelUsage"].as_object();
        let window = models
            .and_then(|models| models.get(&self.model))
            .and_then(|model| model["contextWindow"].as_u64())
            .or_else(|| {
                models?
                    .values()
                    .filter_map(|model| model["contextWindow"].as_u64())
                    .max()
            });
        (self.emit)(Event::Usage(json!({"contextWindowUsedTokens":self.context,
            "contextWindowMaxTokens":window,"totalCostUsd":self.start.cost + self.spent})));
        if !self.states {
            self.idle();
        }
    }

    fn ask(&mut self, request: String, asked: &Value) {
        let tool = asked["tool_name"].as_str().unwrap_or_default().to_owned();
        let input = asked["input"].clone();
        let suggestions = asked["permission_suggestions"].clone();
        let (kind, title, actions) = match tool.as_str() {
            "ExitPlanMode" => (
                "plan",
                json!("Plan"),
                json!([{"id":"reject","label":"Reject","behavior":"deny"},
                    {"id":"implement","label":"Implement","behavior":"allow"}]),
            ),
            "AskUserQuestion" => ("question", Value::Null, json!([])),
            // With rules on offer that would stop it asking again, that
            // is a third answer; with none, a client's own two.
            _ if suggestions
                .as_array()
                .is_some_and(|rules| !rules.is_empty()) =>
            {
                (
                    "tool",
                    Value::Null,
                    json!([{"id":"allow","label":"Allow","behavior":"allow"},
                    {"id":"allow_always","label":"Always allow","behavior":"allow"},
                    {"id":"deny","label":"Deny","behavior":"deny"}]),
                )
            }
            _ => ("tool", Value::Null, json!([])),
        };
        let id = crate::state::id();
        (self.emit)(Event::Asked(
            json!({"id":id,"provider":"claude","name":tool,"kind":kind,"title":title,
            "input":input,"detail":detail(&tool, &input),"actions":actions}),
        ));
        self.asks.insert(
            id,
            Ask {
                request,
                tool,
                input,
                suggestions,
            },
        );
    }
}

/// The commands Claude Code has in `cwd` (its own, the project's, its
/// plugins' and skills'), asked of a process started for the question
/// and stopped once it has answered; nothing is sent to a model.
pub async fn commands(settings: &Value, cwd: &str) -> io::Result<Value> {
    let args = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
    ]
    .map(str::to_owned);
    let mut process = Some(Process::spawn("claude", settings, cwd, &args, &[])?);
    let asking = async {
        if let Some(process) = &mut process {
            process
                .send(&json!({"type":"control_request","request_id":"commands","request":{"subtype":"initialize"}}))
                .await?;
        }
        loop {
            let line = super::line(&mut process)
                .await
                .ok_or_else(|| io::Error::other("Claude Code stopped before answering"))?;
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let response = &event["response"];
            if event["type"] == "control_response" && response["request_id"] == "commands" {
                return Ok(response["response"]["commands"].clone());
            }
        }
    };
    tokio::time::timeout(std::time::Duration::from_secs(20), asking)
        .await
        .map_err(|_| io::Error::other("Claude Code did not answer in time"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_call_is_shown_by_what_its_tool_does() {
        assert_eq!(
            detail("Bash", &json!({"command":"ls"})),
            json!({"type":"shell","command":"ls"})
        );
        assert_eq!(detail("Read", &json!({"file_path":"/a"}))["filePath"], "/a");
        assert_eq!(
            detail("mcp__x__y", &json!({"a":1})),
            json!({"type":"unknown","input":{"a":1}})
        );
        let done = answered(
            detail("Bash", &json!({"command":"false"})),
            "Exit code 2\nboom",
            &Value::Null,
            true,
        );
        assert_eq!(
            (done["output"].clone(), done["exitCode"].clone()),
            (json!("Exit code 2\nboom"), json!(2))
        );
        let read = answered(
            detail("Read", &json!({"file_path":"/a"})),
            "1\thello",
            &json!({"file":{"content":"hello\n"}}),
            false,
        );
        assert_eq!(read["content"], "hello\n");
        let edit = answered(
            detail(
                "Edit",
                &json!({"file_path":"/a","old_string":"a","new_string":"b"}),
            ),
            "ok",
            &json!({"structuredPatch":[{"oldStart":1,"oldLines":1,"newStart":1,"newLines":1,"lines":["-a","+b"]}]}),
            false,
        );
        assert_eq!(edit["unifiedDiff"], "@@ -1,1 +1,1 @@\n-a\n+b\n");
    }

    #[test]
    fn answers_are_filed_under_their_questions() {
        let input = json!({"questions":[{"question":"Which one?","header":"Pick"},{"question":"Sure?"}],
            "answers":{"Pick":"A","Sure?":"Yes","Stray":"x"}});
        assert_eq!(
            answers(input)["answers"],
            json!({"Which one?":"A","Sure?":"Yes"})
        );
    }

    #[test]
    fn long_output_is_cut_on_a_character() {
        let long = "é".repeat(KEPT);
        let cut = clip(&long);
        assert!(cut.len() <= KEPT + 3 && cut.ends_with('…'));
    }
}
