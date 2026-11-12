//! The agent CLIs the desktop apps carry inside themselves.
//!
//! The service looks for `claude` and `codex` on the `PATH`. Someone with
//! only the Claude app and the Codex app has neither there, though both
//! apps hold the CLI they run:
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

/// Where the apps keep their CLIs under `home`, in the order tried.
fn places(provider: &str, home: &Path) -> Vec<PathBuf> {
    match provider {
        "claude" => vec![home.join("Library/Application Support/Claude/claude-code")],
        "codex" => [PathBuf::from("/Applications"), home.join("Applications")]
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

/// The CLI of `provider` an app under `home` carries, the newest of them.
pub(crate) fn find_in(provider: &str, home: &Path) -> Option<PathBuf> {
    let places = places(provider, home);
    match provider {
        "claude" => {
            let mut versions: Vec<(Vec<u64>, PathBuf)> = std::fs::read_dir(places.first()?)
                .ok()?
                .flatten()
                .filter_map(|e| {
                    let bin = e.path().join("claude.app/Contents/MacOS/claude");
                    bin.is_file().then(|| (version_key(&e.file_name().to_string_lossy()), bin))
                })
                .collect();
            versions.sort();
            versions.pop().map(|(_, bin)| bin)
        }
        "codex" => places.iter().flat_map(|r| [r.join("codex-cli/bin/codex"), r.join("codex")]).find(|bin| bin.is_file()),
        _ => None,
    }
}

/// The CLI of `provider` an app on this Mac carries.
pub(crate) fn find(provider: &str) -> Option<PathBuf> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    find_in(provider, &PathBuf::from(std::env::var_os("HOME")?))
}

/// Whether `path` is inside one of the places an app keeps `provider`'s
/// CLI: a `command` there is one Sophon set, and may change again.
fn inside_an_app(provider: &str, path: &str, home: &Path) -> bool {
    places(provider, home).iter().any(|p| Path::new(path).starts_with(p))
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
        assert!(find_in("codex", &home).is_some_and(|found| found == bin || found.starts_with("/Applications")));
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
    fn a_command_set_by_hand_stays() {
        let home = home("hand");
        touch(&claude(&home, "2.1.177"));
        let by_hand = json!({ "providers": { "claude": { "command": ["/opt/missing/claude", "--flag"] } } });
        assert_eq!(patch_for("claude", true, Some(&by_hand), &home), None);
    }
}
