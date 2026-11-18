use crate::{
    provider::{self, Command, Event, Outcome},
    state::{State, id},
    timeline::Timeline,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io,
    path::PathBuf,
    rc::Rc,
};
use tokio::sync::{broadcast, mpsc::UnboundedSender};

pub type Shared = Rc<RefCell<Host>>;

pub struct Host {
    pub home: PathBuf,
    pub state: State,
    pub events: broadcast::Sender<Value>,
    /// Each agent's session, once it has had something to do.
    sessions: HashMap<String, UnboundedSender<Command>>,
    /// Agents told to stop and not stopped yet: told again, their
    /// process goes.
    stopping: HashSet<String>,
    /// Requests to create a workspace whose worktree is being checked
    /// out, by the key each was asked under.
    creating: HashSet<String>,
    /// The commands a provider's CLI has in a directory, as it last
    /// said: by provider and directory.
    commands: HashMap<(String, String), Value>,
    timelines: HashMap<String, Timeline>,
    pub terminals: crate::terminal::Terminals,
}

pub fn envelope(kind: &str, payload: Value) -> Value {
    json!({"type":"session","message":{"type":kind,"payload":payload}})
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key]
        .as_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Missing {key}"))
}

fn now() -> Value {
    json!(chrono::Utc::now().to_rfc3339())
}

/// What a workspace's agents add up to, the most pressing first.
fn standing<'a>(agents: impl Iterator<Item = &'a Value> + Clone) -> &'static str {
    let any = |test: fn(&Value) -> bool| agents.clone().any(test);
    if any(|agent| {
        agent["pendingPermissions"]
            .as_array()
            .is_some_and(|asked| !asked.is_empty())
    }) {
        "needs_input"
    } else if any(|agent| agent["status"] == "running") {
        "running"
    } else if any(|agent| agent["status"] == "error" && agent["requiresAttention"] == true) {
        "failed"
    } else if any(|agent| agent["requiresAttention"] == true) {
        "attention"
    } else {
        "done"
    }
}

impl Host {
    /// A host over `state` as it was saved: nothing in it is running
    /// (whatever was, its process went with the service that ran it),
    /// and each agent has the modes its provider gives.
    pub fn new(home: PathBuf, mut state: State) -> io::Result<Self> {
        let config = crate::config::read(&home)?;
        for agent in state.agents.values_mut() {
            if agent["status"] == "running" || agent["status"] == "initializing" {
                agent["status"] = json!("idle");
            }
            agent["pendingPermissions"] = json!([]);
            let provider = agent["provider"].as_str().unwrap_or_default();
            if let Some(family) = provider::family(provider, &config["providers"][provider]) {
                let wanted = agent["currentModeId"]
                    .as_str()
                    .or(agent.pointer("/config/modeId").and_then(Value::as_str));
                agent["currentModeId"] = json!(provider::mode(family, wanted));
                agent["availableModes"] = provider::modes(family);
            }
        }
        let mut host = Self {
            home,
            state,
            events: broadcast::channel(4096).0,
            sessions: HashMap::new(),
            stopping: HashSet::new(),
            creating: HashSet::new(),
            commands: HashMap::new(),
            timelines: HashMap::new(),
            terminals: Default::default(),
        };
        for started in host.state.subagents.values_mut().flatten() {
            if started["status"] == "running" {
                started["status"] = json!("canceled");
            }
        }
        for (agent, rows) in std::mem::take(&mut host.state.history) {
            if rows.is_empty() || !host.state.agents.contains_key(&agent) {
                continue;
            }
            let timeline = host.timeline(&agent).map_err(io::Error::other)?;
            if timeline.page(&agent, &json!({"limit":1}))["entries"] == json!([]) {
                for row in &rows {
                    timeline.push(row);
                }
                timeline.save()?;
            }
        }
        host.persist()?;
        Ok(host)
    }

    fn persist(&self) -> io::Result<()> {
        self.state.save(&self.home)
    }

    fn publish(&self, kind: &str, payload: Value) {
        let _ = self.events.send(envelope(kind, payload));
    }

