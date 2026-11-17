//! Pull requests on GitHub, through the computer's `gh` (its own sign-in:
//! Sophon keeps no token). The computer runs the commands; a phone, or a
//! desktop following another Mac, asks that Mac's Sophon over the bridge
//! (`sophon_github_request`), as for the usage statistics.
//!
//! - [`Op`]: what is asked: the inbox (open pull requests that involve
//!   the signed-in user), one pull request, its diff, its checks, or an
//!   action (merge, squash, rebase, draft, ready, close, reopen).
//! - [`run`]: on the computer, the `gh` commands, their output normalized
//!   into the JSON the views read ([`Inbox`], [`Detail`], [`Check`]).
//!
//! After monocode's GitHub inbox (hardbeat920/monocode,
//! `crates/monocode-core/src/inbox/github.rs`).

use serde_json::{Value, json};

/// A diff past this is cut, and said to be (the pull request on GitHub has
/// it all).
#[cfg_attr(any(target_os = "ios", target_os = "android", target_arch = "wasm32"), allow(dead_code))]
pub(crate) const MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;

/// The inbox's pull requests asked for at a time, and at most.
pub(crate) const INBOX_PAGE: u32 = 80;
pub(crate) const INBOX_MAX: u32 = 500;

/// The inbox's states: open, merged, or closed without being merged.
pub(crate) const STATES: [&str; 3] = ["open", "merged", "closed"];

/// The actions a pull request takes.
pub(crate) const ACTIONS: [&str; 7] = ["merge", "squash", "rebase", "draft", "ready", "close", "reopen"];

/// What is asked of GitHub.
#[derive(Clone, Debug, PartialEq)]
pub enum Op {
    /// The pull requests that involve the user in `state` (of
    /// [`STATES`]); `roots`: the projects' folders here, matched to their
    /// repositories (a pull request of one of them can be worked on in it).
    /// `limit`: how many, the newest (`INBOX_PAGE` more each "Load more").
    Inbox { roots: Vec<String>, state: String, limit: u32 },
    View { repo: String, number: u64 },
    Diff { repo: String, number: u64 },
    Checks { repo: String, number: u64 },
    Act { repo: String, number: u64, action: String },
    /// A comment on the pull request, by the signed-in user.
    Comment { repo: String, number: u64, body: String },
    /// The pull requests of the GitHub repository of folder `cwd` (its
    /// `origin`), the latest first, all states.
    RepoPrs { cwd: String },
    /// Claude Code's MCP servers of the sessions in `cwd` (`crate::mcp`;
    /// they go by this channel, the computer's).
    McpStatus { cwd: String },
    /// Signing in to MCP server `server` for the sessions in `cwd`, in this
    /// computer's browser.
    McpSignIn { cwd: String, server: String },
}

/// `owner/name`, as GitHub writes them.
pub(crate) fn valid_repo(repo: &str) -> bool {
    let ok = |s: &str| !s.is_empty() && s.len() <= 100 && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.')) && s != "." && s != "..";
    matches!(repo.split_once('/'), Some((owner, name)) if ok(owner) && ok(name) && !name.contains('/'))
}

/// The repository and number of a pull request's address
/// (`https://github.com/owner/name/pull/12`).
pub(crate) fn of_url(url: &str) -> Option<(String, u64)> {
    let rest = url.trim().strip_prefix("https://github.com/")?;
    let mut parts = rest.split('/');
    let (owner, name, pull, number) = (parts.next()?, parts.next()?, parts.next()?, parts.next()?);
    let repo = format!("{owner}/{name}");
    let number: u64 = number.split(['#', '?']).next()?.parse().ok()?;
    (pull == "pull" && valid_repo(&repo) && number > 0).then_some((repo, number))
}

/// The pull requests `text` links to (`github.com/<owner>/<repo>/pull/<n>`),
/// in the order they come, each once.
pub(crate) fn mentioned(text: &str) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = vec![];
    let mut rest = text;
    while let Some(at) = rest.find("github.com/") {
        let tail = &rest[at..];
        let end = tail.find(|c: char| c.is_whitespace() || "\"'<>()[]`,".contains(c)).unwrap_or(tail.len());
        if let Some(pr) = of_url(&format!("https://{}", &tail[..end]))
            && !out.contains(&pr)
        {
            out.push(pr);
        }
        rest = &tail["github.com/".len()..];
    }
    out
}

