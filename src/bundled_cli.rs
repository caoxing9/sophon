//! The agent CLIs the service does not find: installed where its `PATH`
//! does not reach, or carried inside the desktop apps.
//!
//! The service looks for `claude` and `codex` on the `PATH` it has, which
//! is a login shell's: one set in `.zshrc` alone (as Claude Code's own
//! installer does, for `~/.local/bin`) is not on it, nor is a Node version
//! manager's folder. So a CLI installed on its own is looked for in the
//! usual folders ([`installed`]). Failing that, someone with only the
//! Claude app and the Codex app still has the CLI each app runs:
//!
//! - Claude: `~/Library/Application Support/Claude/claude-code/<version>/
//!   claude.app/Contents/MacOS/claude`, a folder per version it downloaded.
//! - Codex: `<app>/Contents/Resources/codex-cli/bin/codex`, the app being
//!   `ChatGPT.app` or `Codex.app` in `/Applications` or `~/Applications`
//!   (earlier builds: `<app>/Contents/Resources/codex`).
//!
//! Sophon points the service at that one (its provider's `command`, the argv it runs), so the
//! agent is there to use, and follows the app's updates: Claude's path
//! names the version.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// The machine's own folders (`/Applications`, `/opt/homebrew/bin`) start
/// here: `/`, but nowhere in tests, which see only the home they make
/// (the live test, against a service of its own, sees the machine).
fn system(path: &str) -> PathBuf {
    let hidden = cfg!(test) && std::env::var_os("PASEO_LISTEN").is_none();
    PathBuf::from(if hidden { "/nonexistent-in-tests" } else { "/" }).join(path)
}

/// Where the apps keep their CLIs under `home`, in the order tried.
fn places(provider: &str, home: &Path) -> Vec<PathBuf> {
    match provider {
        "claude" => vec![home.join("Library/Application Support/Claude/claude-code")],
        "codex" => [system("Applications"), home.join("Applications")]
            .iter()
            .flat_map(|apps| ["ChatGPT.app", "Codex.app"].map(|app| apps.join(app).join("Contents/Resources")))
            .collect(),
        _ => Vec::new(),
    }
}

/// `2.1.177` as numbers, to tell the newest: a part that is no number is 0.
fn version_key(name: &str) -> Vec<u64> {
    name.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// The folders a CLI installed on its own goes to that a login shell's
/// `PATH` may lack: the installers' own (`~/.local/bin`, `~/.claude/local`),
/// Homebrew's, and the package managers' global ones.
pub(crate) fn install_dirs(home: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = [".local/bin", ".claude/local", ".npm-global/bin", ".bun/bin", ".volta/bin", "Library/pnpm", ".local/share/pnpm", ".yarn/bin"]
        .iter()
        .map(|d| home.join(d))
        .collect();
    dirs.extend(["opt/homebrew/bin", "usr/local/bin"].map(system));
    // A Node version manager keeps a folder per Node: the newest first.
    for versions in [home.join(".nvm/versions/node"), home.join(".local/share/fnm/node-versions"), home.join("Library/Application Support/fnm/node-versions")] {
        let mut nodes: Vec<(Vec<u64>, PathBuf)> = std::fs::read_dir(&versions)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| (version_key(e.file_name().to_string_lossy().trim_start_matches('v')), e.path()))
            .collect();
        nodes.sort();
        dirs.extend(nodes.into_iter().rev().flat_map(|(_, node)| [node.join("bin"), node.join("installation/bin")]));
    }
    dirs
}

/// The CLI of `provider` installed on its own under `home`, where the
/// service's `PATH` may not reach.
fn installed(provider: &str, home: &Path) -> Option<PathBuf> {
    if !matches!(provider, "claude" | "codex") {
        return None;
    }
    install_dirs(home).into_iter().map(|d| d.join(provider)).find(|bin| bin.is_file())
}

