//! A conversation's earlier messages, read from where they are kept.
//!
//! Neither this service's predecessor nor an agent's CLI hands a
//! conversation over whole: the CLI keeps each as a record of its own
//! (Claude Code's `<config>/projects/<folder>/<session>.jsonl`, Codex's
//! `<home>/sessions/<date>/rollout-…-<thread>.jsonl`), and a
//! conversation brought over from the old service arrives as its id
//! and nothing said in it. So the first time such a conversation is
//! opened, its record is read into its timeline.
//!
//! Only by the session id a conversation here has: nothing is looked
//! for, or listed, that this service was not already given.

use crate::provider::claude::{answered, detail, said};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

fn tool(call: &str, name: &str, status: &str, detail: Value, error: Option<&str>) -> Value {
    json!({"type":"tool_call","callId":call,"name":name,"status":status,"detail":detail,"error":error})
}

/// Items put one after another, as a timeline takes them: text after
/// text starts a new paragraph, since a timeline joins them.
#[derive(Default)]
struct Items {
    list: Vec<Value>,
}

impl Items {
    fn push(&mut self, item: Value) {
        self.list.push(item);
    }

    fn text(&mut self, kind: &str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        let follows = self.list.last().is_some_and(|last| last["type"] == kind);
        let text = if follows {
            format!("\n\n{text}")
        } else {
            text.to_owned()
        };
        self.list.push(json!({"type":kind,"text":text}));
    }
}

/// What a person typed, of a message Claude Code recorded as theirs:
/// not what it wrote there for the model (a command's output, a
/// reminder, a notice of a task).
fn typed(text: &str) -> Option<&str> {
    let text = text.trim();
    (!text.is_empty() && !text.starts_with('<') && !text.starts_with("[Request interrupted"))
        .then_some(text)
}