impl Op {
    /// Where its answer is kept (the views read it by this).
    pub fn key(&self) -> String {
        match self {
            Op::Inbox { state, .. } => format!("inbox:{state}"),
            Op::View { repo, number } => format!("view:{repo}#{number}"),
            Op::Diff { repo, number } => format!("diff:{repo}#{number}"),
            Op::Checks { repo, number } => format!("checks:{repo}#{number}"),
            // Its answer is the pull request as it is after it.
            Op::Act { repo, number, .. } => format!("view:{repo}#{number}"),
            Op::Comment { repo, number, .. } => format!("view:{repo}#{number}"),
            Op::RepoPrs { cwd } => format!("repo_prs:{cwd}"),
            Op::McpStatus { cwd } => format!("mcp:status:{cwd}"),
            Op::McpSignIn { server, .. } => format!("mcp:signin:{server}"),
        }
    }

    pub fn to_json(&self) -> Value {
        match self {
            Op::Inbox { roots, state, limit } => json!({ "op": "inbox", "roots": roots, "state": state, "limit": limit }),
            Op::View { repo, number } => json!({ "op": "view", "repo": repo, "number": number }),
            Op::Diff { repo, number } => json!({ "op": "diff", "repo": repo, "number": number }),
            Op::Checks { repo, number } => json!({ "op": "checks", "repo": repo, "number": number }),
            Op::Act { repo, number, action } => json!({ "op": "act", "repo": repo, "number": number, "action": action }),
            Op::Comment { repo, number, body } => json!({ "op": "comment", "repo": repo, "number": number, "body": body }),
            Op::RepoPrs { cwd } => json!({ "op": "repo_prs", "cwd": cwd }),
            Op::McpStatus { cwd } => json!({ "op": "mcp_status", "cwd": cwd }),
            Op::McpSignIn { cwd, server } => json!({ "op": "mcp_sign_in", "cwd": cwd, "server": server }),
        }
    }

    /// The op a request asks for, checked: a repository as GitHub names
    /// them, a number, an action of [`ACTIONS`].
    pub(crate) fn from_json(v: &Value) -> Result<Op, String> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        let pr = || -> Result<(String, u64), String> {
            let repo = s("repo");
            let number = v.get("number").and_then(Value::as_u64).unwrap_or(0);
            if !valid_repo(&repo) || number == 0 {
                return Err("Not a pull request".into());
            }
            Ok((repo, number))
        };
        match s("op").as_str() {
            "inbox" => {
                // Before states: the open ones.
                let state = Some(s("state")).filter(|s| !s.is_empty()).unwrap_or_else(|| "open".into());
                if !STATES.contains(&state.as_str()) {
                    return Err("Unknown state".into());
                }
                let limit = v.get("limit").and_then(Value::as_u64).map_or(INBOX_PAGE, |l| (l as u32).clamp(1, INBOX_MAX));
                Ok(Op::Inbox {
                    roots: v.get("roots").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).take(200).map(str::to_owned).collect(),
                    state,
                    limit,
                })
            }
            "view" => pr().map(|(repo, number)| Op::View { repo, number }),
            "diff" => pr().map(|(repo, number)| Op::Diff { repo, number }),
            "checks" => pr().map(|(repo, number)| Op::Checks { repo, number }),
            "act" => {
                let (repo, number) = pr()?;
                let action = s("action");
                if !ACTIONS.contains(&action.as_str()) {
                    return Err("Unknown action".into());
                }
                Ok(Op::Act { repo, number, action })
            }
            "repo_prs" => {
                let cwd = s("cwd");
                if !cwd.starts_with('/') || cwd.split('/').any(|c| c == "..") {
                    return Err("Not a folder".into());
                }
                Ok(Op::RepoPrs { cwd })
            }
            "comment" => {
                let (repo, number) = pr()?;
                let body = s("body");
                if body.trim().is_empty() || body.len() > 65_536 {
                    return Err("A comment says something, not too much".into());
                }
                Ok(Op::Comment { repo, number, body })
            }
            "mcp_status" | "mcp_sign_in" => {
                let cwd = s("cwd");
                if !cwd.starts_with('/') || cwd.split('/').any(|c| c == "..") {
                    return Err("Not a folder".into());
                }
                if s("op") == "mcp_status" {
                    return Ok(Op::McpStatus { cwd });
                }
                let server = s("server");
                if !crate::mcp::valid_server(&server) {
                    return Err("Not an MCP server".into());
                }
                Ok(Op::McpSignIn { cwd, server })
            }
            _ => Err("Unknown request".into()),
        }
    }
}