/// The CLI of `provider` under `home`: one installed on its own, else the
/// one an app carries (the newest of them).
pub(crate) fn find_in(provider: &str, home: &Path) -> Option<PathBuf> {
    if let Some(bin) = installed(provider, home) {
        return Some(bin);
    }
    let places = places(provider, home);
    match provider {
        "claude" => {
            let mut versions: Vec<(Vec<u64>, PathBuf)> = std::fs::read_dir(places.first()?)
                .ok()?
                .flatten()
                .filter_map(|e| {
                    // The app keeps it as an app of its own, or bare.
                    let bin = [e.path().join("claude.app/Contents/MacOS/claude"), e.path().join("claude")].into_iter().find(|b| b.is_file())?;
                    Some((version_key(&e.file_name().to_string_lossy()), bin))
                })
                .collect();
            versions.sort();
            versions.pop().map(|(_, bin)| bin)
        }
        "codex" => places.iter().flat_map(|r| [r.join("codex-cli/bin/codex"), r.join("codex")]).find(|bin| bin.is_file()),
        _ => None,
    }
}

/// The CLI of `provider` on this Mac that the service may not find.
pub(crate) fn find(provider: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    find_in(provider, &PathBuf::from(std::env::var_os("HOME")?))
}

/// Whether `path` is in one of the places Sophon looks for `provider`'s
/// CLI: a `command` there is one Sophon set, and may change again.
fn inside_an_app(provider: &str, path: &str, home: &Path) -> bool {
    let path = Path::new(path);
    places(provider, home).iter().any(|p| path.starts_with(p)) || install_dirs(home).iter().any(|d| path.parent() == Some(d.as_path()))
}

/// Whether `bin` is the one a desktop app carries (not one installed on
/// its own), for what is said about it.
pub(crate) fn in_an_app(provider: &str, bin: &Path) -> bool {
    std::env::var_os("HOME").is_some_and(|home| places(provider, Path::new(&home)).iter().any(|p| bin.starts_with(p)))
}

/// The binary `provider`'s config runs in place of the one on the `PATH`,
/// if it names one: its `command` is the whole argv, a path first. The bare
/// name (`claude`) is the `PATH`'s.
fn replaced(config: Option<&Value>, provider: &str) -> Option<String> {
    let first = config?.get("providers")?.get(provider)?.get("command")?.get(0)?.as_str()?;
    first.starts_with('/').then(|| first.to_owned())
}

/// The patch that runs `provider` from `bin`.
pub(crate) fn use_patch(provider: &str, bin: &Path) -> Value {
    json!({ "providers": { provider: { "command": [bin.to_string_lossy()] } } })
}

/// The patch that runs `provider` from the `PATH` again: its bare name, as
/// a patch cannot take a `command` away.
fn default_patch(provider: &str) -> Value {
    json!({ "providers": { provider: { "command": [provider] } } })
}