    /// The agent's conversation, read from its log the first time. A
    /// call its log left running is not: nothing ran it past the
    /// service that wrote that.
    fn timeline(&mut self, agent: &str) -> Result<&mut Timeline, String> {
        if !self.state.agents.contains_key(agent) {
            return Err("Unknown conversation".into());
        }
        if !self.timelines.contains_key(agent) {
            if agent.is_empty()
                || !agent
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
            {
                return Err("Unknown conversation".into());
            }
            let path = self.home.join("timelines").join(format!("{agent}.jsonl"));
            let mut timeline = Timeline::load(path).map_err(|error| error.to_string())?;
            // A conversation that came with its id and nothing said in
            // it (brought over from the old service): what was said is
            // in its CLI's record of that session.
            let record = &self.state.agents[agent];
            if timeline.is_empty()
                && let Some(session) = record
                    .pointer("/persistence/sessionId")
                    .and_then(Value::as_str)
                && let Some(provider) = record["provider"].as_str()
            {
                let config = crate::config::read(&self.home).map_err(|error| error.to_string())?;
                let settings = &config["providers"][provider];
                if let Some(family) = provider::family(provider, settings) {
                    for item in crate::history::read(family, settings, session) {
                        timeline.push(&item);
                    }
                    timeline.save().map_err(|error| error.to_string())?;
                }
            }
            if !self.sessions.contains_key(agent) {
                for item in timeline.unfinished("canceled") {
                    timeline.push(&item);
                }
                timeline.save().map_err(|error| error.to_string())?;
            }
            self.timelines.insert(agent.to_owned(), timeline);
        }
        Ok(self.timelines.get_mut(agent).unwrap())
    }

    /// The timeline of an agent that `agent` started.
    fn subtimeline(&mut self, agent: &str, started: &str) -> Result<&mut Timeline, String> {
        let named = |id: &str| {
            !id.is_empty()
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        };
        if !self.state.agents.contains_key(agent) || !named(agent) || !named(started) {
            return Err("Unknown sub-agent".into());
        }
        let key = format!("{agent}/{started}");
        if !self.timelines.contains_key(&key) {
            let path = self
                .home
                .join("timelines")
                .join(format!("{agent}.subs"))
                .join(format!("{started}.jsonl"));
            self.timelines.insert(
                key.clone(),
                Timeline::load(path).map_err(|error| error.to_string())?,
            );
        }
        Ok(self.timelines.get_mut(&key).unwrap())
    }

