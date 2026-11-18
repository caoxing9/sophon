//! Claude Code's MCP servers, in Sophon (`/mcp`): their state, and signing
//! in to one that asks for it, without Claude Code's own screen. The
//! service runs the agents headless and exposes none of this, so Sophon
//! asks Claude Code itself: a small script of its own, run with the Node
//! the service carries, opens a session of the Agent SDK in the project
//! and asks it (`mcpServerStatus`, `mcpAuthenticate`,
//! `mcpSubmitOAuthCallbackUrl`). Signing in: the script listens on a port
//! of this computer for the sign-in's return, the browser opens the
//! server's page, and what comes back is handed to Claude Code, which
//! keeps it where every session of the user finds it (a new conversation
//! has the server).
//!
//! The computer runs it; a phone asks through the bridge (`github::Op`'s
//! channel).

use serde_json::Value;

/// A server's state, as Claude Code says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum State {
    Connected,
    NeedsAuth,
    Failed,
    Pending,
    Disabled,
}

/// An MCP server of the project's sessions.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Server {
    /// Its name, as Claude Code knows it (`plugin:posthog:posthog`).
    pub(crate) name: String,
    pub(crate) state: State,
    /// `user`, `project`, `local`, `dynamic` (a plugin's)…
    pub(crate) scope: String,
    /// Its address or command, shortly.
    pub(crate) target: String,
    pub(crate) tools: Option<u64>,
    pub(crate) error: Option<String>,
}

impl Server {
    /// What to call it: a plugin's server by its plugin (`posthog`).
    pub(crate) fn label(&self) -> String {
        match self.name.strip_prefix("plugin:").and_then(|rest| rest.split(':').next()) {
            Some(plugin) if !plugin.is_empty() => plugin.to_owned(),
            _ => self.name.clone(),
        }
    }
}

/// The servers of a `status` answer.
pub(crate) fn servers_of(v: &Value) -> Vec<Server> {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    v.get("servers")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|x| Server {
            name: s(x, "name"),
            state: match s(x, "status").as_str() {
                "connected" => State::Connected,
                "needs-auth" => State::NeedsAuth,
                "failed" => State::Failed,
                "disabled" => State::Disabled,
                _ => State::Pending,
            },
            scope: s(x, "scope"),
            target: s(x, "target"),
            tools: x.get("tools").and_then(Value::as_u64),
            error: x.get("error").and_then(Value::as_str).map(str::to_owned).filter(|e| !e.is_empty()),
        })
        .collect()
}

/// A server name as Claude Code writes them, checked before it goes to the
/// script.
pub(crate) fn valid_server(name: &str) -> bool {
    !name.is_empty() && name.len() <= 120 && name.chars().all(|c| c.is_alphanumeric() || " ._:-@/".contains(c))
}

// ---------------------------------------------------------- on the computer

/// The script: `status <cwd>` prints the servers once none is still
/// connecting (at most ~10 s); `auth <cwd> <server>` prints the sign-in's
/// page, waits (at most 5 minutes) for the browser to come back to its
/// port, hands that to Claude Code and prints how it went.
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
const SCRIPT: &str = r#"
const [sdkPath, claude, mode, cwd, server] = process.argv.slice(2);
const sdk = await import(sdkPath);
let release; const hold = new Promise(r => release = r);
async function* prompt() { await hold; }
const q = sdk.query({ prompt: prompt(), options: { cwd, pathToClaudeCodeExecutable: claude } });
const say = (v) => process.stdout.write(JSON.stringify(v) + "\n");
const quit = (code) => { try { release(); } catch {} setTimeout(() => process.exit(code), 50); };
try {
  if (mode === "status") {
    let list = [];
    for (let i = 0; i < 20; i++) {
      list = await q.mcpServerStatus();
      if (!list.some(s => s.status === "pending")) break;
      await new Promise(r => setTimeout(r, 500));
    }
    const target = (c) => !c ? "" : c.url ? c.url : [c.command, ...(c.args || [])].filter(Boolean).join(" ");
    say({ servers: list.map(s => ({ name: s.name, status: s.status, scope: s.scope || "", target: target(s.config), tools: Array.isArray(s.tools) ? s.tools.length : null, error: s.error || "" })) });
    quit(0);
  } else if (mode === "auth") {
    const http = await import("node:http");
    const port = await new Promise((resolve) => {
      const srv = http.createServer(async (req, res) => {
        const url = new URL(req.url, "http://127.0.0.1");
        if (url.pathname !== "/callback") { res.writeHead(404); res.end(); return; }
        try {
          await q.mcpSubmitOAuthCallbackUrl(server, "http://127.0.0.1:" + srv.address().port + req.url);
          res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
          res.end("<html><body style='font-family:-apple-system;padding:48px'><h2>Signed in</h2><p>" + server + " is ready. You can close this tab and go back to Sophon.</p></body></html>");
          say({ done: true });
        } catch (e) {
          res.writeHead(500, { "content-type": "text/plain; charset=utf-8" });
          res.end("Sign-in failed: " + (e && e.message || e));
          say({ error: String(e && e.message || e) });
        }
        srv.close(); quit(0);
      });
      srv.listen(0, "127.0.0.1", () => resolve(srv.address().port));
    });
    const r = await q.mcpAuthenticate(server, "http://127.0.0.1:" + port + "/callback");
    if (!r || !r.authUrl) { say({ error: (r && r.error) || "The server offers no sign-in" }); quit(1); }
    else { say({ authUrl: r.authUrl }); setTimeout(() => { say({ error: "No answer from the browser in 5 minutes" }); quit(1); }, 300000); }
  } else { say({ error: "unknown mode" }); quit(1); }
} catch (e) { say({ error: String(e && e.message || e) }); quit(1); }
"#;