/// What to change in the service's config for `provider`, if anything:
///
/// - Not installed for the service, with no command of its own, and an app
///   carries it: use the app's.
/// - Already on an app's, which is gone (the app updated, or was removed):
///   the newest the app has now, else the `PATH` again.
///
/// A command someone set to anything else is theirs and stays.
pub(crate) fn patch_for(provider: &str, unavailable: bool, config: Option<&Value>, home: &Path) -> Option<Value> {
    match replaced(config, provider) {
        Some(current) if inside_an_app(provider, &current, home) => {
            if Path::new(&current).is_file() {
                return None;
            }
            Some(find_in(provider, home).map_or_else(|| default_patch(provider), |bin| use_patch(provider, &bin)))
        }
        Some(_) => None,
        None => unavailable.then(|| find_in(provider, home)).flatten().map(|bin| use_patch(provider, &bin)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "").unwrap();
    }

    fn home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sophon-bundled-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn claude(home: &Path, version: &str) -> PathBuf {
        home.join("Library/Application Support/Claude/claude-code").join(version).join("claude.app/Contents/MacOS/claude")
    }

    #[test]
    fn the_claude_app_s_newest_claude_code_is_found() {
        let home = home("newest");
        assert_eq!(find_in("claude", &home), None);
        touch(&claude(&home, "2.1.99"));
        touch(&claude(&home, "2.1.177"));
        // A version still downloading has no binary yet.
        std::fs::create_dir_all(home.join("Library/Application Support/Claude/claude-code/2.2.0")).unwrap();
        assert_eq!(find_in("claude", &home), Some(claude(&home, "2.1.177")));
    }

    #[test]
    fn the_codex_app_s_cli_is_found_in_the_user_s_applications() {
        let home = home("codex");
        let bin = home.join("Applications/Codex.app/Contents/Resources/codex-cli/bin/codex");
        touch(&bin);
        assert_eq!(find_in("codex", &home), Some(bin));
        assert_eq!(find_in("copilot", &home), None);
    }

    #[test]
    fn an_agent_not_installed_runs_from_the_app() {
        let home = home("use");
        touch(&claude(&home, "2.1.177"));
        let patch = patch_for("claude", true, Some(&json!({ "providers": { "claude": { "enabled": true } } })), &home).unwrap();
        assert_eq!(patch["providers"]["claude"]["command"], json!([claude(&home, "2.1.177").to_string_lossy()]));
        // Installed on the PATH: nothing to do.
        assert_eq!(patch_for("claude", false, None, &home), None);
    }

    #[test]
    fn the_command_follows_the_app_s_update_and_its_removal() {
        let home = home("follow");
        touch(&claude(&home, "2.1.178"));
        let on_old = json!({ "providers": { "claude": { "command": [claude(&home, "2.1.177").to_string_lossy()] } } });
        let patch = patch_for("claude", false, Some(&on_old), &home).unwrap();
        assert_eq!(patch["providers"]["claude"]["command"], json!([claude(&home, "2.1.178").to_string_lossy()]));
        // Still there: kept, even with a newer beside it.
        let on_new = json!({ "providers": { "claude": { "command": [claude(&home, "2.1.178").to_string_lossy()] } } });
        touch(&claude(&home, "2.1.179"));
        assert_eq!(patch_for("claude", false, Some(&on_new), &home), None);
        // The app gone: the PATH again.
        std::fs::remove_dir_all(home.join("Library")).unwrap();
        assert_eq!(patch_for("claude", true, Some(&on_new), &home).unwrap()["providers"]["claude"]["command"], json!(["claude"]));
        // On the PATH's again, and still not there: the app's once it is back.
        touch(&claude(&home, "2.1.180"));
        let on_path = json!({ "providers": { "claude": { "command": ["claude"] } } });
        assert_eq!(patch_for("claude", true, Some(&on_path), &home), Some(use_patch("claude", &claude(&home, "2.1.180"))));
    }

    #[test]
    fn a_cli_installed_where_the_service_does_not_look_is_found_first() {
        let home = home("installed");
        touch(&claude(&home, "2.1.177"));
        // Claude Code's own installer: ~/.local/bin, on the PATH by .zshrc alone.
        let own = home.join(".local/bin/claude");
        touch(&own);
        assert_eq!(find_in("claude", &home), Some(own.clone()));
        let patch = patch_for("claude", true, None, &home).unwrap();
        assert_eq!(patch["providers"]["claude"]["command"], json!([own.to_string_lossy()]));
        // Uninstalled: the app's takes its place.
        std::fs::remove_file(&own).unwrap();
        let on_own = json!({ "providers": { "claude": { "command": [own.to_string_lossy()] } } });
        assert_eq!(patch_for("claude", false, Some(&on_own), &home), Some(use_patch("claude", &claude(&home, "2.1.177"))));
        // A Node version manager's global install, the newest Node's.
        let nvm = |v: &str| home.join(".nvm/versions/node").join(v).join("bin/codex");
        touch(&nvm("v20.11.0"));
        touch(&nvm("v22.3.0"));
        assert_eq!(find_in("codex", &home), Some(nvm("v22.3.0")));
        // The app keeping the CLI bare, not as an app of its own.
        let bare = home.join("Library/Application Support/Claude/claude-code/2.2.0/claude");
        touch(&bare);
        assert_eq!(find_in("claude", &home), Some(bare));
    }

    #[test]
    fn a_command_set_by_hand_stays() {
        let home = home("hand");
        touch(&claude(&home, "2.1.177"));
        let by_hand = json!({ "providers": { "claude": { "command": ["/opt/missing/claude", "--flag"] } } });
        assert_eq!(patch_for("claude", true, Some(&by_hand), &home), None);
    }
}
