#![cfg(unix)]

use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command, time::Duration};
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

/// How long a frame is waited for: a moment with the stand-in CLIs,
/// as long as a model takes with the real ones.
static PATIENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(5);

async fn read(socket: &mut Socket) -> Value {
    let patience = Duration::from_secs(PATIENCE.load(std::sync::atomic::Ordering::Relaxed));
    let frame = tokio::time::timeout(patience, socket.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    serde_json::from_str(frame.to_text().unwrap()).unwrap()
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
    assert_eq!(
        rows[1]["item"],
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
        timeline["payload"]["entries"][3]["item"],
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
