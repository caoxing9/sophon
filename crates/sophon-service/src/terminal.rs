use crate::{
    host::{Shared, envelope},
    state::id,
};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, VecDeque},
    io::{Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    time::Duration,
};
use tokio::sync::{broadcast, mpsc};

const HISTORY: usize = 256 * 1024;

pub struct Terminal {
    cwd: String,
    master: Box<dyn MasterPty + Send>,
    input: SyncSender<Vec<u8>>,
    child: Box<dyn ChildKiller + Send + Sync>,
    /// Its shell has ended and been waited for: its process id may be
    /// another process's by now, and is not signalled again.
    exited: Arc<AtomicBool>,
    bytes: VecDeque<u8>,
    offset: u64,
}

/// Hangs up on the shell if it still runs. The terminal's end of the
/// pty closes with this too, which is what tells anything else still
/// attached to it (a job left in the background) that it is gone.
impl Drop for Terminal {
    fn drop(&mut self) {
        if !self.exited.load(Ordering::Relaxed) {
            let _ = self.child.kill();
        }
    }
}

pub struct Terminals {
    entries: HashMap<String, Terminal>,
    pub output: broadcast::Sender<(String, u64, Vec<u8>)>,
}

impl Default for Terminals {
    fn default() -> Self {
        Self {
            entries: HashMap::new(),
            output: broadcast::channel(256).0,
        }
    }
}

impl Terminals {
    fn list(&self, cwd: &str) -> Value {
        json!({"cwd":cwd,"terminals":self.entries.iter().filter(|(_,entry)|entry.cwd == cwd)
            .map(|(id,_)|json!({"id":id,"name":"Terminal"})).collect::<Vec<_>>()})
    }

    /// Takes the terminal off the list and lets it go.
    fn close(&mut self, id: &str, events: &broadcast::Sender<Value>) {
        if let Some(entry) = self.entries.remove(id) {
            let _ = events.send(envelope("terminal_stream_exit", json!({"terminalId":id})));
            let _ = events.send(envelope("terminals_changed", self.list(&entry.cwd)));
        }
    }

    pub fn replay(&self, id: &str) -> Result<(u64, Vec<u8>), String> {
        let entry = self.entries.get(id).ok_or("Unknown terminal")?;
        Ok((entry.offset, entry.bytes.iter().copied().collect()))
    }
}

