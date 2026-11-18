mod account;
mod config;
mod env;
mod files;
mod git;
mod history;
mod host;
mod import;
mod models;
mod projects;
mod provider;
mod schedule;
mod state;
mod terminal;
mod timeline;
mod transfer;

use futures_util::{SinkExt, StreamExt};
use host::{Host, Shared, envelope};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    collections::HashMap,
    env::{self as environment},
    fs, io,
    net::SocketAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    rc::Rc,
    time::Duration,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{broadcast, mpsc},
    time::{MissedTickBehavior, interval, timeout},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

type Error = Box<dyn std::error::Error>;

async fn session(stream: TcpStream, shared: Shared, token: Rc<String>) -> Result<(), Error> {
    let mut socket = timeout(Duration::from_secs(8), accept_async(stream)).await??;
    let first = timeout(Duration::from_secs(8), socket.next())
        .await?
        .ok_or("Missing hello")??;
    let hello: Value = serde_json::from_str(first.to_text()?)?;
    if hello["type"] != "hello"
        || hello["protocolVersion"] != 1
        || hello.pointer("/auth/kind") != Some(&json!("localCredential"))
        || hello.pointer("/auth/token").and_then(Value::as_str) != Some(token.as_str())
    {
        socket
            .send(Message::text(
                json!({"type":"hello.rejected","reason":"Invalid hello or credential"}).to_string(),
            ))
            .await?;
        return Ok(());
    }
    let mut events = shared.borrow().events.subscribe();
    let mut terminal_output = shared.borrow().terminals.output.subscribe();
    let mut slots = HashMap::<String, (u8, u64)>::new();
    let mut uploads = HashMap::new();
    let mut watched_agents: Option<Vec<String>> = None;
    let mut watched_diff: Option<(String, Value)> = None;
    // Looked at again every few seconds while a diff is watched, one
    // look at a time: a tick missed while nothing was watched, or while
    // a look took long, is not made up for.
    let mut refresh = interval(Duration::from_secs(3));
    refresh.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut refreshing = false;
    let (done, mut finished) = mpsc::unbounded_channel();
    let info = envelope(
        "status",
        json!({"status":"server_info","version":env!("CARGO_PKG_VERSION"),
        "hostname":environment::var("HOSTNAME").unwrap_or_else(|_|"Sophon".into())}),
    );
    socket.send(Message::text(info.to_string())).await?;
    loop {
        let reply = tokio::select! {
            event = events.recv() => match event {
                Ok(event) => {
                    if event.pointer("/message/type").and_then(Value::as_str) == Some("agent_stream")
                        && event.pointer("/message/payload/event/type").and_then(Value::as_str) == Some("timeline")
                        && watched_agents.as_ref().is_some_and(|watched|!watched.iter().any(|id|event.pointer("/message/payload/agentId").and_then(Value::as_str)==Some(id))) {
                        continue;
                    }
                    event
                }
                // Too far behind to be caught up from here: the client
                // connects again and asks for everything as it is now.
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    eprintln!("A client fell {missed} events behind; its connection is closed");
                    break;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
            Some(done) = finished.recv() => match done {
                Done::Reply(reply) => reply,
                Done::Diff { cwd, payload, reply: Some(reply) } => {
                    if let Some(payload) = payload {
                        watched_diff = Some((cwd, payload));
                        refresh.reset();
                    }
                    reply
                }
                Done::Diff { cwd, payload, reply: None } => {
                    refreshing = false;
                    match (&mut watched_diff, payload) {
                        (Some((watched, last)), Some(payload)) if *watched == cwd && *last != payload => {
                            *last = payload.clone();
                            envelope("checkout_diff_update", payload)
                        }
                        _ => continue,
                    }
                }
            },
            _ = refresh.tick(), if watched_diff.is_some() && !refreshing => {
                let Some((cwd, _)) = &watched_diff else { continue };
                refreshing = true;
                let (cwd, done) = (cwd.clone(), done.clone());
                tokio::task::spawn_local(async move {
                    let payload = git::execute("subscribe_checkout_diff_request", &cwd, &Value::Null).await.ok();
                    let _ = done.send(Done::Diff { cwd, payload, reply: None });
                });
                continue;
            }
            output = terminal_output.recv() => {
                match output {
                    Ok((id, end, bytes)) => {
                        if let Some((slot, previous)) = slots.get_mut(&id) && end > *previous {
                            let mut frame = vec![1,*slot];
                            frame.extend_from_slice(&bytes[bytes.len().saturating_sub((end-*previous) as usize)..]);
                            *previous = end;
                            socket.send(Message::binary(frame)).await?;
                        }
                    }
                    // Output was missed, and what comes next would not
                    // join on: each terminal attached is sent again as
                    // it stands.
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        for (id, (slot, previous)) in &mut slots {
                            let Ok((end, bytes)) = shared.borrow().terminals.replay(id) else { continue };
                            let mut frame = vec![5,*slot];
                            frame.extend_from_slice(&bytes);
                            *previous = end;
                            socket.send(Message::binary(frame)).await?;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
                continue;
            }
            frame = socket.next() => {
                let Some(frame) = frame else { break };
                let data = match frame? {
                    Message::Text(data) => data,
                    Message::Close(_) => break,
                    Message::Ping(bytes) => {socket.send(Message::Pong(bytes)).await?; continue;}
                    Message::Binary(bytes) => {
                        // A frame that is not an upload's, or a disk
                        // that will not take it, ends that upload and
                        // nothing else.
                        match transfer::Upload::receive(&mut uploads,&bytes) {
                            Ok(Some(reply)) => socket.send(Message::text(reply.to_string())).await?,
                            Ok(None) => {}
                            Err(error) => eprintln!("An upload frame was refused: {error}"),
                        }
                        continue;
                    }
                    _ => continue,
                };
                let Ok(command) = serde_json::from_str::<Value>(&data) else { continue };
                match command["type"].as_str() {
                    Some("ping") => json!({"type":"pong"}),
                    Some("session") => {
                        let request = &command["message"];
                        let kind = request["type"].as_str().unwrap_or_default();
                        let result = match kind {
                            "agent.timeline.set_subscription.request" => {
                                watched_agents = Some(request["agentIds"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect());
                                Ok(envelope("agent.timeline.set_subscription.response",json!({"requestId":request["requestId"]})))
                            }
                            "unsubscribe_checkout_diff_request" => {
                                watched_diff = None;
                                Ok(envelope("unsubscribe_checkout_diff_response",json!({"requestId":request["requestId"]})))
                            }
                            "file.upload.request" => {
                                let home = shared.borrow().home.clone();
                                match transfer::Upload::begin(&home,request,&mut uploads) {
                                    Ok(()) => continue,
                                    Err(error) => Err(error.to_string()),
                                }
                            }
                            kind if kind.starts_with("schedule/") => schedule::handle(&shared,request),
                            "read_project_config_request" | "write_project_config_request" => {
                                let mut payload = projects::settings(&shared,request).unwrap_or_else(|error|json!({"ok":false,"error":{"message":error},"repoRoot":request["repoRoot"]}));
                                payload["requestId"] = request["requestId"].clone();
                                Ok(envelope(&kind.replace("request","response"),payload))
                            }
                            "file_explorer_request" if request["acceptBinary"] == true => {
                                let cwd = request["cwd"].as_str().unwrap_or_default();
                                if shared.borrow().state.workspaces.values().any(|workspace|workspace["workspaceDirectory"] == cwd) {
                                    match transfer::download(&mut socket,cwd,request).await {
                                        Ok(()) => continue,
                                        Err(error) => Err(error.to_string())
                                    }
                                } else {Err("Unknown workspace directory".into())}
                            }
                            // What may take as long as a network does (an
                            // account query, a push, a hook) is answered
                            // when it is done, off this loop: the client
                            // goes on being heard and told meanwhile.
                            "provider.usage.list.request" | "provider.credits.list.request" | "provider.credits.use.request" => {
                                let (kind, request, done) = (kind.to_owned(), request.clone(), done.clone());
                                tokio::task::spawn_local(async move {
                                    let reply = match account::handle(&request).await {
                                        Ok(mut payload) => {
                                            payload["requestId"] = request["requestId"].clone();
                                            envelope(&kind.replace("request","response"),payload)
                                        }
                                        Err(error) => envelope("rpc_error",json!({"requestId":request["requestId"],"error":error})),
                                    };
                                    let _ = done.send(Done::Reply(reply));
                                });
                                continue;
                            }
                            "subscribe_terminal_request" => {
                                let id = request["terminalId"].as_str().unwrap_or_default();
                                slots.remove(id);
                                let replay = shared.borrow().terminals.replay(id);
                                if let (Ok((end, bytes)),Some(slot)) = (replay,(0..=255).find(|slot|!slots.values().any(|(used,_)|used==slot))) {
                                    slots.insert(id.to_owned(),(slot,end));
                                    let reply = envelope("subscribe_terminal_response",json!({"requestId":request["requestId"],"terminalId":id,"slot":slot}));
                                    socket.send(Message::text(reply.to_string())).await?;
                                    let mut frame = vec![5,slot];
                                    frame.extend_from_slice(&bytes);
                                    socket.send(Message::binary(frame)).await?;
                                    continue;
                                }
                                Err("Cannot attach terminal".into())
                            }
                            "unsubscribe_terminal_request" => {
                                slots.remove(request["terminalId"].as_str().unwrap_or_default());
                                continue;
                            }
                            "subscribe_terminals_request" | "create_terminal_request" | "kill_terminal_request" | "terminal_input" => terminal::dispatch(&shared,request),
                            "list_commands_request" => {
                                let (shared, request, done) = (shared.clone(), request.clone(), done.clone());
                                tokio::task::spawn_local(async move {
                                    let reply = host::commands(&shared, &request).await.unwrap_or_else(|error| {
                                        envelope("rpc_error", json!({"requestId":request["requestId"],"error":error}))
                                    });
                                    let _ = done.send(Done::Reply(reply));
                                });
                                continue;
                            }
                            // A workspace on a new worktree waits on
                            // its checkout.
                            "workspace.create.request" => {
                                let (shared, request, done) = (shared.clone(), request.clone(), done.clone());
                                tokio::task::spawn_local(async move {
                                    let reply = host::create(&shared, &request).await.unwrap_or_else(|error| {
                                        envelope("rpc_error", json!({"requestId":request["requestId"],"error":error}))
                                    });
                                    let _ = done.send(Done::Reply(reply));
                                });
                                continue;
                            }
                            kind if git::handles(kind) => {
                                let cwd = request["cwd"].as_str().unwrap_or_default().to_owned();
                                if shared.borrow().state.workspaces.values().any(|workspace|workspace["workspaceDirectory"] == cwd.as_str()) {
                                    let (kind, request, done) = (kind.to_owned(), request.clone(), done.clone());
                                    tokio::task::spawn_local(async move {
                                        let result = git::execute(&kind,&cwd,&request).await;
                                        let mut payload = result.clone().unwrap_or_else(|error|json!({"cwd":cwd,"error":{"message":error}}));
                                        payload["requestId"] = request["requestId"].clone();
                                        let reply = envelope(&kind.replace("request","response"),payload);
                                        let _ = done.send(if kind == "subscribe_checkout_diff_request" {
                                            Done::Diff { cwd, payload: result.ok(), reply: Some(reply) }
                                        } else {
                                            Done::Reply(reply)
                                        });
                                    });
                                    continue;
                                } else {Err("Unknown workspace".into())}
                            }
                            _ => host::dispatch(&shared,request),
                        };
                        result.unwrap_or_else(|error| envelope("rpc_error",json!({"requestId":request["requestId"],"error":error})))
                    }
                    _ => continue,
                }
            }
        };
        socket.send(Message::text(reply.to_string())).await?;
    }
    let _ = socket.close(None).await;
    Ok(())
}

/// What a request answered off a session's loop came back with.
enum Done {
    Reply(Value),
    /// A workspace's diff: as first asked for (with the answer to that),
    /// or as looked at again since.
    Diff {
        cwd: String,
        payload: Option<Value>,
        reply: Option<Value>,
    },
}

fn service_lock(home: &Path) -> io::Result<fs::File> {
    fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(home.join("service.lock"))
}

fn running(home: &Path) -> Result<bool, Error> {
    match service_lock(home)?.try_lock() {
        Ok(()) => Ok(false),
        Err(fs::TryLockError::WouldBlock) => Ok(true),
        Err(fs::TryLockError::Error(error)) => Err(error.into()),
    }
}

async fn serve(home: PathBuf) -> Result<(), Error> {
    let lock = service_lock(&home)?;
    lock.try_lock()?;
    let cfg: Value = match fs::read(home.join("config.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Value::Null,
        Err(error) => return Err(error.into()),
    };
    let address: SocketAddr = environment::var("SOPHON_SERVICE_LISTEN")
        .unwrap_or_else(|_| {
            cfg.pointer("/daemon/listen")
                .and_then(Value::as_str)
                .unwrap_or("127.0.0.1:0")
                .into()
        })
        .parse()?;
    if !address.ip().is_loopback() {
        return Err("The agent service requires a loopback address".into());
    }
    let listener = TcpListener::bind(address).await?;
    let address = listener.local_addr()?;
    // Before anything is read: what the old service kept for this home.
    import::adopt(&home);
    let host = Host::new(home.clone(), state::State::load(&home)?)?;
    transfer::prune(&home);
    let token = Rc::new(state::id());
    state::write(&home, "local-credential", token.as_bytes())?;
    state::write(
        &home,
        "sophon.pid",
        &serde_json::to_vec(&json!({"listen":address.to_string(),"pid":std::process::id()}))?,
    )?;
    let host = Rc::new(RefCell::new(host));
    let mut poll = interval(Duration::from_millis(200));
    let mut schedules = interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = schedules.tick() => schedule::due(&host),
            // A connection that could not be taken (too many files
            // open, one gone before it was taken) is that connection's
            // loss, not the service's.
            result = listener.accept() => match result {
                Ok((stream, _)) => {
                    let (host, token) = (host.clone(), token.clone());
                    tokio::task::spawn_local(async move {
                        if let Err(error) = session(stream, host, token).await {
                            eprintln!("A connection ended: {error}");
                        }
                    });
                }
                Err(error) => {
                    eprintln!("A connection was not taken: {error}");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            },
            _ = poll.tick() => {
                if fs::read_to_string(home.join("stop")).ok().as_deref() == Some(token.as_str()) {
                    break;
                }
            }
        }
    }
    fs::remove_file(home.join("sophon.pid"))?;
    fs::remove_file(home.join("local-credential"))?;
    fs::remove_file(home.join("stop"))?;
    Ok(())
}

fn main() -> Result<(), Error> {
    // Before the runtime: nothing else reads the environment yet.
    env::scrub();
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run())
}

async fn run() -> Result<(), Error> {
    let args: Vec<_> = environment::args().skip(1).collect();
    if args.as_slice() == ["--version"] {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let explicit = args
        .windows(2)
        .find(|pair| pair[0] == "--home")
        .map(|pair| PathBuf::from(&pair[1]));
    let home = explicit
        .or_else(|| environment::var_os("SOPHON_SERVICE_HOME").map(PathBuf::from))
        .or_else(|| {
            environment::var_os("HOME").map(|home| PathBuf::from(home).join(".sophon/service"))
        })
        .ok_or("Missing service home")?;
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory.create(&home)?;
    if args.first().is_some_and(|action| action == "import") {
        let source = args
            .get(1)
            .ok_or("Usage: sophon-service import LEGACY_HOME --home NEW_HOME")?;
        let lock = service_lock(&home)?;
        lock.try_lock()?;
        let previous = state::State::load(&home)?;
        if !(previous.projects.is_empty()
            && previous.workspaces.is_empty()
            && previous.agents.is_empty())
        {
            return Err("Import requires an empty destination".into());
        }
        let state = import::load(Path::new(source))?;
        state.save(&home)?;
        println!(
            "Imported {} projects, {} workspaces and {} agents",
            state.projects.len(),
            state.workspaces.len(),
            state.agents.len()
        );
        return Ok(());
    }
    match args
        .first()
        .map(String::as_str)
        .zip(args.get(1).map(String::as_str))
    {
        Some(("daemon", "run")) => tokio::task::LocalSet::new().run_until(serve(home)).await,
        Some(("daemon", "start")) => {
            if running(&home)? && home.join("sophon.pid").is_file() {
                return Ok(());
            }
            let log = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(home.join("service.log"))?;
            let mut child = Command::new(environment::current_exe()?)
                .args(["daemon", "run", "--home"])
                .arg(&home)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(log)
                .spawn()?;
            for _ in 0..200 {
                if let Ok(bytes) = fs::read(home.join("sophon.pid"))
                    && let Ok(pid) = serde_json::from_slice::<Value>(&bytes)
                    && pid["pid"].as_u64() == Some(u64::from(child.id()))
                {
                    return Ok(());
                }
                if let Some(status) = child.try_wait()? {
                    if running(&home)? && home.join("sophon.pid").is_file() {
                        return Ok(());
                    }
                    return Err(format!(
                        "Service exited ({status}), see {}",
                        home.join("service.log").display()
                    )
                    .into());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            child.kill()?;
            child.wait()?;
            Err("Service startup timed out".into())
        }
        Some(("daemon", "stop")) => {
            if !running(&home)? {
                return Ok(());
            }
            state::write(&home, "stop", &fs::read(home.join("local-credential"))?)?;
            for _ in 0..200 {
                if !running(&home)? {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            Err("Service stop timed out".into())
        }
        _ => Err("Usage: sophon-service daemon run|start|stop [--home DIR]".into()),
    }
}
