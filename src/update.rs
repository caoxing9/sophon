//! The app updating itself from GitHub Releases (`nsh.json` `"updater"`).
//!
//! `nsh package` puts the updater configuration in the package's assets; a
//! development run has none, so nothing is checked there. The checks, the
//! verified download and the swap are `nsh-updater`'s; HTTP is the system's
//! `curl`, which every Mac has, so the app links no second TLS stack. Work
//! runs on a thread of its own and reports to the engine thread through a
//! channel, as the daemon connection does.

#![cfg_attr(not(any(target_os = "macos", target_os = "linux", target_os = "windows")), allow(dead_code, unused_imports))]

use std::{
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

use nsh_core::prelude::*;
#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
use nsh_updater::install::Installation;
#[cfg(not(target_arch = "wasm32"))]
use nsh_updater::{Available, Config, Download};

/// How often a running app looks for a newer version.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Where an update stands, for the sidebar.
#[derive(Clone, Default, PartialEq)]
pub(crate) enum Phase {
    /// Nothing newer, or not checked yet.
    #[default]
    Idle,
    /// A newer version, ready to download.
    Available,
    /// Downloading: the share received, 0 to 1.
    Downloading(f32),
    /// Downloaded and checked: a restart puts it in place.
    Ready,
    /// Swapping the app; it restarts next.
    Installing,
    /// The last attempt failed.
    Failed(String),
}

#[derive(Clone, Default, PartialEq)]
pub(crate) struct UpdateState {
    /// The running version.
    pub(crate) current: String,
    /// The newer version on offer.
    pub(crate) version: Option<String>,
    /// Its release notes.
    pub(crate) notes: Option<String>,
    /// Every release since the running one, newest first: its version and
    /// notes (Markdown), for the update dialog.
    pub(crate) changes: Vec<(String, String)>,
    /// Where to get it by hand when the app cannot replace itself.
    pub(crate) url: Option<String>,
    /// Whether the app can replace itself here.
    pub(crate) can_install: bool,
    pub(crate) phase: Phase,
}

pub(crate) fn state() -> UpdateState {
    global::<UpdateState>().with(Clone::clone)
}

#[cfg(not(target_arch = "wasm32"))]
enum Msg {
    Found(Available, bool, Option<String>, Vec<(String, String)>),
    Progress(f32),
    Downloaded,
    Installing,
    Ready,
    Failed(String),
}

#[cfg(not(target_arch = "wasm32"))]
/// What the package says about updates, and its bytes (the installation
/// check compares them).
fn package_config() -> Option<(Config, Vec<u8>)> {
    let exe = std::env::current_exe().ok()?;
    // <Name>.app/Contents/MacOS/<exe> → Contents/Resources/nsh/assets
    let assets = exe.parent()?.parent()?.join("Resources").join("nsh").join("assets");
    let bytes = std::fs::read(assets.join(nsh_updater::CONFIG_ASSET)).ok()?;
    let config = Config::parse(&bytes).ok()?;
    Some((config, bytes))
}

/// The running version: the package's, else the crate's (development).
pub(crate) fn current_version() -> String {
    #[cfg(target_arch = "wasm32")]
    return env!("CARGO_PKG_VERSION").to_owned();
    #[cfg(not(target_arch = "wasm32"))]
    package_config().map(|(c, _)| c.version).unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned())
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn curl(url: &str) -> Result<Vec<u8>, String> {
    let out = Command::new("curl")
        .args(["-fsSL", "--max-time", "30", url])
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn check(config: &Config) -> Result<Option<Available>, String> {
    let manifest = curl(&config.manifest_url())?;
    let signature = match config.signature_url() {
        Some(url) => Some(curl(&url)?),
        None => None,
    };
    config.check(&manifest, signature.as_deref(), &nsh_updater::platform()).map_err(|e| e.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
/// Downloads `update` into the data directory, reporting progress, and
/// checks its size and SHA-256.
fn download(update: &Available, progress: impl Fn(f32)) -> Result<PathBuf, String> {
    let sha = update.sha256.clone().ok_or("the release has no archive for this Mac")?;
    let dir = std::env::temp_dir().join("sophon-update");
    let _ = std::fs::create_dir_all(&dir);
    let name = update.url.rsplit('/').next().unwrap_or("update.tar.gz");
    let path = dir.join(name);
    let mut child = Command::new("curl")
        .args(["-fsSL", "-o"])
        .arg(&path)
        .arg(&update.url)
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl: {e}"))?;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            if !status.success() {
                return Err("the download failed".into());
            }
            break;
        }
        if let (Some(total), Ok(meta)) = (update.size, std::fs::metadata(&path)) {
            progress((meta.len() as f32 / total.max(1) as f32).min(1.0));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let mut check = Download::new(&sha, update.size);
    check.update(&bytes).map_err(|e| e.to_string())?;
    check.finish().map_err(|e| e.to_string())?;
    Ok(path)
}

/// Starts checking in the background (packaged desktop apps only).
pub(crate) fn start() {
    global::<UpdateState>().set(UpdateState { current: current_version(), ..Default::default() });
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    start_desktop();
}

#[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
fn start_desktop() {
    let Some((config, bytes)) = package_config() else { return };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Msg>();
    let (go_tx, go_rx) = std::sync::mpsc::channel::<()>();
    global::<Installer>().set(Installer(Some(std::sync::Arc::new(std::sync::Mutex::new(go_tx)))));
    let spawned = std::thread::Builder::new().name("sophon-update".into()).spawn(move || {
        let installation = Installation::current(&config.id, &bytes);
        if let Some(i) = &installation {
            i.remove_leftovers();
        }
        let can_install = installation.as_ref().is_some_and(Installation::can_install);
        let page = config.github.as_ref().map(|repo| format!("https://github.com/{repo}/releases/latest"));
        std::thread::sleep(Duration::from_secs(5));
        let current = config.version.clone();
        loop {
            match check(&config) {
                Ok(Some(update)) => {
                    let changes = config.github.as_deref().map(|repo| changes_since(repo, &current)).unwrap_or_default();
                    let installable = can_install && update.sha256.is_some();
                    let _ = tx.send(Msg::Found(update.clone(), installable, page.clone(), changes));
                    let Some(installation) = installation.as_ref().filter(|_| installable) else {
                        // Only a page to open: look again later.
                        std::thread::sleep(CHECK_EVERY);
                        continue;
                    };
                    // Fetched quietly, so the update is one restart away
                    // by the time it is offered.
                    let progress = |p: f32| {
                        let _ = tx.send(Msg::Progress(p));
                    };
                    let archive = match download(&update, progress) {
                        Ok(archive) => archive,
                        Err(e) => {
                            let _ = tx.send(Msg::Failed(e));
                            // A retry (or the next check) starts over.
                            let _ = go_rx.recv_timeout(CHECK_EVERY);
                            continue;
                        }
                    };
                    let _ = tx.send(Msg::Downloaded);
                    // Wait for the user's go; a newer release by then is
                    // fetched in its place.
                    if go_rx.recv_timeout(CHECK_EVERY).is_err() {
                        continue;
                    }
                    let _ = tx.send(Msg::Installing);
                    let _ = tx.send(match installation.install(&archive) {
                        Ok(()) => Msg::Ready,
                        Err(e) => Msg::Failed(e.to_string()),
                    });
                    continue;
                }
                Ok(None) => {}
                Err(e) => log::info!("update check: {e}"),
            }
            std::thread::sleep(CHECK_EVERY);
        }
    });
    if let Err(e) = spawned {
        log::warn!("cannot start the updater: {e}");
        return;
    }
    spawn(async move {
        while let Some(msg) = rx.recv().await {
            let s = global::<UpdateState>();
            match msg {
                Msg::Found(a, can_install, page, changes) => s.update(|s| {
                    s.version = Some(a.version);
                    s.notes = a.notes;
                    s.changes = changes;
                    s.url = page.or(Some(a.url));
                    s.can_install = can_install;
                    s.phase = Phase::Available;
                }),
                Msg::Progress(p) => s.update(|s| s.phase = Phase::Downloading(p)),
                Msg::Downloaded => s.update(|s| s.phase = Phase::Ready),
                Msg::Installing => s.update(|s| s.phase = Phase::Installing),
                Msg::Failed(e) => s.update(|s| s.phase = Phase::Failed(e)),
                Msg::Ready => nsh_core::element::desktop::restart(),
            }
        }
    });
}

#[derive(Clone, Default)]
struct Installer(Option<std::sync::Arc<std::sync::Mutex<std::sync::mpsc::Sender<()>>>>);

/// Puts the downloaded update in place and restarts; after a failure,
/// tries again; where the app cannot replace itself, opens the release.
pub(crate) fn install() {
    let s = state();
    if !s.can_install {
        if let Some(url) = s.url {
            let _ = Command::new("open").arg(url).spawn();
        }
        return;
    }
    match s.phase {
        Phase::Ready => global::<UpdateState>().update(|s| s.phase = Phase::Installing),
        Phase::Failed(_) => global::<UpdateState>().update(|s| s.phase = Phase::Downloading(0.0)),
        _ => return,
    }
    if let Some(tx) = global::<Installer>().with_untracked(|i| i.0.clone())
        && let Ok(tx) = tx.lock()
    {
        let _ = tx.send(());
    }
}

/// The releases of `repo` newer than `current`, newest first, with their
/// notes: GitHub's API, one call per check (well inside its limit for
/// callers with no token). None when it cannot be reached; the dialog then
/// shows the manifest's notes.
#[cfg(not(target_arch = "wasm32"))]
fn changes_since(repo: &str, current: &str) -> Vec<(String, String)> {
    let Ok(bytes) = curl(&format!("https://api.github.com/repos/{repo}/releases?per_page=30")) else {
        return vec![];
    };
    let Ok(serde_json::Value::Array(releases)) = serde_json::from_slice(&bytes) else {
        return vec![];
    };
    releases
        .iter()
        .filter(|r| !r["draft"].as_bool().unwrap_or(false) && !r["prerelease"].as_bool().unwrap_or(false))
        .filter_map(|r| {
            let version = r["tag_name"].as_str()?.trim_start_matches('v').to_owned();
            newer(&version, current).then(|| (version, r["body"].as_str().unwrap_or_default().trim().to_owned()))
        })
        .collect()
}

/// Whether version `a` comes after `b`, compared part by part as numbers
/// (`0.1.10` after `0.1.9`).
pub(crate) fn newer(a: &str, b: &str) -> bool {
    let parts = |v: &str| v.split(['.', '-', '+']).map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parts(a) > parts(b)
}

/// A pretend update for looking at the dialog in development:
/// `SOPHON_PREVIEW_UPDATE=1 nsh run`.
pub(crate) fn preview() {
    if std::env::var_os("SOPHON_PREVIEW_UPDATE").is_none() {
        return;
    }
    global::<UpdateState>().update(|s| {
        s.version = Some("0.1.2".into());
        s.can_install = true;
        s.phase = Phase::Ready;
        s.changes = vec![
            ("0.1.2".into(), "### Fixes\n- Choose folder… opens the folder dialog again.\n- Tool details spring open under their row.".into()),
            ("0.1.1".into(), "### Features\n- Inline chips in the composer, with previews.\n- `Cmd+V` pastes images and files.".into()),
        ];
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn versions_compare_as_numbers() {
        assert!(super::newer("0.1.10", "0.1.9"));
        assert!(super::newer("1.0.0", "0.9.9"));
        assert!(!super::newer("0.1.0", "0.1.0"));
        assert!(!super::newer("0.1.0", "0.2.0"));
    }
}