/// Claude Code's record of a session, as timeline items.
pub fn claude(record: &str) -> Vec<Value> {
    let mut items = Items::default();
    let mut calls: HashMap<String, (String, Value)> = HashMap::new();
    for line in record.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        // An agent it started, and what Claude Code noted for itself.
        if entry["isSidechain"] == true || entry["isMeta"] == true {
            continue;
        }
        let content = &entry["message"]["content"];
        match entry["type"].as_str() {
            Some("user") => {
                if let Some(text) = content.as_str().and_then(typed) {
                    items
                        .push(json!({"type":"user_message","text":text,"messageId":entry["uuid"],"at":entry["timestamp"]}));
                }
                let mut words = Vec::new();
                for block in content.as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => words.extend(block["text"].as_str().and_then(typed)),
                        Some("tool_result") => {
                            let id = block["tool_use_id"].as_str().unwrap_or_default();
                            let Some((name, input)) = calls.remove(id) else {
                                continue;
                            };
                            let text = said(&block["content"]);
                            let failed = block["is_error"] == true;
                            let detail = answered(
                                detail(&name, &input),
                                &text,
                                &entry["toolUseResult"],
                                failed,
                            );
                            let status = if failed { "failed" } else { "completed" };
                            items.push(tool(
                                id,
                                &name,
                                status,
                                detail,
                                failed.then_some(text.as_str()),
                            ));
                        }
                        _ => {}
                    }
                }
                if !words.is_empty() {
                    items.push(json!({"type":"user_message","text":words.join("\n\n"),"messageId":entry["uuid"],"at":entry["timestamp"]}));
                }
            }
            Some("assistant") if entry["message"]["model"] != "<synthetic>" => {
                for block in content.as_array().into_iter().flatten() {
                    match block["type"].as_str() {
                        Some("text") => items.text(
                            "assistant_message",
                            block["text"].as_str().unwrap_or_default(),
                        ),
                        Some("thinking") => {
                            items.text("reasoning", block["thinking"].as_str().unwrap_or_default())
                        }
                        Some("tool_use") => {
                            let (Some(id), Some(name)) =
                                (block["id"].as_str(), block["name"].as_str())
                            else {
                                continue;
                            };
                            calls.insert(id.to_owned(), (name.to_owned(), block["input"].clone()));
                            items.push(tool(
                                id,
                                name,
                                "running",
                                detail(name, &block["input"]),
                                None,
                            ));
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    items.list
}

/// Codex's record of a thread, as timeline items: what it noted as each
/// item of a turn was done.
pub fn codex(record: &str) -> Vec<Value> {
    let mut items = Items::default();
    for line in record.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if entry["type"] != "event_msg" || entry["payload"]["type"] != "item_completed" {
            continue;
        }
        let item = &entry["payload"]["item"];
        let id = item["id"].as_str().unwrap_or_default();
        let words = |list: &Value| -> String {
            list.as_array()
                .into_iter()
                .flatten()
                .filter_map(|part| part["text"].as_str().or(part.as_str()))
                .collect::<Vec<_>>()
                .join("\n\n")
        };
        let status = if item["status"]
            .as_str()
            .is_some_and(|status| status.to_lowercase().contains("fail"))
        {
            "failed"
        } else {
            "completed"
        };
        match item["type"].as_str() {
            Some("UserMessage") => {
                let text = words(&item["content"]);
                if !text.trim().is_empty() {
                    items.push(json!({"type":"user_message","text":text,"messageId":id}));
                }
            }
            Some("AgentMessage") => items.text("assistant_message", &words(&item["content"])),
            Some("Reasoning") => items.text("reasoning", &words(&item["summary_text"])),
            Some("CommandExecution") => {
                let command = match &item["command"] {
                    Value::Array(parts) => parts
                        .iter()
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(" "),
                    other => other.as_str().unwrap_or_default().to_owned(),
                };
                items.push(tool(
                    id,
                    "shell",
                    status,
                    json!({"type":"shell","command":command,"cwd":item["cwd"],
                        "output":item["aggregated_output"],"exitCode":item["exit_code"]}),
                    None,
                ));
            }
            Some("McpToolCall") => items.push(tool(
                id,
                &format!(
                    "{}.{}",
                    item["server"].as_str().unwrap_or_default(),
                    item["tool"].as_str().unwrap_or_default()
                ),
                status,
                json!({"type":"unknown","input":item["arguments"],"output":item["result"]}),
                None,
            )),
            _ => {}
        }
    }
    items.list
}

/// The file named `name` (or whose name ends with it) at most `depth`
/// folders under `dir`.
fn find(dir: &Path, depth: usize, named: &dyn Fn(&str) -> bool) -> Option<PathBuf> {
    for entry in fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_file() && entry.file_name().to_str().is_some_and(named) {
            return Some(path);
        }
        if kind.is_dir()
            && depth > 0
            && let Some(found) = find(&path, depth - 1, named)
        {
            return Some(found);
        }
    }
    None
}

/// Claude Code's record of `session`, where it is.
fn claude_record(settings: &Value, session: &str) -> Option<PathBuf> {
    if session.is_empty()
        || !session
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return None;
    }
    let name = format!("{session}.jsonl");
    find(
        &crate::models::kept(settings, "CLAUDE_CONFIG_DIR", ".claude").join("projects"),
        1,
        &|found| found == name,
    )
}

/// Makes a session that is `session` up to, and not with, the message
/// `message` (by the id Claude Code recorded it under), and answers its
/// id: a record beside the old one, the same lines under the new id.
/// The old record is not changed.
/// `None` when nothing would be left of the conversation (the message is
/// its first): Claude Code resumes no session with no messages in it, so
/// the conversation begins a session anew.
pub fn cut(settings: &Value, session: &str, message: &str) -> Result<Option<String>, String> {
    let path =
        claude_record(settings, session).ok_or("Claude Code has no record of this conversation")?;
    let record = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let fresh = uuid::Uuid::new_v4().to_string();
    let mut kept = String::new();
    let mut found = false;
    let mut said = false;
    for line in record.lines() {
        let entry = serde_json::from_str::<Value>(line).unwrap_or(Value::Null);
        if entry["uuid"] == message {
            found = true;
            break;
        }
        said |= matches!(entry["type"].as_str(), Some("user" | "assistant"));
        kept.push_str(&line.replace(session, &fresh));
        kept.push('\n');
    }
    if !found {
        return Err("Claude Code's record does not have that message".into());
    }
    if !said {
        return Ok(None);
    }
    fs::write(path.with_file_name(format!("{fresh}.jsonl")), kept)
        .map_err(|error| error.to_string())?;
    Ok(Some(fresh))
}

