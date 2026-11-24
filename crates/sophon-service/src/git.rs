use serde_json::{Value, json};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{process::Command, time::timeout};

async fn command(cwd: &str, program: &str, args: &[&str]) -> Result<String, String> {
    let output = timeout(
        Duration::from_secs(90),
        Command::new(program)
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GH_PROMPT_DISABLED", "1")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| format!("{program} timed out"))?
    .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end_matches('\n')
        .to_owned())
}

/// A changed file is shown whole up to this many changed lines and this
/// much patch, and a workspace's changes up to this much in all: past
/// that a file is listed with its counts and no lines. The agent and
/// `git` have the whole of it; a diff of a hundred megabytes, read again
/// every few seconds and sent to a phone, serves no one.
const LINES: u64 = 20_000;
const PATCH: usize = 1024 * 1024;
const PATCHES: usize = 8 * 1024 * 1024;

/// `command`, reading no more than `limit` of what it writes: `None` if
/// it had more to say, and it is stopped there.
async fn capped(
    cwd: &str,
    program: &str,
    args: &[&str],
    limit: usize,
) -> Result<Option<String>, String> {
    use tokio::io::AsyncReadExt;
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| error.to_string())?;
    let stdout = child.stdout.take().ok_or("No command output")?;
    let mut bytes = Vec::new();
    timeout(
        Duration::from_secs(90),
        stdout.take(limit as u64 + 1).read_to_end(&mut bytes),
    )
    .await
    .map_err(|_| format!("{program} timed out"))?
    .map_err(|error| error.to_string())?;
    if bytes.len() > limit {
        let _ = child.kill().await;
        return Ok(None);
    }
    let output = timeout(Duration::from_secs(90), child.wait_with_output())
        .await
        .map_err(|_| format!("{program} timed out"))?
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

/// A changed file listed without its lines: `status` says why
/// (`too_large`, `binary`).
fn unshown(path: &str, status: &str, additions: u64, deletions: u64) -> Value {
    json!({"path":path,"hunks":[],"isNew":false,"isDeleted":false,
        "additions":additions,"deletions":deletions,"status":status})
}

fn hunk_range(value: &str) -> (u64, u64) {
    let (start, count) = value
        .trim_start_matches(['+', '-'])
        .split_once(',')
        .unwrap_or((value.trim_start_matches(['+', '-']), "1"));
    (
        start.parse().unwrap_or_default(),
        count.parse().unwrap_or_default(),
    )
}

/// Adds a worktree of `project` on a new branch, as `source` asks, and
/// answers the workspace it is. Awaited, not run in place: a checkout
/// of a large repository, or one with a hook that installs things,
/// takes as long as it takes, and the service goes on meanwhile.
pub async fn worktree(home: &Path, project: &Value, source: &Value) -> Result<Value, String> {
    let root = project["projectRootPath"]
        .as_str()
        .ok_or("Missing project directory")?;
    let id = crate::state::id();
    let default_branch = format!("sophon/{}", &id[..12]);
    let branch = source["branchName"].as_str().unwrap_or(&default_branch);
    let base = source["baseBranch"].as_str().unwrap_or("HEAD");
    if branch.starts_with('-') || base.starts_with('-') || branch.is_empty() || base.is_empty() {
        return Err("Invalid branch name".into());
    }
    let parent = home.join("worktrees");
    std::fs::create_dir_all(&parent).map_err(|error| error.to_string())?;
    let target = std::fs::canonicalize(parent)
        .map_err(|error| error.to_string())?
        .join(&id);
    let result = timeout(
        Duration::from_secs(15 * 60),
        Command::new("git")
            .current_dir(root)
            .args(["worktree", "add", "-b", branch, "--"])
            .arg(&target)
            .arg(base)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| "Creating the worktree timed out".to_owned())?
    .map_err(|error| error.to_string())?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).trim().into());
    }
    let mut workspace = project.clone();
    workspace["id"] = json!(id);
    workspace["workspaceDirectory"] = json!(target);
    workspace["workspaceKind"] = json!("worktree");
    workspace["name"] = json!(branch);
    workspace["gitRuntime"] = json!({"currentBranch":branch,"baseBranch":base});
    Ok(workspace)
}

