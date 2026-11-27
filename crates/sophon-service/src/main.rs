mod account;
mod artifacts;
mod claude_usage;
mod config;
mod env;
mod files;
mod git;
mod history;
mod host;
mod import;
mod models;
mod pack;
mod plugin;
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
                            // A plugin comes from wherever git reaches.
                            kind if kind.starts_with("plugin/") => {
                                let (shared, request, done) = (shared.clone(), request.clone(), done.clone());
                                tokio::task::spawn_local(async move {
                                    let reply = plugin::handle(&shared, &request).await.unwrap_or_else(|error| {
                                        envelope("rpc_error", json!({"requestId":request["requestId"],"error":error}))
                                    });
                                    let _ = done.send(Done::Reply(reply));
                                });
                                continue;
                            }
                            // A reply script may take as long as the
                            // service it writes to.
                            "automation/act" | "automation/trigger" | "automation/try" => {
                                let (shared, request, done) = (shared.clone(), request.clone(), done.clone());
                                tokio::task::spawn_local(async move {
                                    let reply = schedule::asked(&shared, &request).await.unwrap_or_else(|error| {
                                        envelope("rpc_error", json!({"requestId":request["requestId"],"error":error}))
                                    });
                                    let _ = done.send(Done::Reply(reply));
                                });
                                continue;
                            }
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
                                let home = shared.borrow().home.clone();
                                tokio::task::spawn_local(async move {
                                    let reply = match account::handle(&request, &home).await {
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
                            "provider_diagnostic_request" | "fetch_recent_provider_sessions_request" | "workspace.recovery.restore.request"
                            | "provider.login.start.request" | "provider.login.wait.request" | "provider.login.cancel.request" => {
                                let (shared, request, done) = (shared.clone(), request.clone(), done.clone());
                                tokio::task::spawn_local(async move {
                                    let reply = match host::slow(&shared, &request).await {
                                        Some(Ok(reply)) => reply,
                                        Some(Err(error)) => envelope("rpc_error", json!({"requestId":request["requestId"],"error":error})),
                                        None => return,
                                    };
                                    let _ = done.send(Done::Reply(reply));
                                });
                                continue;
                            }
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
                                    let (kind, mut request, done) = (kind.to_owned(), request.clone(), done.clone());
                                    // A worktree's base is on record here, not in the request.
                                    if kind == "checkout_merge_request" && request["baseRef"].is_null() {
                                        request["baseRef"] = shared.borrow().state.workspaces.values()
                                            .find(|workspace| workspace["workspaceDirectory"] == cwd.as_str())
                                            .map(|workspace| workspace["gitRuntime"]["baseBranch"].clone())
                                            .unwrap_or(Value::Null);
                                    }
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
    let mut kept = state::State::load(&home)?;
    // Automations kept before integrations and plugins were told apart.
    if schedule::adopt(&home, &mut kept.schedules) {
        kept.save(&home)?;
    }
    let host = Host::new(home.clone(), kept)?;
    transfer::prune(&home);
    let token = Rc::new(state::id());
    state::write(&home, "local-credential", token.as_bytes())?;
    state::write(
        &home,
        "sophon.pid",
        &serde_json::to_vec(&json!({"listen":address.to_string(),"pid":std::process::id(),"version":env!("CARGO_PKG_VERSION")}))?,
    )?;
    // `sophon` for the agents of automations: this program, under the
    // name their prompts are told.
    #[cfg(unix)]
    {
        let bin = home.join("bin");
        let _ = fs::create_dir_all(&bin);
        let _ = fs::remove_file(bin.join("sophon"));
        if let Err(error) = environment::current_exe()
            .and_then(|program| std::os::unix::fs::symlink(program, bin.join("sophon")))
        {
            eprintln!("`sophon` is not there for automations: {error}");
        }
    }
    let host = Rc::new(RefCell::new(host));
    let mut again = false;
    let mut poll = interval(Duration::from_millis(200));
    let mut schedules = interval(Duration::from_secs(1));
    // The workspaces' branches and uncommitted lines, counted when one is
    // new or a turn in it began or ended (`watch_checkouts`): this only
    // looks at what is marked.
    let mut checkouts = interval(Duration::from_secs(2));
    checkouts.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            _ = schedules.tick() => schedule::due(&host),
            _ = checkouts.tick() => host::watch_checkouts(&host),
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
                // Asked to start again: once the answer to that has gone.
                if host.borrow().restart {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    again = true;
                    break;
                }
            }
        }
    }
    fs::remove_file(home.join("sophon.pid"))?;
    fs::remove_file(home.join("local-credential"))?;
    let _ = fs::remove_file(home.join("stop"));
    if again {
        // The next one takes the lock this one lets go of, and says so
        // in the same log.
        drop(host);
        drop(lock);
        let log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(home.join("service.log"))?;
        Command::new(environment::current_exe()?)
            .args(["daemon", "run", "--home"])
            .arg(&home)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(log)
            .spawn()?;
    }
    Ok(())
}