    /// Adds what is `known` of an agent `agent` started to what was,
    /// and tells clients.
    fn subagent(&mut self, agent: &str, known: &Value) {
        let Some(id) = known["id"].as_str() else {
            return;
        };
        let started = self.state.subagents.entry(agent.to_owned()).or_default();
        let at = started
            .iter()
            .position(|record| record["id"] == id)
            .unwrap_or_else(|| {
                started.push(json!({"id":id,"parentAgentId":agent}));
                started.len() - 1
            });
        for (key, value) in known
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(_, value)| !value.is_null())
        {
            started[at][key] = value.clone();
        }
        let record = started[at].clone();
        self.publish(
            "agent.provider_subagents.update",
            json!({"kind":"upsert","subagent":record}),
        );
    }

    /// Tells clients the agent as it is now, and its workspace if what
    /// its agents add up to changed with it.
    fn update_agent(&mut self, agent: &str) {
        let Some(record) = self.state.agents.get(agent) else {
            return;
        };
        self.publish("agent_update", json!({"kind":"upsert","agent":record}));
        let Some(workspace) = record["workspaceId"].as_str().map(str::to_owned) else {
            return;
        };
        let status = standing(self.state.agents.values().filter(|agent| {
            agent["workspaceId"] == workspace.as_str() && agent["archivedAt"].is_null()
        }));
        if let Some(record) = self.state.workspaces.get_mut(&workspace)
            && record["status"] != status
        {
            record["status"] = json!(status);
            record["activityAt"] = now();
            let record = record.clone();
            self.publish(
                "workspace_update",
                json!({"kind":"upsert","workspace":record}),
            );
        }
    }

    /// Adds an item to the agent's conversation and its stream. What a
    /// reply is made of is saved with what follows it; anything else
    /// (a message sent, a call made) now.
    fn append(&mut self, agent: &str, item: Value) -> Result<u64, String> {
        let timeline = self.timeline(agent)?;
        let seq = timeline.push(&item);
        if !matches!(
            item["type"].as_str(),
            Some("assistant_message" | "reasoning")
        ) {
            timeline.save().map_err(|error| error.to_string())?;
        }
        self.publish(
            "agent_stream",
            json!({"agentId":agent,"seq":seq,"epoch":agent,"event":{"type":"timeline","item":item}}),
        );
        Ok(seq)
    }

    /// The turn the agent was in is over.
    fn finished(&mut self, agent: &str, outcome: Outcome) {
        if self
            .state
            .agents
            .get(agent)
            .is_none_or(|record| record["status"] != "running")
        {
            return;
        }
        self.stopping.remove(agent);
        let (event, ended) = match &outcome {
            Outcome::Completed => ("turn_completed", "completed"),
            Outcome::Canceled => ("turn_canceled", "canceled"),
            Outcome::Failed(_) => ("turn_failed", "failed"),
        };
        let mut left = Vec::new();
        if let Ok(timeline) = self.timeline(agent) {
            if ended != "completed" {
                left = timeline.unfinished(ended);
            }
        }
        if let Outcome::Failed(message) = &outcome {
            left.push(json!({"type":"error","message":message}));
        }
        // The agents it started end with a turn that did not finish;
        // what each had written is kept either way.
        let started: Vec<String> = self
            .state
            .subagents
            .get(agent)
            .into_iter()
            .flatten()
            .filter_map(|record| record["id"].as_str().map(str::to_owned))
            .collect();
        for id in started {
            let running = self.state.subagents[agent]
                .iter()
                .any(|record| record["id"] == id.as_str() && record["status"] == "running");
            if running && ended != "completed" {
                self.subagent(agent, &json!({"id":id,"status":ended}));
            }
            if let Some(timeline) = self.timelines.get_mut(&format!("{agent}/{id}")) {
                let _ = timeline.save();
            }
        }
        for item in left {
            if let Err(error) = self.append(agent, item) {
                eprintln!("Saving the end of a turn: {error}");
            }
        }
        let reply = self.timeline(agent).ok().and_then(|timeline| {
            let _ = timeline.save();
            timeline.last_reply().map(str::to_owned)
        });
        let record = self.state.agents.get_mut(agent).unwrap();
        record["status"] = json!(if ended == "failed" { "error" } else { "idle" });
        record["pendingPermissions"] = json!([]);
        record["updatedAt"] = now();
        if ended != "canceled" {
            record["requiresAttention"] = json!(true);
            record["attentionReason"] = json!(if ended == "failed" {
                "error"
            } else {
                "finished"
            });
        }
        if let Some(schedule) = record
            .pointer("/labels/sophon.schedule-id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            && let Some(run) = self
                .state
                .runs
                .get_mut(&schedule)
                .and_then(|runs| runs.iter_mut().rev().find(|run| run["agentId"] == agent))
        {
            run["endedAt"] = now();
            run["status"] = json!(ended);
            run["output"] = json!(reply.unwrap_or_default());
        }
        if let Err(error) = self.persist() {
            eprintln!("Saving a turn: {error}");
        }
        self.update_agent(agent);
        self.publish(
            "agent_stream",
            json!({"agentId":agent,"event":{"type":event}}),
        );
    }

    /// What the agent's session said happened.
    fn heard(&mut self, agent: &str, event: Event) {
        let Some(record) = self.state.agents.get_mut(agent) else {
            return;
        };
        match event {
            Event::Item(item) => {
                if let Err(error) = self.append(agent, item) {
                    eprintln!("Saving a timeline item: {error}");
                }
                return;
            }
            Event::Idle(outcome) => return self.finished(agent, outcome),
            Event::Commands(list) => {
                let place = |key: &str| record[key].as_str().unwrap_or_default().to_owned();
                self.commands
                    .insert((place("provider"), place("cwd")), list);
                return;
            }
            Event::Sub { id, item } => {
                let kept = self.subtimeline(agent, &id).and_then(|timeline| {
                    timeline.push(&item);
                    if matches!(
                        item["type"].as_str(),
                        Some("assistant_message" | "reasoning")
                    ) {
                        return Ok(());
                    }
                    timeline.save().map_err(|error| error.to_string())
                });
                if let Err(error) = kept {
                    eprintln!("Saving a sub-agent's item: {error}");
                }
                return;
            }
            Event::Subagent(known) => {
                self.subagent(agent, &known);
                if let Err(error) = self.persist() {
                    eprintln!("Saving a sub-agent: {error}");
                }
                return;
            }
            Event::Busy => {
                if record["status"] == "running" {
                    return;
                }
                record["status"] = json!("running");
                record["requiresAttention"] = json!(false);
                self.stopping.remove(agent);
                self.publish(
                    "agent_stream",
                    json!({"agentId":agent,"event":{"type":"turn_started"}}),
                );
            }
            Event::Session(session) => record["persistence"] = json!({"sessionId":session}),
            Event::Mode(mode) => record["currentModeId"] = json!(mode),
            // Kept with the turn's end; a client sees it now.
            Event::Usage(usage) => {
                for (key, value) in usage
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(_, value)| !value.is_null())
                {
                    record["lastUsage"][key] = value.clone();
                }
                return self.update_agent(agent);
            }
            Event::Asked(request) => {
                match record["pendingPermissions"].as_array_mut() {
                    Some(asked) => asked.push(request.clone()),
                    None => record["pendingPermissions"] = json!([request]),
                }
                record["requiresAttention"] = json!(true);
                record["attentionReason"] = json!("permission");
                self.publish(
                    "agent_stream",
                    json!({"agentId":agent,"event":{"type":"permission_requested","request":request}}),
                );
            }
            Event::Settled(permission) => {
                let Some(asked) = record["pendingPermissions"].as_array_mut() else {
                    return;
                };
                asked.retain(|request| request["id"] != permission.as_str());
                if asked.is_empty() && record["attentionReason"] == "permission" {
                    record["requiresAttention"] = json!(false);
                }
                self.publish(
                    "agent_stream",
                    json!({"agentId":agent,"event":{"type":"permission_resolved","requestId":permission}}),
                );
            }
        }
        if let Err(error) = self.persist() {
            eprintln!("Saving an agent: {error}");
        }
        self.update_agent(agent);
    }
}