fn diff(path: &str, patch: &str) -> Value {
    let mut hunks = Vec::new();
    let mut additions = 0;
    let mut deletions = 0;
    let mut new = false;
    let mut deleted = false;
    for line in patch.lines() {
        if let Some(rest) = line.strip_prefix("@@ ") {
            let mut fields = rest.split_whitespace();
            let (old, old_count) = hunk_range(fields.next().unwrap_or_default());
            let (new, new_count) = hunk_range(fields.next().unwrap_or_default());
            hunks.push(json!({"oldStart":old,"oldCount":old_count,"newStart":new,"newCount":new_count,"lines":[]}));
        } else if let Some(lines) = hunks
            .last_mut()
            .and_then(|hunk| hunk["lines"].as_array_mut())
        {
            if let Some((prefix, content)) = line.split_at_checked(1) {
                let kind = match prefix {
                    "+" => {
                        additions += 1;
                        "add"
                    }
                    "-" => {
                        deletions += 1;
                        "remove"
                    }
                    " " => "context",
                    _ => continue,
                };
                lines.push(json!({"type":kind,"content":content}));
            }
        } else {
            new |= line.starts_with("new file mode ");
            deleted |= line.starts_with("deleted file mode ");
        }
    }
    json!({"path":path,"hunks":hunks,"isNew":new,"isDeleted":deleted,"additions":additions,"deletions":deletions})
}

/// A checkout's branch (empty when detached) and its uncommitted lines,
/// added and removed, as the changes panel counts them (against HEAD);
/// `None` for a folder that is not one.
pub async fn summary(cwd: &str) -> Option<(String, u64, u64)> {
    if command(cwd, "git", &["rev-parse", "--is-inside-work-tree"]).await.ok()? != "true" {
        return None;
    }
    let branch = command(cwd, "git", &["branch", "--show-current"]).await.unwrap_or_default();
    let (mut added, mut removed) = (0, 0);
    if let Ok(stats) = command(cwd, "git", &["diff", "--no-renames", "--numstat", "HEAD", "--"]).await {
        for row in stats.lines() {
            let mut fields = row.split('\t');
            // A binary file says "-": no lines.
            added += fields.next().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
            removed += fields.next().and_then(|n| n.parse::<u64>().ok()).unwrap_or(0);
        }
    }
    Some((branch, added, removed))
}

pub fn handles(kind: &str) -> bool {
    kind.starts_with("checkout_")
        || kind.starts_with("checkout.commits.")
        || matches!(
            kind,
            "branch_suggestions_request"
                | "stash_save_request"
                | "stash_pop_request"
                | "subscribe_checkout_diff_request"
                | "unsubscribe_checkout_diff_request"
        )
}