/// One request of the service running in `home`, as a client of it,
/// and its answer.
async fn request(home: &Path, mut message: Value) -> Result<Value, Error> {
    let pid: Value = serde_json::from_slice(
        &fs::read(home.join("sophon.pid")).map_err(|_| "The Sophon service is not running")?,
    )?;
    let token = fs::read_to_string(home.join("local-credential"))?;
    let url = format!(
        "ws://{}/ws",
        pid["listen"].as_str().ok_or("No service address")?
    );
    let (mut socket, _) = timeout(
        Duration::from_secs(8),
        tokio_tungstenite::connect_async(url),
    )
    .await??;
    let hello =
        json!({"type":"hello","protocolVersion":1,"auth":{"kind":"localCredential","token":token}});
    socket.send(Message::text(hello.to_string())).await?;
    let id = state::id();
    message["requestId"] = json!(id);
    socket
        .send(Message::text(
            json!({"type":"session","message":message}).to_string(),
        ))
        .await?;
    // As long as a reply script may take, and a little.
    let answer = timeout(Duration::from_secs(40), async {
        while let Some(frame) = socket.next().await {
            let Ok(text) = frame?.into_text() else {
                continue;
            };
            let Ok(frame) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if frame["type"] == "hello.rejected" {
                return Err("The service refused this client".into());
            }
            if frame.pointer("/message/payload/requestId") == Some(&json!(id)) {
                return Ok(frame["message"].clone());
            }
        }
        Err::<Value, Error>("The service closed the connection".into())
    })
    .await??;
    let _ = socket.close(None).await;
    if answer["type"] == "rpc_error" {
        return Err(answer["payload"]["error"]
            .as_str()
            .unwrap_or("The service refused")
            .into());
    }
    Ok(answer["payload"].clone())
}

/// What a file handed on holds, a line each: its packages (and whether
/// each is in it), the settings it fills in by name, and its automation.
fn said_of_a_file(about: &Value) {
    for package in about["packages"].as_array().into_iter().flatten() {
        println!(
            "{}\t{}\t{}\t{}",
            package["id"].as_str().unwrap_or_default(),
            package["kind"].as_str().unwrap_or_default(),
            package["name"].as_str().unwrap_or_default(),
            if package["included"] == true {
                "in the file"
            } else {
                "named only"
            }
        );
    }
    for setting in about["settings"].as_array().into_iter().flatten() {
        println!(
            "setting\t{}.{}{}",
            setting["package"].as_str().unwrap_or_default(),
            setting["name"].as_str().unwrap_or_default(),
            if setting["secret"] == true {
                " (secret)"
            } else {
                ""
            }
        );
    }
    if let Some(name) = about["automation"]["name"].as_str() {
        println!("automation\t{name}");
    }
}

