use crate::{
    host::{Shared, create, envelope},
    state::id,
};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};

pub(crate) fn next(cadence: &Value, after: DateTime<Utc>) -> Result<String, String> {
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

async fn launch(shared: &Shared, schedule: &Value, key: &str) -> Result<Value, String> {
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
        "source":source,"agent":{"config":config,"initialPrompt":schedule["prompt"],"labels":{"sophon.schedule-id":schedule["id"]}}}),
    )
    .await?;
    Ok(reply["message"]["payload"].clone())
}

/// Starts a run of `schedule`: the one due at `due`, or one asked for
/// now. The run is on record, and the schedule's next time set, before
/// this returns; the agent starts after, which for a worktree takes as
/// long as its checkout does. A schedule still due would be started
/// again every second.
fn run(shared: &Shared, schedule: &Value, due: Option<&str>) -> Result<(), String> {
    let schedule_id = schedule["id"]
        .as_str()
        .ok_or("Missing schedule ID")?
        .to_owned();
    let now = Utc::now();
    let key = due
        .map(|date| format!("{schedule_id}/{date}"))
        .unwrap_or_else(id);
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
        host.state.runs.entry(schedule_id.clone()).or_default().push(
            json!({"id":key,"startedAt":now.to_rfc3339(),"scheduledFor":due,"status":"running"}),
        );
        host.state
            .save(&host.home)
            .map_err(|error| error.to_string())?;
    }
    let (shared, schedule) = (shared.clone(), schedule.clone());
    tokio::task::spawn_local(async move {
        let result = launch(&shared, &schedule, &key).await;
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
            if let Some(ended) = ended {
                record["status"] = json!(ended);
                record["endedAt"] = json!(Utc::now().to_rfc3339());
            }
        }
        if let Err(error) = host.state.save(&host.home) {
            eprintln!("Saving a scheduled run: {error}");
        }
    });
    Ok(())
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
    for schedule in ready {
        if let Err(error) = run(shared, &schedule, schedule["nextRunAt"].as_str()) {
            eprintln!("Scheduled run: {error}")
        }
    }
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
        run(shared, &schedule, None)?;
    }
    let mut host = shared.borrow_mut();
    let mut changed = false;
    let mut payload = match kind {
        "schedule/list" => json!({"schedules":host.state.schedules.values().collect::<Vec<_>>()}),
        "schedule/logs" => {
            json!({"runs":host.state.runs.get(key).into_iter().flatten().rev().collect::<Vec<_>>()})
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
            for field in ["name", "prompt", "cadence"] {
                if let Some(value) = request.get(field) {
                    schedule[field] = value.clone()
                }
            }
            if schedule["prompt"]
                .as_str()
                .is_none_or(|text| text.trim().is_empty())
            {
                return Err("A scheduled prompt is required".into());
            }
            let date = next(&schedule["cadence"], Utc::now())?;
            if schedule["status"] == "active" {
                schedule["nextRunAt"] = json!(date)
            }
            host.state.schedules.insert(key, schedule.clone());
            changed = true;
            json!({"schedule":schedule})
        }
        "schedule/delete" => {
            host.state.schedules.remove(key).ok_or("Unknown schedule")?;
            host.state.runs.remove(key);
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
                schedule["nextRunAt"] = json!(next(&schedule["cadence"], Utc::now())?);
                schedule["status"] = json!("active");
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
