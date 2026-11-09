//! The client's view of daemon data, decoded leniently from the wire JSON.
//!
//! The Paseo wire contract is append-only: new fields, message types and enum
//! values appear without notice, so everything here reads from
//! `serde_json::Value` and ignores what it does not know.

use serde_json::Value;

/// A project: a repository or directory the daemon knows.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub root_path: String,
    /// `git` | `non_git` | `directory`.
    pub kind: String,
}

impl Project {
    pub fn from_json(v: &Value) -> Option<Self> {
        let root_path = str_of(v, "projectRootPath").unwrap_or_default();
        Some(Self {
            id: str_of(v, "projectId")?,
            name: str_of(v, "projectDisplayName")
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| folder_name(&root_path)),
            root_path,
            kind: str_of(v, "projectKind").unwrap_or_default(),
        })
    }

    pub fn is_git(&self) -> bool {
        self.kind == "git"
    }
}

/// A workspace: a checkout (the project's own, or a worktree) agents run in.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Workspace {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub project_root: String,
    pub name: String,
    pub directory: String,
    /// `local_checkout` | `worktree` | `directory` | `checkout`.
    pub kind: String,
    /// `needs_input` | `failed` | `running` | `attention` | `done`.
    pub status: String,
    pub branch: Option<String>,
    pub additions: u64,
    pub deletions: u64,
    pub activity_at: String,
    pub pinned: bool,
    pub archiving: bool,
    pub scripts: Vec<Script>,
    pub labels: Vec<String>,
}

/// A script or service a workspace defines (`paseo.json`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Script {
    pub name: String,
    pub service: bool,
    pub running: bool,
    pub healthy: Option<bool>,
    pub url: Option<String>,
}