/// `sophon plugin list|add|update|remove`, for integrations and plugins
/// alike: each is named by its id, whichever it is (`sophon integration`
/// is the same command).
async fn plugins(home: &Path, args: &[String]) -> Result<(), Error> {
    let words: Vec<&str> = {
        let mut words = Vec::new();
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            if arg == "--home" {
                rest.next();
            } else {
                words.push(arg.as_str());
            }
        }
        words
    };
    // Its one-time setup (a sign-in), run here where there is someone
    // to answer it: the plugin's own command, with what follows the id.
    if let ["setup", id, rest @ ..] = words.as_slice() {
        let found = plugin::find(home, id).ok_or("Unknown plugin")?;
        let setup = found["setup"]
            .as_str()
            .ok_or("There is nothing to set up here")?;
        let data = plugin::data(home, id);
        fs::create_dir_all(&data)?;
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!("{setup} \"$@\""))
            .arg("sophon-plugin")
            .args(rest)
            .current_dir(found["dir"].as_str().unwrap_or_default())
            .envs(plugin::environment(home, &found))
            .status()?;
        return if status.success() {
            Ok(())
        } else {
            Err(format!("Setup ended with {status}").into())
        };
    }
    // One as a file to hand on: the package, what it counts on and the
    // settings of each, written where asked (its id's name, here, unless
    // a file is named).
    if let ["pack", id, rest @ ..] = words.as_slice() {
        let settings = if rest.contains(&"--no-settings") {
            "none"
        } else if rest.contains(&"--no-secrets") {
            "plain"
        } else {
            "all"
        };
        let answer = request(
            home,
            json!({"type":"plugin/pack","pluginId":id,"settings":settings}),
        )
        .await?;
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(answer["archive"].as_str().unwrap_or_default())?;
        let file = rest
            .iter()
            .find(|word| !word.starts_with("--"))
            .map(|file| (*file).to_owned())
            .unwrap_or_else(|| {
                answer["fileName"]
                    .as_str()
                    .unwrap_or("plugin.sophon")
                    .to_owned()
            });
        fs::write(&file, bytes)?;
        println!("Saved {file}");
        said_of_a_file(&answer["about"]);
        return Ok(());
    }
    // And such a file taken in: said first, then installed. Asking for
    // it here is the saying so that a client waits for.
    if let ["add", file] = words.as_slice()
        && file.ends_with(".sophon")
        && Path::new(file).is_file()
    {
        use base64::Engine;
        let read = request(
            home,
            json!({"type":"plugin/unpack","archive":base64::engine::general_purpose::STANDARD.encode(fs::read(file)?)}),
        )
        .await?;
        said_of_a_file(&read["about"]);
        let done = request(home, json!({"type":"plugin/install","token":read["token"]})).await?;
        println!(
            "Installed {}",
            read["about"]["name"].as_str().unwrap_or_default()
        );
        if let Some(name) = done["installed"]["automation"]["name"].as_str() {
            println!(
                "It came with the automation “{name}”: add the file in the app to have that opened, filled in"
            );
        }
        return Ok(());
    }
    let message = match words.as_slice() {
        ["list"] | [] => json!({"type":"plugin/list"}),
        ["add", source] => {
            // A folder is said in full: the service is somewhere else.
            let folder = Path::new(source);
            let source = match fs::canonicalize(folder) {
                Ok(path) if path.is_dir() => path.display().to_string(),
                _ => (*source).to_owned(),
            };
            json!({"type":"plugin/add","source":source})
        }
        ["update", id] => json!({"type":"plugin/update","pluginId":id}),
        ["export", id] => json!({"type":"plugin/export","pluginId":id}),
        ["import", config] => json!({"type":"plugin/import","config":config}),
        ["set", id, pairs @ ..] if !pairs.is_empty() => {
            let mut values = json!({});
            for pair in pairs {
                let (name, value) = pair.split_once('=').ok_or("Usage: sophon plugin set ID NAME=VALUE...")?;
                values[name] = json!(value);
            }
            json!({"type":"plugin/set","pluginId":id,"values":values})
        }
        ["remove", id] => json!({"type":"plugin/remove","pluginId":id}),
        ["restore", id] => json!({"type":"plugin/restore","pluginId":id}),
        ["on", id] => json!({"type":"plugin/enable","pluginId":id,"enabled":true}),
        ["off", id] => json!({"type":"plugin/enable","pluginId":id,"enabled":false}),
        _ => return Err("Usage: sophon plugin list | add GIT_URL_OR_FOLDER | setup ID [ARGS] | set ID NAME=VALUE... | export ID | import SETTINGS | pack ID [--no-secrets | --no-settings] [FILE] | add FILE.sophon | on ID | off ID | update ID | remove ID | restore ID".into()),
    };
    let answer = request(home, message).await?;
    if let Some(config) = answer["config"].as_str() {
        println!("{config}");
        return Ok(());
    }
    if words.first() == Some(&"import") {
        println!("Set");
        return Ok(());
    }
    if let Some(plugin) = answer["plugin"].as_object() {
        if words.first() == Some(&"set") {
            println!("Set");
            return Ok(());
        }
        println!(
            "Installed {} ({})",
            plugin["name"].as_str().unwrap_or_default(),
            plugin["id"].as_str().unwrap_or_default()
        );
        // What is left for this computer to fill in.
        for param in plugin["params"].as_array().into_iter().flatten() {
            if param["filled"] != true {
                println!(
                    "Needs {}: sophon plugin set {} {}=…",
                    param["label"].as_str().unwrap_or_default(),
                    plugin["id"].as_str().unwrap_or_default(),
                    param["name"].as_str().unwrap_or_default()
                );
            }
        }
        // A sign-in is an integration's: a plugin has none to ask for.
        if plugin.get("setup").is_some_and(Value::is_string) {
            println!(
                "Set it up once: sophon plugin setup {}",
                plugin["id"].as_str().unwrap_or_default()
            );
        }
        return Ok(());
    }
    for plugin in answer["plugins"].as_array().into_iter().flatten() {
        let note = plugin["error"]
            .as_str()
            .or(plugin["description"].as_str())
            .unwrap_or_default();
        println!(
            "{}\t{}\t{}\t{note}",
            plugin["id"].as_str().unwrap_or_default(),
            plugin["kind"].as_str().unwrap_or("unreadable"),
            plugin["name"].as_str().unwrap_or_default()
        );
    }
    Ok(())
}

