#![cfg(unix)]

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

struct Service(tempfile::TempDir);

impl Service {
    fn cli(&self, action: &str) {
        self.cli_with_env(action, &[]);
    }

    fn cli_with_env(&self, action: &str, environment: &[(&str, &str)]) {
        let result = Command::new(env!("CARGO_BIN_EXE_sophon-service"))
            .args(["daemon", action, "--home"])
            .arg(self.0.path())
            .envs(environment.iter().copied())
            .env("SOPHON_SERVICE_LISTEN", "127.0.0.1:0")
            .env(
                "PATH",
                std::env::join_paths([self.0.path(), Path::new("/usr/bin"), Path::new("/bin")])
                    .unwrap(),
            )
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }

    async fn connect(&self, authorized: bool) -> Socket {
        let home = self.0.path();
        let pid: Value =
            serde_json::from_slice(&fs::read(home.join("sophon.pid")).unwrap()).unwrap();
        let url = format!("ws://{}/ws", pid["listen"].as_str().unwrap());
        let (mut socket, _) = connect_async(url).await.unwrap();
        let token = if authorized {
            fs::read_to_string(home.join("local-credential")).unwrap()
        } else {
            "invalid".into()
        };
        send(&mut socket,json!({"type":"hello","protocolVersion":1,"auth":{"kind":"localCredential","token":token}})).await;
        let reply = read(&mut socket).await;
        if authorized {
            assert_eq!(
                reply.pointer("/message/payload/status").unwrap(),
                "server_info"
            );
        } else {
            assert_eq!(reply["type"], "hello.rejected");
        }
        socket
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = Command::new(env!("CARGO_BIN_EXE_sophon-service"))
            .args(["daemon", "stop", "--home"])
            .arg(self.0.path())
            .output();
    }
}

async fn send(socket: &mut Socket, data: Value) {
    socket.send(Message::text(data.to_string())).await.unwrap();
}

/// How long a frame is waited for at most: a frame that comes returns at
/// once, so this only bounds a hang. With the stand-in CLIs a frame takes
/// well under a second, but CI's runners, with every test's service and
/// `git` at once, have been seen to stall one for over 5 s; the real CLIs
/// take as long as a model does.
static PATIENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(30);

async fn read(socket: &mut Socket) -> Value {
    let patience = Duration::from_secs(PATIENCE.load(std::sync::atomic::Ordering::Relaxed));
    let frame = tokio::time::timeout(patience, socket.next())
        .await
        .unwrap_or_else(|_| panic!("no frame within {patience:?}"))
        .expect("the socket closed")
        .expect("a frame");
    serde_json::from_str(frame.to_text().unwrap()).unwrap()
}

/// Where the manifest of `folder` goes, `kind` an `integration` or a
/// `plugin`: the folder it is kept in is made.
fn manifest(folder: &Path, kind: &str) -> PathBuf {
    let kept = folder.join(".sophon-plugin");
    fs::create_dir_all(&kept).unwrap();
    kept.join(format!("{kind}.json"))
}

/// A row's item without when it was written, to compare with what it says.
fn without_time(item: &Value) -> Value {
    let mut item = item.clone();
    if let Some(item) = item.as_object_mut() {
        item.remove("at");
    }
    item
}

async fn ask(socket: &mut Socket, mut request: Value) -> Value {
    let id = uuid::Uuid::new_v4().to_string();
    request["requestId"] = json!(id);
    send(socket, json!({"type":"session","message":request})).await;
    loop {
        let message = read(socket).await;
        if message.pointer("/message/payload/requestId") == Some(&json!(id)) {
            return message["message"].clone();
        }
    }
}

fn install(dir: &Path, name: &str, script: &str) {
    let cli = dir.join(name);
    fs::write(&cli, script).unwrap();
    fs::set_permissions(cli, fs::Permissions::from_mode(0o700)).unwrap();
}

/// A `codex` that is its app server for one conversation: a thread
/// `session-123`, and for each turn a reply written in two pieces.
fn install_provider(dir: &Path) {
    install(
        dir,
        "codex",
        r#"#!/bin/sh
case " $* " in *" app-server "*) ;; *) exit 2 ;; esac
say() { printf '%s\n' "$1"; }
while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    case "$line" in
        *'"method":"initialize"'*) say "{\"id\":$id,\"result\":{}}" ;;
        *'"method":"thread/start"'*) say "{\"id\":$id,\"result\":{\"thread\":{\"id\":\"session-123\"}}}" ;;
        *'"method":"thread/resume"'*)
            case "$line" in *'"threadId":"session-123"'*) ;; *) exit 3 ;; esac
            say "{\"id\":$id,\"result\":{\"thread\":{\"id\":\"session-123\"}}}" ;;
        *'"method":"turn/start"'*)
            say "{\"id\":$id,\"result\":{\"turn\":{\"id\":\"turn-1\"}}}"
            say '{"method":"turn/started","params":{"turn":{"id":"turn-1"}}}'
            say '{"method":"item/started","params":{"item":{"type":"agentMessage","id":"m1","text":""}}}'
            say '{"method":"item/agentMessage/delta","params":{"itemId":"m1","delta":"re"}}'
            say '{"method":"item/agentMessage/delta","params":{"itemId":"m1","delta":"ply"}}'
            say '{"method":"item/completed","params":{"item":{"type":"agentMessage","id":"m1","text":"reply"}}}'
            say '{"method":"thread/tokenUsage/updated","params":{"tokenUsage":{"last":{"totalTokens":42},"modelContextWindow":1000}}}'
            say '{"method":"turn/completed","params":{"turn":{"id":"turn-1","status":"completed"}}}' ;;
    esac
done
"#,
    );
}

/// Reads until the turn underway completes; a failed one fails the test.
async fn finished(socket: &mut Socket) {
    until(socket, "turn_completed").await;
}

/// Reads until the agent's stream says `wanted`, and answers that event.
async fn until(socket: &mut Socket, wanted: &str) -> Value {
    loop {
        let frame = read(socket).await;
        let event = &frame["message"]["payload"]["event"];
        if let Some(kind) = event["type"].as_str() {
            assert_ne!(kind, "turn_failed", "{frame}");
            if kind == wanted {
                return event.clone();
            }
        }
    }
}

#[tokio::test]
async fn claude_runs_on_its_own_login_unless_its_provider_names_an_endpoint() {
    let service = Service(tempfile::tempdir().unwrap());
    install(
        service.0.path(),
        "claude",
        r#"#!/bin/sh
set -eu
test -z "${ANTHROPIC_MODEL:-}${ANTHROPIC_API_KEY:-}${CLAUDECODE:-}${CLAUDE_CODE_CHILD_SESSION:-}"
test "${CLAUDE_CONFIG_DIR:-}" = /kept
case "${ANTHROPIC_BASE_URL:-}|${ANTHROPIC_AUTH_TOKEN:-}" in
    '|'|'https://explicit.invalid|explicit-token') ;;
    *) exit 3 ;;
esac
while IFS= read -r line; do
    case "$line" in *'"type":"user"'*)
        printf '%s\n' "${ANTHROPIC_BASE_URL:-native}" >> routes
        echo '{"type":"result","is_error":false}' ;;
    esac
done
"#,
    );
    fs::write(
        service.0.path().join("config.json"),
        json!({"providers":{"gateway":{"extends":"claude","env":{
            "ANTHROPIC_BASE_URL":"https://explicit.invalid","ANTHROPIC_AUTH_TOKEN":"explicit-token"
        }}}})
        .to_string(),
    )
    .unwrap();
    // As a service started from inside an agent's session is: that
    // session's endpoint, token and marks all around it.
    service.cli_with_env(
        "start",
        &[
            ("ANTHROPIC_BASE_URL", "https://inherited.invalid"),
            ("ANTHROPIC_AUTH_TOKEN", "inherited-token"),
            ("ANTHROPIC_API_KEY", "inherited-key"),
            ("ANTHROPIC_MODEL", "inherited-model"),
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_CHILD_SESSION", "1"),
            ("CLAUDE_CONFIG_DIR", "/kept"),
        ],
    );
    let mut socket = service.connect(true).await;
    for provider in ["claude", "gateway"] {
        let response = ask(
            &mut socket,
            json!({"type":"workspace.create.request",
            "source":{"kind":"directory","path":service.0.path()},
            "agent":{"config":{"provider":provider},"initialPrompt":"hi"}}),
        )
        .await;
        assert_eq!(response["type"], "workspace.create.response");
        finished(&mut socket).await;
    }
    assert_eq!(
        fs::read_to_string(service.0.path().join("routes")).unwrap(),
        "native\nhttps://explicit.invalid\n"
    );
}

/// A `claude` that writes a reply as it goes, asks before running a
/// command, and runs it or not by the answer.
const ASKING_CLAUDE: &str = r#"#!/bin/sh
printf '%s\n' "$*" >> args
say() { printf '%s\n' "$1"; }
while IFS= read -r line; do
    case "$line" in
        *'"type":"user"'*)
            say '{"type":"system","subtype":"session_state_changed","state":"running"}'
            say '{"type":"system","subtype":"init","session_id":"claude-session","permissionMode":"acceptEdits"}'
            say '{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"message_start","message":{"id":"msg1"}}}'
            say '{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_start","content_block":{"type":"text"}}}'
            say '{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hel"}}}'
            say '{"type":"stream_event","parent_tool_use_id":null,"event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"lo"}}}'
            say '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"msg1","model":"m","content":[{"type":"text","text":"Hello"},{"type":"tool_use","id":"tool1","name":"Bash","input":{"command":"ls"}}],"usage":{"input_tokens":10,"output_tokens":5}}}'
            say '{"type":"control_request","request_id":"ask1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"ls"},"tool_use_id":"tool1"}}' ;;
        *'"request_id":"ask1"'*)
            case "$line" in *'"behavior":"allow"'*) out=files; bad=false ;; *) out=refused; bad=true ;; esac
            say "{\"type\":\"user\",\"parent_tool_use_id\":null,\"message\":{\"content\":[{\"type\":\"tool_result\",\"tool_use_id\":\"tool1\",\"content\":\"$out\",\"is_error\":$bad}]}}"
            say '{"type":"result","is_error":false,"total_cost_usd":0.5,"modelUsage":{"m":{"contextWindow":200}}}'
            say '{"type":"system","subtype":"session_state_changed","state":"idle"}' ;;
    esac
done
"#;

#[tokio::test]
async fn a_turn_streams_asks_before_a_command_and_keeps_what_happened() {
    let service = Service(tempfile::tempdir().unwrap());
    install(service.0.path(), "claude", ASKING_CLAUDE);
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request",
        "source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"claude","modeId":"acceptEdits","model":"opus","featureValues":{"x":true}},
            "initialPrompt":"go"}})).await;
    assert_eq!(created["type"], "workspace.create.response", "{created}");
    let agent = created["payload"]["agent"].clone();
    assert_eq!(agent["currentModeId"], "acceptEdits");
    assert_eq!(agent["availableModes"].as_array().unwrap().len(), 5);
    send(&mut socket, json!({"type":"session","message":
        {"type":"agent.timeline.set_subscription.request","agentIds":[agent["id"]],"requestId":"s"}})).await;

    let asked = until(&mut socket, "permission_requested").await;
    assert_eq!(asked["request"]["name"], "Bash");
    let waiting = ask(
        &mut socket,
        json!({"type":"fetch_agent_request","agentId":agent["id"]}),
    )
    .await;
    let waiting = &waiting["payload"]["agent"];
    assert_eq!(
        (&waiting["status"], &waiting["attentionReason"]),
        (&json!("running"), &json!("permission"))
    );
    assert_eq!(waiting["pendingPermissions"][0]["input"]["command"], "ls");
    let live = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent["id"]}),
    )
    .await;
    let rows = live["payload"]["entries"].as_array().unwrap();
    // A reply says when it began to be written.
    assert!(rows[1]["item"]["at"].is_string(), "{live}");
    assert_eq!(
        without_time(&rows[1]["item"]),
        json!({"type":"assistant_message","text":"Hello"})
    );
    assert_eq!(
        (&rows[1]["seqStart"], &rows[1]["seqEnd"]),
        (&json!(2), &json!(3))
    );
    assert_eq!(rows[2]["item"]["status"], "running");

    // A client answers under the permission's own id.
    send(
        &mut socket,
        json!({"type":"session","message":{"type":"agent_permission_response","agentId":agent["id"],
        "requestId":waiting["pendingPermissions"][0]["id"],"response":{"behavior":"allow"}}}),
    )
    .await;
    finished(&mut socket).await;
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent["id"]}),
    )
    .await;
    let rows = timeline["payload"]["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 3, "{timeline}");
    assert_eq!(
        rows[2]["item"]["detail"],
        json!({"type":"shell","command":"ls","output":"files"})
    );
    assert_eq!(rows[2]["item"]["status"], "completed");
    let done = &timeline["payload"]["agent"];
    assert_eq!(
        (&done["status"], &done["attentionReason"]),
        (&json!("idle"), &json!("finished"))
    );
    assert_eq!(done["pendingPermissions"], json!([]));
    assert_eq!(done["persistence"]["sessionId"], "claude-session");
    assert_eq!(
        done["lastUsage"],
        json!({"contextWindowUsedTokens":15,"contextWindowMaxTokens":200,"totalCostUsd":0.5})
    );
    let args = fs::read_to_string(service.0.path().join("args")).unwrap();
    assert!(
        args.contains("--permission-mode acceptEdits") && args.contains("--model opus"),
        "{args}"
    );
    assert!(!args.contains("--resume"), "{args}");

    let refused = ask(
        &mut socket,
        json!({"type":"set_agent_mode_request","agentId":agent["id"],"modeId":"full-access"}),
    )
    .await;
    assert_eq!(refused["type"], "rpc_error");
    let planned = ask(
        &mut socket,
        json!({"type":"set_agent_mode_request","agentId":agent["id"],"modeId":"plan"}),
    )
    .await;
    assert_eq!(planned["payload"]["agent"]["currentModeId"], "plan");

    // The next service resumes the conversation Claude Code kept, in
    // the mode the agent was left in.
    service.cli("stop");
    service.cli("start");
    let mut socket = service.connect(true).await;
    let sent = ask(&mut socket, json!({"type":"send_agent_message_request","agentId":agent["id"],"text":"again","messageId":"m2"})).await;
    assert_eq!(sent["payload"]["seq"], 6, "{sent}");
    let asked = until(&mut socket, "permission_requested").await;
    send(
        &mut socket,
        json!({"type":"session","message":{"type":"agent_permission_response","agentId":agent["id"],
        "requestId":asked["request"]["id"],"response":{"behavior":"deny","message":"no"}}}),
    )
    .await;
    finished(&mut socket).await;
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent["id"]}),
    )
    .await;
    // The stand-in gives every call the same id, so this is that row.
    let call = timeline["payload"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["item"]["callId"] == "tool1")
        .unwrap();
    assert_eq!(
        (&call["item"]["status"], &call["item"]["error"]),
        (&json!("failed"), &json!("refused"))
    );
    let args = fs::read_to_string(service.0.path().join("args")).unwrap();
    let resumed = args.lines().last().unwrap();
    assert!(
        resumed.contains("--resume claude-session") && resumed.contains("--permission-mode plan"),
        "{args}"
    );
}

