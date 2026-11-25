//! Automations: a prompt that starts an agent when something happens.
//!
//! What happens is one of three things. A time comes (the cadence, as a
//! schedule always had). A probe finds something: a script of the
//! user's, run at each of those times, writes a line of JSON for each
//! thing there is (`{"id":"…","vars":{…}}`), and each one not seen
//! before starts a run, its `vars` filled into the prompt's `{{names}}`.
//! Or another program says so: `sophon trigger <automation>`.
//!
//! What a run does is one of two things. It starts an agent with the
//! automation's prompt: a conversation, for what takes judgment. Or it
//! runs the automation's command and keeps what that wrote: no agent,
//! nothing of a model spent, for what can be written down once and
//! done the same each time. Either may be kept to so many runs a day.
//!
//! A run can answer whoever caused it. Its agent has `sophon reply` and
//! `sophon react` on its `PATH`, and the token of its run in its
//! environment; the service hands what it says to the automation's reply
//! script, with the event it answers. The same script is told when a
//! run is picked up (`ack`), and how it ended (`done`, `failed`). The
//! service knows nothing of where events come from or answers go: that
//! is the two scripts'.

use crate::{
    host::{Host, Shared, create, envelope},
    state::id,
};
use chrono::{DateTime, Utc};
use serde_json::{Map, Value, json};
use std::{process::Stdio, time::Duration};

/// Event ids an automation remembers, the oldest forgotten first.
const SEEN: usize = 500;
/// Runs kept of an automation.
const KEPT: usize = 200;
/// How long a probe or a reply script may take.
const PATIENCE: Duration = Duration::from_secs(30);
/// A probe that failed this many times in a row is paused.
const FAILURES: u64 = 5;
/// An automation's events at work at once, unless it says otherwise.
const AT_ONCE: u64 = 2;
/// How long an automation's own command may take.
const WORK: Duration = Duration::from_secs(600);
/// How long a command may take when it is only being tried.
const TRIAL: Duration = Duration::from_secs(60);
/// As much of what a command wrote as a run keeps.
const WROTE: usize = 4000;

/// What `params` are told as for one automation: each `SOPHON_PARAM_`
/// by what `options` says for it, else what its manifest gives; and in
/// `said`, what each puts in a prompt.
fn chosen(
    params: &Value,
    options: &Value,
    told: &mut Map<String, Value>,
    said: &mut Map<String, Value>,
) {
    for param in params.as_array().into_iter().flatten() {
        let name = param["name"].as_str().unwrap_or_default();
        let value = options[name]
            .as_str()
            .or(param["value"].as_str())
            .unwrap_or_default();
        told.insert(
            format!("SOPHON_PARAM_{}", name.to_ascii_uppercase()),
            json!(value),
        );
        // The words its manifest gives for that value, else the value;
        // under the name it gives itself in a prompt, else its own.
        said.insert(
            param["var"].as_str().unwrap_or(name).to_owned(),
            param["prompt"][value]
                .as_str()
                .map_or_else(|| json!(value), |words| json!(words)),
        );
    }
}