fn create(shared: &Shared, cwd: &str) -> Result<Value, String> {
    let pair = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| error.to_string())?;
    let mut command = CommandBuilder::new_default_prog();
    command.cwd(cwd);
    command.env("TERM", "xterm-256color");
    // `sophon` (a plugin's setup, a trigger by hand) is this service's.
    let home = shared.borrow().home.clone();
    command.env("SOPHON_SERVICE_HOME", &home);
    command.env(
        "PATH",
        format!(
            "{}:{}",
            home.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        ),
    );
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| error.to_string())?;
    let mut writer = pair
        .master
        .take_writer()
        .map_err(|error| error.to_string())?;
    let mut child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| error.to_string())?;
    let killer = child.clone_killer();
    let (input, input_rx) = sync_channel::<Vec<u8>>(64);
    let (bytes_tx, mut bytes_rx) = mpsc::channel(32);
    std::thread::spawn(move || {
        for bytes in input_rx {
            if writer
                .write_all(&bytes)
                .and_then(|_| writer.flush())
                .is_err()
            {
                break;
            }
        }
    });
    std::thread::spawn(move || {
        let mut buffer = [0; 8192];
        while let Ok(count) = reader.read(&mut buffer) {
            if count == 0 || bytes_tx.blocking_send(buffer[..count].to_vec()).is_err() {
                break;
            }
        }
    });
    let exited = Arc::new(AtomicBool::new(false));
    let (exit_tx, mut exit_rx) = mpsc::channel::<()>(1);
    {
        let exited = exited.clone();
        std::thread::spawn(move || {
            let _ = child.wait();
            exited.store(true, Ordering::Relaxed);
            let _ = exit_tx.blocking_send(());
        });
    }
    let terminal_id = id();
    let mut host = shared.borrow_mut();
    host.terminals.entries.insert(
        terminal_id.clone(),
        Terminal {
            cwd: cwd.into(),
            master: pair.master,
            input,
            child: killer,
            exited,
            bytes: VecDeque::new(),
            offset: 0,
        },
    );
    let _ = host
        .events
        .send(envelope("terminals_changed", host.terminals.list(cwd)));
    let shared = shared.clone();
    let id = terminal_id.clone();
    tokio::task::spawn_local(async move {
        // The terminal lasts as long as its shell. Output ending is not
        // waited for: a job the shell left in the background keeps the
        // pty open, and would keep a terminal no one can use on the
        // list. What the shell wrote last is read for a moment more.
        let mut last = None;
        loop {
            let bytes = match last {
                None => tokio::select! {
                    bytes = bytes_rx.recv() => bytes,
                    _ = exit_rx.recv() => {
                        last = Some(tokio::time::Instant::now() + Duration::from_millis(400));
                        continue;
                    }
                },
                Some(until) => tokio::time::timeout_at(until, bytes_rx.recv())
                    .await
                    .ok()
                    .flatten(),
            };
            let Some(bytes) = bytes else { break };
            let mut host = shared.borrow_mut();
            let Some(entry) = host.terminals.entries.get_mut(&id) else {
                return;
            };
            entry.offset += bytes.len() as u64;
            let offset = entry.offset;
            entry.bytes.extend(&bytes);
            entry
                .bytes
                .drain(..entry.bytes.len().saturating_sub(HISTORY));
            let _ = host.terminals.output.send((id.clone(), offset, bytes));
        }
        let host = &mut *shared.borrow_mut();
        host.terminals.close(&id, &host.events);
    });
    Ok(json!({"terminal":{"id":terminal_id,"name":"Terminal"}}))
}

pub fn dispatch(shared: &Shared, request: &Value) -> Result<Value, String> {
    let cwd = request["cwd"].as_str().unwrap_or_default();
    let kind = request["type"].as_str().unwrap_or_default();
    let (kind, mut payload) = match kind {
        "create_terminal_request" => {
            if !shared
                .borrow()
                .state
                .workspaces
                .values()
                .any(|workspace| workspace["workspaceDirectory"] == cwd)
            {
                return Err("Unknown workspace".into());
            }
            ("create_terminal_response", create(shared, cwd)?)
        }
        "subscribe_terminals_request" => (
            "list_terminals_response",
            shared.borrow().terminals.list(cwd),
        ),
        "kill_terminal_request" | "terminal_input" => {
            let mut host = shared.borrow_mut();
            let id = request["terminalId"]
                .as_str()
                .ok_or("Missing terminal id")?;
            // Ended here and now, whatever its shell makes of being
            // hung up on.
            if kind == "kill_terminal_request" {
                let host = &mut *host;
                if !host.terminals.entries.contains_key(id) {
                    return Err("Unknown terminal".into());
                }
                host.terminals.close(id, &host.events);
                let mut payload = json!({});
                payload["requestId"] = request["requestId"].clone();
                return Ok(envelope("terminal_input_response", payload));
            }
            let terminal = host
                .terminals
                .entries
                .get_mut(id)
                .ok_or("Unknown terminal")?;
            let message = &request["message"];
            if message["type"] == "resize" {
                let size = |name: &str| message[name].as_u64().unwrap_or(1).clamp(1, 1024) as u16;
                terminal
                    .master
                    .resize(PtySize {
                        rows: size("rows"),
                        cols: size("cols"),
                        pixel_height: 0,
                        pixel_width: 0,
                    })
                    .map_err(|error| error.to_string())?;
            } else {
                terminal
                    .input
                    .try_send(
                        message["data"]
                            .as_str()
                            .ok_or("Missing input")?
                            .as_bytes()
                            .to_vec(),
                    )
                    .map_err(|error| error.to_string())?;
            }
            ("terminal_input_response", json!({}))
        }
        _ => return Err("Unknown terminal request".into()),
    };
    payload["requestId"] = request["requestId"].clone();
    Ok(envelope(kind, payload))
}