/// The Node the service carries, and the Agent SDK beside it.
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
fn runtime() -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    let cli = crate::store::service_cli().ok_or("The service is not installed on the computer")?;
    let bin = cli.parent().ok_or("no service folder")?;
    let node = bin.join("node");
    let sdk = bin.parent().ok_or("no service folder")?.join("lib/node_modules/@getpaseo/cli/node_modules/@anthropic-ai/claude-agent-sdk/sdk.mjs");
    if !node.is_file() || !sdk.is_file() {
        return Err("This service has no Agent SDK to ask".into());
    }
    Ok((node, sdk))
}

/// Claude Code's command line on the computer.
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
fn claude() -> Result<std::path::PathBuf, String> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let mut places: Vec<std::path::PathBuf> = vec![home.join(".local/bin/claude"), "/opt/homebrew/bin/claude".into(), "/usr/local/bin/claude".into(), home.join(".claude/local/claude")];
    if let Some(path) = std::env::var_os("PATH") {
        places.extend(std::env::split_paths(&path).map(|d| d.join("claude")));
    }
    places.into_iter().find(|p| p.is_file()).or_else(|| crate::bundled_cli::find("claude")).ok_or_else(|| "Claude Code is not installed on the computer".to_owned())
}

/// Runs the script with `args`: each line it prints, as JSON, to `line`,
/// until it ends.
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
fn run_script(args: &[&str], mut line: impl FnMut(Value)) -> Result<(), String> {
    use std::io::BufRead;
    let (node, sdk) = runtime()?;
    let claude = claude()?;
    let script = crate::daemon::data_dir().join("sophon-mcp.mjs");
    let _ = std::fs::create_dir_all(crate::daemon::data_dir());
    std::fs::write(&script, SCRIPT).map_err(|e| e.to_string())?;
    let mut child = std::process::Command::new(&node)
        .arg(&script)
        .arg(&sdk)
        .arg(&claude)
        .args(args)
        .env("PATH", crate::store::service_path(&claude))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| e.to_string())?;
    let out = child.stdout.take().ok_or("no output")?;
    for text in std::io::BufReader::new(out).lines().map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<Value>(&text) {
            line(v);
        }
    }
    let _ = child.wait();
    Ok(())
}

/// The MCP servers of the sessions in `cwd`.
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
pub(crate) fn status(cwd: &str) -> Result<Value, String> {
    let mut answer = None;
    run_script(&["status", cwd], |v| answer = Some(v))?;
    let v = answer.ok_or("Claude Code said nothing")?;
    match v.get("error").and_then(Value::as_str) {
        Some(e) => Err(e.to_owned()),
        None => Ok(v),
    }
}

/// Signs in to `server` for the sessions in `cwd`: its page opens in the
/// browser here; done once the browser came back (or it gave up).
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
pub(crate) fn sign_in(cwd: &str, server: &str) -> Result<Value, String> {
    let mut outcome: Result<Value, String> = Err("The sign-in ended without an answer".into());
    run_script(&["auth", cwd, server], |v| {
        if let Some(url) = v.get("authUrl").and_then(Value::as_str) {
            let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
        } else if v.get("done").and_then(Value::as_bool) == Some(true) {
            outcome = Ok(serde_json::json!({ "done": true }));
        } else if let Some(e) = v.get("error").and_then(Value::as_str) {
            outcome = Err(e.to_owned());
        }
    })?;
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn servers_read_as_claude_code_says_them() {
        let v = json!({ "servers": [
            { "name": "plugin:posthog:posthog", "status": "needs-auth", "scope": "dynamic", "target": "https://mcp.posthog.com/mcp", "tools": null, "error": "" },
            { "name": "cloudflare-api", "status": "connected", "scope": "user", "target": "https://mcp.cloudflare.com/mcp", "tools": 12 },
            { "name": "x", "status": "failed", "error": "spawn ENOENT" },
            { "name": "y", "status": "pending" },
        ] });
        let list = servers_of(&v);
        assert_eq!(list.iter().map(|s| s.state).collect::<Vec<_>>(), [State::NeedsAuth, State::Connected, State::Failed, State::Pending]);
        assert_eq!(list[0].label(), "posthog");
        assert_eq!(list[1].label(), "cloudflare-api");
        assert_eq!((list[1].tools, list[2].error.as_deref()), (Some(12), Some("spawn ENOENT")));
        assert_eq!(list[0].error, None);
    }

    #[test]
    fn a_server_name_is_checked() {
        assert!(valid_server("plugin:posthog:posthog") && valid_server("claude.ai Claude Docs"));
        assert!(!valid_server("") && !valid_server("x;rm -rf /") && !valid_server("$(id)"));
    }

    /// Against this Mac's Claude Code (reads the servers, signs in to none).
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore]
    fn the_servers_of_this_project_live() {
        let v = status(env!("CARGO_MANIFEST_DIR")).unwrap();
        let list = servers_of(&v);
        eprintln!("{:?}", list.iter().map(|s| (s.label(), s.state, s.tools)).collect::<Vec<_>>());
        assert!(!list.is_empty());
    }
}