impl Workspace {
    pub fn from_json(v: &Value) -> Option<Self> {
        let project_root = str_of(v, "projectRootPath").unwrap_or_default();
        let diff = v.get("diffStat");
        let num = |k: &str| diff.and_then(|d| d.get(k)).and_then(Value::as_u64).unwrap_or(0);
        Some(Self {
            id: str_of(v, "id")?,
            project_id: str_of(v, "projectId").unwrap_or_default(),
            project_name: str_of(v, "projectDisplayName").unwrap_or_default(),
            directory: str_of(v, "workspaceDirectory").unwrap_or_else(|| project_root.clone()),
            project_root,
            name: str_of(v, "name").unwrap_or_default(),
            kind: str_of(v, "workspaceKind").unwrap_or_default(),
            status: str_of(v, "status").unwrap_or_default(),
            branch: v.pointer("/gitRuntime/currentBranch").and_then(Value::as_str).map(str::to_owned),
            additions: num("additions"),
            deletions: num("deletions"),
            activity_at: str_of(v, "activityAt").unwrap_or_default(),
            pinned: v.get("pinnedAt").is_some_and(|p| !p.is_null()),
            archiving: v.get("archivingAt").is_some_and(|p| !p.is_null()),
            labels: v
                .get("labels")
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
                .unwrap_or_default(),
            scripts: v
                .get("scripts")
                .and_then(Value::as_array)
                .map(|l| {
                    l.iter()
                        .filter_map(|sc| {
                            Some(Script {
                                name: str_of(sc, "scriptName")?,
                                service: str_of(sc, "type").as_deref() != Some("script"),
                                running: str_of(sc, "lifecycle").as_deref() == Some("running"),
                                healthy: str_of(sc, "health").map(|h| h == "healthy"),
                                url: str_of(sc, "localProxyUrl").or_else(|| str_of(sc, "proxyUrl")).or_else(|| {
                                    sc.get("port").and_then(Value::as_u64).map(|p| format!("http://localhost:{p}"))
                                }),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    pub fn is_worktree(&self) -> bool {
        self.kind == "worktree"
    }
}

/// A selectable option (a mode, a thinking level).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Opt {
    pub id: String,
    pub label: String,
    pub description: String,
}

impl Opt {
    fn from_json(v: &Value) -> Option<Self> {
        let id = str_of(v, "id")?;
        Some(Self {
            label: str_of(v, "label").filter(|l| !l.is_empty()).unwrap_or_else(|| id.clone()),
            description: str_of(v, "description").unwrap_or_default(),
            id,
        })
    }
}

/// A provider feature on an agent or a draft: a toggle or a choice.
#[derive(Clone, Debug, PartialEq)]
pub struct Feature {
    pub id: String,
    pub label: String,
    pub description: String,
    /// `Ok(on)` for a toggle, `Err((value, options))` for a choice.
    pub value: Result<bool, (Option<String>, Vec<Opt>)>,
}

impl Feature {
    pub fn from_json(v: &Value) -> Option<Self> {
        let id = str_of(v, "id")?;
        let value = match str_of(v, "type").as_deref() {
            Some("toggle") => Ok(v.get("value").and_then(Value::as_bool).unwrap_or(false)),
            Some("select") => Err((
                str_of(v, "value"),
                v.get("options").and_then(Value::as_array).map(|o| o.iter().filter_map(Opt::from_json).collect()).unwrap_or_default(),
            )),
            _ => return None,
        };
        Some(Self {
            label: str_of(v, "label").unwrap_or_else(|| id.clone()),
            description: str_of(v, "description").or_else(|| str_of(v, "tooltip")).unwrap_or_default(),
            id,
            value,
        })
    }
}

/// A model a provider offers.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Model {
    pub id: String,
    pub label: String,
    pub description: String,
    pub is_default: bool,
    pub thinking: Vec<Opt>,
    pub default_thinking: Option<String>,
    pub context_window: Option<u64>,
}

/// An agent provider (`claude`, `codex`, ...) from the providers snapshot.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Provider {
    pub id: String,
    pub label: String,
    /// `ready` | `loading` | `error` | `unavailable`.
    pub status: String,
    pub enabled: bool,
    pub error: Option<String>,
    pub models: Vec<Model>,
    pub modes: Vec<Opt>,
    pub default_mode: Option<String>,
}

impl Provider {
    pub fn from_json(v: &Value) -> Option<Self> {
        let id = str_of(v, "provider")?;
        let models = v
            .get("models")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter(|m| m.get("isSelectable").and_then(Value::as_bool) != Some(false))
                    .filter_map(|m| {
                        let id = str_of(m, "id")?;
                        Some(Model {
                            label: str_of(m, "label").unwrap_or_else(|| id.clone()),
                            description: str_of(m, "description").unwrap_or_default(),
                            is_default: m.get("isDefault").and_then(Value::as_bool).unwrap_or(false),
                            thinking: m
                                .get("thinkingOptions")
                                .and_then(Value::as_array)
                                .map(|o| o.iter().filter_map(Opt::from_json).collect())
                                .unwrap_or_default(),
                            default_thinking: str_of(m, "defaultThinkingOptionId"),
                            context_window: m.get("contextWindowMaxTokens").and_then(Value::as_u64),
                            id,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            label: str_of(v, "label").filter(|l| !l.is_empty()).unwrap_or_else(|| title_case(&id)),
            status: str_of(v, "status").unwrap_or_default(),
            enabled: v.get("enabled").and_then(Value::as_bool).unwrap_or(true),
            error: str_of(v, "error"),
            models,
            modes: v
                .get("modes")
                .and_then(Value::as_array)
                .map(|o| o.iter().filter_map(Opt::from_json).collect())
                .unwrap_or_default(),
            default_mode: str_of(v, "defaultModeId"),
            id,
        })
    }

    /// Usable now: enabled and installed.
    pub fn available(&self) -> bool {
        self.enabled && self.status != "unavailable" && self.status != "error"
    }

    pub fn default_model(&self) -> Option<&Model> {
        self.models.iter().find(|m| m.is_default).or_else(|| self.models.first())
    }

    pub fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }
}

fn title_case(id: &str) -> String {
    let mut c = id.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// One agent, as the sidebar and header show it.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Agent {
    pub workspace_id: Option<String>,
    pub model: Option<String>,
    pub mode_id: Option<String>,
    pub thinking_id: Option<String>,
    pub modes: Vec<Opt>,
    pub parent_id: Option<String>,
    pub created_at: String,
    /// Context window used / max, in tokens, and the cost so far.
    pub features: Vec<Feature>,
    pub context_used: Option<u64>,
    pub context_max: Option<u64>,
    pub cost_usd: Option<f64>,
    pub id: String,
    pub title: String,
    pub provider: String,
    pub cwd: String,
    /// `initializing` | `idle` | `running` | `error` | `closed`, or a newer value.
    pub status: String,
    pub requires_attention: bool,
    pub archived: bool,
    pub updated_at: String,
    pub pending: Vec<Permission>,
}

impl Agent {
    pub fn from_json(v: &Value) -> Option<Self> {
        let id = str_of(v, "id")?;
        let cwd = str_of(v, "cwd").unwrap_or_default();
        let title = str_of(v, "title")
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| folder_name(&cwd));
        Some(Self {
            workspace_id: str_of(v, "workspaceId"),
            model: str_of(v, "model"),
            mode_id: str_of(v, "currentModeId"),
            thinking_id: str_of(v, "thinkingOptionId"),
            modes: v
                .get("availableModes")
                .and_then(Value::as_array)
                .map(|o| o.iter().filter_map(Opt::from_json).collect())
                .unwrap_or_default(),
            parent_id: str_of(v, "parentAgentId"),
            created_at: str_of(v, "createdAt").unwrap_or_default(),
            features: v
                .get("features")
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(Feature::from_json).collect())
                .unwrap_or_default(),
            context_used: v.pointer("/lastUsage/contextWindowUsedTokens").and_then(Value::as_u64),
            context_max: v.pointer("/lastUsage/contextWindowMaxTokens").and_then(Value::as_u64),
            cost_usd: v.pointer("/lastUsage/totalCostUsd").and_then(Value::as_f64),
            id,
            title,
            provider: str_of(v, "provider").unwrap_or_default(),
            cwd,
            status: str_of(v, "status").unwrap_or_else(|| "idle".into()),
            requires_attention: v.get("requiresAttention").and_then(Value::as_bool).unwrap_or(false),
            archived: v.get("archivedAt").is_some_and(|a| !a.is_null()),
            updated_at: str_of(v, "updatedAt").unwrap_or_default(),
            pending: v
                .get("pendingPermissions")
                .and_then(Value::as_array)
                .map(|list| list.iter().filter_map(Permission::from_json).collect())
                .unwrap_or_default(),
        })
    }

    pub fn is_running(&self) -> bool {
        self.status == "running" || self.status == "initializing"
    }
}

/// A permission the agent waits on.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Permission {
    pub id: String,
    /// `tool` | `plan` | `question` | `mode` | `other`.
    pub kind: String,
    /// The tool's input as sent (questions and plans read it).
    pub input: Option<Value>,
    pub title: String,
    pub description: String,
    pub actions: Vec<PermissionAction>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct PermissionAction {
    pub id: String,
    pub label: String,
    pub allow: bool,
}

impl Permission {
    pub fn from_json(v: &Value) -> Option<Self> {
        let id = str_of(v, "id")?;
        let title = str_of(v, "title")
            .or_else(|| str_of(v, "name"))
            .unwrap_or_else(|| "Permission requested".into());
        let description = str_of(v, "description")
            .or_else(|| v.get("input").map(summarize_input))
            .unwrap_or_default();
        let actions = v
            .get("actions")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|a| {
                        Some(PermissionAction {
                            id: str_of(a, "id")?,
                            label: str_of(a, "label").unwrap_or_default(),
                            allow: a.get("behavior").and_then(Value::as_str) == Some("allow"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            id,
            kind: str_of(v, "kind").unwrap_or_default(),
            input: v.get("input").cloned(),
            title,
            description,
            actions,
        })
    }
}

/// One row of an agent's conversation.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// `images`: what was attached, as data URIs, for the bubble's
    /// thumbnails (the host's echo of a message carries its text only).
    User { text: String, message_id: Option<String>, images: Vec<String> },
    Assistant { text: String },
    Reasoning { text: String },
    /// `kind` is the detail type: `shell`, `read`, `edit`, `write`, `search`, `fetch`, `sub_agent`, ...
    /// `detail`: the call's detail object as the daemon sent it (its
    /// `type` says which fields it has), `error` what failed.
    Tool { call_id: String, name: String, kind: String, status: String, summary: String, detail: Value, error: Option<String> },
    Todo { lines: Vec<(bool, String)> },
    Error { message: String },
    Notice { message: String },
}

impl Item {
    /// Decodes a timeline item; `None` for kinds this client does not show.
    pub fn from_json(v: &Value) -> Option<Self> {
        let text = || str_of(v, "text").unwrap_or_default();
        Some(match v.get("type")?.as_str()? {
            "user_message" => Item::User { text: text(), message_id: str_of(v, "messageId"), images: vec![] },
            "assistant_message" => Item::Assistant { text: text() },
            "reasoning" => Item::Reasoning { text: text() },
            "tool_call" => Item::Tool {
                call_id: str_of(v, "callId").unwrap_or_default(),
                name: str_of(v, "name").unwrap_or_default(),
                kind: v.pointer("/detail/type").and_then(Value::as_str).unwrap_or_default().to_owned(),
                status: str_of(v, "status").unwrap_or_default(),
                summary: v.get("detail").map(summarize_tool).unwrap_or_default(),
                detail: v.get("detail").cloned().unwrap_or(Value::Null),
                error: v.get("error").filter(|e| !e.is_null()).map(|e| match e.as_str() {
                    Some(s) => s.to_owned(),
                    None => e.get("message").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| e.to_string()),
                }),
            },
            "todo" => Item::Todo {
                lines: v
                    .get("items")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .map(|t| {
                                let done = t.get("completed").and_then(Value::as_bool).unwrap_or(false)
                                    || t.get("status").and_then(Value::as_str) == Some("completed");
                                (done, str_of(t, "text").unwrap_or_default())
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            },
            "error" => Item::Error { message: str_of(v, "message").unwrap_or_default() },
            "notification" => Item::Notice { message: str_of(v, "message").unwrap_or_default() },
            "compaction" => Item::Notice {
                message: if str_of(v, "status").as_deref() == Some("loading") {
                    "Compacting context…".into()
                } else {
                    "Context compacted".into()
                },
            },
            _ => return None,
        })
    }
}

/// Applies a live stream item the way Paseo's own client does: assistant and
/// reasoning text arrive as deltas appended to the row being written, tool
/// calls replace their earlier state by `callId`, and an echoed user message
/// that is already shown (sent from here) is not added twice.
pub fn apply_live(items: &mut Vec<Item>, incoming: Item) {
    match incoming {
        Item::Assistant { text } => match items.last_mut() {
            Some(Item::Assistant { text: last }) => last.push_str(&text),
            _ if text.trim().is_empty() => {}
            _ => items.push(Item::Assistant { text }),
        },
        Item::Reasoning { text } => match items.last_mut() {
            Some(Item::Reasoning { text: last }) => last.push_str(&text),
            _ => items.push(Item::Reasoning { text }),
        },
        Item::Tool { ref call_id, .. } if !call_id.is_empty() => {
            let existing = items
                .iter_mut()
                .rev()
                .find(|i| matches!(i, Item::Tool { call_id: c, .. } if c == call_id));
            match existing {
                Some(row) => *row = incoming,
                None => items.push(incoming),
            }
        }
        Item::User { ref text, ref message_id, .. } => {
            let duplicate = items.iter().rev().take(4).any(|i| match i {
                Item::User { text: t, message_id: m, .. } => {
                    (message_id.is_some() && m == message_id) || (m.is_none() && t == text)
                }
                _ => false,
            });
            if !duplicate {
                items.push(incoming);
            }
        }
        other => items.push(other),
    }
}

fn str_of(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

pub(crate) fn folder_name(path: &str) -> String {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path).to_owned()
}

fn summarize_tool(detail: &Value) -> String {
    let field = |k: &str| str_of(detail, k);
    match detail.get("type").and_then(Value::as_str) {
        Some("shell") => field("command"),
        Some("read" | "edit" | "write") => field("filePath"),
        Some("search") => field("query"),
        Some("fetch") => field("url"),
        Some("sub_agent") => field("description").or_else(|| field("title")),
        Some("plain_text") => field("text"),
        _ => None,
    }
    .unwrap_or_default()
}

fn summarize_input(input: &Value) -> String {
    for key in ["command", "file_path", "filePath", "path", "url", "query", "description"] {
        if let Some(s) = input.get(key).and_then(Value::as_str) {
            return s.to_owned();
        }
    }
    let s = input.to_string();
    if s.len() > 400 { format!("{}…", &s[..s.floor_char_boundary(400)]) } else { s }
}

/// One changed file of a workspace, with its hunks as unified patch text
/// (what `DiffView` reads).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub is_new: bool,
    pub is_deleted: bool,
    pub additions: u64,
    pub deletions: u64,
    /// `ok` | `too_large` | `binary`.
    pub status: String,
    pub patch: String,
}

impl DiffFile {
    pub fn from_json(v: &Value) -> Option<Self> {
        let path = str_of(v, "path")?;
        let old_path = str_of(v, "oldPath");
        let is_new = v.get("isNew").and_then(Value::as_bool).unwrap_or(false);
        let is_deleted = v.get("isDeleted").and_then(Value::as_bool).unwrap_or(false);
        let old = old_path.clone().unwrap_or_else(|| path.clone());
        let mut patch = format!("diff --git a/{old} b/{path}\n");
        if is_new {
            patch.push_str("new file mode 100644\n");
        }
        if is_deleted {
            patch.push_str("deleted file mode 100644\n");
        }
        patch.push_str(&format!("--- {}\n", if is_new { "/dev/null".to_owned() } else { format!("a/{old}") }));
        patch.push_str(&format!("+++ {}\n", if is_deleted { "/dev/null".to_owned() } else { format!("b/{path}") }));
        for h in v.get("hunks").and_then(Value::as_array).into_iter().flatten() {
            let n = |k: &str| h.get(k).and_then(Value::as_u64).unwrap_or(0);
            patch.push_str(&format!(
                "@@ -{},{} +{},{} @@\n",
                n("oldStart"),
                n("oldCount"),
                n("newStart"),
                n("newCount")
            ));
            for line in h.get("lines").and_then(Value::as_array).into_iter().flatten() {
                let content = line.get("content").and_then(Value::as_str).unwrap_or_default();
                let prefix = match line.get("type").and_then(Value::as_str) {
                    Some("add") => '+',
                    Some("remove") => '-',
                    Some("context") => ' ',
                    _ => continue,
                };
                patch.push(prefix);
                patch.push_str(content);
                patch.push('\n');
            }
        }
        Some(Self {
            additions: v.get("additions").and_then(Value::as_u64).unwrap_or(0),
            deletions: v.get("deletions").and_then(Value::as_u64).unwrap_or(0),
            status: str_of(v, "status").unwrap_or_else(|| "ok".into()),
            path,
            old_path,
            is_new,
            is_deleted,
            patch,
        })
    }
}

/// One question of an `AskUserQuestion`-style permission.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Question {
    pub header: String,
    pub question: String,
    pub options: Vec<(String, String)>,
    pub multi: bool,
}

/// The questions a `question` permission asks, when its input carries them.
pub fn questions_of(input: &Value) -> Option<Vec<Question>> {
    let list = input.get("questions")?.as_array()?;
    list.iter()
        .map(|q| {
            let question = str_of(q, "question").unwrap_or_default();
            Some(Question {
                header: str_of(q, "header").filter(|h| !h.is_empty()).unwrap_or_else(|| question.clone()),
                options: q
                    .get("options")
                    .and_then(Value::as_array)
                    .map(|o| {
                        o.iter()
                            .filter_map(|o| Some((str_of(o, "label")?, str_of(o, "description").unwrap_or_default())))
                            .collect()
                    })
                    .unwrap_or_default(),
                multi: q.get("multiSelect").and_then(Value::as_bool).unwrap_or(false),
                question,
            })
        })
        .collect()
}

/// The plan text of a `plan` permission.
pub fn plan_of(input: &Value) -> Option<String> {
    str_of(input, "plan").filter(|p| !p.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn assistant_deltas_merge_into_one_row() {
        let mut items = vec![Item::User { text: "hi".into(), message_id: None, images: vec![] }];
        apply_live(&mut items, Item::Assistant { text: "Hel".into() });
        apply_live(&mut items, Item::Assistant { text: "lo".into() });
        assert_eq!(items.len(), 2);
        assert_eq!(items[1], Item::Assistant { text: "Hello".into() });
    }

    #[test]
    fn tool_calls_replace_by_call_id() {
        let mut items = vec![];
        let tool = |status: &str| Item::Tool {
            call_id: "c1".into(),
            name: "Bash".into(),
            kind: "shell".into(),
            status: status.into(),
            detail: Value::Null,
            error: None,
            summary: "ls".into(),
        };
        apply_live(&mut items, tool("running"));
        apply_live(&mut items, Item::Assistant { text: "ok".into() });
        apply_live(&mut items, tool("completed"));
        assert_eq!(items.len(), 2);
        assert!(matches!(&items[0], Item::Tool { status, .. } if status == "completed"));
    }

    #[test]
    fn echoed_user_message_is_not_duplicated() {
        let mut items = vec![Item::User { text: "fix it".into(), message_id: Some("m1".into()), images: vec![] }];
        apply_live(&mut items, Item::User { text: "fix it".into(), message_id: Some("m1".into()), images: vec![] });
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn decodes_agent_with_permission() {
        let agent = Agent::from_json(&json!({
            "id": "a1", "provider": "claude", "cwd": "/Users/me/code/app", "status": "running",
            "title": null, "futureField": 1,
            "pendingPermissions": [{
                "id": "p1", "provider": "claude", "name": "Bash", "kind": "tool",
                "input": {"command": "rm -rf build"},
                "actions": [{"id": "allow", "label": "Allow", "behavior": "allow"}]
            }]
        }))
        .unwrap();
        assert_eq!(agent.title, "app");
        assert!(agent.is_running());
        assert_eq!(agent.pending[0].description, "rm -rf build");
        assert!(agent.pending[0].actions[0].allow);
    }

    #[test]
    fn rebuilds_a_unified_patch() {
        let f = DiffFile::from_json(&json!({
            "path": "src/a.rs", "isNew": false, "isDeleted": false, "additions": 1, "deletions": 1,
            "hunks": [{"oldStart": 1, "oldCount": 2, "newStart": 1, "newCount": 2, "lines": [
                {"type": "header", "content": "@@ -1,2 +1,2 @@"},
                {"type": "context", "content": "fn a() {"},
                {"type": "remove", "content": "    1"},
                {"type": "add", "content": "    2"}
            ]}]
        }))
        .unwrap();
        assert!(f.patch.contains("--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,2 @@\n fn a() {\n-    1\n+    2\n"));
    }

    #[test]
    fn parses_questions() {
        let qs = questions_of(&json!({"questions": [
            {"question": "Which db?", "header": "DB", "multiSelect": false,
             "options": [{"label": "Postgres", "description": "relational"}, {"label": "Redis"}]}
        ]}))
        .unwrap();
        assert_eq!(qs[0].header, "DB");
        assert_eq!(qs[0].options.len(), 2);
        assert!(!qs[0].multi);
    }

    #[test]
    fn unknown_item_types_are_skipped() {
        assert!(Item::from_json(&json!({"type": "something_new"})).is_none());
        let tool = Item::from_json(&json!({
            "type": "tool_call", "callId": "c", "name": "Read", "status": "completed",
            "detail": {"type": "read", "filePath": "src/main.rs"}
        }));
        assert!(matches!(tool, Some(Item::Tool { summary, .. }) if summary == "src/main.rs"));
    }
}