/// `schedule` as it runs. What starts it is its integration's: the
/// trigger's probe, run in the integration's folder. What it does is
/// its plugin's, if it was made from one: how it answers, how many at
/// once, what its choices put in the prompt. Both are read now (what
/// was updated is what runs next). A view to run by, never what is kept.
fn plugged(home: &std::path::Path, schedule: &Value) -> Value {
    let mut view = schedule.clone();
    let env = |told: Vec<(String, String)>| -> Map<String, Value> {
        told.into_iter()
            .map(|(name, value)| (name, json!(value)))
            .collect()
    };
    // Its own scripts reach the service too: `sophon use`.
    view["probeEnv"] = env(crate::plugin::reach(home)).into();
    view["replyEnv"] = view["probeEnv"].clone();
    let mut said = Map::new();
    // Not there, or not to be read: said where a failing probe is, on
    // the automation, which rests after a few of these.
    let missing = |view: &mut Value, what: &str, id: &str| {
        view["probe"] = json!(format!(
            "echo 'The {what} {id} is not installed, or its manifest cannot be read' >&2; exit 1"
        ));
        view["reply"] = Value::Null;
    };
    // Turned off: nothing of it runs, and the automation says whose
    // doing that is.
    let off = |view: &mut Value, name: &Value| {
        view["off"] = name.clone();
        view["probe"] = Value::Null;
        view["reply"] = Value::Null;
    };
    if let Some(id) = set(schedule, "integration") {
        let found = crate::plugin::find(home, &id)
            .filter(|found| found["error"].is_null() && found["kind"] == "integration");
        match found {
            Some(integration) if integration["enabled"] == false => {
                off(&mut view, &integration["name"]);
                return view;
            }
            Some(integration) => {
                // The trigger it was made of, by its id; else the first.
                let wanted = schedule["trigger"].as_str();
                let trigger = integration["triggers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|trigger| wanted.is_none_or(|wanted| trigger["id"] == wanted));
                let Some(trigger) = trigger else {
                    missing(
                        &mut view,
                        "trigger",
                        &format!("{id}/{}", wanted.unwrap_or_default()),
                    );
                    return view;
                };
                view["probe"] = trigger["probe"].clone();
                view["reply"] = trigger["reply"].clone();
                let mut told = env(crate::plugin::environment(home, &integration));
                chosen(
                    &trigger["params"],
                    &schedule["triggerOptions"],
                    &mut told,
                    &mut said,
                );
                view["probeDir"] = integration["dir"].clone();
                view["probeEnv"] = told.into();
                view["replyDir"] = view["probeDir"].clone();
                view["replyEnv"] = view["probeEnv"].clone();
            }
            None => {
                missing(&mut view, "integration", &id);
                return view;
            }
        }
    }
    if let Some(id) = set(schedule, "plugin") {
        let found = crate::plugin::find(home, &id)
            .filter(|found| found["error"].is_null() && found["kind"] == "plugin");
        match found {
            Some(plugin) if plugin["enabled"] == false => {
                off(&mut view, &plugin["name"]);
                return view;
            }
            Some(plugin) => {
                let wanted = schedule["template"].as_str();
                let automation = plugin["automations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|automation| wanted.is_none_or(|wanted| automation["id"] == wanted));
                let Some(automation) = automation else {
                    missing(
                        &mut view,
                        "plugin's automation",
                        &format!("{id}/{}", wanted.unwrap_or_default()),
                    );
                    return view;
                };
                let mut told = env(crate::plugin::environment(home, &plugin));
                chosen(
                    &automation["params"],
                    &schedule["options"],
                    &mut told,
                    &mut said,
                );
                // Its own way of answering goes over the trigger's.
                if automation["reply"].is_string() {
                    view["reply"] = automation["reply"].clone();
                }
                view["replyDir"] = plugin["dir"].clone();
                view["replyEnv"] = told.into();
                view["skills"] = plugin["skills"].clone();
                if view["concurrency"].is_null() {
                    view["concurrency"] = automation["concurrency"].clone();
                }
            }
            None => {
                missing(&mut view, "plugin", &id);
                return view;
            }
        }
    }
    view["said"] = said.into();
    view
}

/// Which of an automation's scripts is to run: the one that looks for
/// events (its integration's), or one of what it does (its plugin's:
/// the answer, a command of its own).
#[derive(Clone, Copy)]
enum Part {
    Probe,
    Reply,
}

/// Where that script of an automation runs, and what it is told besides
/// its own: in the folder of whose it is, else in its project.
fn place(schedule: &Value, part: Part) -> (String, Vec<(String, String)>) {
    let (dir, told) = match part {
        Part::Probe => ("probeDir", "probeEnv"),
        Part::Reply => ("replyDir", "replyEnv"),
    };
    let project = schedule["target"]["config"]["cwd"]
        .as_str()
        .unwrap_or_default();
    let mut env = vec![
        (
            "SOPHON_AUTOMATION_ID".to_owned(),
            schedule["id"].as_str().unwrap_or_default().to_owned(),
        ),
        ("SOPHON_PROJECT_DIR".to_owned(), project.to_owned()),
    ];
    for (name, value) in schedule[told].as_object().into_iter().flatten() {
        env.push((name.clone(), value.as_str().unwrap_or_default().to_owned()));
    }
    (schedule[dir].as_str().unwrap_or(project).to_owned(), env)
}

fn set(schedule: &Value, key: &str) -> Option<String> {
    schedule[key]
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// Runs `command` with the shell where the automation works, and
/// answers what it wrote: an error with the end of what it complained
/// of when it failed, or took too long.
async fn script(
    command: String,
    cwd: String,
    env: Vec<(String, String)>,
    patience: Duration,
) -> Result<String, String> {
    let mut shell = tokio::process::Command::new("sh");
    shell
        .arg("-c")
        .arg(&command)
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if std::path::Path::new(&cwd).is_dir() {
        shell.current_dir(&cwd);
    }
    let output = tokio::time::timeout(patience, shell.output())
        .await
        .map_err(|_| format!("Took longer than {}s", patience.as_secs()))?
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        let said: Vec<_> = said.lines().rev().take(3).collect();
        let said: Vec<_> = said.into_iter().rev().collect();
        return Err(format!("{}: {}", output.status, said.join(" / "))
            .trim_end_matches([':', ' '])
            .to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// An event as it is kept: its id, and what it carries, each a text.
fn event(value: &Value) -> Result<Value, String> {
    let key = match &value["id"] {
        Value::String(text) if !text.trim().is_empty() => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return Err("An event needs an id".into()),
    };
    let mut vars = Map::new();
    for (name, value) in value["vars"].as_object().into_iter().flatten() {
        let text = match value {
            Value::String(text) => text.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        vars.insert(name.clone(), json!(text));
    }
    Ok(json!({"id":key,"vars":vars}))
}

/// The events a probe wrote, a line each. A line that is not one is the
/// probe's mistake, said rather than passed over: a probe that writes
/// something else would never start anything, and nobody would know.
fn events(output: &str) -> Result<Vec<Value>, String> {
    let mut list: Vec<Value> = Vec::new();
    for (number, line) in output.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let said = || line.chars().take(80).collect::<String>();
        let value: Value = serde_json::from_str(line)
            .map_err(|_| format!("Line {} is not JSON: {}", number + 1, said()))?;
        let found =
            event(&value).map_err(|error| format!("Line {}: {error}: {}", number + 1, said()))?;
        if !list.iter().any(|known| known["id"] == found["id"]) {
            list.push(found);
        }
    }
    Ok(list)
}

/// `prompt` with each `{{name}}` put in: what the event carries by
/// that name (`{{id}}` its id), else what the automation's choices say
/// (`said`). A name neither has stays as written.
fn fill(prompt: &str, event: &Value, said: &Value) -> String {
    let mut filled = String::with_capacity(prompt.len());
    let mut rest = prompt;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open..].find("}}") else {
            break;
        };
        let name = rest[open + 2..open + close].trim();
        let value = if name == "id" {
            event["id"].as_str()
        } else {
            event["vars"][name].as_str().or(said[name].as_str())
        };
        filled.push_str(&rest[..open]);
        match value {
            Some(value) => filled.push_str(value),
            None => filled.push_str(&rest[open..open + close + 2]),
        }
        rest = &rest[open + close + 2..];
    }
    filled.push_str(rest);
    filled
}

/// What a run's agent is told of answering, after its prompt.
const ANSWERING: &str = "\n\n---\nAn automation started this conversation. To answer whoever caused it, run `sophon reply \"<text>\"`; to react to their message, run `sophon react <NAME>`.";

/// What an agent is told of the skills its automation's plugin brings,
/// after its prompt: where each is, and when it is for. Nothing without any.
fn skills_note(skills: &Value) -> String {
    let lines: Vec<String> = skills
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|skill| {
            Some(format!(
                "- {}: {} ({})",
                skill["name"].as_str()?,
                skill["description"].as_str().unwrap_or("no description"),
                skill["file"].as_str()?
            ))
        })
        .collect();
    if lines.is_empty() {
        return String::new();
    }
    format!(
        "\n\n---\nThis automation's plugin brings skills. When one fits what you are doing, read its file first and follow it:\n{}",
        lines.join("\n")
    )
}

/// A script to run: its command, where, and what it is told.
type Call = (String, String, Vec<(String, String)>);

/// The reply script's call for `action` on `run`, when the automation
/// has one.
fn call(schedule: &Value, run: &Value, action: &str, text: &str) -> Option<Call> {
    let command = set(schedule, "reply")?;
    let (cwd, mut env) = place(schedule, Part::Reply);
    env.extend([
        ("SOPHON_ACTION".to_owned(), action.to_owned()),
        ("SOPHON_TEXT".to_owned(), text.to_owned()),
    ]);
    env.extend(told(run));
    Some((command, cwd, env))
}

/// What a script is told of the run it works for: which run, and the
/// event that caused it, each thing the event carries by its name.
fn told(run: &Value) -> Vec<(String, String)> {
    let mut env = vec![
        (
            "SOPHON_RUN_ID".to_owned(),
            run["id"].as_str().unwrap_or_default().to_owned(),
        ),
        (
            "SOPHON_EVENT_ID".to_owned(),
            run["event"]["id"].as_str().unwrap_or_default().to_owned(),
        ),
        (
            "SOPHON_EVENT".to_owned(),
            if run["event"].is_object() {
                run["event"].to_string()
            } else {
                "{}".to_owned()
            },
        ),
    ];
    for (name, value) in run["event"]["vars"].as_object().into_iter().flatten() {
        let name: String = name
            .chars()
            .map(|letter| {
                if letter.is_ascii_alphanumeric() {
                    letter.to_ascii_uppercase()
                } else {
                    '_'
                }
            })
            .collect();
        env.push((
            format!("SOPHON_VAR_{name}"),
            value.as_str().unwrap_or_default().to_owned(),
        ));
    }
    env
}

/// The runs `schedule_id` started on this computer's day so far.
fn today(host: &Host, schedule_id: &str) -> u64 {
    let day = chrono::Local::now().date_naive();
    host.state
        .runs
        .get(schedule_id)
        .into_iter()
        .flatten()
        .filter(|run| {
            run["startedAt"]
                .as_str()
                .and_then(|when| DateTime::parse_from_rfc3339(when).ok())
                .is_some_and(|when| when.with_timezone(&chrono::Local).date_naive() == day)
        })
        .count() as u64
}

/// How many more runs `schedule` may start today: none without a limit
/// of its own.
fn left_today(host: &Host, schedule: &Value) -> Option<u64> {
    let limit = schedule["dailyLimit"].as_u64().filter(|limit| *limit > 0)?;
    Some(limit.saturating_sub(today(host, schedule["id"].as_str().unwrap_or_default())))
}

/// Tells the reply script, without waiting on it: a step of the run's
/// own life, which nobody is there to hear the failure of but the log.
fn tell(schedule: &Value, run: &Value, action: &'static str, text: &str) {
    let Some((command, cwd, env)) = call(schedule, run, action, text) else {
        return;
    };
    tokio::task::spawn_local(async move {
        if let Err(error) = script(command, cwd, env, PATIENCE).await {
            eprintln!("An automation's reply script ({action}): {error}");
        }
    });
}

/// A run of an automation ended for the first time: its reply script
/// hears how.
pub(crate) fn ended(host: &Host, schedule: &str, run: &Value) {
    let Some(schedule) = host.state.schedules.get(schedule) else {
        return;
    };
    let schedule = &plugged(&host.home, schedule);
    if run["status"] == "failed" {
        tell(
            schedule,
            run,
            "failed",
            run["error"].as_str().unwrap_or_default(),
        );
    } else if run["status"] == "completed" {
        tell(
            schedule,
            run,
            "done",
            run["output"].as_str().unwrap_or_default(),
        );
    }
}

/// What the CLI of an automation's run has in its environment: the
/// token of its run, where the service is, and `sophon` on its `PATH`.
pub(crate) fn environment(host: &Host, agent: &Value) -> Vec<(String, String)> {
    let label = |name: &str| agent["labels"][name].as_str();
    let (Some(schedule), Some(run)) = (label("sophon.schedule-id"), label("sophon.run-id")) else {
        return Vec::new();
    };
    let Some(token) = host
        .state
        .runs
        .get(schedule)
        .and_then(|runs| runs.iter().rev().find(|entry| entry["id"] == run))
        .and_then(|entry| entry["token"].as_str())
    else {
        return Vec::new();
    };
    let path = std::env::var("PATH").unwrap_or_default();
    vec![
        ("SOPHON_RUN_TOKEN".to_owned(), token.to_owned()),
        (
            "SOPHON_SERVICE_HOME".to_owned(),
            host.home.display().to_string(),
        ),
        (
            "PATH".to_owned(),
            format!("{}:{path}", host.home.join("bin").display()),
        ),
    ]
}

/// What the CLI of an automation's run loads as plugins of its own: the
/// skills of the plugin its automation was made from, when that brings
/// any (and is there, and on).
pub(crate) fn brought(host: &Host, agent: &Value) -> Vec<String> {
    let Some(id) = agent["labels"]["sophon.schedule-id"]
        .as_str()
        .and_then(|schedule| host.state.schedules.get(schedule))
        .and_then(|schedule| set(schedule, "plugin"))
    else {
        return Vec::new();
    };
    crate::plugin::find(&host.home, &id)
        .filter(|plugin| {
            plugin["kind"] == "plugin" && plugin["error"].is_null() && plugin["enabled"] != false
        })
        .and_then(|plugin| crate::plugin::claude_plugin(&host.home, &plugin))
        .map(|folder| vec![folder.display().to_string()])
        .unwrap_or_default()
}

pub(crate) fn next(cadence: &Value, after: DateTime<Utc>) -> Result<String, String> {
    // To the second: a time on the minute is that minute, not a
    // fraction of a second into it.
    let after = DateTime::from_timestamp(after.timestamp(), 0).unwrap_or(after);
    let when = match cadence["type"].as_str() {
        Some("every") => {
            let milliseconds = cadence["everyMs"]
                .as_i64()
                .filter(|value| *value > 0)
                .ok_or("Expected a positive interval")?;
            after
                .checked_add_signed(chrono::Duration::milliseconds(milliseconds))
                .ok_or("Interval exceeds date range")?
        }
        Some("cron") => {
            let expression = cadence["expression"]
                .as_str()
                .ok_or("Missing cron expression")?;
            let schedule = expression
                .parse::<croner::Cron>()
                .map_err(|error| error.to_string())?;
            let timezone = cadence["timezone"]
                .as_str()
                .unwrap_or("UTC")
                .parse::<chrono_tz::Tz>()
                .map_err(|error| error.to_string())?;
            schedule
                .find_next_occurrence(&after.with_timezone(&timezone), false)
                .map_err(|error| error.to_string())?
                .with_timezone(&Utc)
        }
        _ => return Err("Unknown cadence".into()),
    };
    Ok(when.to_rfc3339())
}

async fn launch(
    shared: &Shared,
    schedule: &Value,
    key: &str,
    prompt: &str,
) -> Result<Value, String> {
    let config = &schedule["target"]["config"];
    let cwd = config["cwd"].as_str().ok_or("Missing working directory")?;
    let source = if config["isolation"] == "worktree" {
        json!({"kind":"worktree","cwd":cwd})
    } else {
        json!({"kind":"directory","path":cwd})
    };
    let reply = create(
        shared,
        &json!({"type":"workspace.create.request","idempotencyKey":key,
        "source":source,"agent":{"config":config,"initialPrompt":prompt,
            "labels":{"sophon.schedule-id":schedule["id"],"sophon.run-id":key}}}),
    )
    .await?;
    Ok(reply["message"]["payload"].clone())
}

/// Starts a run of `schedule`: the one due at `due`, one an `event`
/// causes, or one asked for now. The run is on record, and the
/// schedule's next time set, before this returns; the agent starts
/// after, which for a worktree takes as long as its checkout does. A
/// schedule still due would be started again every second.
fn run(
    shared: &Shared,
    schedule: &Value,
    due: Option<&str>,
    event: Option<Value>,
) -> Result<String, String> {
    let schedule_id = schedule["id"]
        .as_str()
        .ok_or("Missing schedule ID")?
        .to_owned();
    let now = Utc::now();
    let key = due
        .map(|date| format!("{schedule_id}/{date}"))
        .unwrap_or_else(id);
    let mut prompt = fill(
        schedule["prompt"].as_str().unwrap_or_default(),
        event.as_ref().unwrap_or(&Value::Null),
        &schedule["said"],
    );
    if set(schedule, "reply").is_some() {
        prompt.push_str(ANSWERING);
    }
    // The skills its plugin brings: Claude Code is started with them
    // (`brought`); any other agent is told where each is, to read.
    let claude = {
        let host = shared.borrow();
        let provider = schedule["target"]["config"]["provider"]
            .as_str()
            .unwrap_or_default();
        let settings = crate::config::read(&host.home)
            .map(|config| config["providers"][provider].clone())
            .unwrap_or_default();
        crate::provider::family(provider, &settings) == Some("claude")
    };
    if !claude {
        prompt.push_str(&skills_note(&schedule["skills"]));
    }
    let mut record = json!({"id":key,"token":id(),"startedAt":now.to_rfc3339(),"scheduledFor":due,"status":"running"});
    if let Some(event) = event {
        record["event"] = event;
    }
    {
        let mut host = shared.borrow_mut();
        let current = host
            .state
            .schedules
            .get_mut(&schedule_id)
            .ok_or("Unknown schedule")?;
        current["lastRunAt"] = json!(now.to_rfc3339());
        // A schedule with no time left to run at (a cron for one year,
        // that year gone) rests.
        if due.is_some() {
            match next(&current["cadence"], now) {
                Ok(when) => current["nextRunAt"] = json!(when),
                Err(_) => {
                    current["nextRunAt"] = Value::Null;
                    current["status"] = json!("paused");
                }
            }
        }
        let runs = host.state.runs.entry(schedule_id.clone()).or_default();
        runs.push(record.clone());
        if runs.len() > KEPT {
            let extra = runs.len() - KEPT;
            runs.drain(..extra);
        }
        host.state
            .save(&host.home)
            .map_err(|error| error.to_string())?;
    }
    tell(schedule, &record, "ack", "");
    // Its own command, and no agent: what it wrote is the run's output.
    if let Some(command) = set(schedule, "command") {
        let project = schedule["target"]["config"]["cwd"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let mut env = place(schedule, Part::Reply).1;
        env.extend(told(&record));
        let (shared, started) = (shared.clone(), key.clone());
        tokio::task::spawn_local(async move {
            let result = script(command, project, env, WORK).await;
            let mut host = shared.borrow_mut();
            let mut closed = None;
            if let Some(record) = host.state.runs.get_mut(&schedule_id).and_then(|runs| {
                runs.iter_mut()
                    .rev()
                    .find(|run| run["id"] == started.as_str())
            }) {
                match result {
                    Ok(wrote) => {
                        // The end of it: where a command says how it went.
                        let wrote = wrote.trim();
                        let from = wrote
                            .char_indices()
                            .rev()
                            .nth(WROTE)
                            .map_or(0, |(at, _)| at);
                        record["status"] = json!("completed");
                        record["output"] = json!(&wrote[from..]);
                    }
                    Err(error) => {
                        record["status"] = json!("failed");
                        record["error"] = json!(error);
                    }
                }
                record["endedAt"] = json!(Utc::now().to_rfc3339());
                closed = Some(record.clone());
            }
            if let Some(record) = closed {
                self::ended(&host, &schedule_id, &record);
            }
            if let Err(error) = host.state.save(&host.home) {
                eprintln!("Saving an automation's run: {error}");
            }
        });
        return Ok(key);
    }
    let (shared, schedule, started) = (shared.clone(), schedule.clone(), key.clone());
    tokio::task::spawn_local(async move {
        let key = started;
        let result = launch(&shared, &schedule, &key, &prompt).await;
        let mut host = shared.borrow_mut();
        // How its agent stands, if it has one: a turn that ended
        // before its agent was on record here is not heard of again.
        let ended = match &result {
            Ok(payload) => match host
                .state
                .agents
                .get(payload["agent"]["id"].as_str().unwrap_or_default())
            {
                Some(agent) if agent["status"] == "running" => None,
                Some(agent) if agent["status"] == "error" => Some("failed"),
                _ => Some("completed"),
            },
            Err(_) => Some("failed"),
        };
        let mut closed = None;
        if let Some(record) = host
            .state
            .runs
            .get_mut(&schedule_id)
            .and_then(|runs| runs.iter_mut().rev().find(|run| run["id"] == key.as_str()))
        {
            match &result {
                Ok(payload) => {
                    record["agentId"] = payload["agent"]["id"].clone();
                    record["workspaceId"] = payload["workspace"]["id"].clone();
                }
                Err(error) => {
                    eprintln!("Scheduled run: {error}");
                    record["error"] = json!(error);
                }
            }
            if let Some(ended) = ended
                && record["status"] == "running"
            {
                record["status"] = json!(ended);
                record["endedAt"] = json!(Utc::now().to_rfc3339());
                closed = Some(record.clone());
            }
        }
        if let Some(record) = closed {
            self::ended(&host, &schedule_id, &record);
        }
        if let Err(error) = host.state.save(&host.home) {
            eprintln!("Saving a scheduled run: {error}");
        }
    });
    Ok(key)
}

/// Whether `id` is among what `schedule_id` has seen; remembered if not.
fn seen(host: &mut Host, schedule_id: &str, id: &str) -> bool {
    let known = host.state.seen.entry(schedule_id.to_owned()).or_default();
    if known.iter().any(|entry| entry == id) {
        return true;
    }
    known.push(id.to_owned());
    if known.len() > SEEN {
        let extra = known.len() - SEEN;
        known.drain(..extra);
    }
    false
}

/// Runs the automation's probe, and starts a run for each event it has
/// not seen: as many as it may have at work at once, the rest left for
/// the next look (a probe says again what is still there). The first
/// look of an automation only notes what there is: what was there
/// before it was set up is not its business; it keeps those apart
/// (`skipped`, while the probe still finds them), and a look asked for
/// by hand (`asked`) starts them too.
fn look(shared: &Shared, schedule: &Value, asked: bool) {
    let (Some(schedule_id), Some(probe)) = (
        schedule["id"].as_str().map(str::to_owned),
        set(schedule, "probe"),
    ) else {
        return;
    };
    {
        let mut host = shared.borrow_mut();
        if !host.probing.insert(schedule_id.clone()) {
            return;
        }
        let now = Utc::now();
        if let Some(current) = host.state.schedules.get_mut(&schedule_id) {
            current["lastProbeAt"] = json!(now.to_rfc3339());
            if !asked {
                match next(&current["cadence"], now) {
                    Ok(when) => current["nextRunAt"] = json!(when),
                    Err(_) => {
                        current["nextRunAt"] = Value::Null;
                        current["status"] = json!("paused");
                    }
                }
            }
        }
        if let Err(error) = host.state.save(&host.home) {
            eprintln!("Saving an automation: {error}");
        }
    }
    let (cwd, env) = place(schedule, Part::Probe);
    let shared = shared.clone();
    tokio::task::spawn_local(async move {
        let found = script(probe, cwd, env, PATIENCE)
            .await
            .and_then(|output| events(&output));
        let mut start = Vec::new();
        let current = {
            let mut host = shared.borrow_mut();
            host.probing.remove(&schedule_id);
            let Some(current) = host.state.schedules.get(&schedule_id).cloned() else {
                return;
            };
            let mut current = current;
            match found {
                Err(error) => {
                    eprintln!("An automation's probe: {error}");
                    let failures = current["probeFailures"].as_u64().unwrap_or(0) + 1;
                    current["probeError"] = json!(error);
                    current["probeFailures"] = json!(failures);
                    if failures >= FAILURES {
                        current["status"] = json!("paused");
                        current["nextRunAt"] = Value::Null;
                    }
                }
                Ok(list) => {
                    current["probeError"] = Value::Null;
                    current["probeFailures"] = json!(0);
                    let primed = current["primed"] == true;
                    current["primed"] = json!(true);
                    // Those left aside when it was set up, while still there.
                    let found: Vec<&str> = list
                        .iter()
                        .filter_map(|event| event["id"].as_str())
                        .collect();
                    let mut skipped: Vec<String> = current["skipped"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .filter(|id| found.contains(id))
                        .map(str::to_owned)
                        .collect();
                    if !primed && !asked {
                        for event in &list {
                            let id = event["id"].as_str().unwrap_or_default();
                            if !seen(&mut host, &schedule_id, id) {
                                skipped.push(id.to_owned());
                            }
                        }
                    } else if asked || current["status"] == "active" {
                        let working = host
                            .state
                            .runs
                            .get(&schedule_id)
                            .into_iter()
                            .flatten()
                            .filter(|run| run["status"] == "running")
                            .count() as u64;
                        let room = current["concurrency"]
                            .as_u64()
                            .unwrap_or(AT_ONCE)
                            .max(1)
                            .saturating_sub(working)
                            // What the day has no room for waits for the
                            // next: a probe says again what is still there.
                            .min(left_today(&host, &current).unwrap_or(u64::MAX));
                        for event in list {
                            if start.len() as u64 >= room {
                                break;
                            }
                            let id = event["id"].as_str().unwrap_or_default().to_owned();
                            // Asked for by hand, those left aside start too.
                            let aside = asked && skipped.contains(&id);
                            if aside {
                                skipped.retain(|kept| *kept != id);
                            }
                            if !seen(&mut host, &schedule_id, &id) || aside {
                                start.push(event);
                            }
                        }
                    }
                    current["skipped"] = json!(skipped);
                }
            }
            host.state
                .schedules
                .insert(schedule_id.clone(), current.clone());
            if let Err(error) = host.state.save(&host.home) {
                eprintln!("Saving an automation: {error}");
            }
            current
        };
        if start.is_empty() {
            return;
        }
        let home = shared.borrow().home.clone();
        let current = plugged(&home, &current);
        for event in start {
            if let Err(error) = run(&shared, &current, None, Some(event)) {
                eprintln!("An automation's run: {error}");
            }
        }
    });
}

/// Automations kept before integrations and plugins were told apart
/// named one thing for both (`plugin`, with `pluginTrigger`). What that
/// was is an integration now: its trigger starts the automation, and
/// the plugin whose automation that trigger starts, if there is one,
/// is what it runs by. What it chose goes where each is asked for now,
/// and what was set for the old one that is the plugin's own follows
/// it there. Whether anything changed, to be kept.
pub fn adopt(
    home: &std::path::Path,
    schedules: &mut std::collections::BTreeMap<String, Value>,
) -> bool {
    let waiting = |schedule: &Value| {
        set(schedule, "plugin").is_some()
            && schedule["template"].is_null()
            && schedule["integration"].is_null()
    };
    if !schedules.values().any(waiting) {
        return false;
    }
    let packages = crate::plugin::list(home);
    let mut changed = false;
    for schedule in schedules.values_mut().filter(|schedule| waiting(schedule)) {
        let old = set(schedule, "plugin").unwrap_or_default();
        let Some(integration) = packages
            .iter()
            .find(|package| package["id"] == old.as_str() && package["kind"] == "integration")
        else {
            continue;
        };
        let wanted = set(schedule, "pluginTrigger");
        let Some(trigger) = integration["triggers"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|trigger| {
                wanted
                    .as_deref()
                    .is_none_or(|wanted| trigger["id"] == wanted)
            })
        else {
            continue;
        };
        let made_of = packages
            .iter()
            .filter(|package| package["kind"] == "plugin" && package["error"].is_null())
            .find_map(|plugin| {
                plugin["automations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|automation| {
                        automation["trigger"]["integration"] == old.as_str()
                            && automation["trigger"]["id"] == trigger["id"]
                    })
                    .map(|automation| (plugin, automation))
            });
        let (mut of_trigger, mut of_plugin) = (Map::new(), Map::new());
        for (name, value) in schedule["options"].as_object().into_iter().flatten() {
            let asked = trigger["params"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|param| param["name"] == name.as_str());
            if asked {
                &mut of_trigger
            } else {
                &mut of_plugin
            }
            .insert(name.clone(), value.clone());
        }
        schedule["integration"] = json!(old);
        schedule["trigger"] = trigger["id"].clone();
        schedule["triggerOptions"] = of_trigger.into();
        match made_of {
            Some((plugin, automation)) => {
                schedule["plugin"] = plugin["id"].clone();
                schedule["template"] = automation["id"].clone();
                schedule["options"] = of_plugin.into();
                crate::plugin::inherit(home, integration, plugin);
            }
            None => {
                schedule["plugin"] = Value::Null;
                schedule["options"] = Value::Null;
            }
        }
        if let Some(fields) = schedule.as_object_mut() {
            fields.remove("pluginTrigger");
        }
        changed = true;
    }
    changed
}

/// A time of `schedule` passed over: its next one is set, and `why`
/// (if it is anyone's to know) said where a probe's failure is.
fn pass(shared: &Shared, schedule: &Value, why: Option<String>) {
    let mut host = shared.borrow_mut();
    if let Some(current) = schedule["id"]
        .as_str()
        .and_then(|id| host.state.schedules.get_mut(id))
    {
        match next(&current["cadence"], Utc::now()) {
            Ok(when) => current["nextRunAt"] = json!(when),
            Err(_) => {
                current["nextRunAt"] = Value::Null;
                current["status"] = json!("paused");
            }
        }
        if let Some(why) = why {
            current["probeError"] = json!(why);
        }
    }
    if let Err(error) = host.state.save(&host.home) {
        eprintln!("Saving an automation: {error}");
    }
}

pub fn due(shared: &Shared) {
    let now = Utc::now();
    let ready: Vec<_> = shared
        .borrow()
        .state
        .schedules
        .values()
        .filter(|schedule| {
            schedule["status"] == "active"
                && schedule["nextRunAt"]
                    .as_str()
                    .and_then(|when| DateTime::parse_from_rfc3339(when).ok())
                    .is_some_and(|when| when <= now)
        })
        .cloned()
        .collect();
    let home = shared.borrow().home.clone();
    for schedule in ready {
        let schedule = plugged(&home, &schedule);
        // Its integration is turned off: this time is passed over, and why
        // is said on the automation.
        if let Some(name) = schedule["off"].as_str() {
            pass(shared, &schedule, Some(format!("{name} is turned off")));
            continue;
        }
        if set(&schedule, "probe").is_some() {
            look(shared, &schedule, false);
            continue;
        }
        // It has had its runs for the day: this time is passed over.
        if left_today(&shared.borrow(), &schedule) == Some(0) {
            pass(shared, &schedule, None);
            continue;
        }
        if let Err(error) = run(shared, &schedule, schedule["nextRunAt"].as_str(), None) {
            eprintln!("Scheduled run: {error}")
        }
    }
}

/// What a program outside asks of an automation: its run's agent to
/// answer whoever caused it (`automation/act`, by the run's token), or
/// anything at all to cause a run (`automation/trigger`). And what a
/// client asks before there is one: a command tried (`automation/try`).
pub async fn asked(shared: &Shared, request: &Value) -> Result<Value, String> {
    let kind = request["type"].as_str().unwrap_or_default();
    // A command tried before it is kept: run now, where an automation
    // would run it and told what one is told of the service. Nothing of
    // it is kept but what is said back: no run, no automation.
    if kind == "automation/try" {
        let command = request["command"]
            .as_str()
            .map(str::trim)
            .filter(|command| !command.is_empty())
            .ok_or("A command is required")?
            .to_owned();
        let project = request["cwd"].as_str().unwrap_or_default().to_owned();
        if !std::path::Path::new(&project).is_dir() {
            return Err("Pick the folder it runs in first".into());
        }
        let home = shared.borrow().home.clone();
        let mut env = crate::plugin::reach(&home);
        env.push(("SOPHON_PROJECT_DIR".to_owned(), project.clone()));
        let began = std::time::Instant::now();
        let (ok, wrote) = match script(command, project, env, TRIAL).await {
            Ok(wrote) => (true, wrote),
            Err(error) => (false, error),
        };
        // The end of it, as a run keeps: where a command says how it went.
        let wrote = wrote.trim();
        let from = wrote
            .char_indices()
            .rev()
            .nth(WROTE)
            .map_or(0, |(at, _)| at);
        return Ok(envelope(
            "automation/try/response",
            json!({"requestId":request["requestId"],"ok":ok,"output":&wrote[from..],
                "tookMs":began.elapsed().as_millis() as u64}),
        ));
    }
    let mut payload = match kind {
        "automation/act" => {
            let token = request["token"]
                .as_str()
                .filter(|token| !token.is_empty())
                .ok_or("Missing run token")?;
            let action = match request["action"].as_str() {
                Some(action @ ("reply" | "react")) => action,
                _ => return Err("Unknown action".into()),
            };
            let text = request["text"]
                .as_str()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .ok_or("Nothing to say")?;
            let call = {
                let mut host = shared.borrow_mut();
                let (schedule_id, run) = host
                    .state
                    .runs
                    .iter_mut()
                    .find_map(|(schedule, runs)| {
                        runs.iter_mut()
                            .rev()
                            .find(|run| run["token"] == token)
                            .map(|run| (schedule.clone(), run))
                    })
                    .ok_or("Unknown run")?;
                let said = json!({"action":action,"text":text,"at":Utc::now().to_rfc3339()});
                match run["replies"].as_array_mut() {
                    Some(replies) => replies.push(said),
                    None => run["replies"] = json!([said]),
                }
                let run = run.clone();
                host.state
                    .save(&host.home)
                    .map_err(|error| error.to_string())?;
                host.state
                    .schedules
                    .get(&schedule_id)
                    .and_then(|schedule| call(&plugged(&host.home, schedule), &run, action, text))
            };
            match call {
                Some((command, cwd, env)) => {
                    script(command, cwd, env, PATIENCE).await?;
                    json!({"delivered":true})
                }
                None => json!({"delivered":false}),
            }
        }
        "automation/trigger" => {
            let wanted = request["automation"]
                .as_str()
                .filter(|name| !name.is_empty())
                .ok_or("Missing automation")?;
            let mut host = shared.borrow_mut();
            let schedule = match host.state.schedules.get(wanted) {
                Some(schedule) => schedule.clone(),
                None => {
                    let mut named = host
                        .state
                        .schedules
                        .values()
                        .filter(|schedule| schedule["name"] == wanted);
                    match (named.next(), named.next()) {
                        (Some(schedule), None) => schedule.clone(),
                        (Some(_), Some(_)) => {
                            return Err(format!(
                                "More than one automation is named {wanted}: use its id"
                            ));
                        }
                        _ => return Err(format!("No automation is {wanted}")),
                    }
                }
            };
            if schedule["status"] != "active" {
                return Err("This automation is paused".into());
            }
            if left_today(&host, &schedule) == Some(0) {
                return Err("This automation has had its runs for today".into());
            }
            let schedule = plugged(&host.home, &schedule);
            if let Some(name) = schedule["off"].as_str() {
                return Err(format!("{name} is turned off"));
            }
            let mut given = request["event"].clone();
            if !given.is_object() {
                given = json!({});
            }
            if given["id"].is_null() {
                given["id"] = json!(id());
            }
            let event = event(&given)?;
            let schedule_id = schedule["id"].as_str().unwrap_or_default().to_owned();
            if seen(
                &mut host,
                &schedule_id,
                event["id"].as_str().unwrap_or_default(),
            ) {
                json!({"duplicate":true})
            } else {
                drop(host);
                json!({"runId":run(shared, &schedule, None, Some(event))?})
            }
        }
        _ => return Err("Unknown automation operation".into()),
    };
    payload["requestId"] = request["requestId"].clone();
    Ok(envelope(&format!("{kind}/response"), payload))
}

pub fn handle(shared: &Shared, request: &Value) -> Result<Value, String> {
    let kind = request["type"]
        .as_str()
        .ok_or("Missing schedule operation")?;
    let key = request["scheduleId"].as_str().unwrap_or_default();
    if kind == "schedule/run-once" {
        let schedule = shared
            .borrow()
            .state
            .schedules
            .get(key)
            .ok_or("Unknown schedule")?
            .clone();
        let home = shared.borrow().home.clone();
        let schedule = plugged(&home, &schedule);
        if let Some(name) = schedule["off"].as_str() {
            return Err(format!(
                "{name} is turned off: turn it on under Integrations"
            ));
        }
        // An automation with a probe looks now, and starts what it finds.
        if set(&schedule, "probe").is_some() {
            look(shared, &schedule, true);
        } else {
            run(shared, &schedule, None, None)?;
        }
    }
    let mut host = shared.borrow_mut();
    let mut changed = false;
    let mut payload = match kind {
        // Each with how many runs it started today, for the one that
        // is kept to so many.
        "schedule/list" => {
            let schedules: Vec<Value> = host
                .state
                .schedules
                .iter()
                .map(|(id, schedule)| {
                    let mut schedule = schedule.clone();
                    schedule["runsToday"] = json!(today(&host, id));
                    schedule
                })
                .collect();
            json!({"schedules":schedules})
        }
        "schedule/logs" => {
            // A run's token is its agent's alone.
            let runs: Vec<Value> = host
                .state
                .runs
                .get(key)
                .into_iter()
                .flatten()
                .rev()
                .map(|run| {
                    let mut run = run.clone();
                    if let Some(fields) = run.as_object_mut() {
                        fields.remove("token");
                    }
                    run
                })
                .collect();
            json!({"runs":runs})
        }
        "schedule/run-once" => json!({}),
        "schedule/create" | "schedule/update" => {
            let create = kind == "schedule/create";
            let key = if create { id() } else { key.to_owned() };
            let mut schedule = if create {
                json!({"id":key,"status":"active","target":request["target"]})
            } else {
                host.state
                    .schedules
                    .get(&key)
                    .ok_or("Unknown schedule")?
                    .clone()
            };
            if !schedule["target"].is_object() {
                return Err("Unknown schedule target".into());
            }
            if let Some(config) = request["newAgentConfig"].as_object() {
                if !schedule["target"]["config"].is_object() {
                    schedule["target"]["config"] = json!({});
                }
                for (key, value) in config {
                    schedule["target"]["config"][key] = value.clone()
                }
            }
            if schedule["target"]["type"] != "new-agent" {
                return Err("Unknown schedule target".into());
            }
            let probed = (
                set(&schedule, "integration"),
                set(&plugged(&host.home, &schedule), "probe"),
            );
            for field in [
                "name",
                "prompt",
                "cadence",
                "probe",
                "reply",
                "concurrency",
                // What starts it: an integration's trigger, and what it
                // says for that trigger's own choices.
                "integration",
                "trigger",
                "triggerOptions",
                // What it was made from: a plugin's automation, and what
                // it says for that one's choices.
                "plugin",
                "template",
                "options",
                "command",
                "dailyLimit",
            ] {
                if let Some(value) = request.get(field) {
                    schedule[field] = value.clone()
                }
            }
            // One made from a plugin begins with what the plugin's
            // automation suggests: its prompt, its name, the trigger
            // that starts it, and the mode its agents need to work
            // unattended.
            if let Some(id) = set(&schedule, "plugin") {
                let plugin = crate::plugin::find(&host.home, &id)
                    .filter(|plugin| plugin["kind"] == "plugin" || plugin["error"].is_string())
                    .ok_or_else(|| format!("The plugin {id} is not installed"))?;
                if let Some(error) = plugin["error"].as_str() {
                    return Err(format!("The plugin {id} cannot be read: {error}"));
                }
                let wanted = schedule["template"].as_str();
                let suggested = plugin["automations"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|automation| wanted.is_none_or(|wanted| automation["id"] == wanted))
                    .ok_or_else(|| {
                        format!(
                            "The plugin {id} has no automation {}",
                            wanted.unwrap_or("to make")
                        )
                    })?;
                // Kept by its id: which of the plugin's it is stays said
                // when the plugin lists another before it.
                schedule["template"] = suggested["id"].clone();
                if create {
                    if set(&schedule, "prompt").is_none() {
                        schedule["prompt"] = suggested["prompt"].clone();
                    }
                    if set(&schedule, "name").is_none() {
                        schedule["name"] = suggested["name"].clone();
                    }
                    if set(&schedule, "integration").is_none() && suggested["trigger"].is_object() {
                        schedule["integration"] = suggested["trigger"]["integration"].clone();
                        schedule["trigger"] = suggested["trigger"]["id"].clone();
                    }
                    if schedule["target"]["config"]["modeId"].is_null()
                        && suggested["modeId"].is_string()
                    {
                        schedule["target"]["config"]["modeId"] = suggested["modeId"].clone();
                    }
                }
            }
            // What starts it is there to start it; how often its trigger
            // looks is the integration's to suggest, unless the form said.
            if let Some(id) = set(&schedule, "integration") {
                let integration = crate::plugin::find(&host.home, &id)
                    .filter(|found| found["kind"] == "integration" || found["error"].is_string())
                    .ok_or_else(|| format!("The integration {id} is not installed"))?;
                if let Some(error) = integration["error"].as_str() {
                    return Err(format!("The integration {id} cannot be read: {error}"));
                }
                let wanted = schedule["trigger"].as_str();
                let trigger = integration["triggers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|trigger| wanted.is_none_or(|wanted| trigger["id"] == wanted))
                    .ok_or_else(|| {
                        format!(
                            "The integration {id} has no trigger {}",
                            wanted.unwrap_or("to start on")
                        )
                    })?;
                schedule["trigger"] = trigger["id"].clone();
                if create
                    && schedule["cadence"].is_null()
                    && let Some(every) = trigger["everyMs"].as_u64()
                {
                    schedule["cadence"] = json!({"type":"every","everyMs":every});
                }
            }
            // What it does: a prompt for an agent, or a command of its own.
            if set(&schedule, "prompt").is_none() && set(&schedule, "command").is_none() {
                return Err("A prompt or a command is required".into());
            }
            let view = plugged(&host.home, &schedule);
            // Another probe is another source: what the last one had
            // seen says nothing of it.
            if (set(&schedule, "integration"), set(&view, "probe")) != probed {
                schedule["primed"] = json!(false);
                schedule["probeError"] = Value::Null;
                schedule["probeFailures"] = json!(0);
                host.state.seen.remove(&key);
            }
            // Without a cadence it waits to be told (`sophon trigger`);
            // a probe has to be run at some time.
            let date = if schedule["cadence"].is_null() {
                if set(&view, "probe").is_some() {
                    return Err("A probe needs to be told how often to look".into());
                }
                Value::Null
            } else {
                json!(next(&schedule["cadence"], Utc::now())?)
            };
            if schedule["status"] == "active" {
                schedule["nextRunAt"] = date
            }
            host.state.schedules.insert(key, schedule.clone());
            changed = true;
            json!({"schedule":schedule})
        }
        "schedule/delete" => {
            host.state.schedules.remove(key).ok_or("Unknown schedule")?;
            host.state.runs.remove(key);
            host.state.seen.remove(key);
            changed = true;
            json!({})
        }
        "schedule/pause" | "schedule/resume" => {
            let schedule = host
                .state
                .schedules
                .get_mut(key)
                .ok_or("Unknown schedule")?;
            if kind == "schedule/pause" {
                schedule["status"] = json!("paused");
                schedule["nextRunAt"] = Value::Null;
            } else {
                schedule["nextRunAt"] = if schedule["cadence"].is_null() {
                    Value::Null
                } else {
                    json!(next(&schedule["cadence"], Utc::now())?)
                };
                schedule["status"] = json!("active");
                // What came while it rested is not picked up now.
                schedule["primed"] = json!(false);
                schedule["probeError"] = Value::Null;
                schedule["probeFailures"] = json!(0);
            }
            changed = true;
            json!({"schedule":schedule})
        }
        _ => return Err("Unknown schedule operation".into()),
    };
    if changed {
        host.state
            .save(&host.home)
            .map_err(|error| error.to_string())?;
    }
    payload["requestId"] = request["requestId"].clone();
    Ok(envelope(&format!("{kind}/response"), payload))
}

#[cfg(test)]
mod tests {
    use super::{adopt, events, fill, plugged, skills_note};
    use serde_json::{Value, json};
    use std::fs;

    #[test]
    fn a_probe_s_lines_are_events_and_anything_else_is_said() {
        let list = events("{\"id\":\"a\",\"vars\":{\"url\":\"https://x/1\",\"n\":3}}\n\n{\"id\":7}\n{\"id\":\"a\"}\n").unwrap();
        assert_eq!(
            list,
            vec![
                json!({"id":"a","vars":{"url":"https://x/1","n":"3"}}),
                json!({"id":"7","vars":{}})
            ]
        );
        assert!(
            events("checking…\n")
                .unwrap_err()
                .starts_with("Line 1 is not JSON")
        );
        assert!(
            events("{\"vars\":{}}\n")
                .unwrap_err()
                .contains("needs an id")
        );
    }

    #[test]
    fn a_prompt_takes_what_its_event_carries() {
        let event = json!({"id":"om_1","vars":{"url":"https://x/1","who":"Ada"}});
        assert_eq!(
            fill(
                "Review {{url}} for {{ who }} ({{id}}), not {{other}} {{",
                &event,
                &Value::Null
            ),
            "Review https://x/1 for Ada (om_1), not {{other}} {{"
        );
        // And what the automation's choices say, where the event says nothing.
        let said = json!({"rule":"Only comment.","url":"never this"});
        assert_eq!(
            fill("{{url}}: {{rule}}", &event, &said),
            "https://x/1: Only comment."
        );
        assert_eq!(
            fill("{{rule}} {{url}}", &Value::Null, &Value::Null),
            "{{rule}} {{url}}"
        );
    }

    /// A home with an integration (`hub`, one trigger with a choice of
    /// its own) and a plugin (`watch`) whose automation that trigger starts.
    fn home() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        // `words` as the file `inside` of a package, the folders on the way made.
        let put = |package: &str, inside: &str, words: &str| {
            let file = home.path().join("plugins").join(package).join(inside);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, words).unwrap();
        };
        put(
            "hub",
            crate::plugin::INTEGRATION,
            r#"{"id":"hub","name":"Hub","params":[{"name":"token","value":"t0"}],
                "triggers":[{"id":"asked","probe":"./probe","reply":"./thank","everyMs":60000,
                    "params":[{"name":"repos","value":"all"}]}]}"#,
        );
        put(
            "watch",
            crate::plugin::PLUGIN,
            r#"{"id":"watch","name":"Watch","params":[{"name":"people","optional":true}]}"#,
        );
        put(
            "watch",
            &format!("automations/review/{}", crate::plugin::AUTOMATION),
            r#"{"trigger":"hub/asked","prompt":"Review {{url}}. {{rule}}","reply":"./tell","concurrency":3,
                "params":[{"name":"approve","var":"rule","type":"switch","value":false,"prompt":{"yes":"Approve it.","no":"Only comment."}}]}"#,
        );
        home
    }

    #[test]
    fn what_starts_a_run_is_the_integration_s_and_what_it_does_the_plugin_s() {
        let home = home();
        let folder = |id: &str| {
            fs::canonicalize(home.path().join("plugins").join(id))
                .unwrap()
                .display()
                .to_string()
        };
        // A trigger alone: the automation is the user's own, and the
        // integration answers for it.
        let alone = plugged(
            home.path(),
            &json!({"id":"a","integration":"hub","trigger":"asked","triggerOptions":{"repos":"acme/*"}}),
        );
        assert_eq!(alone["probe"], "./probe");
        assert_eq!(alone["reply"], "./thank");
        assert_eq!(alone["probeDir"], folder("hub"));
        assert_eq!(alone["replyDir"], folder("hub"));
        assert_eq!(alone["probeEnv"]["SOPHON_PARAM_REPOS"], "acme/*");
        assert_eq!(alone["probeEnv"]["SOPHON_PARAM_TOKEN"], "t0");
        assert!(alone["concurrency"].is_null());
        // Made from the plugin: it answers its own way, in its own folder,
        // told its own settings and never the integration's.
        let made = plugged(
            home.path(),
            &json!({"id":"b","integration":"hub","trigger":"asked","plugin":"watch","template":"review","options":{"approve":"yes"}}),
        );
        assert_eq!(made["probe"], "./probe");
        assert_eq!(made["probeDir"], folder("hub"));
        assert_eq!(made["probeEnv"]["SOPHON_PARAM_REPOS"], "all");
        assert_eq!(made["reply"], "./tell");
        assert_eq!(made["replyDir"], folder("watch"));
        assert_eq!(made["replyEnv"]["SOPHON_PARAM_APPROVE"], "yes");
        assert!(made["replyEnv"]["SOPHON_PARAM_TOKEN"].is_null());
        assert!(made["probeEnv"]["SOPHON_PARAM_APPROVE"].is_null());
        assert_eq!(made["concurrency"], 3);
        assert_eq!(
            made["said"]["rule"], "Approve it.",
            "under the name it gives itself in a prompt"
        );
        assert!(made["said"]["approve"].is_null());
        // Its skills come with it; an agent that has no way of its own to
        // load them is told where each is.
        assert_eq!(made["skills"], json!([]));
        assert_eq!(skills_note(&made["skills"]), "");
        let note = skills_note(
            &json!([{"id":"pair","name":"pair","description":"Pairs two accounts","file":"/p/skills/pair/SKILL.md"}]),
        );
        assert!(
            note.contains("- pair: Pairs two accounts (/p/skills/pair/SKILL.md)"),
            "{note}"
        );
        assert_eq!(made["said"]["repos"], "all");
        // Either turned off, and nothing of the automation runs.
        fs::write(home.path().join("plugins.json"), r#"{"off":["watch"]}"#).unwrap();
        let resting = plugged(
            home.path(),
            &json!({"id":"b","integration":"hub","trigger":"asked","plugin":"watch","template":"review"}),
        );
        assert_eq!(resting["off"], "Watch");
        assert!(resting["probe"].is_null() && resting["reply"].is_null());
        // One that is gone is said where a failing probe is.
        let gone = plugged(home.path(), &json!({"id":"c","integration":"nowhere"}));
        assert!(
            gone["probe"]
                .as_str()
                .unwrap()
                .contains("nowhere is not installed")
        );
        let lost = plugged(
            home.path(),
            &json!({"id":"d","integration":"hub","trigger":"other"}),
        );
        assert!(
            lost["probe"]
                .as_str()
                .unwrap()
                .contains("hub/other is not installed")
        );
    }

    #[test]
    fn an_automation_kept_before_the_two_were_told_apart_is_carried_over() {
        let home = home();
        // What was set for the old one, some of it the plugin's own now.
        let kept = home.path().join("plugin-data/hub");
        fs::create_dir_all(&kept).unwrap();
        fs::write(
            kept.join("params.json"),
            r#"{"token":"t1","people":"ada=ada@x.io"}"#,
        )
        .unwrap();
        let mut schedules = std::collections::BTreeMap::new();
        schedules.insert(
            "old".to_owned(),
            json!({"id":"old","plugin":"hub","pluginTrigger":"asked","options":{"repos":"acme/*","approve":"yes"}}),
        );
        schedules.insert("timed".to_owned(), json!({"id":"timed","prompt":"Hello"}));
        schedules.insert(
            "new".to_owned(),
            json!({"id":"new","integration":"hub","trigger":"asked","plugin":"watch","template":"review"}),
        );
        let before = schedules["new"].clone();
        assert!(adopt(home.path(), &mut schedules));
        let old = &schedules["old"];
        assert_eq!(old["integration"], "hub");
        assert_eq!(old["trigger"], "asked");
        assert_eq!(old["triggerOptions"], json!({"repos":"acme/*"}));
        assert_eq!(old["plugin"], "watch");
        assert_eq!(old["template"], "review");
        assert_eq!(old["options"], json!({"approve":"yes"}));
        assert!(old.get("pluginTrigger").is_none());
        assert_eq!(schedules["new"], before);
        let followed: Value = serde_json::from_slice(
            &fs::read(home.path().join("plugin-data/watch/params.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            followed,
            json!({"people":"ada=ada@x.io"}),
            "only what is the plugin's own"
        );
        // Once: there is nothing left to carry over.
        assert!(!adopt(home.path(), &mut schedules));
    }
}