// ------------------------------------------------------------ what is read

/// A pull request of the inbox.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct InboxItem {
    pub(crate) repo: String,
    pub(crate) number: u64,
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) draft: bool,
    pub(crate) author: String,
    pub(crate) updated_at: String,
    pub(crate) created_at: String,
    /// `open`, `merged` or `closed` (the state it was asked for in).
    pub(crate) state: String,
    pub(crate) labels: Vec<String>,
    /// The user is asked to review it.
    pub(crate) review_requested: bool,
}

/// The inbox: its pull requests, newest first, and the project folders
/// here of their repositories.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Inbox {
    pub(crate) items: Vec<InboxItem>,
    pub(crate) local: Vec<(String, String)>,
}

impl Inbox {
    pub(crate) fn from_json(v: &Value) -> Self {
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        Self {
            items: v
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|i| {
                    Some(InboxItem {
                        repo: s(i, "repo"),
                        number: i.get("number")?.as_u64()?,
                        title: s(i, "title"),
                        url: s(i, "url"),
                        draft: i.get("draft").and_then(Value::as_bool).unwrap_or(false),
                        author: s(i, "author"),
                        updated_at: s(i, "updatedAt"),
                        created_at: s(i, "createdAt"),
                        state: s(i, "state"),
                        labels: i.get("labels").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect(),
                        review_requested: i.get("reviewRequested").and_then(Value::as_bool).unwrap_or(false),
                    })
                })
                .collect(),
            local: v.get("local").and_then(Value::as_object).into_iter().flatten().filter_map(|(repo, p)| Some((repo.clone(), p.as_str()?.to_owned()))).collect(),
        }
    }

    /// The project folder here of `repo`, if one is.
    pub(crate) fn local_of(&self, repo: &str) -> Option<&str> {
        self.local.iter().find(|(r, _)| r.eq_ignore_ascii_case(repo)).map(|(_, p)| p.as_str())
    }
}

/// A pull request of a folder's repository (`Op::RepoPrs`).
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct RepoPr {
    pub(crate) number: u64,
    pub(crate) title: String,
    /// `open`, `merged` or `closed`.
    pub(crate) state: String,
    pub(crate) draft: bool,
    pub(crate) author: String,
    pub(crate) updated_at: String,
    pub(crate) head: String,
}

/// The repository and its pull requests of an `Op::RepoPrs` answer.
pub(crate) fn repo_prs_of(v: &Value) -> (String, Vec<RepoPr>) {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let items = v
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| {
            Some(RepoPr {
                number: p.get("number")?.as_u64()?,
                title: s(p, "title"),
                state: s(p, "state"),
                draft: p.get("draft").and_then(Value::as_bool).unwrap_or(false),
                author: s(p, "author"),
                updated_at: s(p, "updatedAt"),
                head: s(p, "head"),
            })
        })
        .collect();
    (s(v, "repo"), items)
}

/// One pull request.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Detail {
    pub(crate) number: u64,
    pub(crate) title: String,
    /// `OPEN`, `CLOSED` or `MERGED`.
    pub(crate) state: String,
    pub(crate) draft: bool,
    pub(crate) author: String,
    pub(crate) body: String,
    pub(crate) url: String,
    pub(crate) head: String,
    pub(crate) base: String,
    pub(crate) updated_at: String,
    pub(crate) additions: u64,
    pub(crate) deletions: u64,
    pub(crate) changed_files: u64,
    /// `MERGEABLE`, `CONFLICTING` or `UNKNOWN`.
    pub(crate) mergeable: String,
    pub(crate) review_decision: String,
    pub(crate) labels: Vec<String>,
    pub(crate) created_at: String,
    pub(crate) assignees: Vec<String>,
    /// Its conversation, oldest first.
    pub(crate) comments: Vec<Comment>,
}