/// The agent's session, started if it has none (or its last one ended).
fn session(
    shared: &Shared,
    host: &mut Host,
    agent_id: &str,
) -> Result<UnboundedSender<Command>, String> {
    if let Some(commands) = host
        .sessions
        .get(agent_id)
        .filter(|commands| !commands.is_closed())
    {
        return Ok(commands.clone());
    }
    let agent = host.state.agents.get(agent_id).ok_or("Unknown agent")?;
    let provider = text(agent, "provider")?;
    let config = crate::config::read(&host.home).map_err(|error| error.to_string())?;
    let settings = config["providers"][provider].clone();
    if settings["enabled"] == false {
        return Err("Provider is disabled".into());
    }
    let family = provider::family(provider, &settings).ok_or("Unknown provider")?;
    let field = |key: &str| {
        agent[key]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let start = provider::Start {
        family,
        cwd: text(agent, "cwd")?.to_owned(),
        settings,
        session: agent
            .pointer("/persistence/sessionId")
            .and_then(Value::as_str)
            .map(str::to_owned),
        model: field("model"),
        thinking: field("thinkingOptionId"),
        mode: provider::mode(family, agent["currentModeId"].as_str()),
        cost: agent
            .pointer("/lastUsage/totalCostUsd")
            .and_then(Value::as_f64)
            .unwrap_or(0.0),
    };
    let (shared, agent) = (shared.clone(), agent_id.to_owned());
    let commands = provider::start(
        start,
        Box::new(move |event| shared.borrow_mut().heard(&agent, event)),
    );
    host.sessions.insert(agent_id.to_owned(), commands.clone());
    Ok(commands)
}

/// Sends `prompt` to the agent: into the turn underway if there is one
/// (after stopping it, if the client asked for that), else as a new
/// turn. Answers where the message is in the conversation; the same
/// `message` sent again (a client that never saw the answer) gets that
/// place back and nothing is sent twice.
fn turn(
    shared: &Shared,
    host: &mut Host,
    agent_id: &str,
    prompt: &str,
    message: &Value,
    input: &Value,
) -> Result<Value, String> {
    let message_id = message
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(id);
    let receipt = |seq: u64| json!({"agentId":agent_id,"messageId":message_id,"seq":seq});
    if let Some(seq) = host.timeline(agent_id)?.sent(&message_id) {
        return Ok(receipt(seq));
    }
    let mut provider_prompt = prompt.to_owned();
    for attachment in input["attachments"].as_array().into_iter().flatten() {
        let path =
            std::fs::canonicalize(text(attachment, "path")?).map_err(|error| error.to_string())?;
        let root =
            std::fs::canonicalize(host.home.join("uploads")).map_err(|error| error.to_string())?;
        if path.parent() != Some(root.as_path()) || !path.is_file() {
            return Err("Unknown uploaded file".into());
        }
        provider_prompt.push_str(&format!(
            "\n\nAttached file {}: {}",
            text(attachment, "fileName")?,
            path.display()
        ));
    }
    let commands = session(shared, host, agent_id)?;
    let running = host.state.agents[agent_id]["status"] == "running";
    if running && input["activeTurnBehavior"] == "interrupt" {
        let _ = commands.send(Command::Interrupt);
    }
    let seq = host.append(
        agent_id,
        json!({"type":"user_message","text":prompt,"messageId":message_id}),
    )?;
    host.stopping.remove(agent_id);
    let record = host.state.agents.get_mut(agent_id).unwrap();
    // A conversation begun with nothing said is named by what is
    // first said in it.
    if record["title"].as_str().is_none_or(str::is_empty) {
        record["title"] = json!(prompt.chars().take(72).collect::<String>());
    }
    record["status"] = json!("running");
    record["requiresAttention"] = json!(false);
    record["lastUserMessageAt"] = now();
    record["updatedAt"] = now();
    host.persist().map_err(|error| error.to_string())?;
    host.update_agent(agent_id);
    if !running {
        host.publish(
            "agent_stream",
            json!({"agentId":agent_id,"event":{"type":"turn_started"}}),
        );
    }
    let images = input["images"].as_array().cloned().unwrap_or_default();
    if commands
        .send(Command::Prompt {
            text: provider_prompt,
            images,
        })
        .is_err()
    {
        host.finished(
            agent_id,
            Outcome::Failed("The agent's session ended".into()),
        );
    }
    Ok(receipt(seq))
}

fn check_input(input: &Value) -> Result<(), String> {
    if input["images"]
        .as_array()
        .is_some_and(|images| images.len() > 16)
        || input["attachments"]
            .as_array()
            .is_some_and(|files| files.len() > 64)
    {
        return Err("Too many attachments".into());
    }
    Ok(())
}

/// The CLI the agent a client asks for would be, if what it asks is
/// in order: no more attached than is taken, a provider this host has
/// and has not turned off.
fn family_of(home: &std::path::Path, input: &Value) -> Result<&'static str, String> {
    check_input(input)?;
    let provider = text(&input["config"], "provider")?;
    let settings = crate::config::read(home).map_err(|error| error.to_string())?;
    let entry = &settings["providers"][provider];
    provider::family(provider, entry)
        .filter(|_| entry["enabled"] != false)
        .ok_or_else(|| "Unknown provider".into())
}