/// `sophon reply|react|trigger`: what an automation's agent, or a
/// program that wants one started, says to the service.
async fn automation(home: &Path, action: &str, args: &[String]) -> Result<(), Error> {
    let mut words = Vec::new();
    let mut event = json!({"vars":{}});
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match (action, arg.as_str()) {
            (_, "--home") => {
                rest.next();
            }
            ("trigger", "--id") => event["id"] = json!(rest.next().ok_or("--id needs a value")?),
            ("trigger", "--var") => {
                let (name, value) = rest
                    .next()
                    .and_then(|pair| pair.split_once('='))
                    .ok_or("--var needs name=value")?;
                event["vars"][name] = json!(value);
            }
            _ => words.push(arg.as_str()),
        }
    }
    if action == "trigger" {
        let [automation] = words.as_slice() else {
            return Err(
                "Usage: sophon trigger AUTOMATION [--id EVENT_ID] [--var name=value]...".into(),
            );
        };
        let answer = request(
            home,
            json!({"type":"automation/trigger","automation":automation,"event":event}),
        )
        .await?;
        if answer["duplicate"] == true {
            println!("Already had this event");
        } else {
            println!(
                "Started run {}",
                answer["runId"].as_str().unwrap_or_default()
            );
        }
        return Ok(());
    }
    let token = environment::var("SOPHON_RUN_TOKEN")
        .map_err(|_| "This is not an automation's run: there is nobody to answer")?;
    let text = words.join(" ");
    if text.trim().is_empty() {
        return Err(format!(
            "Usage: sophon {action} {}",
            if action == "react" { "NAME" } else { "TEXT" }
        )
        .into());
    }
    let answer = request(
        home,
        json!({"type":"automation/act","token":token,"action":action,"text":text}),
    )
    .await?;
    if answer["delivered"] == true {
        println!("Sent");
    } else {
        println!("Noted on the run: this automation has no reply script to send it with");
    }
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
    // What a plugin offers, used: its answer is this command's.
    if args.first().is_some_and(|action| action == "use") {
        let Some(id) = args.get(1) else {
            return Err("Usage: sophon use PLUGIN [ARGUMENTS]".into());
        };
        let answer = request(
            &home,
            json!({"type":"plugin/use","pluginId":id,"args":&args[2..]}),
        )
        .await?;
        print!("{}", answer["output"].as_str().unwrap_or_default());
        eprint!("{}", answer["errors"].as_str().unwrap_or_default());
        let code = answer["code"].as_i64().unwrap_or(1);
        if code != 0 {
            std::process::exit(code as i32);
        }
        return Ok(());
    }
    if args
        .first()
        .is_some_and(|action| action == "plugin" || action == "integration")
    {
        return plugins(&home, &args[1..]).await;
    }
    if let Some(action @ ("reply" | "react" | "trigger")) = args.first().map(String::as_str) {
        return automation(&home, action, &args[1..]).await;
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
