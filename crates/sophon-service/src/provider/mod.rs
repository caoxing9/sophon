//! Agents: the host's own `claude` and `codex`, each driven over the
//! protocol its CLI speaks to a program (Claude Code's stream of JSON
//! with its control requests, Codex's app server).
//!
//! An agent here is a session that outlives its process: it is given
//! commands (a prompt, a stop, an answer to what it asked) and says what
//! happens as events, and starts its CLI when there is something to do,
//! resuming the conversation the CLI keeps under the session's id. The
//! process goes after a while with nothing to do and comes back with the
//! next prompt.

mod claude;
mod codex;

use serde_json::{Value, json};
use std::{
    cell::RefCell, collections::VecDeque, env, io, path::PathBuf, process::Stdio, rc::Rc,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout},
    sync::mpsc,
};

/// How long an agent's process stays with nothing running in it.
const IDLE: Duration = Duration::from_secs(15 * 60);

pub enum Command {
    /// Into the turn underway if there is one, else a new turn.
    Prompt {
        text: String,
        images: Vec<Value>,
    },
    Interrupt,
    /// The client's answer to the permission `id`.
    Answer {
        id: String,
        response: Value,
    },
    Mode(String),
    Model(Option<String>),
    Thinking(Option<String>),
}

pub enum Event {
    /// The id the CLI keeps the conversation under.
    Session(String),
    /// One item of the timeline's stream.
    Item(Value),
    /// The agent waits on this.
    Asked(Value),
    /// It no longer waits on the permission of this id.
    Settled(String),
    Usage(Value),
    Mode(String),
    /// The commands the CLI has where the agent runs.
    Commands(Value),
    /// An agent this one started, by its id: what is known of it now
    /// (`title`, `subtitle`, `status`), to add to what was.
    Subagent(Value),
    /// An item of that agent's own timeline.
    Sub {
        id: String,
        item: Value,
    },
    /// A turn is underway.
    Busy,
    /// Nothing is; how what was underway ended.
    Idle(Outcome),
}

pub enum Outcome {
    Completed,
    Canceled,
    Failed(String),
}

/// What an agent starts as: its record's settings and its provider's.
pub struct Start {
    pub family: &'static str,
    pub cwd: String,
    pub settings: Value,
    pub session: Option<String>,
    pub model: Option<String>,
    pub thinking: Option<String>,
    pub mode: String,
    pub cost: f64,
}

pub type Emit = Box<dyn FnMut(Event)>;

/// Starts the agent's session on this thread's `LocalSet`; it ends when
/// the sender is dropped, and its process with it.
pub fn start(start: Start, emit: Emit) -> mpsc::UnboundedSender<Command> {
    let (commands, inbox) = mpsc::unbounded_channel();
    if start.family == "codex" {
        tokio::task::spawn_local(codex::run(start, inbox, emit));
    } else {
        tokio::task::spawn_local(claude::run(start, inbox, emit));
    }
    commands
}

/// The commands `family`'s CLI offers in `cwd`, as a client lists them
/// when `/` is typed. Codex's are its terminal's own and it does not
/// say what they are.
pub async fn commands(family: &str, settings: &Value, cwd: &str) -> io::Result<Value> {
    if family == "claude" {
        claude::commands(settings, cwd).await
    } else {
        Ok(json!([]))
    }
}

pub(crate) fn binary(provider: &str) -> Option<PathBuf> {
    if !matches!(provider, "codex" | "claude") {
        return None;
    }
    let mut paths: Vec<_> = env::split_paths(&env::var_os("PATH").unwrap_or_default())
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(provider))
        .collect();
    if let Some(home) = env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(".local/bin").join(provider));
    }
    if cfg!(target_os = "macos") && provider == "codex" {
        for app in ["ChatGPT", "Codex"] {
            paths.push(
                format!("/Applications/{app}.app/Contents/Resources/codex-cli/bin/codex").into(),
            );
        }
    }
    paths.into_iter().find(|path| path.is_file())
}

fn executable(family: &str, settings: &Value) -> Option<PathBuf> {
    let explicit = settings.pointer("/command/0").and_then(Value::as_str);
    if explicit.is_none() || explicit == Some(family) {
        return binary(family);
    }
    explicit
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
}

/// The CLI a provider is: itself, or the one its settings extend.
pub fn family(provider: &str, settings: &Value) -> Option<&'static str> {
    match settings["extends"].as_str().unwrap_or(provider) {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        _ => None,
    }
}

/// The modes an agent of `family` can be in, the first the default.
pub fn modes(family: &str) -> Value {
    if family == "codex" {
        json!([
            {"id":"auto","label":"Default Permissions","description":"Edit files and run commands with Codex's own approval settings."},
            {"id":"read-only","label":"Read Only","description":"Read files and answer; edits and commands ask first."},
            {"id":"full-access","label":"Full Access","description":"Edit files, run commands, and access the network without additional prompts."},
        ])
    } else {
        json!([
            {"id":"default","label":"Always Ask","description":"Prompts for permission the first time a tool is used"},
            {"id":"acceptEdits","label":"Accept File Edits","description":"Automatically approves edit-focused tools without prompting"},
            {"id":"plan","label":"Plan Mode","description":"Analyze the codebase without executing tools or edits"},
            {"id":"auto","label":"Auto mode","description":"Uses a model classifier to review permission prompts automatically"},
            {"id":"bypassPermissions","label":"Bypass","description":"Skip all permission prompts (use with caution)"},
        ])
    }
}