/// What was said in `session` of a provider of `family`, by its CLI's
/// record: nothing if there is no such record here.
pub fn read(family: &str, settings: &Value, session: &str) -> Vec<Value> {
    // An id, as the CLIs make them: not a path to anywhere.
    if session.is_empty()
        || !session
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Vec::new();
    }
    let record = if family == "codex" {
        let tail = format!("-{session}.jsonl");
        find(
            &crate::models::kept(settings, "CODEX_HOME", ".codex").join("sessions"),
            3,
            &|name| name.starts_with("rollout-") && name.ends_with(&tail),
        )
    } else {
        let name = format!("{session}.jsonl");
        find(
            &crate::models::kept(settings, "CLAUDE_CONFIG_DIR", ".claude").join("projects"),
            1,
            &|found| found == name,
        )
    };
    let Some(text) = record.and_then(|path| fs::read_to_string(path).ok()) else {
        return Vec::new();
    };
    if family == "codex" {
        codex(&text)
    } else {
        claude(&text)
    }
}

/// Where a folder's repository is kept, from its `origin` (`git@github.com:
/// me/app.git`, `https://github.com/me/app`): its host and path
/// (`github.com/me/app`), the same for every clone and worktree of it, and
/// its name (`me/app`).
pub fn remote_key(url: &str) -> Option<(String, String)> {
    let url = url.trim().trim_end_matches('/').trim_end_matches(".git");
    let rest = match url.split_once("://") {
        Some((_, rest)) => rest.split_once('@').map_or(rest, |(_, after)| after).to_owned(),
        // scp-like: `git@host:path`.
        None => {
            let (host, path) = url.split_once(':')?;
            let host = host.split_once('@').map_or(host, |(_, after)| after);
            format!("{host}/{path}")
        }
    };
    let mut parts = rest.splitn(2, '/');
    let host = parts.next()?.split(':').next()?.to_lowercase();
    let path = parts.next()?.trim_matches('/');
    if host.is_empty() || path.is_empty() {
        return None;
    }
    let name: Vec<&str> = path.rsplitn(3, '/').take(2).collect();
    let repository = name.into_iter().rev().collect::<Vec<_>>().join("/");
    Some((format!("{host}/{path}"), repository))
}