/// A comment on a pull request, or a review.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Comment {
    pub(crate) author: String,
    pub(crate) body: String,
    pub(crate) at: String,
    /// A review's decision (`APPROVED`, `CHANGES_REQUESTED`, `COMMENTED`);
    /// empty for a comment.
    pub(crate) review: String,
}

impl Detail {
    pub(crate) fn from_json(v: &Value) -> Self {
        let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
        let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
        Self {
            number: n("number"),
            title: s("title"),
            state: s("state"),
            draft: v.get("draft").and_then(Value::as_bool).unwrap_or(false),
            author: s("author"),
            body: s("body"),
            url: s("url"),
            head: s("head"),
            base: s("base"),
            updated_at: s("updatedAt"),
            additions: n("additions"),
            deletions: n("deletions"),
            changed_files: n("changedFiles"),
            mergeable: s("mergeable"),
            review_decision: s("reviewDecision"),
            labels: v.get("labels").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect(),
            created_at: s("createdAt"),
            assignees: v.get("assignees").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_owned).collect(),
            comments: v
                .get("comments")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|c| {
                    let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
                    Comment { author: s("author"), body: s("body"), at: s("at"), review: s("review") }
                })
                .collect(),
        }
    }

    pub(crate) fn open(&self) -> bool {
        self.state.eq_ignore_ascii_case("open")
    }

    pub(crate) fn merged(&self) -> bool {
        self.state.eq_ignore_ascii_case("merged")
    }
}

/// How a check ended, or that it has not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckState {
    Passed,
    Failed,
    Running,
    Skipped,
}

/// A check of a pull request's head.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Check {
    pub(crate) name: String,
    pub(crate) workflow: String,
    pub(crate) state: CheckState,
    pub(crate) url: String,
}

/// The checks in a `checks` answer.
pub(crate) fn checks_of(v: &Value) -> Vec<Check> {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    v.get("checks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|c| Check {
            name: s(c, "name"),
            workflow: s(c, "workflow"),
            state: match s(c, "state").as_str() {
                "passed" => CheckState::Passed,
                "failed" => CheckState::Failed,
                "skipped" => CheckState::Skipped,
                _ => CheckState::Running,
            },
            url: s(c, "url"),
        })
        .collect()
}

/// What an agent is asked to fix the failing `checks` of a pull request.
pub(crate) fn fix_prompt(repo: &str, d: &Detail, failing: &[Check]) -> String {
    let evidence: Vec<String> = failing.iter().map(|c| format!("- {}{}: {}", if c.workflow.is_empty() { String::new() } else { format!("{} / ", c.workflow) }, c.name, c.url)).collect();
    format!(
        "Investigate and repair the failing CI checks of {repo}#{} ({}): {}\n\nFailing:\n{}\n\nRead the check logs (`gh run view --log-failed`), reproduce the failures, fix them on the branch {}, and run the relevant checks before pushing.",
        d.number,
        d.title,
        d.url,
        evidence.join("\n"),
        d.head
    )
}

// ---------------------------------------------------------- on the computer

/// Where `gh` is: a Mac app's PATH is the system's, without Homebrew's.
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
fn gh_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default();
    let mut dirs: Vec<std::path::PathBuf> = vec!["/opt/homebrew/bin".into(), "/usr/local/bin".into(), "/usr/bin".into(), home.join(".local/bin"), home.join("bin")];
    if let Some(path) = std::env::var_os("PATH") {
        dirs.extend(std::env::split_paths(&path));
    }
    dirs.into_iter().map(|d| d.join("gh")).find(|p| p.is_file())
}

#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
fn gh(args: &[&str]) -> Result<String, String> {
    let path = gh_path().ok_or("GitHub CLI (gh) is not installed on the computer: brew install gh, then gh auth login")?;
    let out = std::process::Command::new(path)
        .args(args)
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let line = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("gh failed").to_owned();
    Err(if err.contains("gh auth login") { "GitHub CLI on the computer is not signed in: run gh auth login".into() } else { line })
}

#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
fn gh_json(args: &[&str]) -> Result<Value, String> {
    serde_json::from_str(&gh(args)?).map_err(|e| format!("gh answered something else: {e}"))
}