/// A `claude` that says what commands it has, starts an agent of its
/// own, and asks before a command with a rule on offer to keep.
const DELEGATING_CLAUDE: &str = r#"#!/bin/sh
say() { printf '%s\n' "$1"; }
while IFS= read -r line; do
    case "$line" in
        *'"subtype":"initialize"'*)
            id=$(printf '%s' "$line" | sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')
            say "{\"type\":\"control_response\",\"response\":{\"subtype\":\"success\",\"request_id\":\"$id\",\"response\":{\"commands\":[{\"name\":\"review\",\"description\":\"Review a change\",\"argumentHint\":\"<pr>\"}]}}}" ;;
        *'"type":"user"'*)
            say '{"type":"system","subtype":"session_state_changed","state":"running"}'
            say '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m1","content":[{"type":"tool_use","id":"task1","name":"Task","input":{"description":"Look around","subagent_type":"Explore","prompt":"look"}}]}}'
            say '{"type":"assistant","parent_tool_use_id":"task1","message":{"id":"s1","content":[{"type":"text","text":"Searching"},{"type":"tool_use","id":"sub1","name":"Bash","input":{"command":"ls"}}]}}'
            say '{"type":"user","parent_tool_use_id":"task1","message":{"content":[{"type":"tool_result","tool_use_id":"sub1","content":"found"}]}}'
            say '{"type":"control_request","request_id":"ask1","request":{"subtype":"can_use_tool","tool_name":"Bash","input":{"command":"make"},"permission_suggestions":[{"type":"addRules","rules":[{"toolName":"Bash","ruleContent":"make"}],"behavior":"allow","destination":"localSettings"}]}}' ;;
        *'"request_id":"ask1"'*)
            case "$line" in *'"updatedPermissions":[{'*) : > kept-rule ;; esac
            say '{"type":"user","parent_tool_use_id":null,"message":{"content":[{"type":"tool_result","tool_use_id":"task1","content":"all found"}]}}'
            say '{"type":"result","is_error":false}'
            say '{"type":"system","subtype":"session_state_changed","state":"idle"}' ;;
    esac
done
"#;

#[tokio::test]
async fn an_agent_s_own_agents_commands_and_kept_rules() {
    let service = Service(tempfile::tempdir().unwrap());
    install(service.0.path(), "claude", DELEGATING_CLAUDE);
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request",
        "source":{"kind":"directory","path":service.0.path()},"agent":{"config":{"provider":"claude"}}})).await;
    let agent = created["payload"]["agent"]["id"].clone();
    // Asked of the CLI itself, with no conversation started.
    let commands = ask(
        &mut socket,
        json!({"type":"list_commands_request","agentId":agent}),
    )
    .await;
    assert_eq!(
        commands["payload"]["commands"],
        json!([{"name":"review","description":"Review a change","argumentHint":"<pr>"}]),
        "{commands}"
    );
    // A new conversation's, by its provider and folder, answered as its key's.
    let draft = ask(
        &mut socket,
        json!({"type":"list_commands_request","agentId":"draft:claude","provider":"claude","cwd":service.0.path()}),
    )
    .await;
    assert_eq!(draft["payload"]["agentId"], "draft:claude", "{draft}");
    assert_eq!(draft["payload"]["commands"], commands["payload"]["commands"], "{draft}");

    ask(
        &mut socket,
        json!({"type":"send_agent_message_request","agentId":agent,"text":"go"}),
    )
    .await;
    let asked = until(&mut socket, "permission_requested").await;
    let actions: Vec<_> = asked["request"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|action| action["id"].clone())
        .collect();
    assert_eq!(actions, ["allow", "allow_always", "deny"]);
    send(&mut socket, json!({"type":"session","message":{"type":"agent_permission_response","agentId":agent,
        "requestId":asked["request"]["id"],"response":{"behavior":"allow","selectedActionId":"allow_always"}}})).await;
    finished(&mut socket).await;
    assert!(service.0.path().join("kept-rule").exists());

    let started = ask(
        &mut socket,
        json!({"type":"agent.provider_subagents.list.request","parentAgentId":agent}),
    )
    .await;
    assert_eq!(
        started["payload"]["subagents"],
        json!([{"id":"task1","parentAgentId":agent,
        "status":"completed","title":"Look around","subtitle":"Explore"}]),
        "{started}"
    );
    let theirs = ask(
        &mut socket,
        json!({"type":"agent.provider_subagents.timeline.get.request",
        "parentAgentId":agent,"subagentId":"task1","direction":"tail","limit":300}),
    )
    .await;
    let rows = theirs["payload"]["rows"].as_array().unwrap();
    assert_eq!(
        rows[0]["item"],
        json!({"type":"assistant_message","text":"Searching"}),
        "{theirs}"
    );
    assert_eq!(
        (
            &rows[1]["item"]["status"],
            &rows[1]["item"]["detail"]["output"]
        ),
        (&json!("completed"), &json!("found"))
    );
    // Here, only the call that started it.
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let rows = timeline["payload"]["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "{timeline}");
    assert_eq!(
        (
            &rows[1]["item"]["detail"]["type"],
            &rows[1]["item"]["status"]
        ),
        (&json!("sub_agent"), &json!("completed"))
    );
}