/// The folders Claude Code's and Codex's sessions ran in, as projects to
/// bring in with their sessions (as T3 Code brings them in): each
/// folder still there, its sessions newest first, how many, when last,
/// from which agents, and its repository (`origin`'s, the same for every
/// clone and worktree). `sessions` are what [`recent`] lists; the newest
/// folder first.
pub fn projects(sessions: Vec<Value>) -> Vec<Value> {
    let mut folders: Vec<(String, Vec<Value>)> = Vec::new();
    for session in sessions {
        let Some(cwd) = session["cwd"].as_str().map(str::to_owned) else { continue };
        match folders.iter_mut().find(|(path, _)| *path == cwd) {
            Some((_, of)) => of.push(session),
            None => folders.push((cwd, vec![session])),
        }
    }
    let mut projects: Vec<Value> = folders
        .into_iter()
        .filter(|(path, _)| Path::new(path).is_dir())
        .map(|(path, mut of)| {
            of.sort_by(|a, b| b["lastActivityAt"].as_str().cmp(&a["lastActivityAt"].as_str()));
            let mut sources: Vec<&str> = Vec::new();
            for session in &of {
                if let Some(provider) = session["providerId"].as_str()
                    && !sources.contains(&provider)
                {
                    sources.push(provider);
                }
            }
            let sources: Vec<String> = sources.into_iter().map(str::to_owned).collect();
            let origin = std::process::Command::new("git")
                .args(["-C", &path, "config", "--get", "remote.origin.url"])
                .stdin(std::process::Stdio::null())
                .output()
                .ok()
                .filter(|out| out.status.success())
                .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned());
            let git = std::process::Command::new("git")
                .args(["-C", &path, "rev-parse", "--is-inside-work-tree"])
                .stdin(std::process::Stdio::null())
                .output()
                .is_ok_and(|out| out.status.success());
            let (remote, repository) = origin.as_deref().and_then(remote_key).unzip();
            let title = path.trim_end_matches('/').rsplit('/').next().unwrap_or(&path).to_owned();
            json!({"path":path,"title":title,"git":git,"remoteKey":remote,"repository":repository,
                "threads":of.len(),"lastActivityAt":of[0]["lastActivityAt"],"sources":sources,"sessions":of})
        })
        .collect();
    projects.sort_by(|a, b| b["lastActivityAt"].as_str().cmp(&a["lastActivityAt"].as_str()));
    projects
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_repository_is_known_by_its_origin_however_written() {
        for url in [
            "git@github.com:me/app.git",
            "https://github.com/me/app",
            "https://user@github.com/me/app.git/",
            "ssh://git@github.com:22/me/app.git",
        ] {
            assert_eq!(remote_key(url), Some(("github.com/me/app".into(), "me/app".into())), "{url}");
        }
        assert_eq!(
            remote_key("https://gitlab.example.com/group/sub/app.git"),
            Some(("gitlab.example.com/group/sub/app".into(), "sub/app".into()))
        );
        assert_eq!(remote_key("not a url"), None);
    }

    #[test]
    fn sessions_become_the_projects_of_their_folders() {
        let here = tempfile::tempdir().unwrap();
        let (a, b) = (here.path().join("a"), here.path().join("b"));
        std::fs::create_dir_all(&a).unwrap();
        std::fs::create_dir_all(&b).unwrap();
        let session = |cwd: &Path, id: &str, at: &str, provider: &str| {
            json!({"providerId":provider,"providerHandleId":id,"cwd":cwd,"lastActivityAt":at})
        };
        let gone = here.path().join("gone");
        let found = projects(vec![
            session(&a, "1", "2026-11-01T00:00:00Z", "claude"),
            session(&b, "2", "2026-11-20T00:00:00Z", "codex"),
            session(&a, "3", "2026-11-10T00:00:00Z", "codex"),
            session(&gone, "4", "2026-11-25T00:00:00Z", "claude"),
        ]);
        let paths: Vec<_> = found.iter().map(|p| p["path"].clone()).collect();
        assert_eq!(paths, [json!(b), json!(a)], "newest first, a folder no longer there left out");
        assert_eq!(found[1]["threads"], 2);
        assert_eq!(found[1]["lastActivityAt"], "2026-11-10T00:00:00Z");
        assert_eq!(found[1]["sessions"][0]["providerHandleId"], "3");
        assert_eq!(found[1]["sources"], json!(["codex", "claude"]));
        assert_eq!(found[1]["git"], false);
    }

    use super::*;

    #[test]
    fn a_cut_before_the_first_message_leaves_no_session_to_resume() {
        let config = tempfile::tempdir().unwrap();
        let folder = config.path().join("projects/-w");
        fs::create_dir_all(&folder).unwrap();
        let lines = [
            json!({"type":"queue-operation","sessionId":"s1"}),
            json!({"type":"user","uuid":"u1","sessionId":"s1","message":{"content":"hi"}}),
            json!({"type":"assistant","uuid":"a1","sessionId":"s1","message":{"content":[{"type":"text","text":"hello"}]}}),
            json!({"type":"user","uuid":"u2","sessionId":"s1","message":{"content":"more"}}),
        ];
        let record: String = lines.iter().map(|l| format!("{l}\n")).collect();
        fs::write(folder.join("s1.jsonl"), record).unwrap();
        let settings = json!({"env":{"CLAUDE_CONFIG_DIR":config.path()}});
        // Before the first message: nothing said would be left.
        assert_eq!(cut(&settings, "s1", "u1"), Ok(None));
        // Before a later one: a session with what came before, under its own id.
        let fresh = cut(&settings, "s1", "u2").unwrap().unwrap();
        let kept = fs::read_to_string(folder.join(format!("{fresh}.jsonl"))).unwrap();
        assert_eq!(kept.lines().count(), 3);
        assert!(!kept.contains("\"s1\"") && !kept.contains("u2"), "{kept}");
        assert_eq!(
            cut(&settings, "s1", "gone"),
            Err("Claude Code's record does not have that message".to_owned())
        );
    }

    #[test]
    fn claude_code_s_record_is_what_was_said_and_done() {
        let record = [
            json!({"type":"user","uuid":"u1","message":{"content":"fix it"}}),
            json!({"type":"user","isMeta":true,"message":{"content":"a note to itself"}}),
            json!({"type":"user","message":{"content":"<local-command-stdout>set</local-command-stdout>"}}),
            json!({"type":"assistant","message":{"model":"m","content":[{"type":"text","text":"Looking."},
                {"type":"tool_use","id":"t1","name":"Bash","input":{"command":"ls"}}]}}),
            json!({"type":"assistant","isSidechain":true,"message":{"content":[{"type":"text","text":"a sub-agent"}]}}),
            json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":"a.txt"}]},
                "toolUseResult":{"stdout":"a.txt","stderr":""}}),
            json!({"type":"assistant","message":{"model":"m","content":[{"type":"text","text":"One file."}]}}),
            json!({"type":"assistant","message":{"model":"m","content":[{"type":"text","text":"Done."}]}}),
            json!({"type":"assistant","message":{"model":"<synthetic>","content":[{"type":"text","text":"API Error"}]}}),
        ]
        .map(|entry| entry.to_string())
        .join("\n");
        let items = claude(&record);
        let kinds: Vec<_> = items
            .iter()
            .map(|item| item["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            kinds,
            [
                "user_message",
                "assistant_message",
                "tool_call",
                "tool_call",
                "assistant_message",
                "assistant_message"
            ]
        );
        assert_eq!(
            (&items[0]["text"], &items[0]["messageId"]),
            (&json!("fix it"), &json!("u1"))
        );
        assert_eq!(
            (&items[3]["status"], &items[3]["detail"]["output"]),
            (&json!("completed"), &json!("a.txt"))
        );
        assert_eq!(items[5]["text"], "\n\nDone.");
    }

    #[test]
    fn codex_s_record_is_its_finished_items() {
        let done = |item: Value| {
            json!({"type":"event_msg","payload":{"type":"item_completed","item":item}}).to_string()
        };
        let record = [
            json!({"type":"response_item","payload":{"type":"message","role":"user"}}).to_string(),
            done(json!({"type":"UserMessage","id":"u","content":[{"type":"text","text":"go"}]})),
            done(json!({"type":"Reasoning","id":"r","summary_text":["Thinking it over"]})),
            done(json!({"type":"CommandExecution","id":"c","command":"ls","cwd":"/w","aggregated_output":"a\n","exit_code":0,"status":"completed"})),
            done(json!({"type":"AgentMessage","id":"a","content":[{"type":"Text","text":"done"}]})),
        ]
        .join("\n");
        let items = codex(&record);
        assert_eq!(items.len(), 4);
        assert_eq!(
            items[0],
            json!({"type":"user_message","text":"go","messageId":"u"})
        );
        assert_eq!(
            items[2]["detail"],
            json!({"type":"shell","command":"ls","cwd":"/w","output":"a\n","exitCode":0})
        );
        assert_eq!(items[3], json!({"type":"assistant_message","text":"done"}));
    }

    #[test]
    fn only_an_id_is_looked_for() {
        assert!(read("claude", &Value::Null, "../../etc/passwd").is_empty());
        assert!(read("codex", &Value::Null, "").is_empty());
    }
}