/// The GitHub repository (`owner/name`) a folder's `origin` points at.
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
fn repo_of_folder(root: &str) -> Option<String> {
    let out = std::process::Command::new("git").args(["-C", root, "remote", "get-url", "origin"]).output().ok()?;
    repo_of_remote(String::from_utf8_lossy(&out.stdout).trim())
}

/// `owner/name` of a GitHub remote (`git@github.com:o/n.git`,
/// `https://github.com/o/n`).
#[cfg_attr(any(target_os = "ios", target_os = "android", target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn repo_of_remote(remote: &str) -> Option<String> {
    let rest = remote.strip_prefix("git@github.com:").or_else(|| remote.strip_prefix("ssh://git@github.com/")).or_else(|| remote.strip_prefix("https://github.com/"))?;
    let repo = rest.trim_end_matches('/').trim_end_matches(".git");
    valid_repo(repo).then(|| repo.to_owned())
}

/// Does `op` with the computer's `gh`: the JSON the views read.
#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
pub(crate) fn run(op: &Op) -> Result<Value, String> {
    let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    match op {
        Op::Inbox { roots, state, limit } => {
            let fields = "number,title,repository,updatedAt,createdAt,isDraft,author,url,labels";
            let limit = limit.to_string();
            let mut args = vec!["search", "prs", "--involves", "@me", "--sort", "updated", "--limit", &limit, "--json", fields];
            match state.as_str() {
                "merged" => args.push("--merged"),
                // Closed without being merged.
                "closed" => args.extend(["--state", "closed", "--", "-is:merged"]),
                _ => args.extend(["--state", "open"]),
            }
            // Asked to review: marked (a second, small search, at the same
            // time; only open pull requests ask).
            let open = !matches!(state.as_str(), "merged" | "closed");
            let asked = open.then(|| {
                std::thread::spawn(|| gh_json(&["search", "prs", "--state", "open", "--review-requested", "@me", "--limit", "60", "--json", "number,repository"]).unwrap_or(Value::Null))
            });
            let involved = gh_json(&args)?;
            let asked = asked.and_then(|h| h.join().ok()).unwrap_or(Value::Null);
            let asked: Vec<(String, u64)> = asked
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| Some((p.get("repository")?.get("nameWithOwner")?.as_str()?.to_owned(), p.get("number")?.as_u64()?)))
                .collect();
            let items: Vec<Value> = involved
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    let repo = p.get("repository")?.get("nameWithOwner")?.as_str()?.to_owned();
                    let number = p.get("number")?.as_u64()?;
                    let review_requested = asked.iter().any(|(r, n)| *r == repo && *n == number);
                    Some(json!({
                        "repo": repo,
                        "number": number,
                        "title": s(p, "title"),
                        "url": s(p, "url"),
                        "draft": p.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
                        "author": p.get("author").map(|a| s(a, "login")).unwrap_or_default(),
                        "updatedAt": s(p, "updatedAt"),
                        "createdAt": s(p, "createdAt"),
                        "state": state,
                        "labels": names(p.get("labels")),
                        "reviewRequested": review_requested,
                    }))
                })
                .collect();
            let mut local = serde_json::Map::new();
            for root in roots {
                if let Some(repo) = repo_of_folder(root) {
                    local.entry(repo).or_insert_with(|| root.clone().into());
                }
            }
            Ok(json!({ "items": items, "local": local }))
        }
        Op::View { repo, number } => view(repo, *number),
        Op::Diff { repo, number } => {
            let mut patch = gh(&["pr", "diff", &number.to_string(), "--repo", repo])?;
            let truncated = patch.len() > MAX_DIFF_BYTES;
            if truncated {
                let mut end = MAX_DIFF_BYTES;
                while !patch.is_char_boundary(end) {
                    end -= 1;
                }
                // At a file's start, so no file shows half.
                let cut = patch[..end].rfind("\ndiff --git ").map_or(end, |i| i + 1);
                patch.truncate(cut);
            }
            Ok(json!({ "patch": patch, "truncated": truncated }))
        }
        Op::Checks { repo, number } => {
            let v = gh_json(&["pr", "view", &number.to_string(), "--repo", repo, "--json", "statusCheckRollup"])?;
            let checks: Vec<Value> = v.get("statusCheckRollup").and_then(Value::as_array).into_iter().flatten().map(check_json).collect();
            Ok(json!({ "checks": checks }))
        }
        Op::McpStatus { cwd } => crate::mcp::status(cwd),
        Op::McpSignIn { cwd, server } => crate::mcp::sign_in(cwd, server),
        Op::Act { repo, number, action } => {
            let n = number.to_string();
            let args: Vec<&str> = match action.as_str() {
                "merge" => vec!["pr", "merge", &n, "--repo", repo, "--merge"],
                "squash" => vec!["pr", "merge", &n, "--repo", repo, "--squash"],
                "rebase" => vec!["pr", "merge", &n, "--repo", repo, "--rebase"],
                "draft" => vec!["pr", "ready", &n, "--repo", repo, "--undo"],
                "ready" => vec!["pr", "ready", &n, "--repo", repo],
                "close" => vec!["pr", "close", &n, "--repo", repo],
                "reopen" => vec!["pr", "reopen", &n, "--repo", repo],
                _ => return Err("Unknown action".into()),
            };
            gh(&args)?;
            // The pull request as it is now.
            view(repo, *number)
        }
        Op::RepoPrs { cwd } => {
            let repo = repo_of_folder(cwd).ok_or("This folder has no GitHub remote")?;
            let list = gh_json(&["pr", "list", "--repo", &repo, "--state", "all", "--limit", "60", "--json", "number,title,state,isDraft,author,updatedAt,headRefName,url"])?;
            let items: Vec<Value> = list
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|p| {
                    Some(json!({
                        "number": p.get("number")?.as_u64()?,
                        "title": s(p, "title"),
                        "state": s(p, "state").to_ascii_lowercase(),
                        "draft": p.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
                        "author": p.get("author").map(|a| s(a, "login")).unwrap_or_default(),
                        "updatedAt": s(p, "updatedAt"),
                        "head": s(p, "headRefName"),
                        "url": s(p, "url"),
                    }))
                })
                .collect();
            Ok(json!({ "repo": repo, "items": items }))
        }
        Op::Comment { repo, number, body } => {
            gh(&["pr", "comment", &number.to_string(), "--repo", repo, "--body", body])?;
            view(repo, *number)
        }
    }
}