/// `wanted` if `family` has that mode, else its default.
pub fn mode(family: &str, wanted: Option<&str>) -> String {
    let modes = modes(family);
    let ids = || {
        modes
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|mode| mode["id"].as_str())
    };
    wanted
        .filter(|wanted| ids().any(|id| id == *wanted))
        .or_else(|| ids().next())
        .unwrap_or_default()
        .to_owned()
}

pub fn snapshot(config: &Value) -> Value {
    let mut providers = std::collections::BTreeSet::from(["codex", "claude"]);
    if let Some(settings) = config["providers"].as_object() {
        providers.extend(settings.keys().map(String::as_str));
    }
    let entries: Vec<_> = providers
        .into_iter()
        .map(|provider| {
            let settings = &config["providers"][provider];
            let family = family(provider, settings);
            let ready = family.is_some_and(|family| executable(family, settings).is_some());
            let modes = family.map(modes).unwrap_or_else(|| json!([]));
            // The models its settings list, if they list any (a
            // provider set up for another endpoint names its own);
            // else what its CLI lists on this host.
            let models = match (settings["models"].as_array(), family) {
                (Some(named), _) if !named.is_empty() => settings["models"].clone(),
                (_, Some(family)) => json!(crate::models::list(family, settings)),
                _ => json!([]),
            };
            json!({"provider":provider,"label":settings["label"],
                "enabled":settings["enabled"].as_bool().unwrap_or(true),"models":models,
                "defaultModeId":modes[0]["id"],"modes":modes,
                "status":if ready {"ready"} else {"unavailable"}})
        })
        .collect();
    json!({"entries":entries})
}

/// A CLI running with its input and output held, and the last of what
/// it wrote to its error stream (why it stopped, when it stops).
struct Process {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
    errors: Rc<RefCell<VecDeque<String>>>,
}

impl Process {
    /// Starts `family`'s CLI in `cwd` as its provider's settings say:
    /// their command, then `args`, with `env` and then their own `env`
    /// over the service's.
    fn spawn(
        family: &str,
        settings: &Value,
        cwd: &str,
        args: &[String],
        env: &[(&str, &str)],
    ) -> io::Result<Self> {
        if settings["enabled"] == false {
            return Err(io::Error::other("This provider is disabled"));
        }
        let program = executable(family, settings)
            .ok_or_else(|| io::Error::other(format!("{family} is not installed on this host")))?;
        let mut command = tokio::process::Command::new(program);
        if let Some(extra) = settings["command"].as_array() {
            command.args(extra.iter().skip(1).filter_map(Value::as_str));
        }
        command.args(args).envs(env.iter().copied());
        if let Some(env) = settings["env"].as_object() {
            command.envs(
                env.iter()
                    .filter_map(|(key, value)| Some((key, value.as_str()?))),
            );
        }
        let mut child = command
            .current_dir(cwd)
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let errors = Rc::new(RefCell::new(VecDeque::new()));
        if let Some(stderr) = child.stderr.take() {
            let errors = errors.clone();
            let name = family.to_owned();
            tokio::task::spawn_local(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("{name}: {line}");
                    let mut kept = errors.borrow_mut();
                    if kept.len() == 8 {
                        kept.pop_front();
                    }
                    kept.push_back(line);
                }
            });
        }
        Ok(Self {
            input: child
                .stdin
                .take()
                .ok_or_else(|| io::Error::other("No CLI input"))?,
            output: BufReader::new(
                child
                    .stdout
                    .take()
                    .ok_or_else(|| io::Error::other("No CLI output"))?,
            )
            .lines(),
            child,
            errors,
        })
    }

    async fn send(&mut self, message: &Value) -> io::Result<()> {
        self.input
            .write_all(format!("{message}\n").as_bytes())
            .await?;
        self.input.flush().await
    }

    /// Why it stopped, once its output has ended: how it exited, and
    /// what it last said on its error stream.
    async fn ended(mut self) -> String {
        let status = match tokio::time::timeout(Duration::from_secs(3), self.child.wait()).await {
            Ok(Ok(status)) => status.to_string(),
            _ => "no exit status".to_owned(),
        };
        // Its error stream ends with it; the reader gets a moment to
        // take what was written last.
        tokio::task::yield_now().await;
        let said = self
            .errors
            .borrow()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        if said.is_empty() {
            status
        } else {
            format!("{status}: {said}")
        }
    }
}

/// The next line of `process`'s output (`None` once it has ended), or
/// never while there is no process.
async fn line(process: &mut Option<Process>) -> Option<String> {
    match process {
        Some(process) => process.output.next_line().await.ok().flatten(),
        None => std::future::pending().await,
    }
}

/// An image a client attached: its type and its bytes in base64.
fn image(image: &Value) -> io::Result<(&str, &str, &str)> {
    let data = image["data"]
        .as_str()
        .ok_or_else(|| io::Error::other("Missing image data"))?;
    let mime = image["mimeType"].as_str().unwrap_or_default();
    let extension = match mime {
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "image/webp" => ".webp",
        "image/gif" => ".gif",
        _ => return Err(io::Error::other("Unsupported image type")),
    };
    Ok((mime, extension, data))
}

fn tool(call: &str, name: &str, status: &str, detail: Value, error: Option<&str>) -> Value {
    json!({"type":"tool_call","callId":call,"name":name,"status":status,"detail":detail,"error":error})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mode_a_family_lacks_is_its_default() {
        assert_eq!(mode("claude", Some("plan")), "plan");
        assert_eq!(mode("claude", Some("full-access")), "default");
        assert_eq!(mode("codex", None), "auto");
        assert_eq!(family("work", &json!({"extends":"claude"})), Some("claude"));
        assert_eq!(family("opencode", &Value::Null), None);
    }
}