/// Answers a request to create a workspace, or an agent in one. A
/// workspace that is a new worktree has its checkout made first, with
/// nothing of the host held while `git` works; then, or for anything
/// else, this is `dispatch`.
pub async fn create(shared: &Shared, request: &Value) -> Result<Value, String> {
    let source = &request["source"];
    if request["type"] != "workspace.create.request" || source["kind"] != "worktree" {
        return dispatch(shared, request);
    }
    let key = request["idempotencyKey"].as_str();
    let (home, project) = {
        let mut host = shared.borrow_mut();
        // Made already: answered as it was. Being made: this is a
        // second asking of the same, and one checkout is enough.
        if key.is_some_and(|key| host.state.creations.contains_key(key)) {
            drop(host);
            return dispatch(shared, request);
        }
        if key.is_some_and(|key| host.creating.contains(key)) {
            return Err("This workspace is being created".into());
        }
        family_of(&host.home, request.get("agent").unwrap_or(request))?;
        let project = host.state.project(text(source, "cwd")?)?;
        if let Some(key) = key {
            host.creating.insert(key.to_owned());
        }
        (host.home.clone(), project)
    };
    let made = crate::git::worktree(&home, &project, source).await;
    if let Some(key) = key {
        shared.borrow_mut().creating.remove(key);
    }
    answer(shared, request, Some(made?))
}

/// Answers what commands the agent's CLI has where the agent runs: as
/// that CLI last said, or by asking it, which takes a moment and is
/// awaited with nothing of the host held.
pub async fn commands(shared: &Shared, request: &Value) -> Result<Value, String> {
    let agent_id = text(request, "agentId")?;
    let reply = |list: &Value| {
        envelope(
            "list_commands_response",
            json!({"agentId":agent_id,"commands":list,"requestId":request["requestId"]}),
        )
    };
    let (family, settings, place) = {
        let host = shared.borrow();
        let agent = host.state.agents.get(agent_id).ok_or("Unknown agent")?;
        let place = (
            text(agent, "provider")?.to_owned(),
            text(agent, "cwd")?.to_owned(),
        );
        if let Some(known) = host.commands.get(&place) {
            return Ok(reply(known));
        }
        let config = crate::config::read(&host.home).map_err(|error| error.to_string())?;
        let settings = config["providers"][&place.0].clone();
        let family = provider::family(&place.0, &settings).ok_or("Unknown provider")?;
        (family, settings, place)
    };
    // A CLI that cannot say has none for now, and is asked again.
    match provider::commands(family, &settings, &place.1).await {
        Ok(list) if list.is_array() => {
            shared.borrow_mut().commands.insert(place, list.clone());
            Ok(reply(&list))
        }
        _ => Ok(reply(&json!([]))),
    }
}

pub fn dispatch(shared: &Shared, request: &Value) -> Result<Value, String> {
    answer(shared, request, None)
}