pub async fn execute(kind: &str, cwd: &str, request: &Value) -> Result<Value, String> {
    let git = |args: &[&str]| {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        async move {
            command(
                cwd,
                "git",
                &args.iter().map(String::as_str).collect::<Vec<_>>(),
            )
            .await
        }
    };
    let field = |key: &str| {
        request[key]
            .as_str()
            .ok_or_else(|| format!("Missing {key}"))
    };
    let mut payload = json!({"cwd":cwd});
    match kind {
        "checkout_status_request" => {
            let valid = git(&["rev-parse", "--is-inside-work-tree"])
                .await
                .as_deref()
                == Ok("true");
            payload["isGit"] = json!(valid);
            if valid {
                payload["currentBranch"] = json!(git(&["branch", "--show-current"]).await?);
                payload["isDirty"] = json!(!git(&["status", "--porcelain"]).await?.is_empty());
                payload["hasRemote"] = json!(!git(&["remote"]).await?.is_empty());
                if let Ok(remote) =
                    git(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]).await
                {
                    payload["baseRef"] = json!(remote);
                }
                if let Ok(counts) =
                    git(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"]).await
                {
                    let mut values = counts
                        .split_whitespace()
                        .filter_map(|word| word.parse::<u64>().ok());
                    payload["aheadOfOrigin"] = json!(values.next());
                    payload["behindOfOrigin"] = json!(values.next());
                }
            }
        }
        "checkout_commit_request" => {
            let message = field("message")?;
            if request["addAll"] == true {
                git(&["add", "-A", "--", "."]).await?;
            }
            git(&["commit", "-m", message]).await?;
        }
        "checkout_pull_request" => {
            git(&["pull", "--ff-only"]).await?;
        }
        "checkout_push_request" => {
            git(&["push"]).await?;
        }
        "checkout_switch_branch_request" => {
            git(&["switch", "--", field("branch")?]).await?;
        }
        "stash_save_request" => {
            git(&["stash", "push", "--include-untracked"]).await?;
        }
        "stash_pop_request" => {
            git(&["stash", "pop"]).await?;
        }
        "branch_suggestions_request" => {
            let query = request["query"].as_str().unwrap_or_default();
            payload["branches"] = json!(
                git(&[
                    "for-each-ref",
                    "--format=%(refname:short)",
                    "refs/heads",
                    "refs/remotes"
                ])
                .await?
                .lines()
                .filter(|branch| branch.contains(query))
                .take(200)
                .collect::<Vec<_>>()
            );
        }
        "checkout.commits.list.request" => {
            let rows = git(&["log", "-60", "--format=%H%x00%h%x00%s%x00%an%x00%aI"]).await?;
            payload["commits"] = json!(rows.lines().filter_map(|row|{
                let fields:Vec<_> = row.split('\0').collect();
                (fields.len()==5).then(||json!({"sha":fields[0],"shortSha":fields[1],"subject":fields[2],"authorName":fields[3],"authorDate":fields[4]}))
            }).collect::<Vec<_>>());
        }
        "checkout.commits.file_diff.request" => {
            let sha = field("sha")?;
            if !(7..=64).contains(&sha.len()) || !sha.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err("Invalid commit ID".into());
            }
            let path = field("path")?;
            let patch = capped(
                cwd,
                "git",
                &[
                    "show",
                    "--format=",
                    "--no-ext-diff",
                    "--no-textconv",
                    sha,
                    "--",
                    path,
                ],
                PATCH,
            )
            .await?;
            payload["sha"] = json!(sha);
            payload["path"] = json!(path);
            payload["file"] = match patch {
                Some(patch) => diff(path, &patch),
                None => unshown(path, "too_large", 0, 0),
            };
        }
        "subscribe_checkout_diff_request" => {
            let base = if git(&["rev-parse", "--verify", "HEAD"]).await.is_ok() {
                "HEAD".to_owned()
            } else {
                git(&["hash-object", "-t", "tree", "--stdin"]).await?
            };
            // Each file's counts first: they say which are worth
            // reading, before anything large is read.
            let stats = git(&["diff", "--no-renames", "--numstat", "-z", &base, "--"]).await?;
            let mut files = Vec::new();
            let mut budget = PATCHES;
            for row in stats.split('\0').filter(|row| !row.is_empty()) {
                if files.len() == 200 {
                    payload["diffTooLarge"] = json!(true);
                    break;
                }
                let mut fields = row.splitn(3, '\t');
                let (Some(added), Some(removed), Some(path)) =
                    (fields.next(), fields.next(), fields.next())
                else {
                    continue;
                };
                // Counted in lines, or `-` for what has none.
                let Some((added, removed)) =
                    added.parse::<u64>().ok().zip(removed.parse::<u64>().ok())
                else {
                    files.push(unshown(path, "binary", 0, 0));
                    continue;
                };
                let patch = if added + removed > LINES || budget == 0 {
                    None
                } else {
                    capped(
                        cwd,
                        "git",
                        &[
                            "diff",
                            "--no-renames",
                            "--no-ext-diff",
                            "--no-textconv",
                            &base,
                            "--",
                            path,
                        ],
                        PATCH.min(budget),
                    )
                    .await?
                };
                files.push(match patch {
                    Some(patch) => {
                        budget -= patch.len();
                        diff(path, &patch)
                    }
                    None => unshown(path, "too_large", added, removed),
                });
            }
            for path in git(&["ls-files", "--others", "--exclude-standard", "-z"])
                .await?
                .split('\0')
                .filter(|path| !path.is_empty())
            {
                if files.len() >= 200 {
                    payload["diffTooLarge"] = json!(true);
                    break;
                }
                let full = Path::new(cwd).join(path);
                let mut file = diff(path, "new file mode 100644");
                let size = std::fs::symlink_metadata(&full)
                    .ok()
                    .filter(|meta| meta.is_file())
                    .map(|meta| meta.len() as usize);
                match size {
                    Some(size) if size < 512_000 && size <= budget => {
                        match tokio::fs::read_to_string(full).await {
                            Ok(content) => {
                                budget -= size;
                                let lines: Vec<_> = content
                                    .lines()
                                    .map(|line| json!({"type":"add","content":line}))
                                    .collect();
                                file["additions"] = json!(lines.len());
                                file["hunks"] = json!([{"oldStart":0,"oldCount":0,"newStart":1,"newCount":lines.len(),"lines":lines}]);
                            }
                            Err(_) => file["status"] = json!("binary"),
                        }
                    }
                    Some(_) => file["status"] = json!("too_large"),
                    None => {}
                }
                files.push(file);
            }
            payload["files"] = json!(files);
            payload["subscriptionId"] = json!(cwd);
        }
        "checkout_pr_status_request" => {
            let result = command(cwd,"gh",&["pr","view","--json","number,url,title,state,isDraft,baseRefName,headRefName,mergeable,reviewDecision"]).await;
            match result {
                Ok(text) => {
                    payload["status"] =
                        serde_json::from_str(&text).map_err(|error| error.to_string())?
                }
                Err(error) if error.contains("no pull requests") => payload["status"] = Value::Null,
                Err(error) => return Err(error),
            }
        }
        "checkout_pr_create_request" => {
            payload["url"] = json!(command(cwd, "gh", &["pr", "create", "--fill"]).await?)
        }
        "checkout_pr_merge_request" => {
            let method = match field("mergeMethod")? {
                "squash" => "--squash",
                "merge" => "--merge",
                "rebase" => "--rebase",
                _ => return Err("Unknown merge method".into()),
            };
            command(cwd, "gh", &["pr", "merge", method]).await?;
        }
        // The branch here into its base: where the base is checked out
        // (the project's own folder, usually), merged there; checked
        // out nowhere, moved forward to this branch if it can be.
        "checkout_merge_request" => {
            let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"]).await?;
            let base = match request["baseRef"].as_str().filter(|base| !base.is_empty()) {
                Some(base) => base.trim_start_matches("origin/").to_owned(),
                None => git(&["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
                    .await
                    .map(|head| head.trim_start_matches("origin/").to_owned())
                    .map_err(|_| "This branch has no base to merge into".to_owned())?,
            };
            if branch == base || base.starts_with('-') {
                return Err("This branch is its own base".into());
            }
            let listed = git(&["worktree", "list", "--porcelain"]).await?;
            let mut held = None;
            let mut at = None;
            for line in listed.lines() {
                if let Some(path) = line.strip_prefix("worktree ") {
                    at = Some(path.to_owned());
                } else if line.strip_prefix("branch refs/heads/") == Some(base.as_str()) {
                    held = at.take();
                }
            }
            let squash = request["strategy"] == "squash";
            match held {
                Some(place) if squash => {
                    command(&place, "git", &["merge", "--squash", &branch]).await?;
                    command(
                        &place,
                        "git",
                        &["commit", "--no-edit", "-m", &format!("Merge {branch}")],
                    )
                    .await?;
                }
                Some(place) => {
                    command(&place, "git", &["merge", "--no-edit", &branch]).await?;
                }
                None => {
                    git(&["fetch", ".", &format!("{branch}:{base}")]).await.map_err(|error| {
                        format!("{base} has moved on from {branch}: check it out and merge there ({error})")
                    })?;
                }
            }
            payload["baseRef"] = json!(base);
        }
        _ => return Err(format!("Unsupported Git operation: {kind}")),
    }
    Ok(payload)
}

/// Checks a worktree's branch out again where its folder was, for one
/// whose folder went missing: the branch and its commits are in the
/// project's repository still.
pub async fn restore(root: &str, folder: &str, branch: &str) -> Result<(), String> {
    if branch.is_empty() || branch.starts_with('-') {
        return Err("This workspace has no branch on record".into());
    }
    // The repository still lists the folder that is gone.
    command(root, "git", &["worktree", "prune"]).await?;
    command(root, "git", &["worktree", "add", "--", folder, branch])
        .await
        .map(drop)
}