#[tokio::test]
async fn stop_asked_twice_ends_a_turn_that_would_not_stop() {
    let service = Service(tempfile::tempdir().unwrap());
    install(
        service.0.path(),
        "claude",
        r#"#!/bin/sh
while IFS= read -r line; do
    case "$line" in *'"type":"user"'*)
        echo '{"type":"system","subtype":"session_state_changed","state":"running"}'
        echo '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m","content":[{"type":"tool_use","id":"t","name":"Bash","input":{"command":"sleep 99"}}]}}' ;;
    esac
done
"#,
    );
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(
        &mut socket,
        json!({"type":"workspace.create.request",
        "source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"claude"},"initialPrompt":"go"}}),
    )
    .await;
    let agent = created["payload"]["agent"]["id"].clone();
    loop {
        let frame = read(&mut socket).await;
        if frame["message"]["payload"]["event"]["item"]["type"] == "tool_call" {
            break;
        }
    }
    ask(
        &mut socket,
        json!({"type":"cancel_agent_request","agentId":agent}),
    )
    .await;
    let asked = ask(
        &mut socket,
        json!({"type":"fetch_agent_request","agentId":agent}),
    )
    .await;
    assert_eq!(asked["payload"]["agent"]["status"], "running");
    ask(
        &mut socket,
        json!({"type":"cancel_agent_request","agentId":agent}),
    )
    .await;
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    assert_eq!(timeline["payload"]["agent"]["status"], "idle");
    let rows = timeline["payload"]["entries"].as_array().unwrap();
    assert_eq!(
        rows.last().unwrap()["item"]["status"],
        "canceled",
        "{timeline}"
    );
}

#[tokio::test]
async fn a_cli_that_cannot_run_ends_the_turn_with_why() {
    let service = Service(tempfile::tempdir().unwrap());
    install(
        service.0.path(),
        "claude",
        "#!/bin/sh\necho 'not signed in' >&2\nexit 7\n",
    );
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(
        &mut socket,
        json!({"type":"workspace.create.request",
        "source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"claude"},"initialPrompt":"go"}}),
    )
    .await;
    let agent = &created["payload"]["agent"]["id"];
    loop {
        let frame = read(&mut socket).await;
        if frame["message"]["payload"]["event"]["type"] == "turn_failed" {
            break;
        }
    }
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let last = timeline["payload"]["entries"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()
        .clone();
    assert_eq!(last["item"]["type"], "error");
    let why = last["item"]["message"].as_str().unwrap();
    assert!(
        why.contains("exit status: 7") && why.contains("not signed in"),
        "{why}"
    );
    assert_eq!(timeline["payload"]["agent"]["status"], "error");
}

#[tokio::test]
async fn authenticated_conversation_survives_service_restart() {
    let service = Service(tempfile::tempdir().unwrap());
    install_provider(service.0.path());
    service.cli("start");
    service.cli("start");
    drop(service.connect(false).await);
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket,json!({"type":"workspace.create.request","source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"codex"},"initialPrompt":"first","clientMessageId":"m1"}})).await;
    assert_eq!(created["type"], "workspace.create.response");
    let agent = &created["payload"]["agent"]["id"];
    finished(&mut socket).await;
    service.cli("stop");
    service.cli("start");
    let mut socket = service.connect(true).await;
    let mut request = json!({"type":"send_agent_message_request","agentId":agent,"text":"second","messageId":" m2 "});
    let sent = ask(&mut socket, request.clone()).await;
    assert_eq!(sent["type"], "send_agent_message_response");
    assert_eq!(sent["payload"]["messageId"], "m2");
    assert_eq!(sent["payload"]["seq"], 4);
    finished(&mut socket).await;
    service.cli("stop");
    service.cli("start");
    let mut socket = service.connect(true).await;
    request["messageId"] = json!("m2");
    let retry = ask(&mut socket, request).await;
    assert_eq!(retry["type"], "send_agent_message_response");
    assert_eq!(retry["payload"]["messageId"], sent["payload"]["messageId"]);
    assert_eq!(retry["payload"]["seq"], sent["payload"]["seq"]);
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    assert_eq!(timeline["payload"]["entries"].as_array().unwrap().len(), 4);
    assert_eq!(timeline["payload"]["entries"][2]["item"]["text"], "second");
    assert_eq!(
        without_time(&timeline["payload"]["entries"][3]["item"]),
        json!({"type":"assistant_message","text":"reply"})
    );
    assert_eq!(
        timeline["payload"]["agent"]["persistence"]["sessionId"],
        "session-123"
    );
    assert_eq!(
        timeline["payload"]["agent"]["lastUsage"]["contextWindowUsedTokens"],
        42
    );
    let unknown = ask(&mut socket, json!({"type":"unknown.request"})).await;
    assert_eq!(unknown["type"], "rpc_error");
}

#[tokio::test]
async fn terminal_replay_and_file_conflicts_use_the_workspace() {
    let service = Service(tempfile::tempdir().unwrap());
    let project = tempfile::tempdir().unwrap();
    let cwd = fs::canonicalize(project.path()).unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket,json!({"type":"workspace.create.request","source":{"kind":"directory","path":cwd},"agent":{"config":{"provider":"codex"}}})).await;
    assert_eq!(created["type"], "workspace.create.response");
    fs::write(cwd.join("test.txt"), "original").unwrap();
    let file = ask(
        &mut socket,
        json!({"type":"file_explorer_request","cwd":cwd,"path":"test.txt","mode":"file"}),
    )
    .await;
    assert_eq!(file["payload"]["file"]["content"], "original");
    let conflict = ask(&mut socket,json!({"type":"fs.file.write.request","cwd":cwd,"path":"test.txt","content":"changed","expectedRevision":"stale"})).await;
    assert_eq!(conflict["payload"]["result"]["status"], "conflict");
    assert_eq!(
        fs::read_to_string(cwd.join("test.txt")).unwrap(),
        "original"
    );
    std::os::unix::fs::symlink(service.0.path(), cwd.join("outside")).unwrap();
    let denied = ask(&mut socket,json!({"type":"file_explorer_request","cwd":cwd,"path":"outside/local-credential","mode":"file"})).await;
    assert!(denied["payload"]["error"].is_string());
    let created = ask(
        &mut socket,
        json!({"type":"create_terminal_request","cwd":cwd}),
    )
    .await;
    let terminal = created["payload"]["terminal"]["id"].as_str().unwrap();
    let attached = ask(
        &mut socket,
        json!({"type":"subscribe_terminal_request","terminalId":terminal}),
    )
    .await;
    assert_eq!(attached["type"], "subscribe_terminal_response");
    let replay = socket.next().await.unwrap().unwrap().into_data();
    assert_eq!(replay[0], 5);
    send(&mut socket,json!({"type":"session","message":{"type":"terminal_input","terminalId":terminal,
        "message":{"type":"input","data":"printf '\\123\\117\\120\\110\\117\\116\\137\\120\\124\\131\\n'\n"}}})).await;
    let mut text = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !text.contains("SOPHON_PTY") {
            let frame = socket.next().await.unwrap().unwrap();
            if let Message::Binary(bytes) = frame {
                text.push_str(&String::from_utf8_lossy(&bytes[2..]));
            }
        }
    })
    .await
    .expect("shell output");
    // `sophon` is there to type: this service's, for a plugin's setup.
    send(
        &mut socket,
        json!({"type":"session","message":{"type":"terminal_input","terminalId":terminal,
        "message":{"type":"input","data":"sophon plugin list; echo SOPHON_CLI_$?\n"}}}),
    )
    .await;
    let mut text = String::new();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !text.contains("SOPHON_CLI_0") {
            let frame = socket.next().await.unwrap().unwrap();
            if let Message::Binary(bytes) = frame {
                text.push_str(&String::from_utf8_lossy(&bytes[2..]));
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("sophon in a terminal: {text}"));
}

#[tokio::test]
async fn the_workspace_itself_is_not_an_entry_to_delete() {
    let service = Service(tempfile::tempdir().unwrap());
    let project = tempfile::tempdir().unwrap();
    let cwd = fs::canonicalize(project.path()).unwrap();
    fs::write(cwd.join("kept.txt"), "kept").unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":cwd},"agent":{"config":{"provider":"codex"}}})).await;
    for path in [json!(null), json!("."), json!("./")] {
        let reply = ask(
            &mut socket,
            json!({"type":"fs.entry.delete.request","cwd":cwd,"path":path}),
        )
        .await;
        assert!(reply["payload"]["error"].is_string(), "{reply}");
        assert_eq!(fs::read_to_string(cwd.join("kept.txt")).unwrap(), "kept");
    }
    let renamed = ask(
        &mut socket,
        json!({"type":"fs.entry.rename.request","cwd":cwd,"path":"kept.txt","name":"KEPT.txt"}),
    )
    .await;
    assert!(renamed["payload"]["error"].is_null(), "{renamed}");
    assert!(
        fs::read_dir(&cwd)
            .unwrap()
            .any(|entry| entry.unwrap().file_name() == "KEPT.txt")
    );
    let gone = ask(
        &mut socket,
        json!({"type":"fs.entry.delete.request","cwd":cwd,"path":"KEPT.txt"}),
    )
    .await;
    assert!(gone["payload"]["error"].is_null(), "{gone}");
    assert_eq!(fs::read_dir(&cwd).unwrap().count(), 0);
}

/// How many terminals the workspace at `cwd` has.
async fn terminals(socket: &mut Socket, cwd: &Path) -> usize {
    let list = ask(
        socket,
        json!({"type":"subscribe_terminals_request","cwd":cwd}),
    )
    .await;
    list["payload"]["terminals"].as_array().unwrap().len()
}

#[tokio::test]
async fn a_terminal_lasts_as_long_as_its_shell() {
    let service = Service(tempfile::tempdir().unwrap());
    let project = tempfile::tempdir().unwrap();
    let cwd = fs::canonicalize(project.path()).unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":cwd},"agent":{"config":{"provider":"codex"}}})).await;
    // Its shell gone, with a job of its own still holding the pty.
    let created = ask(
        &mut socket,
        json!({"type":"create_terminal_request","cwd":cwd}),
    )
    .await;
    let terminal = created["payload"]["terminal"]["id"].clone();
    send(
        &mut socket,
        json!({"type":"session","message":{"type":"terminal_input","terminalId":terminal,
        "message":{"type":"input","data":"nohup sleep 30 >/dev/null 2>&1 &\ndisown\nexit\n"}}}),
    )
    .await;
    let mut left = 1;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        left = terminals(&mut socket, &cwd).await;
        if left == 0 {
            break;
        }
    }
    assert_eq!(left, 0, "the terminal outlived its shell");
    // Ended when asked, then and there.
    let created = ask(
        &mut socket,
        json!({"type":"create_terminal_request","cwd":cwd}),
    )
    .await;
    assert_eq!(terminals(&mut socket, &cwd).await, 1);
    ask(
        &mut socket,
        json!({"type":"kill_terminal_request","terminalId":created["payload"]["terminal"]["id"]}),
    )
    .await;
    assert_eq!(terminals(&mut socket, &cwd).await, 0);
}

fn git(dir: &Path, args: &[&str]) {
    let result = Command::new("git")
        .current_dir(dir)
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[tokio::test]
async fn a_change_too_large_to_read_is_listed_with_its_counts() {
    let service = Service(tempfile::tempdir().unwrap());
    let project = tempfile::tempdir().unwrap();
    let cwd = fs::canonicalize(project.path()).unwrap();
    git(&cwd, &["init", "-b", "main"]);
    fs::write(cwd.join("small.txt"), "one\n").unwrap();
    fs::write(cwd.join("large.txt"), "").unwrap();
    fs::write(cwd.join("image.bin"), [0u8, 1, 2]).unwrap();
    git(&cwd, &["add", "."]);
    git(&cwd, &["commit", "-m", "initial"]);
    fs::write(cwd.join("small.txt"), "two\n").unwrap();
    fs::write(cwd.join("large.txt"), "line\n".repeat(25_000)).unwrap();
    fs::write(cwd.join("image.bin"), [0u8, 9, 9, 0]).unwrap();
    fs::write(cwd.join("huge-new.txt"), "x".repeat(600_000)).unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":cwd},"agent":{"config":{"provider":"codex"}}})).await;
    let diff = ask(
        &mut socket,
        json!({"type":"subscribe_checkout_diff_request","cwd":cwd}),
    )
    .await;
    let file = |path: &str| {
        diff["payload"]["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["path"] == path)
            .unwrap_or_else(|| panic!("{path} in {diff}"))
            .clone()
    };
    let small = file("small.txt");
    assert_eq!(
        (&small["additions"], &small["deletions"]),
        (&json!(1), &json!(1))
    );
    assert_eq!(
        small["hunks"][0]["lines"][1],
        json!({"type":"add","content":"two"})
    );
    let large = file("large.txt");
    assert_eq!(
        (&large["status"], &large["additions"], &large["hunks"]),
        (&json!("too_large"), &json!(25_000), &json!([]))
    );
    assert_eq!(file("image.bin")["status"], "binary");
    assert_eq!(file("huge-new.txt")["status"], "too_large");
}

#[test]
fn import_preserves_ids_and_refuses_to_overwrite_existing_state() {
    let legacy = tempfile::tempdir().unwrap();
    let service = tempfile::tempdir().unwrap();
    fs::create_dir(legacy.path().join("agents")).unwrap();
    let original = json!({"id":"a1","provider":"codex","cwd":"/old/project","persistence":{"sessionId":"thread-1"}});
    let path = legacy.path().join("agents/a1.json");
    fs::write(&path, original.to_string()).unwrap();
    let import = || {
        Command::new(env!("CARGO_BIN_EXE_sophon-service"))
            .arg("import")
            .arg(legacy.path())
            .arg("--home")
            .arg(service.path())
            .output()
            .unwrap()
    };
    assert!(import().status.success());
    let saved = fs::read(service.path().join("state.json")).unwrap();
    let state: Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(
        state.pointer("/agents/a1/persistence/sessionId").unwrap(),
        "thread-1"
    );
    assert!(!import().status.success());
    assert_eq!(fs::read(service.path().join("state.json")).unwrap(), saved);
    assert_eq!(fs::read_to_string(path).unwrap(), original.to_string());
}

#[tokio::test]
async fn worktree_retry_reuses_agent_and_git_reads_the_new_checkout() {
    let service = Service(tempfile::tempdir().unwrap());
    let project = tempfile::tempdir().unwrap();
    for args in [
        vec!["init", "-b", "main"],
        vec![
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            "initial",
        ],
    ] {
        let result = Command::new("git")
            .current_dir(project.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    service.cli("start");
    let mut socket = service.connect(true).await;
    let request = json!({"type":"workspace.create.request","idempotencyKey":"request-1",
        "source":{"kind":"worktree","branchName":"feature","baseBranch":"main","cwd":project.path()},
        "agent":{"config":{"provider":"codex"}}});
    let first = ask(&mut socket, request.clone()).await;
    assert_eq!(first["type"], "workspace.create.response", "{first}");
    let second = ask(&mut socket, request).await;
    assert_eq!(
        first["payload"]["agent"]["id"],
        second["payload"]["agent"]["id"]
    );
    let cwd = first["payload"]["workspace"]["workspaceDirectory"]
        .as_str()
        .unwrap();
    let status = ask(
        &mut socket,
        json!({"type":"checkout_status_request","cwd":cwd}),
    )
    .await;
    assert_eq!(status["payload"]["currentBranch"], "feature");
    fs::write(Path::new(cwd).join("new-file"), "untracked\n").unwrap();
    let diff = ask(
        &mut socket,
        json!({"type":"subscribe_checkout_diff_request","cwd":cwd}),
    )
    .await;
    assert_eq!(
        diff["payload"]["files"][0]["hunks"][0]["lines"][0]["content"],
        "untracked"
    );
}

#[tokio::test]
async fn paused_schedule_survives_restart_and_manual_run_is_logged() {
    let service = Service(tempfile::tempdir().unwrap());
    install_provider(service.0.path());
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket,json!({"type":"schedule/create","name":"Morning","prompt":"first",
        "cadence":{"type":"cron","expression":"15 9 * * 1-5","timezone":"Asia/Singapore"},
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":service.0.path(),"isolation":"local"}}})).await;
    assert_eq!(created["type"], "schedule/create/response", "{created}");
    let id = &created["payload"]["schedule"]["id"];
    let paused = ask(
        &mut socket,
        json!({"type":"schedule/pause","scheduleId":id}),
    )
    .await;
    assert_eq!(paused["payload"]["schedule"]["status"], "paused");
    service.cli("stop");
    service.cli("start");
    let mut socket = service.connect(true).await;
    let list = ask(&mut socket, json!({"type":"schedule/list"})).await;
    assert_eq!(list["payload"]["schedules"][0]["status"], "paused");
    let run = ask(
        &mut socket,
        json!({"type":"schedule/run-once","scheduleId":id}),
    )
    .await;
    assert_eq!(run["type"], "schedule/run-once/response");
    finished(&mut socket).await;
    let logs = ask(&mut socket, json!({"type":"schedule/logs","scheduleId":id})).await;
    assert_eq!(logs["payload"]["runs"][0]["status"], "completed");
    assert_eq!(logs["payload"]["runs"][0]["output"], "reply");
}

/// Reads `path` until it has `wanted` in it, and answers all of it.
async fn file_with(path: &Path, wanted: &str) -> String {
    for _ in 0..100 {
        let text = fs::read_to_string(path).unwrap_or_default();
        if text.contains(wanted) {
            return text;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "{} never had {wanted:?}: {:?}",
        path.display(),
        fs::read_to_string(path).unwrap_or_default()
    );
}

#[tokio::test]
async fn a_probe_s_new_event_starts_a_run_that_answers_who_caused_it() {
    let service = Service(tempfile::tempdir().unwrap());
    let work = tempfile::tempdir().unwrap();
    let work = work.path().canonicalize().unwrap();
    // A `codex` that also keeps what it was asked and the token of its run.
    install_provider(service.0.path());
    let cli = service.0.path().join("codex");
    let script = fs::read_to_string(&cli).unwrap().replace(
        "*'\"method\":\"turn/start\"'*)",
        "*'\"method\":\"turn/start\"'*)\n            printf '%s\\n' \"$line\" >> \"$PWD/asked\"\n            printf '%s' \"$SOPHON_RUN_TOKEN\" > \"$PWD/token\"\n            command -v sophon > \"$PWD/sophon\"",
    );
    assert!(script.contains("SOPHON_RUN_TOKEN"));
    fs::write(&cli, script).unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    // What was there before the automation is not its business.
    fs::write(
        work.join("events"),
        "{\"id\":\"old\",\"vars\":{\"url\":\"https://x/pull/1\"}}\n",
    )
    .unwrap();
    let created = ask(&mut socket,json!({"type":"schedule/create","name":"Reviews","prompt":"Review {{url}}",
        "cadence":{"type":"every","everyMs":300},
        "probe":"cat events","reply":"printf '%s|%s|%s\\n' \"$SOPHON_ACTION\" \"$SOPHON_EVENT_ID\" \"$SOPHON_TEXT\" >> said",
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":work,"isolation":"local"}}})).await;
    assert_eq!(created["type"], "schedule/create/response", "{created}");
    let id = created["payload"]["schedule"]["id"].clone();
    for _ in 0..100 {
        let list = ask(&mut socket, json!({"type":"schedule/list"})).await;
        if list["payload"]["schedules"][0]["primed"] == true {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let logs = ask(&mut socket, json!({"type":"schedule/logs","scheduleId":id})).await;
    assert_eq!(logs["payload"]["runs"], json!([]), "{logs}");

    // A new one starts a run, once, with what it carries in the prompt.
    let mut events = fs::OpenOptions::new()
        .append(true)
        .open(work.join("events"))
        .unwrap();
    std::io::Write::write_all(
        &mut events,
        b"{\"id\":\"new\",\"vars\":{\"url\":\"https://x/pull/2\"}}\n",
    )
    .unwrap();
    finished(&mut socket).await;
    let asked = file_with(&work.join("asked"), "pull/2").await;
    assert!(!asked.contains("pull/1"), "{asked}");
    assert!(asked.contains("sophon reply"), "{asked}");
    assert_eq!(
        fs::read_to_string(work.join("sophon")).unwrap().trim(),
        service.0.path().join("bin/sophon").to_str().unwrap()
    );
    let said = file_with(&work.join("said"), "done|new|reply").await;
    assert!(said.starts_with("ack|new|\n"), "{said}");
    tokio::time::sleep(Duration::from_millis(700)).await;
    let logs = ask(&mut socket, json!({"type":"schedule/logs","scheduleId":id})).await;
    let runs = logs["payload"]["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 1, "{logs}");
    assert_eq!(runs[0]["status"], "completed");
    assert_eq!(runs[0]["event"]["id"], "new");
    assert!(runs[0]["token"].is_null(), "{logs}");

    // Its agent answers through `sophon`, with the token it was given.
    let token = fs::read_to_string(work.join("token")).unwrap();
    assert!(!token.is_empty());
    let sophon = |args: &[&str], token: Option<&str>| {
        let mut command = Command::new(service.0.path().join("bin/sophon"));
        command
            .args(args)
            .env("SOPHON_SERVICE_HOME", service.0.path())
            .env_remove("SOPHON_RUN_TOKEN");
        if let Some(token) = token {
            command.env("SOPHON_RUN_TOKEN", token);
        }
        let output = command.output().unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned()
                + &String::from_utf8_lossy(&output.stderr),
        )
    };
    assert_eq!(
        sophon(&["reply", "looks", "good"], Some(&token)),
        (true, "Sent\n".into())
    );
    assert_eq!(
        sophon(&["react", "DONE"], Some(&token)),
        (true, "Sent\n".into())
    );
    let said = file_with(&work.join("said"), "react|new|DONE").await;
    assert!(said.contains("reply|new|looks good\n"), "{said}");
    assert!(!sophon(&["reply", "hello"], Some("not-a-token")).0);
    assert!(!sophon(&["reply", "hello"], None).0);
    let logs = ask(&mut socket, json!({"type":"schedule/logs","scheduleId":id})).await;
    assert_eq!(
        logs["payload"]["runs"][0]["replies"][0]["text"], "looks good",
        "{logs}"
    );

    // Another program causes a run by name; the same event twice is one.
    let (ok, out) = sophon(
        &[
            "trigger",
            "Reviews",
            "--id",
            "pushed",
            "--var",
            "url=https://x/pull/9",
        ],
        None,
    );
    assert!(ok && out.starts_with("Started run "), "{out}");
    finished(&mut socket).await;
    file_with(&work.join("asked"), "pull/9").await;
    assert_eq!(
        sophon(&["trigger", "Reviews", "--id", "pushed"], None),
        (true, "Already had this event\n".into())
    );
    assert!(!sophon(&["trigger", "Nothing"], None).0);
    let logs = ask(&mut socket, json!({"type":"schedule/logs","scheduleId":id})).await;
    assert_eq!(
        logs["payload"]["runs"].as_array().unwrap().len(),
        2,
        "{logs}"
    );
}

/// The integration or plugin `id` in an answer that lists them.
fn installed<'a>(answer: &'a Value, id: &str) -> &'a Value {
    answer["payload"]["plugins"]
        .as_array()
        .unwrap()
        .iter()
        .find(|plugin| plugin["id"] == id)
        .unwrap_or_else(|| panic!("No plugin {id}: {answer}"))
}

#[tokio::test]
async fn an_integration_starts_what_a_plugin_s_automation_does() {
    let service = Service(tempfile::tempdir().unwrap());
    let work = tempfile::tempdir().unwrap();
    let work = work.path().canonicalize().unwrap();
    install_provider(service.0.path());
    // One that comes with the service: a folder of them it is told of.
    let shipped = tempfile::tempdir().unwrap();
    let shipped = shipped.path().canonicalize().unwrap();
    fs::create_dir(shipped.join("builtin")).unwrap();
    install(
        &shipped.join("builtin"),
        "look",
        "#!/bin/sh\nprintf '%s|%s\\n' \"$SOPHON_PARAM_LOUD\" \"$SOPHON_PARAM_TOKEN\" > \"$SOPHON_PROJECT_DIR/builtin-saw\"\n",
    );
    fs::write(manifest(&shipped.join("builtin"), "integration"), r#"{"name":"Built in",
        "params":[{"name":"token","label":"Token","value":"kept"}],
        "triggers":[{"id":"look","label":"Something in the folder","probe":"./look","everyMs":3600000,
            "params":[{"name":"loud","label":"Loud","type":"switch","value":false}]}]}"#).unwrap();
    service.cli_with_env(
        "start",
        &[("SOPHON_BUNDLED_PLUGINS", shipped.to_str().unwrap())],
    );
    let mut socket = service.connect(true).await;
    let sophon = |args: &[&str]| {
        let output = Command::new(service.0.path().join("bin/sophon"))
            .args(args)
            .env("SOPHON_SERVICE_HOME", service.0.path())
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned()
                + &String::from_utf8_lossy(&output.stderr),
        )
    };
    let author = tempfile::tempdir().unwrap();
    let author = author.path().canonicalize().unwrap();
    // An integration: a manifest beside the scripts it names. It signs in,
    // and says what it watches for; what to do then is not its to say.
    let hub = author.join("hub");
    fs::create_dir(&hub).unwrap();
    install(
        &hub,
        "probe",
        "#!/bin/sh\nprintf '%s' \"$SOPHON_PARAM_ONLY\" > \"$SOPHON_PLUGIN_DATA/only\"\ncat \"$SOPHON_PROJECT_DIR/events\"\n",
    );
    install(
        &hub,
        "login",
        "#!/bin/sh\nprintf '%s' \"${1:-from the app}\" > \"$SOPHON_PLUGIN_DATA/signed-in\"\necho signed in\n",
    );
    install(
        &hub,
        "whoami",
        "#!/bin/sh\ncat \"$SOPHON_PLUGIN_DATA/signed-in\" 2>/dev/null || { echo 'not signed in' >&2; exit 1; }\n",
    );
    // How it answers for an automation that has no way of its own: through
    // another integration, which the service runs as the one it is.
    install(
        &hub,
        "thank",
        "#!/bin/sh\n[ \"$SOPHON_ACTION\" = done ] && sophon use chat \"about $SOPHON_VAR_URL\" \"$SOPHON_TEXT\"\nexit 0\n",
    );
    fs::write(manifest(&hub, "integration"), r#"{"id":"hub","name":"Hub","description":"Where the pull requests are","setup":"./login","status":"./whoami",
        "params":[{"name":"app_id","label":"App ID","value":"cli_shared"},{"name":"app_secret","label":"App Secret","value":"s3cret","secret":true},{"name":"repos","label":"Repositories"},{"name":"domain","label":"Domain","optional":true}],
        "triggers":[{"id":"quiet","label":"Nothing ever","probe":"true"},
            {"id":"sent","label":"Someone sends a pull request","probe":"./probe","reply":"./thank","everyMs":300,
                "params":[{"name":"only","label":"Only these","optional":true}]}]}"#).unwrap();
    assert!(
        !sophon(&["plugin", "add", work.to_str().unwrap()]).0,
        "a folder without a manifest is neither"
    );
    let (ok, out) = sophon(&["plugin", "add", hub.to_str().unwrap()]);
    // What its author filled in comes with it; what is left is asked for.
    assert_eq!(
        out,
        "Installed Hub (hub)\nNeeds Repositories: sophon plugin set hub repos=…\nSet it up once: sophon plugin setup hub\n"
    );
    assert!(ok);
    assert_eq!(
        sophon(&["integration", "set", "hub", "repos=acme/*"]),
        (true, "Set\n".into())
    );
    assert!(!sophon(&["plugin", "set", "hub", "nothing=1"]).0);
    assert!(
        !sophon(&["plugin", "set", "hub", "only=x"]).0,
        "what a trigger asks is each automation's to say"
    );
    // Its setup from a client has no terminal: it is run, and how it
    // stands is said with the list.
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    assert!(installed(&listed, "hub")["account"].is_null(), "{listed}");
    let set_up = ask(&mut socket, json!({"type":"plugin/setup","pluginId":"hub"})).await;
    assert_eq!(set_up["payload"]["said"], "signed in", "{set_up}");
    assert_eq!(installed(&set_up, "hub")["account"], "from the app");
    let (ok, out) = sophon(&["plugin", "setup", "hub", "ada"]);
    assert!(ok, "{out}");
    let kept = service.0.path().join("plugin-data/hub");
    assert_eq!(fs::read_to_string(kept.join("signed-in")).unwrap(), "ada");
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    let entry = installed(&listed, "hub");
    assert_eq!(
        (&entry["account"], &entry["uses"]),
        (&json!("ada"), &json!(0)),
        "{listed}"
    );
    assert_eq!(
        (&entry["kind"], &entry["source"]),
        (&json!("integration"), &json!("folder")),
        "{listed}"
    );
    assert!(entry["automations"].is_null(), "{listed}");
    // What it watches for has a name; its probe, and how it answers, are
    // not a client's to read.
    assert_eq!(
        entry["triggers"][1],
        json!({"id":"sent","label":"Someone sends a pull request","description":null,"everyMs":300,"replies":true,
            "params":[{"name":"only","label":"Only these","hint":null,"type":"text","options":null,"secret":false,"optional":true,"value":"","dynamic":false}]}),
        "{listed}"
    );
    assert_eq!(entry["triggers"][0]["replies"], false);
    // A secret is said to be there, and not what it is.
    let secret = &entry["params"][1];
    assert_eq!(
        (
            &secret["name"],
            &secret["secret"],
            &secret["filled"],
            &secret["value"]
        ),
        (
            &json!("app_secret"),
            &json!(true),
            &json!(true),
            &Value::Null
        ),
        "{listed}"
    );
    assert_eq!(entry["params"][2]["value"], "acme/*");

    // A plugin: what to do when that trigger fires, packed to hand on. It
    // answers its own way, in its own folder, with what is its own.
    let plugin = author.join("watch");
    fs::create_dir(&plugin).unwrap();
    // Its automation is a folder of its own: what it is told, and the rest.
    let review = plugin.join("automations/review");
    fs::create_dir_all(&review).unwrap();
    fs::write(review.join("prompt.md"), "Review {{url}}. {{tone}}\n").unwrap();
    install(
        &plugin,
        "reply",
        "#!/bin/sh\nprintf '%s|%s|%s\\n' \"$SOPHON_ACTION\" \"$SOPHON_VAR_URL\" \"$PWD\" >> \"$SOPHON_PLUGIN_DATA/said\"\nprintf '%s %s %s' \"$SOPHON_PARAM_PEOPLE\" \"$SOPHON_PARAM_TONE\" \"${SOPHON_PARAM_APP_SECRET:-none of the hub}\" > \"$SOPHON_PLUGIN_DATA/params\"\n",
    );
    fs::write(manifest(&plugin, "plugin"), r#"{"id":"watch","name":"Watch","description":"Reviews what it is sent","requires":["chat","mail"],
        "params":[{"name":"people","label":"People","value":"ada"}]}"#).unwrap();
    fs::write(review.join("automation.json"), r#"{"trigger":"hub/sent","reply":"./reply","modeId":"full-access","concurrency":2,
        "params":[{"name":"tone","label":"Kindly","type":"switch","value":true,"prompt":{"yes":"Be kind.","no":"Be brief."}}]}"#).unwrap();
    let (ok, out) = sophon(&["plugin", "add", plugin.to_str().unwrap()]);
    assert_eq!(out, "Installed Watch (watch)\n");
    assert!(ok);
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    let entry = installed(&listed, "watch");
    assert_eq!(entry["kind"], "plugin", "{listed}");
    assert!(entry["triggers"].is_null(), "{listed}");
    let offered = &entry["automations"][0];
    assert_eq!(offered["prompt"], "Review {{url}}. {{tone}}");
    assert_eq!(offered["trigger"], json!({"integration":"hub","id":"sent"}));
    assert_eq!(offered["replies"], true);
    assert_eq!(offered["params"][0]["name"], "tone");
    assert!(offered["params"][0]["prompt"].is_null(), "{listed}");
    // What it counts on, and how each of those stands.
    assert_eq!(
        entry["requires"],
        json!([{"id":"chat","name":"chat","installed":false,"ready":false},{"id":"mail","name":"mail","installed":false,"ready":false},
            {"id":"hub","name":"Hub","installed":true,"ready":true}]),
        "{listed}"
    );

    // An automation of it needs only where to work: the rest is the
    // plugin's, and what starts it the integration's.
    fs::write(work.join("events"), "").unwrap();
    let created = ask(&mut socket,json!({"type":"schedule/create","plugin":"watch","triggerOptions":{"only":"acme/app"},"options":{"tone":"no"},
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":work,"isolation":"local"}}})).await;
    assert_eq!(created["type"], "schedule/create/response", "{created}");
    let schedule = &created["payload"]["schedule"];
    assert_eq!(schedule["name"], "Watch");
    assert_eq!(schedule["prompt"], "Review {{url}}. {{tone}}");
    assert_eq!(schedule["template"], "review");
    assert_eq!(
        (&schedule["integration"], &schedule["trigger"]),
        (&json!("hub"), &json!("sent")),
        "{schedule}"
    );
    assert_eq!(schedule["cadence"], json!({"type":"every","everyMs":300}));
    assert_eq!(schedule["target"]["config"]["modeId"], "full-access");
    assert!(
        schedule["probe"].is_null(),
        "the scripts stay whose they are: {schedule}"
    );
    let id = schedule["id"].clone();
    for _ in 0..100 {
        let list = ask(&mut socket, json!({"type":"schedule/list"})).await;
        if list["payload"]["schedules"][0]["primed"] == true {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    fs::write(
        work.join("events"),
        "{\"id\":\"m1\",\"vars\":{\"url\":\"https://x/pull/7\"}}\n",
    )
    .unwrap();
    finished(&mut socket).await;
    let data = service.0.path().join("plugin-data/watch");
    let said = file_with(&data.join("said"), "done|").await;
    let folder = plugin.to_str().unwrap();
    // Picked up and done are each said once (two scripts: in either order),
    // by the plugin's script in the plugin's folder.
    assert!(
        said.contains(&format!("ack|https://x/pull/7|{folder}\n")),
        "{said}"
    );
    let logs = ask(&mut socket, json!({"type":"schedule/logs","scheduleId":id})).await;
    assert_eq!(logs["payload"]["runs"][0]["event"]["id"], "m1", "{logs}");
    // It is told its own settings and this automation's choice, and
    // nothing of the integration's; the probe is told the trigger's.
    assert_eq!(
        fs::read_to_string(data.join("params")).unwrap(),
        "ada no none of the hub"
    );
    assert_eq!(fs::read_to_string(kept.join("only")).unwrap(), "acme/app");
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    assert_eq!(installed(&listed, "hub")["uses"], 1, "{listed}");
    assert_eq!(installed(&listed, "watch")["active"], 1, "{listed}");

    // In use either stays; the automation gone, each goes, and what it
    // kept with it.
    for used in ["hub", "watch"] {
        let (ok, out) = sophon(&["plugin", "remove", used]);
        assert!(!ok && out.contains("automations use this"), "{out}");
    }
    ask(
        &mut socket,
        json!({"type":"schedule/delete","scheduleId":id}),
    )
    .await;
    assert!(sophon(&["plugin", "remove", "watch"]).0);
    assert!(
        plugin.join("reply").exists(),
        "a linked folder is its author's"
    );
    assert!(!data.exists(), "what it kept goes with it");

    // An integration that is a connection and nothing else: a command to
    // use. Another's script calls it through the service, which runs it
    // as the one it is.
    let chat = author.join("chat");
    fs::create_dir(&chat).unwrap();
    install(
        &chat,
        "say",
        "#!/bin/sh\nprintf '%s|%s|%s\\n' \"$1\" \"$2\" \"$SOPHON_PARAM_LOUD\" >> \"$SOPHON_PLUGIN_DATA/sent\"\necho sent\n[ \"$1\" != fail ] || { echo refused >&2; exit 7; }\n",
    );
    install(
        &chat,
        "rooms",
        "#!/bin/sh\necho general\necho '{\"value\":\"ops\",\"label\":\"Operations\"}'\n",
    );
    fs::write(manifest(&chat, "integration"), r#"{"id":"chat","name":"Chat","use":"./say",
        "params":[{"name":"loud","label":"Loud","type":"switch","value":true},{"name":"rooms","label":"Rooms","type":"multi","optional":true,"optionsFrom":"./rooms"}]}"#).unwrap();
    assert!(sophon(&["plugin", "add", chat.to_str().unwrap()]).0);
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    let entry = installed(&listed, "chat");
    assert_eq!(
        (&entry["id"], &entry["triggers"]),
        (&json!("chat"), &json!([])),
        "{listed}"
    );
    assert_eq!(entry["params"][0]["type"], "switch");
    assert_eq!(entry["params"][0]["value"], "yes");
    assert_eq!(entry["params"][1]["dynamic"], true);
    assert!(entry["params"][1]["optionsFrom"].is_null(), "{listed}");
    let options = ask(
        &mut socket,
        json!({"type":"plugin/options","pluginId":"chat","name":"rooms"}),
    )
    .await;
    assert_eq!(
        options["payload"]["options"],
        json!([{"value":"general","label":"general","hint":null},{"value":"ops","label":"Operations","hint":null}]),
        "{options}"
    );
    // A trigger's own choices are asked for by naming the trigger.
    let refused = ask(
        &mut socket,
        json!({"type":"plugin/options","pluginId":"hub","name":"only"}),
    )
    .await;
    assert_eq!(refused["type"], "rpc_error", "{refused}");
    let options = ask(
        &mut socket,
        json!({"type":"plugin/options","pluginId":"hub","trigger":"sent","name":"only"}),
    )
    .await;
    assert_eq!(options["payload"]["options"], json!([]), "{options}");
    assert_eq!(
        sophon(&["use", "chat", "to ada", "hello there"]),
        (true, "sent\n".into())
    );
    assert_eq!(
        fs::read_to_string(service.0.path().join("plugin-data/chat/sent")).unwrap(),
        "to ada|hello there|yes\n"
    );
    // Its failure is the caller's to hear, with what it said.
    let (ok, out) = sophon(&["use", "chat", "fail", "x"]);
    assert!(!ok && out.contains("refused"), "{out}");
    assert!(
        !sophon(&["use", "hub"]).0,
        "one that offers nothing cannot be used"
    );
    // The plugin back: what it counts on is there now, and ready.
    assert!(sophon(&["plugin", "add", plugin.to_str().unwrap()]).0);
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    let watch = installed(&listed, "watch");
    assert_eq!(
        watch["requires"][0],
        json!({"id":"chat","name":"Chat","installed":true,"ready":true}),
        "{listed}"
    );
    assert_eq!(watch["uses"], 0);
    fs::write(work.join("events"), "").unwrap();
    // An automation of one's own on a trigger: one's own words to the
    // agent, the integration's probe behind them, as often as that says,
    // and no plugin in it. The integration answers for it.
    let created = ask(&mut socket,json!({"type":"schedule/create","integration":"hub","trigger":"sent","prompt":"Look at {{url}} for me",
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":work,"isolation":"local"}}})).await;
    assert_eq!(
        created["payload"]["schedule"]["cadence"],
        json!({"type":"every","everyMs":300}),
        "{created}"
    );
    assert_eq!(
        created["payload"]["schedule"]["prompt"],
        "Look at {{url}} for me"
    );
    assert!(
        created["payload"]["schedule"]["plugin"].is_null(),
        "{created}"
    );
    let id = created["payload"]["schedule"]["id"].clone();
    for _ in 0..100 {
        let list = ask(&mut socket, json!({"type":"schedule/list"})).await;
        if list["payload"]["schedules"][0]["primed"] == true {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    fs::write(
        work.join("events"),
        "{\"id\":\"m2\",\"vars\":{\"url\":\"https://x/pull/8\"}}\n",
    )
    .unwrap();
    finished(&mut socket).await;
    file_with(
        &service.0.path().join("plugin-data/chat/sent"),
        "about https://x/pull/8|reply|yes",
    )
    .await;
    assert!(
        !service.0.path().join("plugin-data/watch/said").exists(),
        "the plugin has no part in an automation not made from it"
    );
    ask(
        &mut socket,
        json!({"type":"schedule/delete","scheduleId":id}),
    )
    .await;
    // Neither is what the other is asked for as.
    for wrong in [
        json!({"plugin":"hub"}),
        json!({"integration":"watch"}),
        json!({"plugin":"missing"}),
        json!({"integration":"hub","trigger":"never"}),
        json!({"plugin":"watch","template":"never"}),
    ] {
        let mut request = json!({"type":"schedule/create","prompt":"Do it",
            "target":{"type":"new-agent","config":{"provider":"codex","cwd":work,"isolation":"local"}}});
        for (key, value) in wrong.as_object().unwrap() {
            request[key] = value.clone();
        }
        let refused = ask(&mut socket, request).await;
        assert_eq!(refused["type"], "rpc_error", "{wrong}: {refused}");
    }
    assert!(sophon(&["plugin", "remove", "watch"]).0);
    assert!(sophon(&["plugin", "remove", "chat"]).0);
    assert!(sophon(&["plugin", "remove", "hub"]).0);
    assert!(!kept.exists(), "what it kept goes with it");

    // Some come with Sophon: there without being installed, and not to be
    // removed. An automation on its trigger says its own for what the
    // trigger asks, and never for what is the integration's own.
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    let own = installed(&listed, "builtin");
    assert_eq!(
        (&own["source"], &own["name"], &own["kind"]),
        (&json!("builtin"), &json!("Built in"), &json!("integration")),
        "{listed}"
    );
    assert_eq!(own["triggers"][0]["params"][0]["name"], "loud");
    let created = ask(&mut socket,json!({"type":"schedule/create","integration":"builtin","name":"Watch the folder","prompt":"Look",
        "triggerOptions":{"loud":"yes","token":"not an automation's"},
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":work,"isolation":"local"}}})).await;
    assert_eq!(created["type"], "schedule/create/response", "{created}");
    assert_eq!(created["payload"]["schedule"]["trigger"], "look");
    let id = created["payload"]["schedule"]["id"].clone();
    ask(
        &mut socket,
        json!({"type":"schedule/run-once","scheduleId":id}),
    )
    .await;
    assert_eq!(
        file_with(&work.join("builtin-saw"), "|").await,
        "yes|kept\n"
    );
    // Turned off, it stays and nothing of it runs: its automation says why.
    let off = ask(
        &mut socket,
        json!({"type":"plugin/enable","pluginId":"builtin","enabled":false}),
    )
    .await;
    assert_eq!(installed(&off, "builtin")["enabled"], false, "{off}");
    let refused = ask(
        &mut socket,
        json!({"type":"schedule/run-once","scheduleId":id}),
    )
    .await;
    assert_eq!(refused["type"], "rpc_error", "{refused}");
    assert!(
        refused["payload"]["error"]
            .as_str()
            .unwrap()
            .starts_with("Built in is turned off"),
        "{refused}"
    );
    let on = ask(
        &mut socket,
        json!({"type":"plugin/enable","pluginId":"builtin","enabled":true}),
    )
    .await;
    assert_eq!(installed(&on, "builtin")["enabled"], true, "{on}");
    // In use it stays.
    assert!(!sophon(&["plugin", "remove", "builtin"]).0);
    ask(
        &mut socket,
        json!({"type":"schedule/delete","scheduleId":id}),
    )
    .await;

    // Its own settings are handed on as a line, and taken in from one: the
    // secret with them.
    assert!(sophon(&["plugin", "set", "builtin", "token=handed-on"]).0);
    let (ok, line) = sophon(&["plugin", "export", "builtin"]);
    assert!(ok && line.starts_with("sophon-integration:"), "{line}");
    assert!(sophon(&["plugin", "set", "builtin", "token=changed"]).0);
    assert_eq!(
        sophon(&["plugin", "import", line.trim()]),
        (true, "Set\n".into())
    );
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    assert_eq!(
        installed(&listed, "builtin")["params"][0]["value"],
        "handed-on",
        "{listed}"
    );
    assert!(!sophon(&["plugin", "import", "sophon-integration:bm90IGpzb24"]).0);
    assert!(!sophon(&["plugin", "import", "https://example.com/x.git"]).0);
    // No automation uses it now: it is put away with what it kept, and can
    // be brought back as it comes.
    let gone = ask(
        &mut socket,
        json!({"type":"plugin/remove","pluginId":"builtin"}),
    )
    .await;
    assert!(
        gone["payload"]["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .all(|plugin| plugin["id"] != "builtin"),
        "{gone}"
    );
    assert_eq!(gone["payload"]["away"][0]["id"], "builtin", "{gone}");
    assert_eq!(gone["payload"]["away"][0]["kind"], "integration", "{gone}");
    assert!(!service.0.path().join("plugin-data/builtin").exists());
    let back = ask(
        &mut socket,
        json!({"type":"plugin/restore","pluginId":"builtin"}),
    )
    .await;
    assert_eq!(
        installed(&back, "builtin")["params"][0]["value"],
        "kept",
        "{back}"
    );
    assert_eq!(back["payload"]["away"], json!([]));
    // Its settings pasted where it was put away or turned off bring it
    // back themselves, on, whatever was said around the line.
    ask(
        &mut socket,
        json!({"type":"plugin/remove","pluginId":"builtin"}),
    )
    .await;
    let taken = ask(
        &mut socket,
        json!({"type":"plugin/import","config":format!("Here it is:\n{}\nthanks", line.trim())}),
    )
    .await;
    assert_eq!(
        installed(&taken, "builtin")["params"][0]["value"],
        "handed-on",
        "{taken}"
    );
    assert_eq!(taken["payload"]["away"], json!([]));
    ask(
        &mut socket,
        json!({"type":"plugin/enable","pluginId":"builtin","enabled":false}),
    )
    .await;
    let taken = ask(
        &mut socket,
        json!({"type":"plugin/import","config":line.trim()}),
    )
    .await;
    assert_eq!(installed(&taken, "builtin")["enabled"], true, "{taken}");

    // A plugin from a git address is a copy of its own, and what it
    // brings with it is there while it is.
    let brought = plugin.join("integrations/mail");
    fs::create_dir_all(&brought).unwrap();
    install(&brought, "send", "#!/bin/sh\necho mailed \"$1\"\n");
    fs::write(
        manifest(&brought, "integration"),
        r#"{"name":"Mail","use":"./send"}"#,
    )
    .unwrap();
    git(&plugin, &["init", "-q"]);
    git(&plugin, &["add", "."]);
    git(
        &plugin,
        &[
            "-c",
            "user.name=a",
            "-c",
            "user.email=a@b",
            "commit",
            "-qm",
            "plugin",
        ],
    );
    let (ok, out) = sophon(&["plugin", "add", &format!("file://{folder}")]);
    assert!(ok, "{out}");
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    assert_eq!(installed(&listed, "watch")["source"], "git", "{listed}");
    assert!(service.0.path().join("plugins/watch/reply").is_file());
    let mail = installed(&listed, "mail");
    assert_eq!(
        (&mail["kind"], &mail["source"], &mail["from"]),
        (&json!("integration"), &json!("plugin"), &json!("watch")),
        "{listed}"
    );
    assert_eq!(
        installed(&listed, "watch")["requires"][1],
        json!({"id":"mail","name":"Mail","installed":true,"ready":true}),
        "{listed}"
    );
    assert_eq!(
        sophon(&["use", "mail", "ada"]),
        (true, "mailed ada\n".into())
    );
    // All of it handed on as a file: the plugin's folder, the
    // integration it brings riding in it. Read back it is shown first,
    // and installed only when that is asked.
    let packed = ask(
        &mut socket,
        json!({"type":"plugin/pack","pluginId":"watch"}),
    )
    .await;
    assert_eq!(packed["payload"]["fileName"], "watch.sophon", "{packed}");
    assert!(
        !packed["payload"]["about"].to_string().contains("s3cret"),
        "{packed}"
    );
    let read = ask(
        &mut socket,
        json!({"type":"plugin/unpack","archive":packed["payload"]["archive"]}),
    )
    .await;
    let about = &read["payload"]["about"];
    assert_eq!(about["root"], "watch", "{read}");
    assert_eq!(about["packages"][0]["here"], "installed", "{read}");
    assert!(
        about["packages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|package| package["id"] == "mail" && package["included"] == true),
        "{read}"
    );
    let token = read["payload"]["token"].clone();
    let done = ask(&mut socket, json!({"type":"plugin/install","token":token})).await;
    assert_eq!(done["payload"]["installed"]["root"], "watch", "{done}");
    assert_eq!(installed(&done, "watch")["source"], "file", "{done}");
    assert!(service.0.path().join("plugins/watch/reply").is_file());
    assert_eq!(
        sophon(&["use", "mail", "ada"]),
        (true, "mailed ada\n".into())
    );
    // A file is installed once: its token is gone with what was read.
    let again = ask(&mut socket, json!({"type":"plugin/install","token":token})).await;
    assert_eq!(again["type"], "rpc_error", "{again}");
    let refused = ask(
        &mut socket,
        json!({"type":"plugin/unpack","archive":"bm90IGEgZmlsZQ=="}),
    )
    .await;
    assert_eq!(refused["type"], "rpc_error", "{refused}");
    // The same from a terminal: a file written, then added from.
    let handed = author.join("handed.sophon");
    let (ok, out) = sophon(&[
        "plugin",
        "pack",
        "watch",
        "--no-settings",
        handed.to_str().unwrap(),
    ]);
    assert!(
        ok && out.contains("watch\tplugin\tWatch\tin the file"),
        "{out}"
    );
    let (ok, out) = sophon(&["plugin", "add", handed.to_str().unwrap()]);
    assert!(ok && out.contains("Installed Watch"), "{out}");
    // It goes when its plugin does, and not before.
    let (ok, out) = sophon(&["plugin", "remove", "mail"]);
    assert!(!ok && out.contains("came with the plugin watch"), "{out}");
    assert!(sophon(&["plugin", "remove", "watch"]).0);
    let listed = ask(&mut socket, json!({"type":"plugin/list"})).await;
    assert!(
        listed["payload"]["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .all(|package| package["id"] != "mail"),
        "{listed}"
    );
}

#[tokio::test]
async fn a_command_runs_with_no_agent_and_only_so_often_a_day() {
    let service = Service(tempfile::tempdir().unwrap());
    let work = tempfile::tempdir().unwrap();
    let work = work.path().canonicalize().unwrap();
    // No `codex` here at all: a command needs no agent's CLI.
    service.cli("start");
    let mut socket = service.connect(true).await;
    let sophon = |args: &[&str]| {
        let output = Command::new(service.0.path().join("bin/sophon"))
            .args(args)
            .env("SOPHON_SERVICE_HOME", service.0.path())
            .output()
            .unwrap();
        (
            output.status.success(),
            String::from_utf8_lossy(&output.stdout).into_owned()
                + &String::from_utf8_lossy(&output.stderr),
        )
    };
    let nothing = ask(&mut socket,json!({"type":"schedule/create","name":"Empty",
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":work,"isolation":"local"}}})).await;
    assert_eq!(nothing["type"], "rpc_error", "{nothing}");
    let created = ask(&mut socket,json!({"type":"schedule/create","name":"Sync","dailyLimit":2,
        "command":"echo \"synced $SOPHON_VAR_WHAT in $PWD\"; echo ran >> ran",
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":work,"isolation":"local"}}})).await;
    assert_eq!(created["type"], "schedule/create/response", "{created}");
    let id = created["payload"]["schedule"]["id"].clone();
    assert!(created["payload"]["schedule"]["nextRunAt"].is_null());
    let run = |what: &str| sophon(&["trigger", "Sync", "--var", &format!("what={what}")]);
    assert!(run("one").0);
    assert!(run("two").0);
    file_with(&work.join("ran"), "ran\nran\n").await;
    // The day's two are had: a third is refused, and said so.
    let (ok, out) = run("three");
    assert!(!ok && out.contains("had its runs for today"), "{out}");
    let logs = ask(&mut socket, json!({"type":"schedule/logs","scheduleId":id})).await;
    let runs = logs["payload"]["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2, "{logs}");
    assert_eq!(runs[1]["status"], "completed", "{logs}");
    assert_eq!(
        runs[1]["output"],
        format!("synced one in {}", work.display())
    );
    assert!(runs[1]["agentId"].is_null(), "{logs}");
    let list = ask(&mut socket, json!({"type":"schedule/list"})).await;
    assert_eq!(list["payload"]["schedules"][0]["runsToday"], 2, "{list}");
    // Asked for by hand it runs all the same; a command that fails says why.
    ask(
        &mut socket,
        json!({"type":"schedule/update","scheduleId":id,"command":"echo broken >&2; exit 4"}),
    )
    .await;
    ask(
        &mut socket,
        json!({"type":"schedule/run-once","scheduleId":id}),
    )
    .await;
    for _ in 0..100 {
        let logs = ask(&mut socket, json!({"type":"schedule/logs","scheduleId":id})).await;
        let last = &logs["payload"]["runs"][0];
        if last["status"] == "failed" {
            assert!(
                last["error"].as_str().unwrap().ends_with("broken"),
                "{logs}"
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("The failing command never ended");
}

#[tokio::test]
async fn a_command_is_tried_before_it_is_kept() {
    let service = Service(tempfile::tempdir().unwrap());
    let work = tempfile::tempdir().unwrap();
    let work = work.path().canonicalize().unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    // Run where an automation would run it, and told what one is told.
    let tried = ask(
        &mut socket,
        json!({"type":"automation/try","command":"echo \"in $PWD\"; echo \"dir $SOPHON_PROJECT_DIR\"; command -v sophon >/dev/null && echo reaches","cwd":work}),
    )
    .await;
    assert_eq!(tried["type"], "automation/try/response", "{tried}");
    assert_eq!(tried["payload"]["ok"], true, "{tried}");
    let folder = work.to_str().unwrap();
    assert_eq!(
        tried["payload"]["output"],
        format!("in {folder}\ndir {folder}\nreaches"),
        "{tried}"
    );
    // One that fails says so, with the end of what it complained of.
    let failed = ask(
        &mut socket,
        json!({"type":"automation/try","command":"echo nearly; echo broken >&2; exit 3","cwd":work}),
    )
    .await;
    assert_eq!(failed["payload"]["ok"], false, "{failed}");
    assert!(
        failed["payload"]["output"]
            .as_str()
            .unwrap()
            .ends_with("broken"),
        "{failed}"
    );
    // Nothing is kept of either: no automation, no run.
    let list = ask(&mut socket, json!({"type":"schedule/list"})).await;
    assert_eq!(list["payload"]["schedules"], json!([]), "{list}");
    // And it needs words to run, and somewhere to run them.
    for wrong in [
        json!({"type":"automation/try","command":"  ","cwd":work}),
        json!({"type":"automation/try","command":"true","cwd":"/nowhere/at/all"}),
    ] {
        let refused = ask(&mut socket, wrong.clone()).await;
        assert_eq!(refused["type"], "rpc_error", "{wrong}: {refused}");
    }
}

/// The runs of the automation `id`, newest first.
async fn runs_of(socket: &mut Socket, id: &Value) -> Vec<Value> {
    ask(socket, json!({"type":"schedule/logs","scheduleId":id})).await["payload"]["runs"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

#[tokio::test]
async fn a_run_the_service_stopped_under_is_started_again() {
    let service = Service(tempfile::tempdir().unwrap());
    let work = tempfile::tempdir().unwrap();
    let work = work.path().canonicalize().unwrap();
    // A `codex` whose turn never ends: the run is underway when the
    // service stops.
    install_provider(service.0.path());
    let cli = service.0.path().join("codex");
    let script = fs::read_to_string(&cli).unwrap();
    let hanging: String = script
        .lines()
        .filter(|line| !line.contains("turn/completed"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(hanging.len(), script.len());
    fs::write(&cli, hanging).unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    fs::write(work.join("events"), "").unwrap();
    let created = ask(&mut socket,json!({"type":"schedule/create","name":"Reviews","prompt":"Review {{url}}",
        "cadence":{"type":"every","everyMs":300},"probe":"cat events",
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":work,"isolation":"local"}}})).await;
    let id = created["payload"]["schedule"]["id"].clone();
    for _ in 0..100 {
        let list = ask(&mut socket, json!({"type":"schedule/list"})).await;
        if list["payload"]["schedules"][0]["primed"] == true {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    fs::write(
        work.join("events"),
        "{\"id\":\"pr-1\",\"vars\":{\"url\":\"https://x/pull/1\"}}\n",
    )
    .unwrap();
    for _ in 0..100 {
        if runs_of(&mut socket, &id).await.len() == 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(runs_of(&mut socket, &id).await[0]["status"], "running");
    // The service stops under it, and starts again (an update does this).
    service.cli("stop");
    service.cli("start");
    let mut socket = service.connect(true).await;
    // The one cut off is said to be, and what caused it starts another.
    for _ in 0..100 {
        if runs_of(&mut socket, &id).await.len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let all = runs_of(&mut socket, &id).await;
    assert_eq!(all.len(), 2, "{all:?}");
    assert_eq!(all[1]["status"], "failed");
    assert_eq!(all[1]["error"], "The service stopped while it ran");
    assert_eq!(all[0]["event"]["id"], "pr-1");
    assert_eq!(all[0]["status"], "running");
}

#[tokio::test]
async fn a_probe_that_keeps_failing_is_paused_with_why() {
    let service = Service(tempfile::tempdir().unwrap());
    install_provider(service.0.path());
    service.cli("start");
    let mut socket = service.connect(true).await;
    let refused = ask(&mut socket,json!({"type":"schedule/create","prompt":"x","probe":"true",
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":service.0.path(),"isolation":"local"}}})).await;
    assert_eq!(refused["type"], "rpc_error", "{refused}");
    let created = ask(&mut socket,json!({"type":"schedule/create","prompt":"x",
        "cadence":{"type":"every","everyMs":100},"probe":"echo gone >&2; exit 3",
        "target":{"type":"new-agent","config":{"provider":"codex","cwd":service.0.path(),"isolation":"local"}}})).await;
    assert_eq!(created["type"], "schedule/create/response", "{created}");
    for _ in 0..200 {
        let list = ask(&mut socket, json!({"type":"schedule/list"})).await;
        let schedule = &list["payload"]["schedules"][0];
        if schedule["status"] == "paused" {
            assert_eq!(schedule["probeFailures"], 5);
            assert!(
                schedule["probeError"].as_str().unwrap().ends_with("gone"),
                "{schedule}"
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("The automation went on");
}

/// The real `claude`, signed in on this machine, through a whole turn:
/// `cargo test -p sophon-service --test session live -- --ignored`.
#[tokio::test]
#[ignore = "runs the host's own Claude Code, on its account"]
async fn live_claude_asks_runs_and_stops() {
    PATIENCE.store(120, std::sync::atomic::Ordering::Relaxed);
    let service = Service(tempfile::tempdir().unwrap());
    let project = tempfile::tempdir().unwrap();
    let cwd = fs::canonicalize(project.path()).unwrap();
    // Started as from inside a session routed somewhere that answers
    // nothing: the agent must not go there.
    service.cli_with_env(
        "start",
        &[
            ("ANTHROPIC_BASE_URL", "http://127.0.0.1:9"),
            ("ANTHROPIC_AUTH_TOKEN", "not-a-token"),
            ("CLAUDECODE", "1"),
        ],
    );
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request",
        "source":{"kind":"directory","path":cwd},
        "agent":{"config":{"provider":"claude","model":"haiku","modeId":"default"},
            "initialPrompt":"Run the shell command `echo sophon-live > live.txt`, then reply with the single word done."}})).await;
    assert_eq!(created["type"], "workspace.create.response", "{created}");
    let agent = created["payload"]["agent"]["id"].clone();
    let asked = until(&mut socket, "permission_requested").await;
    assert_eq!(asked["request"]["name"], "Bash", "{asked}");
    assert!(!cwd.join("live.txt").exists());
    let commands = ask(
        &mut socket,
        json!({"type":"list_commands_request","agentId":agent}),
    )
    .await;
    assert!(
        commands["payload"]["commands"]
            .as_array()
            .is_some_and(|list| !list.is_empty()),
        "{commands}"
    );
    send(
        &mut socket,
        json!({"type":"session","message":{"type":"agent_permission_response","agentId":agent,
        "requestId":asked["request"]["id"],"response":{"behavior":"allow"}}}),
    )
    .await;
    finished(&mut socket).await;
    assert_eq!(
        fs::read_to_string(cwd.join("live.txt")).unwrap().trim(),
        "sophon-live"
    );
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let rows = timeline["payload"]["entries"].as_array().unwrap();
    let call = rows
        .iter()
        .find(|row| row["item"]["type"] == "tool_call")
        .unwrap();
    assert_eq!(
        (&call["item"]["status"], &call["item"]["detail"]["type"]),
        (&json!("completed"), &json!("shell")),
        "{timeline}"
    );
    assert!(
        rows.last().unwrap()["item"]["text"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("done"),
        "{timeline}"
    );
    let record = &timeline["payload"]["agent"];
    assert!(record["persistence"]["sessionId"].is_string(), "{record}");
    assert!(
        record["lastUsage"]["contextWindowMaxTokens"]
            .as_u64()
            .unwrap()
            > 0,
        "{record}"
    );

    // In a mode that takes edits, a file is written without asking.
    let mode = ask(
        &mut socket,
        json!({"type":"set_agent_mode_request","agentId":agent,"modeId":"acceptEdits"}),
    )
    .await;
    assert_eq!(
        mode["payload"]["agent"]["currentModeId"], "acceptEdits",
        "{mode}"
    );
    let sent = ask(
        &mut socket,
        json!({"type":"send_agent_message_request","agentId":agent,
        "text":"Use the Write tool to create note.txt containing the word hi. Then reply done."}),
    )
    .await;
    assert_eq!(sent["type"], "send_agent_message_response", "{sent}");
    loop {
        let frame = read(&mut socket).await;
        let kind = &frame["message"]["payload"]["event"]["type"];
        assert!(
            kind != "permission_requested" && kind != "turn_failed",
            "{frame}"
        );
        if kind == "turn_completed" {
            break;
        }
    }
    assert_eq!(
        fs::read_to_string(cwd.join("note.txt")).unwrap().trim(),
        "hi"
    );

    // Stopped while it waits to be allowed a command: nothing runs.
    ask(
        &mut socket,
        json!({"type":"set_agent_mode_request","agentId":agent,"modeId":"default"}),
    )
    .await;
    ask(
        &mut socket,
        json!({"type":"send_agent_message_request","agentId":agent,
        "text":"Run the shell command `echo second > second.txt`."}),
    )
    .await;
    until(&mut socket, "permission_requested").await;
    ask(
        &mut socket,
        json!({"type":"cancel_agent_request","agentId":agent}),
    )
    .await;
    until(&mut socket, "turn_canceled").await;
    assert!(!cwd.join("second.txt").exists());
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let rows = timeline["payload"]["entries"].as_array().unwrap();
    let stopped = rows
        .iter()
        .rev()
        .find(|row| row["item"]["type"] == "tool_call")
        .unwrap();
    assert_eq!(stopped["item"]["status"], "canceled", "{timeline}");
    let record = &timeline["payload"]["agent"];
    assert_eq!(
        (&record["status"], &record["pendingPermissions"]),
        (&json!("idle"), &json!([])),
        "{record}"
    );
}

/// The real `codex`, as this machine has it set up.
#[tokio::test]
#[ignore = "runs the host's own Codex, on its account"]
async fn live_codex_replies() {
    PATIENCE.store(120, std::sync::atomic::Ordering::Relaxed);
    let service = Service(tempfile::tempdir().unwrap());
    let project = tempfile::tempdir().unwrap();
    service.cli_with_env(
        "start",
        &[
            ("CODEX_SANDBOX_NETWORK_DISABLED", "1"),
            ("CODEX_THREAD_ID", "not-a-thread"),
        ],
    );
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request",
        "source":{"kind":"directory","path":fs::canonicalize(project.path()).unwrap()},
        "agent":{"config":{"provider":"codex"},
            "initialPrompt":"Run the shell command `echo sophon-live` and then reply with the single word done."}})).await;
    assert_eq!(created["type"], "workspace.create.response", "{created}");
    let agent = created["payload"]["agent"]["id"].clone();
    finished(&mut socket).await;
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let rows = timeline["payload"]["entries"].as_array().unwrap();
    assert!(
        rows.iter()
            .any(|row| row["item"]["detail"]["type"] == "shell"
                && row["item"]["status"] == "completed"),
        "{timeline}"
    );
    assert!(
        rows.last().unwrap()["item"]["text"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("done"),
        "{timeline}"
    );
    assert!(timeline["payload"]["agent"]["persistence"]["sessionId"].is_string());
}

/// What the old service kept, in a home made for the test: a project
/// with a conversation whose words are in Claude Code's record, a
/// schedule that fell due while nothing ran, a relay, and another space.
#[tokio::test]
async fn the_first_start_brings_over_what_the_old_service_kept() {
    let user = tempfile::tempdir().unwrap();
    let home = user.path();
    let old = home.join(".paseo");
    fs::create_dir_all(old.join("projects")).unwrap();
    fs::create_dir_all(old.join("agents/p")).unwrap();
    fs::create_dir_all(old.join("schedules")).unwrap();
    fs::create_dir_all(home.join(".paseo-spaces/lin/agents")).unwrap();
    fs::create_dir_all(home.join(".claude/projects/-old-project")).unwrap();
    fs::create_dir_all(home.join(".sophon")).unwrap();
    fs::write(
        old.join("projects/projects.json"),
        json!([{"projectId":"p1","rootPath":"/old/project","displayName":"project","kind":"git"}])
            .to_string(),
    )
    .unwrap();
    fs::write(old.join("projects/workspaces.json"),
        json!([{"workspaceId":"w1","projectId":"p1","cwd":"/old/project","kind":"local_checkout","title":"main"}]).to_string()).unwrap();
    fs::write(old.join("agents/p/a1.json"), json!({"id":"a1","provider":"claude","cwd":"/old/project","workspaceId":"w1",
        "title":"Fix it","lastModeId":"acceptEdits","config":{"model":"claude-opus-5-5","thinkingOptionId":"high"},
        "persistence":{"sessionId":"session-1"},"updatedAt":"2026-01-01T00:00:00Z"}).to_string()).unwrap();
    fs::write(
        old.join("agents/p/a2.json"),
        json!({"id":"a2","provider":"claude","cwd":"/old/project","internal":true}).to_string(),
    )
    .unwrap();
    fs::write(
        old.join("schedules/s1.json"),
        json!({"id":"s1","name":"Nightly","prompt":"check","status":"active",
        "cadence":{"type":"every","everyMs":3_600_000},"nextRunAt":"2020-01-01T00:00:00Z",
        "target":{"type":"new-agent","config":{"provider":"claude","cwd":"/old/project"}},
        "runs":[{"id":"r1","status":"completed"}]})
        .to_string(),
    )
    .unwrap();
    fs::write(old.join("config.json"), json!({"daemon":{"listen":"127.0.0.1:6767","relay":{"endpoint":"relay.example:8443","useTls":false}},
        "agents":{"providers":{"work":{"extends":"claude","env":{"CLAUDE_CONFIG_DIR":"/h/.claude-work"}},"opencode":{"enabled":false}}}}).to_string()).unwrap();
    fs::write(home.join(".paseo-spaces/lin/agents/b1.json"),
        json!({"id":"b1","provider":"codex","cwd":"/lin/project","persistence":{"sessionId":"thread-9"}}).to_string()).unwrap();
    fs::write(home.join(".claude/projects/-old-project/session-1.jsonl"), [
        json!({"type":"user","uuid":"u1","message":{"content":"fix it"}}),
        json!({"type":"assistant","message":{"model":"m","content":[{"type":"text","text":"Fixed."}]}}),
        // Another conversation of Claude Code's, never one of Sophon's.
    ].map(|entry| entry.to_string()).join("\n")).unwrap();
    fs::write(
        home.join(".claude/projects/-old-project/session-other.jsonl"),
        json!({"type":"user","message":{"content":"not Sophon's"}}).to_string(),
    )
    .unwrap();
    let before = fs::read(old.join("agents/p/a1.json")).unwrap();

    let service = Service(
        tempfile::Builder::new()
            .prefix("service")
            .rand_bytes(0)
            .tempdir_in(home.join(".sophon"))
            .unwrap(),
    );
    assert_eq!(service.0.path(), home.join(".sophon/service"));
    let own = [("HOME", home.to_str().unwrap())];
    service.cli_with_env("start", &own);
    let mut socket = service.connect(true).await;
    let projects = ask(&mut socket, json!({"type":"project.list.request"})).await;
    assert_eq!(
        projects["payload"]["projects"][0]["projectRootPath"],
        "/old/project"
    );
    let agents = ask(&mut socket, json!({"type":"fetch_agents_request"})).await;
    let agents = agents["payload"]["entries"].as_array().unwrap();
    assert_eq!(agents.len(), 1, "{agents:?}");
    let agent = &agents[0];
    assert_eq!(
        (&agent["id"], &agent["title"], &agent["workspaceId"]),
        (&json!("a1"), &json!("Fix it"), &json!("w1"))
    );
    assert_eq!(
        (
            &agent["model"],
            &agent["thinkingOptionId"],
            &agent["currentModeId"]
        ),
        (
            &json!("claude-opus-5-5"),
            &json!("high"),
            &json!("acceptEdits")
        )
    );
    // What was said, from Claude Code's record of that session alone.
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":"a1"}),
    )
    .await;
    let said: Vec<_> = timeline["payload"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["item"]["text"].clone())
        .collect();
    assert_eq!(said, ["fix it", "Fixed."]);
    let schedules = ask(&mut socket, json!({"type":"schedule/list"})).await;
    let schedule = &schedules["payload"]["schedules"][0];
    assert_eq!(
        (&schedule["name"], &schedule["status"]),
        (&json!("Nightly"), &json!("active"))
    );
    assert!(
        schedule["nextRunAt"].as_str().unwrap() > "2026",
        "{schedule}"
    );
    let logs = ask(
        &mut socket,
        json!({"type":"schedule/logs","scheduleId":"s1"}),
    )
    .await;
    assert_eq!(logs["payload"]["runs"][0]["id"], "r1");
    let config = ask(&mut socket, json!({"type":"get_daemon_config_request"})).await;
    let config = &config["payload"]["config"];
    assert_eq!(
        config["daemon"],
        json!({"relay":{"endpoint":"relay.example:8443","useTls":false}})
    );
    assert_eq!(
        config["providers"],
        json!({"work":{"extends":"claude","env":{"CLAUDE_CONFIG_DIR":"/h/.claude-work"}}})
    );
    // The other space, in its own folder; the old folders as they were.
    let lin: Value =
        serde_json::from_slice(&fs::read(home.join(".sophon/spaces/lin/state.json")).unwrap())
            .unwrap();
    assert_eq!(lin["agents"]["b1"]["persistence"]["sessionId"], "thread-9");
    assert_eq!(fs::read(old.join("agents/p/a1.json")).unwrap(), before);

    // Once: what is changed here since is not replaced by the old again.
    ask(
        &mut socket,
        json!({"type":"update_agent_request","agentId":"a1","name":"Renamed"}),
    )
    .await;
    service.cli_with_env("stop", &own);
    service.cli_with_env("start", &own);
    let mut socket = service.connect(true).await;
    let agents = ask(&mut socket, json!({"type":"fetch_agents_request"})).await;
    assert_eq!(agents["payload"]["entries"][0]["title"], "Renamed");
}

/// Whether the conversation `agent` waits to be looked at.
async fn attention(socket: &mut Socket, agent: &Value) -> Value {
    ask(
        socket,
        json!({"type":"fetch_agent_request","agentId":agent}),
    )
    .await["payload"]["agent"]["requiresAttention"]
        .clone()
}

#[tokio::test]
async fn a_workspace_is_seen_put_away_and_brought_back() {
    let service = Service(tempfile::tempdir().unwrap());
    install_provider(service.0.path());
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"codex"},"initialPrompt":"first"}})).await;
    let (agent, workspace) = (
        created["payload"]["agent"]["id"].clone(),
        created["payload"]["workspace"]["id"].clone(),
    );
    finished(&mut socket).await;
    assert_eq!(attention(&mut socket, &agent).await, true);
    let seen = ask(
        &mut socket,
        json!({"type":"workspace.clear_attention.request","workspaceId":workspace}),
    )
    .await;
    assert_eq!(seen["type"], "workspace.clear_attention.response", "{seen}");
    assert_eq!(attention(&mut socket, &agent).await, false);

    let count = |reply: Value| reply["payload"]["entries"].as_array().unwrap().len();
    let away = ask(
        &mut socket,
        json!({"type":"archive_workspace_request","workspaceId":workspace}),
    )
    .await;
    assert_eq!(away["type"], "archive_workspace_response", "{away}");
    assert_eq!(
        count(ask(&mut socket, json!({"type":"fetch_workspaces_request"})).await),
        0
    );
    assert_eq!(
        count(ask(&mut socket, json!({"type":"fetch_agents_request"})).await),
        0
    );
    let back = ask(
        &mut socket,
        json!({"type":"workspace.archive.undo.request","workspaceId":workspace}),
    )
    .await;
    assert_eq!(back["type"], "workspace.archive.undo.response", "{back}");
    assert_eq!(
        count(ask(&mut socket, json!({"type":"fetch_workspaces_request"})).await),
        1
    );
    assert_eq!(
        count(ask(&mut socket, json!({"type":"fetch_agents_request"})).await),
        1
    );
}

#[tokio::test]
async fn copies_merges_and_a_restart() {
    let service = Service(tempfile::tempdir().unwrap());
    install_provider(service.0.path());
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("repo")).unwrap();
    let cwd = fs::canonicalize(project.path().join("repo")).unwrap();
    git(&cwd, &["init", "-b", "main"]);
    fs::write(cwd.join("notes.txt"), "one\n").unwrap();
    fs::create_dir(cwd.join("docs")).unwrap();
    fs::write(cwd.join("docs/a.md"), "a").unwrap();
    git(&cwd, &["add", "."]);
    git(&cwd, &["commit", "-m", "initial"]);
    service.cli("start");
    let mut socket = service.connect(true).await;
    let made = ask(
        &mut socket,
        json!({"type":"workspace.create.request","idempotencyKey":"k",
        "source":{"kind":"worktree","branchName":"feature","baseBranch":"main","cwd":cwd},
        "agent":{"config":{"provider":"codex"}}}),
    )
    .await;
    let tree = made["payload"]["workspace"]["workspaceDirectory"]
        .as_str()
        .unwrap()
        .to_owned();

    // A copy of a file, and of a folder with what is in it.
    for path in ["notes.txt", "notes.txt", "docs"] {
        let copied = ask(
            &mut socket,
            json!({"type":"fs.entry.duplicate.request","cwd":tree,"path":path}),
        )
        .await;
        assert!(copied["payload"]["error"].is_null(), "{copied}");
    }
    assert_eq!(
        fs::read_to_string(Path::new(&tree).join("notes copy.txt")).unwrap(),
        "one\n"
    );
    assert!(Path::new(&tree).join("notes copy 2.txt").exists());
    assert_eq!(
        fs::read_to_string(Path::new(&tree).join("docs copy/a.md")).unwrap(),
        "a"
    );

    // The worktree's branch into its base, which the project's folder has checked out.
    fs::write(Path::new(&tree).join("feature.txt"), "new\n").unwrap();
    git(Path::new(&tree), &["add", "feature.txt"]);
    git(Path::new(&tree), &["commit", "-m", "feature"]);
    let merged = ask(
        &mut socket,
        json!({"type":"checkout_merge_request","cwd":tree,"strategy":"merge"}),
    )
    .await;
    assert!(merged["payload"]["error"].is_null(), "{merged}");
    assert_eq!(
        fs::read_to_string(cwd.join("feature.txt")).unwrap(),
        "new\n"
    );

    let said = ask(
        &mut socket,
        json!({"type":"provider_diagnostic_request","provider":"codex"}),
    )
    .await;
    assert!(
        said["payload"]["diagnostic"]
            .as_str()
            .unwrap()
            .contains("Binary:"),
        "{said}"
    );

    // The worktree's folder gone, and back from its branch.
    fs::remove_dir_all(&tree).unwrap();
    let back = ask(&mut socket, json!({"type":"workspace.recovery.restore.request","workspaceId":made["payload"]["workspace"]["id"]})).await;
    assert_eq!(
        back["type"], "workspace.recovery.restore.response",
        "{back}"
    );
    assert_eq!(
        fs::read_to_string(Path::new(&tree).join("feature.txt")).unwrap(),
        "new\n"
    );

    // Started again when asked, with what it had.
    let before = fs::read_to_string(service.0.path().join("sophon.pid")).unwrap();
    let restarting = ask(
        &mut socket,
        json!({"type":"restart_server_request","reason":"test"}),
    )
    .await;
    assert_eq!(
        restarting["type"], "restart_server_response",
        "{restarting}"
    );
    for _ in 0..100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        if fs::read_to_string(service.0.path().join("sophon.pid")).is_ok_and(|now| now != before) {
            break;
        }
    }
    assert_ne!(
        fs::read_to_string(service.0.path().join("sophon.pid")).unwrap(),
        before
    );
    let mut socket = service.connect(true).await;
    let projects = ask(&mut socket, json!({"type":"project.list.request"})).await;
    assert_eq!(projects["payload"]["projects"].as_array().unwrap().len(), 1);
}

/// A `claude` with something running in the background, which stops it
/// when told and gives files back when asked.
const REWINDING_CLAUDE: &str = r#"#!/bin/sh
say() { printf '%s\n' "$1"; }
while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed -n 's/.*"request_id":"\([^"]*\)".*/\1/p')
    case "$line" in
        *'"subtype":"rewind_files"'*)
            printf '%s\n' "$line" >> rewound
            say "{\"type\":\"control_response\",\"response\":{\"subtype\":\"success\",\"request_id\":\"$id\",\"response\":{}}}" ;;
        *'"subtype":"stop_task"'*) printf '%s\n' "$line" >> stopped ;;
        *'"type":"user"'*)
            printf '%s\n' "$line" >> prompts
            say '{"type":"system","subtype":"session_state_changed","state":"running"}'
            say '{"type":"system","subtype":"init","session_id":"session-1"}'
            say '{"type":"system","subtype":"background_tasks_changed","tasks":[{"task_id":"bg1","task_type":"local_bash","description":"Dev server"}]}'
            say '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m","model":"m","content":[{"type":"text","text":"ok"}]}}'
            say '{"type":"result","is_error":false}'
            say '{"type":"system","subtype":"session_state_changed","state":"idle"}' ;;
    esac
done
"#;

#[tokio::test]
async fn background_work_a_fork_and_going_back() {
    let user = tempfile::tempdir().unwrap();
    let home = user.path();
    let record = home.join(".claude/projects/-p");
    fs::create_dir_all(&record).unwrap();
    let first = "11111111-1111-4111-8111-111111111111";
    let second = "22222222-2222-4222-8222-222222222222";
    // Claude Code's record of the session, as it would have written it.
    fs::write(record.join("session-1.jsonl"), [
        json!({"type":"user","uuid":first,"sessionId":"session-1","message":{"content":"one"}}),
        json!({"type":"assistant","uuid":"a1","sessionId":"session-1","message":{"model":"m","content":[{"type":"text","text":"ok"}]}}),
        json!({"type":"user","uuid":second,"sessionId":"session-1","message":{"content":"two"}}),
        json!({"type":"assistant","uuid":"a2","sessionId":"session-1","message":{"model":"m","content":[{"type":"text","text":"ok"}]}}),
    ].map(|entry| entry.to_string()).join("\n")).unwrap();
    let service = Service(tempfile::tempdir().unwrap());
    install(service.0.path(), "claude", REWINDING_CLAUDE);
    let own = [("HOME", home.to_str().unwrap())];
    service.cli_with_env("start", &own);
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"claude"},"initialPrompt":"one","clientMessageId":first}})).await;
    let agent = created["payload"]["agent"]["id"].clone();
    finished(&mut socket).await;
    ask(&mut socket, json!({"type":"send_agent_message_request","agentId":agent,"text":"two","messageId":second})).await;
    finished(&mut socket).await;
    // Each message went to Claude Code under its own id.
    let prompts = fs::read_to_string(service.0.path().join("prompts")).unwrap();
    assert!(
        prompts.contains(&format!("\"uuid\":\"{first}\""))
            && prompts.contains(&format!("\"uuid\":\"{second}\"")),
        "{prompts}"
    );

    let running = ask(
        &mut socket,
        json!({"type":"agent.background_work.list.request","agentId":agent}),
    )
    .await;
    assert_eq!(
        running["payload"]["items"],
        json!([{"id":"bg1","kind":"local_bash","description":"Dev server",
        "command":null,"canStop":true,"stopToken":"bg1"}]),
        "{running}"
    );
    ask(&mut socket, json!({"type":"agent.background_work.stop.request","agentId":agent,"itemId":"bg1","stopToken":"bg1"})).await;

    let fork = ask(
        &mut socket,
        json!({"type":"agent.fork_context.request","agentId":agent}),
    )
    .await;
    let attachment = &fork["payload"]["attachment"];
    let words = fs::read_to_string(attachment["path"].as_str().unwrap()).unwrap();
    assert!(
        words.contains("## User\n\ntwo") && words.contains("## Assistant\n\nok"),
        "{words}"
    );
    // What a new conversation is given to read is taken as an upload is.
    let next = ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"claude"},"initialPrompt":"go on","attachments":[attachment]}})).await;
    assert_eq!(next["type"], "workspace.create.response", "{next}");
    finished(&mut socket).await;

    // Where a client that has it all open stands in its stream.
    let place = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await["payload"]["endCursor"]
        .clone();

    // Back to before the second message, files and conversation both.
    let id = "rewind-1";
    send(
        &mut socket,
        json!({"type":"session","message":{"type":"agent.rewind.request","requestId":id,
        "agentId":agent,"messageId":second,"mode":"both"}}),
    )
    .await;
    let done = loop {
        let frame = read(&mut socket).await;
        if frame["message"]["type"] == "agent.rewind.response" {
            break frame["message"]["payload"].clone();
        }
    };
    assert_eq!(
        (&done["requestId"], &done["error"]),
        (&json!(id), &Value::Null),
        "{done}"
    );
    assert!(
        fs::read_to_string(service.0.path().join("rewound"))
            .unwrap()
            .contains(&format!("\"user_message_id\":\"{second}\""))
    );
    assert!(
        fs::read_to_string(service.0.path().join("stopped"))
            .unwrap()
            .contains("\"task_id\":\"bg1\"")
    );
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let said: Vec<_> = timeline["payload"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["item"]["text"].clone())
        .collect();
    assert_eq!(said, ["one", "ok"]);
    // A client that kept its place from before the rewind, in what was taken
    // away, is given the conversation as it now is: "what came after" would
    // leave the message taken back on its screen.
    let kept = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent,"direction":"after","cursor":place}),
    )
    .await;
    assert_eq!(kept["payload"]["reset"], true, "{kept}");
    let said: Vec<_> = kept["payload"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["item"]["text"].clone())
        .collect();
    assert_eq!(said, ["one", "ok"]);
    // A session of its own, Claude Code's record of it ending where the conversation now does.
    let session = timeline["payload"]["agent"]["persistence"]["sessionId"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(session, "session-1");
    let kept = fs::read_to_string(record.join(format!("{session}.jsonl"))).unwrap();
    assert_eq!(kept.lines().count(), 2);
    assert!(
        kept.contains(&session) && !kept.contains("session-1"),
        "{kept}"
    );
    assert_eq!(
        fs::read_to_string(record.join("session-1.jsonl"))
            .unwrap()
            .lines()
            .count(),
        4
    );

    // A session the CLI has and Sophon does not, listed and taken in.
    fs::write(record.join("other-9.jsonl"),
        json!({"type":"user","uuid":"u","sessionId":"other-9","cwd":service.0.path(),"message":{"content":"from a terminal"}}).to_string()).unwrap();
    let recent = ask(
        &mut socket,
        json!({"type":"fetch_recent_provider_sessions_request","limit":50,"providers":["claude"]}),
    )
    .await;
    let listed: Vec<_> = recent["payload"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["providerHandleId"].clone())
        .collect();
    assert!(
        listed.contains(&json!("other-9")) && !listed.contains(&json!(session)),
        "{recent}"
    );
    let taken = ask(&mut socket, json!({"type":"import_agent_request","providerId":"claude","providerHandleId":"other-9","cwd":service.0.path()})).await;
    assert_eq!(
        (
            &taken["payload"]["status"],
            &taken["payload"]["agent"]["title"]
        ),
        (&json!("agent_created"), &json!("from a terminal")),
        "{taken}"
    );
    let theirs = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":taken["payload"]["agent"]["id"]}),
    )
    .await;
    assert_eq!(
        theirs["payload"]["entries"][0]["item"]["text"],
        "from a terminal"
    );
}

/// Going back, with the real Claude Code: its files and its memory.
#[tokio::test]
#[ignore = "runs the host's own Claude Code, on its account"]
async fn live_claude_goes_back_to_before_a_message() {
    PATIENCE.store(120, std::sync::atomic::Ordering::Relaxed);
    let service = Service(tempfile::tempdir().unwrap());
    let project = tempfile::tempdir().unwrap();
    let cwd = fs::canonicalize(project.path()).unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    let (first, second) = (
        uuid::Uuid::new_v4().to_string(),
        uuid::Uuid::new_v4().to_string(),
    );
    let created = ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":cwd},
        "agent":{"config":{"provider":"claude","model":"haiku","modeId":"acceptEdits"},"clientMessageId":first,
            "initialPrompt":"Use the Write tool to create a.txt containing exactly the word one. Then reply done."}})).await;
    let agent = created["payload"]["agent"]["id"].clone();
    finished(&mut socket).await;
    assert_eq!(fs::read_to_string(cwd.join("a.txt")).unwrap().trim(), "one");
    ask(&mut socket, json!({"type":"send_agent_message_request","agentId":agent,"messageId":second,
        "text":"Use the Write tool to overwrite a.txt so it contains exactly the word two. Then reply done."})).await;
    finished(&mut socket).await;
    assert_eq!(fs::read_to_string(cwd.join("a.txt")).unwrap().trim(), "two");

    send(
        &mut socket,
        json!({"type":"session","message":{"type":"agent.rewind.request","requestId":"r",
        "agentId":agent,"messageId":second,"mode":"both"}}),
    )
    .await;
    let done = loop {
        let frame = read(&mut socket).await;
        if frame["message"]["type"] == "agent.rewind.response" {
            break frame["message"]["payload"].clone();
        }
    };
    assert_eq!(done["error"], Value::Null, "{done}");
    assert_eq!(fs::read_to_string(cwd.join("a.txt")).unwrap().trim(), "one");
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let asked: Vec<_> = timeline["payload"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["item"]["type"] == "user_message")
        .collect();
    assert_eq!(asked.len(), 1, "{timeline}");

    // It goes on from there, with no memory of what was taken back.
    ask(&mut socket, json!({"type":"send_agent_message_request","agentId":agent,
        "text":"Without using any tools: how many times have I asked you to write a.txt in this conversation? Reply with just the number."})).await;
    finished(&mut socket).await;
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let last = timeline["payload"]["entries"]
        .as_array()
        .unwrap()
        .last()
        .unwrap()["item"]["text"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(last.contains('1') && !last.contains('2'), "{last}");
}

/// A phone lists a project by its workspaces and starts a conversation in
/// one: a folder added as a project has its own from the start, and one
/// added before that was so gets it when the service next starts.
#[tokio::test]
async fn a_project_added_has_its_folder_s_workspace() {
    let service = Service(tempfile::tempdir().unwrap());
    let added = service.0.path().join("added");
    let earlier = service.0.path().join("earlier");
    fs::create_dir_all(&added).unwrap();
    fs::create_dir_all(&earlier).unwrap();
    let earlier = fs::canonicalize(&earlier).unwrap();
    // As the service kept a project added before: no workspace with it.
    fs::write(
        service.0.path().join("state.json"),
        json!({"projects":{"p-earlier":{"projectId":"p-earlier","projectRootPath":earlier,
            "projectDisplayName":"earlier","projectKind":"directory"}},"workspaces":{},"agents":{}})
        .to_string(),
    )
    .unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    let folders = |reply: Value| -> Vec<String> {
        let mut list: Vec<String> = reply["payload"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|workspace| workspace["name"].as_str().unwrap().to_owned())
            .collect();
        list.sort();
        list
    };
    assert_eq!(
        folders(ask(&mut socket, json!({"type":"fetch_workspaces_request"})).await),
        ["earlier"]
    );
    let reply = ask(
        &mut socket,
        json!({"type":"project.add.request","cwd":added}),
    )
    .await;
    assert_eq!(reply["type"], "project.add.response", "{reply}");
    let project = reply["payload"]["project"]["projectId"].clone();
    let listed = ask(&mut socket, json!({"type":"fetch_workspaces_request"})).await;
    let workspace = listed["payload"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|workspace| workspace["projectId"] == project)
        .expect("the added folder's workspace")
        .clone();
    assert_eq!(
        workspace["workspaceDirectory"],
        reply["payload"]["project"]["projectRootPath"]
    );
    // Added again: the same project, the same workspace.
    ask(
        &mut socket,
        json!({"type":"project.add.request","cwd":added}),
    )
    .await;
    assert_eq!(
        folders(ask(&mut socket, json!({"type":"fetch_workspaces_request"})).await),
        ["added", "earlier"]
    );
    service.cli("stop");
}

/// A conversation taken out of the archive comes back where a client can
/// show it: its workspace was archived with it, and is listed again.
#[tokio::test]
async fn a_conversation_restored_brings_its_workspace_back() {
    let service = Service(tempfile::tempdir().unwrap());
    let folder = service.0.path().join("project");
    fs::create_dir_all(&folder).unwrap();
    let folder = fs::canonicalize(&folder).unwrap();
    let at = "2026-11-12T08:58:53.000Z";
    let conversation = |id: &str| {
        json!({"id":id,"provider":"claude","cwd":folder,"workspaceId":"w1","status":"idle","title":id,
            "archivedAt":at,"persistence":{},"createdAt":at,"updatedAt":at})
    };
    fs::write(
        service.0.path().join("state.json"),
        json!({
            "projects":{"p1":{"projectId":"p1","projectRootPath":folder,"projectDisplayName":"project","projectKind":"directory"}},
            "workspaces":{"w1":{"id":"w1","projectId":"p1","projectRootPath":folder,"workspaceDirectory":folder,
                "name":"project","workspaceKind":"directory","archivedAt":at}},
            "agents":{"kept":conversation("kept"),"other":conversation("other")}
        })
        .to_string(),
    )
    .unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    let ids = |reply: Value| -> Vec<String> {
        reply["payload"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| entry["id"].as_str().unwrap().to_owned())
            .collect()
    };
    // Archived: neither is listed, both are in the history.
    assert!(ids(ask(&mut socket, json!({"type":"fetch_workspaces_request"})).await).is_empty());
    assert!(ids(ask(&mut socket, json!({"type":"fetch_agents_request"})).await).is_empty());
    assert_eq!(
        ids(ask(&mut socket, json!({"type":"fetch_agent_history_request"})).await).len(),
        2
    );
    let restored = ask(
        &mut socket,
        json!({"type":"refresh_agent_request","agentId":"kept","restore":true}),
    )
    .await;
    assert!(
        restored["payload"]["agent"]["archivedAt"].is_null(),
        "{restored}"
    );
    assert_eq!(
        ids(ask(&mut socket, json!({"type":"fetch_workspaces_request"})).await),
        ["w1"]
    );
    // The one asked for, not the workspace's other conversations.
    assert_eq!(
        ids(ask(&mut socket, json!({"type":"fetch_agents_request"})).await),
        ["kept"]
    );
    service.cli("stop");
}

/// Choosing a folder to add as a project: a folder that is no project yet
/// says which folders it holds, and nothing of its files.
#[tokio::test]
async fn a_folder_that_is_no_project_lists_its_folders_alone() {
    let service = Service(tempfile::tempdir().unwrap());
    let outside = tempfile::tempdir().unwrap();
    let outside = fs::canonicalize(outside.path()).unwrap();
    fs::create_dir_all(outside.join("code")).unwrap();
    fs::create_dir_all(outside.join("docs")).unwrap();
    fs::write(outside.join("notes.txt"), "private").unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    let listed = ask(
        &mut socket,
        json!({"type":"file_explorer_request","cwd":outside,"path":".","mode":"list"}),
    )
    .await;
    assert_eq!(listed["type"], "file_explorer_response", "{listed}");
    assert_eq!(listed["payload"]["cwd"], json!(outside));
    let names: Vec<&str> = listed["payload"]["directory"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["code", "docs"]);
    // Nothing else of it: not a file's text, not a folder under it by a
    // path of the request's, not a change.
    for refused in [
        json!({"type":"file_explorer_request","cwd":outside,"path":"notes.txt","mode":"file"}),
        json!({"type":"file_explorer_request","cwd":outside,"path":"code","mode":"list"}),
        json!({"type":"fs.entry.create.request","cwd":outside,"path":".","name":"made","kind":"directory"}),
    ] {
        let reply = ask(&mut socket, refused.clone()).await;
        assert_eq!(reply["type"], "rpc_error", "{refused} → {reply}");
    }
    assert!(!outside.join("made").exists());
    service.cli("stop");
}

#[tokio::test]
async fn a_conversation_s_artifacts_are_the_files_it_made_that_are_there() {
    let service = Service(tempfile::tempdir().unwrap());
    // A `codex` whose turn writes a page and says where a picture is.
    install(
        service.0.path(),
        "codex",
        r#"#!/bin/sh
case " $* " in *" app-server "*) ;; *) exit 2 ;; esac
say() { printf '%s\n' "$1"; }
while IFS= read -r line; do
    id=$(printf '%s' "$line" | sed -n 's/.*"id":\([0-9]*\).*/\1/p')
    case "$line" in
        *'"method":"initialize"'*) say "{\"id\":$id,\"result\":{}}" ;;
        *'"method":"thread/start"'*) say "{\"id\":$id,\"result\":{\"thread\":{\"id\":\"t-1\"}}}" ;;
        *'"method":"turn/start"'*)
            mkdir -p out && printf '<p>hi</p>' > out/page.html && printf 'x' > shot.png
            say "{\"id\":$id,\"result\":{\"turn\":{\"id\":\"turn-1\"}}}"
            say '{"method":"turn/started","params":{"turn":{"id":"turn-1"}}}'
            say "{\"method\":\"item/completed\",\"params\":{\"item\":{\"type\":\"fileChange\",\"id\":\"f1\",\"status\":\"completed\",\"changes\":[{\"path\":\"$PWD/out/page.html\",\"kind\":{\"type\":\"add\"},\"diff\":\"<p>hi</p>\"}]}}}"
            say '{"method":"item/completed","params":{"item":{"type":"agentMessage","id":"m1","text":"Saved `shot.png` and the page; see also missing.png"}}}'
            say '{"method":"turn/completed","params":{"turn":{"id":"turn-1","status":"completed"}}}' ;;
    esac
done
"#,
    );
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"codex"},"initialPrompt":"make it"}})).await;
    let agent = created["payload"]["agent"]["id"].clone();
    finished(&mut socket).await;
    for scope in ["agent", "project"] {
        let answer = ask(
            &mut socket,
            json!({"type":"agent.artifacts.request","agentId":agent,"scope":scope}),
        )
        .await;
        assert_eq!(answer["type"], "agent.artifacts.response", "{answer}");
        let mut names: Vec<_> = answer["payload"]["artifacts"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| {
                (
                    a["name"].as_str().unwrap().to_owned(),
                    a["kind"].as_str().unwrap().to_owned(),
                    a["source"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                (
                    "page.html".to_owned(),
                    "page".to_owned(),
                    "written".to_owned()
                ),
                (
                    "shot.png".to_owned(),
                    "image".to_owned(),
                    "named".to_owned()
                ),
            ],
            "{answer}"
        );
    }
}

#[tokio::test]
async fn a_session_claude_code_no_longer_has_goes_on_in_a_new_one() {
    let service = Service(tempfile::tempdir().unwrap());
    // A `claude` that resumes no session: asked to, it says it has none
    // and stops; started afresh, it begins `fresh-N` and replies.
    install(
        service.0.path(),
        "claude",
        r#"#!/bin/sh
printf '%s\n' "$*" >> args
say() { printf '%s\n' "$1"; }
n=$(cat starts 2>/dev/null || echo 0); n=$((n + 1)); echo "$n" > starts
while IFS= read -r line; do
    case "$line" in *'"type":"user"'*)
        printf '%s\n' "$line" >> sent
        case " $* " in
            *" --resume "*)
                say '{"type":"result","is_error":true,"errors":["No conversation found with session ID: gone"]}'
                exit 1 ;;
        esac
        say "{\"type\":\"system\",\"subtype\":\"init\",\"session_id\":\"fresh-$n\"}"
        say '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m","content":[{"type":"text","text":"ok"}]}}'
        say '{"type":"result","is_error":false}' ;;
    esac
done
"#,
    );
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"claude"},"initialPrompt":"first"}})).await;
    let agent = created["payload"]["agent"]["id"].clone();
    finished(&mut socket).await;
    // Its process gone, the next message resumes `fresh-1`, which it has not.
    service.cli("stop");
    service.cli("start");
    let mut socket = service.connect(true).await;
    ask(
        &mut socket,
        json!({"type":"send_agent_message_request","agentId":agent,"text":"second"}),
    )
    .await;
    finished(&mut socket).await;
    let timeline = ask(
        &mut socket,
        json!({"type":"fetch_agent_timeline_request","agentId":agent}),
    )
    .await;
    let kinds: Vec<_> = timeline["payload"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["item"]["type"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        kinds,
        [
            "user_message",
            "assistant_message",
            "user_message",
            "notification",
            "assistant_message"
        ],
        "{timeline}"
    );
    assert_eq!(
        timeline["payload"]["agent"]["persistence"]["sessionId"],
        "fresh-3"
    );
    let args = fs::read_to_string(service.0.path().join("args")).unwrap();
    assert_eq!(args.matches("--resume fresh-1").count(), 1, "{args}");
    // The new session is told what was said before, as T3 Code hands over.
    let sent = fs::read_to_string(service.0.path().join("sent")).unwrap();
    let last: Value = serde_json::from_str(sent.lines().last().unwrap()).unwrap();
    let told = last["message"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        told.contains("User: first\n\nAssistant: ok\n</earlier_conversation>"),
        "{last}"
    );
    assert!(!told.contains("second"), "{last}");
}

#[tokio::test]
async fn claude_code_s_guess_at_the_next_message_is_offered_until_one_is_sent() {
    let service = Service(tempfile::tempdir().unwrap());
    // A `claude` that, once a turn is over, guesses the next message as
    // Claude Code's prompt suggestions do (when asked to at initialize).
    install(
        service.0.path(),
        "claude",
        r#"#!/bin/sh
say() { printf '%s\n' "$1"; }
n=0
while IFS= read -r line; do
    case "$line" in
        *'"subtype":"initialize"'*) printf '%s\n' "$line" >> initialized ;;
        *'"type":"user"'*)
            n=$((n + 1))
            say '{"type":"system","subtype":"init","session_id":"s-1"}'
            say '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m","content":[{"type":"text","text":"ok"}]}}'
            say '{"type":"result","is_error":false}'
            say "{\"type\":\"prompt_suggestion\",\"suggestion\":\"go on with step $n\",\"session_id\":\"s-1\"}" ;;
    esac
done
"#,
    );
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"claude"},"initialPrompt":"first"}})).await;
    let agent = created["payload"]["agent"]["id"].clone();
    finished(&mut socket).await;
    let suggested = async |socket: &mut Socket| {
        for _ in 0..50 {
            let timeline = ask(socket, json!({"type":"fetch_agent_timeline_request","agentId":agent})).await;
            if let Some(text) = timeline["payload"]["agent"]["promptSuggestion"].as_str() {
                return text.to_owned();
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        panic!("no suggestion came");
    };
    assert_eq!(suggested(&mut socket).await, "go on with step 1");
    let asked = fs::read_to_string(service.0.path().join("initialized")).unwrap();
    assert!(asked.contains(r#""promptSuggestions":true"#), "{asked}");
    // A message sent: the guess is gone, and the next turn's comes.
    ask(&mut socket, json!({"type":"send_agent_message_request","agentId":agent,"text":"go on with step 1"})).await;
    finished(&mut socket).await;
    assert_eq!(suggested(&mut socket).await, "go on with step 2");
}

#[tokio::test]
async fn a_workspace_tells_its_branch_and_uncommitted_lines() {
    let service = Service(tempfile::tempdir().unwrap());
    install_provider(service.0.path());
    let repo = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let done = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "init.defaultBranch=main"])
            .args(args)
            .current_dir(repo.path())
            .output()
            .unwrap();
        assert!(done.status.success(), "{}", String::from_utf8_lossy(&done.stderr));
    };
    git(&["init"]);
    fs::write(repo.path().join("a.txt"), "one\ntwo\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "first"]);
    // One line changed, one added.
    fs::write(repo.path().join("a.txt"), "one\n2\nthree\n").unwrap();
    service.cli("start");
    let mut socket = service.connect(true).await;
    ask(&mut socket, json!({"type":"workspace.create.request",
        "source":{"kind":"directory","path":repo.path()},"agent":{"config":{"provider":"codex"}}})).await;
    loop {
        let frame = read(&mut socket).await;
        let workspace = &frame["message"]["payload"]["workspace"];
        if frame["message"]["type"] == "workspace_update" && !workspace["diffStat"].is_null() {
            assert_eq!(workspace["gitRuntime"]["currentBranch"], "main", "{workspace}");
            assert_eq!(workspace["diffStat"], json!({"additions":2,"deletions":1}), "{workspace}");
            break;
        }
    }
}

#[tokio::test]
async fn a_message_claude_code_lets_fall_as_a_turn_ends_is_sent_again() {
    let service = Service(tempfile::tempdir().unwrap());
    // A `claude` that, as its first turn ends, takes no message sent into
    // it (as Claude Code once did): it neither queues it nor says it back.
    // Sent again, it answers it.
    install(
        service.0.path(),
        "claude",
        r#"#!/bin/sh
printf '%s\n' "$*" >> args
say() { printf '%s\n' "$1"; }
turns=0
while IFS= read -r line; do
    case "$line" in *'"type":"user"'*)
        printf '%s\n' "$line" >> sent
        case "$line" in
            *'"second"'*)
                if [ ! -f dropped ]; then touch dropped; continue; fi
                id=$(printf '%s' "$line" | sed -n 's/.*"uuid":"\([^"]*\)".*/\1/p')
                say "{\"type\":\"user\",\"isReplay\":true,\"uuid\":\"$id\",\"message\":{\"role\":\"user\",\"content\":\"second\"}}"
                say '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m2","content":[{"type":"text","text":"two"}]}}'
                say '{"type":"result","is_error":false}' ;;
            *)
                say '{"type":"system","subtype":"init","session_id":"s-1"}'
                sleep 1
                say '{"type":"assistant","parent_tool_use_id":null,"message":{"id":"m1","content":[{"type":"text","text":"one"}]}}'
                say '{"type":"result","is_error":false}' ;;
        esac ;;
    esac
done
"#,
    );
    service.cli("start");
    let mut socket = service.connect(true).await;
    let created = ask(&mut socket, json!({"type":"workspace.create.request","source":{"kind":"directory","path":service.0.path()},
        "agent":{"config":{"provider":"claude"},"initialPrompt":"first"}})).await;
    let agent = created["payload"]["agent"]["id"].clone();
    // Into the turn underway, which ends without it.
    ask(&mut socket, json!({"type":"send_agent_message_request","agentId":agent,"text":"second",
        "messageId":"8f2c1d7e-0000-4000-8000-000000000002"})).await;
    let mut answered = false;
    for _ in 0..80 {
        let timeline = ask(&mut socket, json!({"type":"fetch_agent_timeline_request","agentId":agent})).await;
        let said: Vec<String> = timeline["payload"]["entries"].as_array().unwrap().iter()
            .filter(|row| row["item"]["type"] == "assistant_message")
            .map(|row| row["item"]["text"].as_str().unwrap_or_default().to_owned())
            .collect();
        if said.iter().any(|t| t.ends_with("two")) {
            answered = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    let sent = fs::read_to_string(service.0.path().join("sent")).unwrap_or_default();
    let timeline = ask(&mut socket, json!({"type":"fetch_agent_timeline_request","agentId":agent})).await;
    assert!(answered, "the dropped message was not sent again: {sent}\n{timeline}");
    assert_eq!(sent.matches(r#""second""#).count(), 2, "{sent}");
    let args = fs::read_to_string(service.0.path().join("args")).unwrap();
    assert!(args.contains("--replay-user-messages"), "{args}");
}