/// `made`: the worktree checked out for the workspace this request
/// creates, when it asks for one (`create`).
fn answer(shared: &Shared, request: &Value, mut made: Option<Value>) -> Result<Value, String> {
    let mut host = shared.borrow_mut();
    let kind = text(request, "type")?;
    let mut response = kind.replace("request", "response");
    let mut payload = match kind {
        "session.events.set_subscription.request" | "agent.timeline.set_subscription.request" => {
            json!({})
        }
        "project.list.request" => {
            json!({"projects": host.state.projects.values().filter(|project|project["archivedAt"].is_null()).collect::<Vec<_>>()})
        }
        "fetch_workspaces_request" => {
            json!({"entries":host.state.workspaces.values().filter(|workspace|workspace["archivedAt"].is_null()).collect::<Vec<_>>()})
        }
        "fetch_agents_request" => {
            json!({"entries":host.state.agents.values().filter(|agent| agent["archivedAt"].is_null()).collect::<Vec<_>>()})
        }
        "fetch_agent_request" => {
            json!({"agent":host.state.agents.get(text(request,"agentId")?).ok_or("Unknown agent")?})
        }
        "get_providers_snapshot_request" | "refresh_providers_snapshot_request" => {
            provider::snapshot(&crate::config::read(&host.home).map_err(|error| error.to_string())?)
        }
        "get_daemon_config_request" => {
            json!({"config":crate::config::read(&host.home).map_err(|error|error.to_string())?})
        }
        "set_daemon_config_request" => {
            let config = crate::config::update(&host.home, &request["config"])
                .map_err(|error| error.to_string())?;
            // A session has its provider's settings as they were when it
            // started; those with nothing underway start again with these.
            let agents = &host.state.agents;
            let busy: HashSet<String> = host
                .sessions
                .keys()
                .filter(|agent| {
                    agents
                        .get(*agent)
                        .is_some_and(|record| record["status"] == "running")
                })
                .cloned()
                .collect();
            host.sessions.retain(|agent, _| busy.contains(agent));
            host.publish("providers_snapshot_update", provider::snapshot(&config));
            json!({"config":config})
        }
        "fetch_agent_timeline_request" => {
            let agent = text(request, "agentId")?;
            let mut page = host.timeline(agent)?.page(agent, request);
            page["agent"] = host.state.agents[agent].clone();
            page
        }
        "daemon.get_pairing_offer.request" => json!({"relayEnabled":false,"url":null}),
        "agent.provider_subagents.list.request" => {
            let parent = text(request, "parentAgentId")?;
            json!({"parentAgentId":parent,
                "subagents":host.state.subagents.get(parent).cloned().unwrap_or_default()})
        }
        "agent.provider_subagents.timeline.get.request" => {
            let (parent, started) = (
                text(request, "parentAgentId")?,
                text(request, "subagentId")?,
            );
            let page = host.subtimeline(parent, started)?.page(started, request);
            json!({"parentAgentId":parent,"subagentId":started,"rows":page["entries"],"hasOlder":page["hasOlder"]})
        }
        "list_provider_features_request" => json!({"provider":request["provider"],"features":[]}),
        "fetch_agent_history_request" => {
            let query = request["search"]
                .as_str()
                .unwrap_or_default()
                .to_lowercase();
            let mut entries: Vec<_> = host
                .state
                .agents
                .values()
                .filter(|agent| {
                    agent["title"]
                        .as_str()
                        .unwrap_or_default()
                        .to_lowercase()
                        .contains(&query)
                })
                .collect();
            entries.sort_by(|left, right| {
                right["updatedAt"].as_str().cmp(&left["updatedAt"].as_str())
            });
            json!({"entries":entries})
        }
        "update_agent_request"
        | "set_agent_model_request"
        | "set_agent_mode_request"
        | "set_agent_thinking_request"
        | "archive_agent_request"
        | "clear_agent_attention"
        | "refresh_agent_request" => {
            let agent_id = text(request, "agentId")?;
            let host = &mut *host;
            let agent = host.state.agents.get_mut(agent_id).ok_or("Unknown agent")?;
            // Told to the agent's session if it has one; one started
            // later starts with what its record says.
            let tell = |command: Command| {
                if let Some(commands) = host.sessions.get(agent_id) {
                    let _ = commands.send(command);
                }
            };
            let chosen = |key: &str| {
                request[key]
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            };
            match kind {
                "archive_agent_request" => {
                    agent["archivedAt"] = now();
                    agent["status"] = json!("idle");
                    agent["pendingPermissions"] = json!([]);
                    host.sessions.remove(agent_id);
                }
                "set_agent_model_request" => {
                    agent["model"] = request["modelId"].clone();
                    tell(Command::Model(chosen("modelId")));
                }
                "set_agent_thinking_request" => {
                    agent["thinkingOptionId"] = request["thinkingOptionId"].clone();
                    tell(Command::Thinking(chosen("thinkingOptionId")));
                }
                "set_agent_mode_request" => {
                    let mode = text(request, "modeId")?;
                    if !agent["availableModes"]
                        .as_array()
                        .is_some_and(|modes| modes.iter().any(|known| known["id"] == mode))
                    {
                        return Err("This agent has no such mode".into());
                    }
                    agent["currentModeId"] = json!(mode);
                    tell(Command::Mode(mode.to_owned()));
                }
                "clear_agent_attention" => agent["requiresAttention"] = json!(false),
                "refresh_agent_request" => {
                    if request["restore"] == true {
                        agent["archivedAt"] = Value::Null
                    }
                }
                _ => {
                    if let Some(name) = request["name"].as_str() {
                        agent["title"] = json!(name)
                    }
                    if let Some(labels) = request["labels"].as_object() {
                        if !agent["labels"].is_object() {
                            agent["labels"] = json!({});
                        }
                        for (key, value) in labels {
                            agent["labels"][key] = value.clone()
                        }
                    }
                }
            }
            host.persist().map_err(|error| error.to_string())?;
            host.update_agent(agent_id);
            json!({"agent":host.state.agents[agent_id]})
        }
        // The client's answer to what an agent asked. The session says
        // when it is settled; with no session left to tell (the service
        // started again since), it is settled here.
        "agent_permission_response" => {
            let agent_id = text(request, "agentId")?;
            let permission = text(request, "requestId")?;
            let told = host.sessions.get(agent_id).is_some_and(|commands| {
                commands
                    .send(Command::Answer {
                        id: permission.to_owned(),
                        response: request["response"].clone(),
                    })
                    .is_ok()
            });
            if !told {
                host.heard(agent_id, Event::Settled(permission.to_owned()));
            }
            return Ok(envelope(
                "agent_permission_resolved",
                json!({"agentId":agent_id,"requestId":permission}),
            ));
        }
        "workspace.title.set.request"
        | "workspace.pin.set.request"
        | "workspace.mark_unread.request" => {
            let workspace = host
                .state
                .workspaces
                .get_mut(text(request, "workspaceId")?)
                .ok_or("Unknown workspace")?;
            match kind {
                "workspace.title.set.request" => workspace["name"] = json!(text(request, "title")?),
                "workspace.mark_unread.request" => workspace["status"] = json!("attention"),
                _ => {
                    workspace["pinnedAt"] = if request["pinned"] == true {
                        now()
                    } else {
                        Value::Null
                    }
                }
            }
            let workspace = workspace.clone();
            host.persist().map_err(|error| error.to_string())?;
            host.publish(
                "workspace_update",
                json!({"kind":"upsert","workspace":workspace}),
            );
            json!({"workspace":workspace})
        }
        kind if kind == "file_explorer_request" || kind.starts_with("fs.") => {
            let cwd = text(request, "cwd")?;
            if !host
                .state
                .workspaces
                .values()
                .any(|workspace| workspace["workspaceDirectory"] == cwd)
                && !host
                    .state
                    .projects
                    .values()
                    .any(|project| project["projectRootPath"] == cwd)
            {
                return Err("Unknown workspace directory".into());
            }
            crate::files::execute(std::path::Path::new(cwd), request).unwrap_or_else(|error| {
                json!({
                "cwd":cwd,"path":request["path"],"mode":request["mode"],"error":error.to_string(),
                "result":{"status":"error","error":error.to_string()}})
            })
        }
        "directory_suggestions_request" => crate::projects::suggestions(request)?,
        "workspace.label.list.request" => {
            json!({"labels":host.state.labels.values().collect::<Vec<_>>()})
        }
        "workspace.label.assignment.set.request" => {
            let workspace_id = text(request, "workspaceId")?;
            let name = text(&request["label"], "name")?;
            let workspace = host
                .state
                .workspaces
                .get_mut(workspace_id)
                .ok_or("Unknown workspace")?;
            let mut labels = workspace["labels"].as_array().cloned().unwrap_or_default();
            labels.retain(|label| label != name);
            if request["assigned"] == true {
                labels.push(json!(name));
            }
            workspace["labels"] = json!(labels);
            let workspace = workspace.clone();
            host.state
                .labels
                .insert(name.into(), request["label"].clone());
            host.persist().map_err(|error| error.to_string())?;
            host.publish(
                "workspace_update",
                json!({"kind":"upsert","workspace":workspace}),
            );
            json!({})
        }
        "project.rename.request" | "project.remove.request" => {
            let key = text(request, "projectId")?;
            let project = host.state.projects.get_mut(key).ok_or("Unknown project")?;
            let removing = kind == "project.remove.request";
            if removing {
                project["archivedAt"] = now()
            } else {
                project["projectDisplayName"] = json!(text(request, "customName")?)
            }
            let project = project.clone();
            let mut workspaces = Vec::new();
            for workspace in host
                .state
                .workspaces
                .values_mut()
                .filter(|workspace| workspace["projectId"] == key)
            {
                if removing {
                    workspace["archivedAt"] = project["archivedAt"].clone()
                }
                workspace["projectDisplayName"] = project["projectDisplayName"].clone();
                workspaces.push(workspace.clone());
            }
            host.persist().map_err(|error| error.to_string())?;
            if removing {
                host.publish("project.update", json!({"kind":"remove","projectId":key}))
            } else {
                host.publish("project.update", json!({"kind":"upsert","project":project}))
            }
            for workspace in workspaces {
                host.publish(
                    "workspace_update",
                    if removing {
                        json!({"kind":"remove","id":workspace["id"]})
                    } else {
                        json!({"kind":"upsert","workspace":workspace})
                    },
                );
            }
            if removing {
                json!({})
            } else {
                json!({"project":project})
            }
        }
        "project.add.request" | "project.create_directory.request" => {
            let cwd = if kind == "project.create_directory.request" {
                crate::projects::create_dir(request)?
            } else {
                text(request, "cwd")?.to_owned()
            };
            let project = host.state.project(&cwd)?;
            host.persist().map_err(|error| error.to_string())?;
            host.publish("project.update", json!({"kind":"upsert","project":project}));
            json!({"project":project})
        }
        "workspace.create.request" | "create_agent_request" => {
            let key = request["idempotencyKey"].as_str();
            if let Some(agent) = key
                .and_then(|key| host.state.creations.get(key))
                .and_then(|agent| host.state.agents.get(agent))
            {
                let workspace = agent["workspaceId"]
                    .as_str()
                    .and_then(|id| host.state.workspaces.get(id));
                return Ok(envelope(
                    &response,
                    json!({"requestId":request["requestId"],"agent":agent,"workspace":workspace}),
                ));
            }
            let input = request.get("agent").unwrap_or(request);
            let family = family_of(&host.home, input)?;
            let config = &input["config"];
            let provider = text(config, "provider")?;
            let workspace = if kind == "create_agent_request" {
                host.state
                    .workspaces
                    .get(text(request, "workspaceId")?)
                    .ok_or("Unknown workspace")?
                    .clone()
            } else {
                let source = &request["source"];
                let worktree = match text(source, "kind")? {
                    "directory" => false,
                    "worktree" => true,
                    _ => return Err("Unknown workspace source".into()),
                };
                let project = host
                    .state
                    .project(text(source, if worktree { "cwd" } else { "path" })?)?;
                host.publish("project.update", json!({"kind":"upsert","project":project}));
                if worktree {
                    let workspace = made
                        .take()
                        .ok_or("A worktree is checked out before its workspace is made")?;
                    host.state
                        .workspaces
                        .insert(text(&workspace, "id")?.to_owned(), workspace.clone());
                    workspace
                } else {
                    host.state.workspace(&project)
                }
            };
            let agent_id = id();
            if let Some(key) = key {
                host.state
                    .creations
                    .insert(key.to_owned(), agent_id.clone());
            }
            let prompt = input["initialPrompt"].as_str().unwrap_or_default();
            let created = now();
            let labels = if input["labels"].is_object() {
                input["labels"].clone()
            } else {
                json!({})
            };
            let agent = json!({"id":agent_id,"provider":provider,"cwd":workspace["workspaceDirectory"],
                "workspaceId":workspace["id"],"status":"idle","model":config["model"],
                "thinkingOptionId":config["thinkingOptionId"],"labels":labels,
                "currentModeId":provider::mode(family, config["modeId"].as_str()),
                "availableModes":provider::modes(family),"pendingPermissions":[],
                "title":prompt.chars().take(72).collect::<String>(),"createdAt":created,"updatedAt":created});
            host.state.agents.insert(agent_id.clone(), agent);
            host.persist().map_err(|error| error.to_string())?;
            host.publish(
                "workspace_update",
                json!({"kind":"upsert","workspace":workspace}),
            );
            host.update_agent(&agent_id);
            if !prompt.is_empty() {
                turn(
                    shared,
                    &mut host,
                    &agent_id,
                    prompt,
                    &input["clientMessageId"],
                    input,
                )?;
            }
            if kind == "create_agent_request" {
                response = "status".into();
            }
            json!({"workspace":workspace,"agent":host.state.agents[&agent_id],"status":"agent_created"})
        }
        "send_agent_message_request" => {
            check_input(request)?;
            turn(
                shared,
                &mut host,
                text(request, "agentId")?,
                text(request, "text")?,
                &request["messageId"],
                request,
            )?
        }
        // Asked once, the agent is told to stop and says when it has.
        // Asked again before it has, or with no session to tell, its
        // process goes and the turn is over as of now.
        "cancel_agent_request" => {
            let agent = text(request, "agentId")?;
            if host
                .state
                .agents
                .get(agent)
                .is_some_and(|record| record["status"] == "running")
            {
                let told = !host.stopping.contains(agent)
                    && host
                        .sessions
                        .get(agent)
                        .is_some_and(|commands| commands.send(Command::Interrupt).is_ok());
                if told {
                    host.stopping.insert(agent.to_owned());
                } else {
                    host.sessions.remove(agent);
                    host.finished(agent, Outcome::Canceled);
                }
            }
            json!({"agentId":agent})
        }
        _ => return Err(format!("Unsupported service request: {kind}")),
    };
    payload["requestId"] = request["requestId"].clone();
    Ok(envelope(&response, payload))
}