/// The files named as `named` says at most `depth` folders under `dir`,
/// each with when it was last written.
fn all(
    dir: &Path,
    depth: usize,
    named: &dyn Fn(&str) -> bool,
    found: &mut Vec<(std::time::SystemTime, PathBuf)>,
) {
    for entry in fs::read_dir(dir).into_iter().flatten().flatten() {
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_file() && entry.file_name().to_str().is_some_and(named) {
            if let Ok(written) = entry.metadata().and_then(|metadata| metadata.modified()) {
                found.push((written, entry.path()));
            }
        } else if kind.is_dir() && depth > 0 {
            all(&entry.path(), depth - 1, named, found);
        }
    }
}

/// The sessions `family`'s CLI has records of on this host, the latest
/// first: for a person to pick one to bring into Sophon. Each is read
/// only as far as it says where it ran and what was first asked.
pub fn recent(
    provider: &str,
    family: &str,
    settings: &Value,
    limit: usize,
    query: &str,
) -> Vec<Value> {
    use std::io::{BufRead, BufReader};
    let mut records = Vec::new();
    if family == "codex" {
        let dir = crate::models::kept(settings, "CODEX_HOME", ".codex").join("sessions");
        all(
            &dir,
            3,
            &|name| name.starts_with("rollout-") && name.ends_with(".jsonl"),
            &mut records,
        );
    } else {
        let dir = crate::models::kept(settings, "CLAUDE_CONFIG_DIR", ".claude").join("projects");
        all(&dir, 1, &|name| name.ends_with(".jsonl"), &mut records);
    }
    records.sort_by(|left, right| right.0.cmp(&left.0));
    let query = query.to_lowercase();
    let mut sessions = Vec::new();
    // With something to find, further back is looked through for it.
    for (written, path) in
        records
            .into_iter()
            .take(if query.is_empty() { limit } else { limit * 5 })
    {
        let Ok(file) = fs::File::open(&path) else {
            continue;
        };
        let (mut id, mut cwd, mut first) = (None::<String>, None::<String>, None::<String>);
        for line in BufReader::new(file).lines().take(120).map_while(Result::ok) {
            let Ok(entry) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if family == "codex" {
                let payload = &entry["payload"];
                if entry["type"] == "session_meta" {
                    id = payload["id"].as_str().map(str::to_owned);
                    cwd = payload["cwd"].as_str().map(str::to_owned);
                } else if payload["type"] == "item_completed"
                    && payload["item"]["type"] == "UserMessage"
                {
                    first = payload["item"]["content"][0]["text"]
                        .as_str()
                        .map(str::to_owned);
                }
            } else {
                id = id.or_else(|| entry["sessionId"].as_str().map(str::to_owned));
                cwd = cwd.or_else(|| entry["cwd"].as_str().map(str::to_owned));
                if entry["type"] == "user"
                    && entry["isMeta"] != true
                    && entry["isSidechain"] != true
                {
                    let content = &entry["message"]["content"];
                    first = content
                        .as_str()
                        .or_else(|| {
                            content
                                .as_array()?
                                .iter()
                                .find_map(|block| block["text"].as_str())
                        })
                        .and_then(typed)
                        .map(str::to_owned);
                }
            }
            if id.is_some() && cwd.is_some() && first.is_some() {
                break;
            }
        }
        let (Some(id), Some(cwd)) = (id, cwd) else {
            continue;
        };
        let title: String = first.unwrap_or_default().chars().take(120).collect();
        if !query.is_empty()
            && !title.to_lowercase().contains(&query)
            && !cwd.to_lowercase().contains(&query)
        {
            continue;
        }
        sessions.push(json!({"providerId":provider,"providerLabel":if family == "codex" { "Codex" } else { "Claude" },
            "providerHandleId":id,"cwd":cwd,"title":if title.is_empty() { Value::Null } else { json!(title) },
            "lastActivityAt":chrono::DateTime::<chrono::Utc>::from(written).to_rfc3339()}));
        if sessions.len() == limit {
            break;
        }
    }
    sessions
}