#[cfg(not(any(target_os = "ios", target_os = "android", target_arch = "wasm32")))]
fn view(repo: &str, number: u64) -> Result<Value, String> {
    let fields = "number,title,state,isDraft,author,body,url,headRefName,baseRefName,updatedAt,createdAt,additions,deletions,changedFiles,mergeable,reviewDecision,labels,assignees,comments,reviews";
    let v = gh_json(&["pr", "view", &number.to_string(), "--repo", repo, "--json", fields])?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let n = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0);
    Ok(json!({
        "number": n("number"),
        "title": s("title"),
        "state": s("state"),
        "draft": v.get("isDraft").and_then(Value::as_bool).unwrap_or(false),
        "author": v.get("author").and_then(|a| a.get("login")).and_then(Value::as_str).unwrap_or_default(),
        "body": s("body"),
        "url": s("url"),
        "head": s("headRefName"),
        "base": s("baseRefName"),
        "updatedAt": s("updatedAt"),
        "additions": n("additions"),
        "deletions": n("deletions"),
        "changedFiles": n("changedFiles"),
        "mergeable": s("mergeable"),
        "reviewDecision": s("reviewDecision"),
        "labels": names(v.get("labels")),
        "createdAt": s("createdAt"),
        "assignees": v.get("assignees").and_then(Value::as_array).into_iter().flatten().filter_map(|a| a.get("login").and_then(Value::as_str)).collect::<Vec<_>>(),
        "comments": discussion(&v),
    }))
}

/// The names of a list of labels.
#[cfg_attr(any(target_os = "ios", target_os = "android", target_arch = "wasm32"), allow(dead_code))]
fn names(labels: Option<&Value>) -> Vec<String> {
    labels.and_then(Value::as_array).into_iter().flatten().filter_map(|l| l.get("name").and_then(Value::as_str)).map(str::to_owned).collect()
}

/// A pull request's conversation: its comments and its reviews (those that
/// say something, or decide), oldest first.
#[cfg_attr(any(target_os = "ios", target_os = "android", target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn discussion(v: &Value) -> Vec<Value> {
    let s = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let login = |x: &Value| x.get("author").and_then(|a| a.get("login")).and_then(Value::as_str).unwrap_or_default().to_owned();
    let mut out: Vec<Value> = v
        .get("comments")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|c| json!({ "author": login(c), "body": s(c, "body"), "at": s(c, "createdAt"), "review": "" }))
        .collect();
    for r in v.get("reviews").and_then(Value::as_array).into_iter().flatten() {
        let state = s(r, "state");
        if s(r, "body").trim().is_empty() && !matches!(state.as_str(), "APPROVED" | "CHANGES_REQUESTED") {
            continue;
        }
        out.push(json!({ "author": login(r), "body": s(r, "body"), "at": s(r, "submittedAt"), "review": state }));
    }
    out.sort_by(|a, b| s(a, "at").cmp(&s(b, "at")));
    out
}

/// A row of `statusCheckRollup` (a check run or a commit status) as the
/// views read it: its name, workflow, how it went and its page.
#[cfg_attr(any(target_os = "ios", target_os = "android", target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn check_json(c: &Value) -> Value {
    let s = |k: &str| c.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let name = if s("name").is_empty() { s("context") } else { s("name") };
    let ended = s("conclusion").to_ascii_uppercase();
    let status = s("status").to_ascii_uppercase();
    let state = s("state").to_ascii_uppercase();
    let how = match (ended.as_str(), status.as_str(), state.as_str()) {
        ("SUCCESS", ..) | (_, _, "SUCCESS") => "passed",
        ("FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE", ..) | (_, _, "FAILURE" | "ERROR") => "failed",
        ("SKIPPED" | "NEUTRAL" | "STALE", ..) => "skipped",
        _ => "running",
    };
    let url = if s("detailsUrl").is_empty() { s("targetUrl") } else { s("detailsUrl") };
    json!({ "name": name, "workflow": s("workflowName"), "state": how, "url": url })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_request_is_checked_before_gh_sees_it() {
        assert_eq!(Op::from_json(&json!({ "op": "view", "repo": "caoxing9/sophon", "number": 61 })), Ok(Op::View { repo: "caoxing9/sophon".into(), number: 61 }));
        assert!(Op::from_json(&json!({ "op": "view", "repo": "--help", "number": 61 })).is_err());
        assert!(Op::from_json(&json!({ "op": "view", "repo": "a/b/c", "number": 1 })).is_err());
        assert!(Op::from_json(&json!({ "op": "view", "repo": "a/b", "number": 0 })).is_err());
        assert!(Op::from_json(&json!({ "op": "act", "repo": "a/b", "number": 1, "action": "delete" })).is_err());
        assert_eq!(
            Op::from_json(&json!({ "op": "act", "repo": "a/b", "number": 1, "action": "squash" })),
            Ok(Op::Act { repo: "a/b".into(), number: 1, action: "squash".into() })
        );
        let op = Op::Inbox { roots: vec!["/Users/me/sophon".into()], state: "merged".into(), limit: 160 };
        assert_eq!(Op::from_json(&op.to_json()), Ok(op));
        assert!(Op::from_json(&json!({ "op": "inbox", "state": "deleted" })).is_err());
        // From before states: the open ones.
        assert_eq!(Op::from_json(&json!({ "op": "inbox" })), Ok(Op::Inbox { roots: vec![], state: "open".into(), limit: INBOX_PAGE }));
        // A limit past the most is the most.
        assert!(matches!(Op::from_json(&json!({ "op": "inbox", "limit": 100000 })), Ok(Op::Inbox { limit: INBOX_MAX, .. })));
    }

    #[test]
    fn a_text_s_pull_requests_are_its_links_to_them() {
        let text = "PR: https://github.com/caoxing9/sophon/pull/68\n(see https://github.com/caoxing9/sophon/pull/67), again https://github.com/caoxing9/sophon/pull/68 and \"https://github.com/a/b/pull/3#issuecomment-1\"; not https://github.com/a/b/issues/4";
        assert_eq!(mentioned(text), [("caoxing9/sophon".to_owned(), 68), ("caoxing9/sophon".to_owned(), 67), ("a/b".to_owned(), 3)]);
        assert!(mentioned("no links").is_empty());
    }

    #[test]
    fn addresses_and_remotes_name_their_repository() {
        assert_eq!(of_url("https://github.com/caoxing9/sophon/pull/61"), Some(("caoxing9/sophon".into(), 61)));
        assert_eq!(of_url("https://github.com/caoxing9/sophon/issues/61"), None);
        assert_eq!(repo_of_remote("git@github.com:teableio/teable-ee.git"), Some("teableio/teable-ee".into()));
        assert_eq!(repo_of_remote("https://github.com/caoxing9/sophon"), Some("caoxing9/sophon".into()));
        assert_eq!(repo_of_remote("https://gitlab.com/a/b"), None);
    }

    #[test]
    fn checks_read_as_passed_failed_running_or_skipped() {
        let rows = [
            json!({ "__typename": "CheckRun", "name": "Relay", "workflowName": "CI", "status": "COMPLETED", "conclusion": "SUCCESS", "detailsUrl": "u1" }),
            json!({ "__typename": "CheckRun", "name": "UI tests", "status": "COMPLETED", "conclusion": "FAILURE" }),
            json!({ "__typename": "CheckRun", "name": "Build", "status": "IN_PROGRESS", "conclusion": "" }),
            json!({ "__typename": "StatusContext", "context": "vercel", "state": "ERROR", "targetUrl": "u4" }),
            json!({ "__typename": "CheckRun", "name": "Android", "status": "COMPLETED", "conclusion": "SKIPPED" }),
        ];
        let checks = checks_of(&json!({ "checks": rows.iter().map(check_json).collect::<Vec<_>>() }));
        let states: Vec<CheckState> = checks.iter().map(|c| c.state).collect();
        assert_eq!(states, [CheckState::Passed, CheckState::Failed, CheckState::Running, CheckState::Failed, CheckState::Skipped]);
        assert_eq!((checks[0].workflow.as_str(), checks[0].url.as_str()), ("CI", "u1"));
        assert_eq!((checks[3].name.as_str(), checks[3].url.as_str()), ("vercel", "u4"));
    }

    #[test]
    fn the_fix_prompt_names_the_failures_and_the_branch() {
        let d = Detail { number: 61, title: "copy".into(), url: "https://github.com/a/b/pull/61".into(), head: "feat/x".into(), ..Default::default() };
        let failing = vec![Check { name: "UI tests".into(), workflow: "CI".into(), state: CheckState::Failed, url: "https://ci/1".into() }];
        let p = fix_prompt("a/b", &d, &failing);
        assert!(p.contains("a/b#61") && p.contains("- CI / UI tests: https://ci/1") && p.contains("branch feat/x"));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod live {
    /// Against GitHub with this Mac's `gh` (read only): `cargo test -- --ignored github`.
    #[test]
    #[ignore]
    fn github_view_diff_checks_and_inbox_live() {
        use super::*;
        let d = Detail::from_json(&run(&Op::View { repo: "caoxing9/sophon".into(), number: 61 }).unwrap());
        assert_eq!(d.number, 61);
        eprintln!("{} {} {} +{} -{} {}", d.state, d.head, d.mergeable, d.additions, d.deletions, d.title);
        let diff = run(&Op::Diff { repo: "caoxing9/sophon".into(), number: 61 }).unwrap();
        assert!(diff["patch"].as_str().unwrap().starts_with("diff --git"));
        let checks = checks_of(&run(&Op::Checks { repo: "caoxing9/sophon".into(), number: 61 }).unwrap());
        eprintln!("{:?}", checks.iter().map(|c| (&c.name, c.state)).collect::<Vec<_>>());
        let inbox = Inbox::from_json(&run(&Op::Inbox { roots: vec![env!("CARGO_MANIFEST_DIR").into()], state: "open".into(), limit: INBOX_PAGE }).unwrap());
        let merged = Inbox::from_json(&run(&Op::Inbox { roots: vec![], state: "merged".into(), limit: 120 }).unwrap());
        let closed = Inbox::from_json(&run(&Op::Inbox { roots: vec![], state: "closed".into(), limit: INBOX_PAGE }).unwrap());
        eprintln!("{} merged, {} closed", merged.items.len(), closed.items.len());
        eprintln!("{} open, local {:?}", inbox.items.len(), inbox.local);
        assert_eq!(inbox.local_of("caoxing9/sophon").is_some(), true);
    }
}
