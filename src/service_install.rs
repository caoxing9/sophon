//! The agent service, installed by Sophon itself on a Mac that has none.
//!
//! Sophon is the window; the agents run in a service (Paseo's daemon,
//! `@getpaseo/cli`, Apache-2.0). A Mac with the Paseo app or its command
//! line has it; one with only Sophon does not, and nothing would start.
//! So Sophon brings its own: a Node runtime of its own and the service's
//! package, under `~/.sophon/service`, with no admin rights and nothing
//! added to the user's shell:
//!
//! ```text
//! ~/.sophon/service/node/bin/node     Node, from nodejs.org (checked by its SHA-256)
//! ~/.sophon/service/node/bin/paseo    the service's command line (npm, into the same prefix)
//! ```
//!
//! A service the user installed themselves is found first
//! (`store::service_cli`): this one is only for a Mac without any.

use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// The Node runtime the service runs on.
const NODE: &str = "v22.23.5";
/// The service's version: the one Sophon is built against.
const SERVICE: &str = "0.11.4";

/// How far the install is, for the sidebar.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Step {
    /// Downloading Node: 0 to 1.
    Downloading(f32),
    /// Unpacking Node, installing the service's package.
    Installing,
}

/// Where Sophon keeps its own service.
fn dir() -> PathBuf {
    crate::daemon::data_dir().join("service")
}

/// The command line of the service Sophon installed, if it did.
pub(crate) fn cli() -> Option<PathBuf> {
    let cli = dir().join("node/bin/paseo");
    cli.is_file().then_some(cli)
}

/// Node's name for this Mac's processor.
fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" }
}

fn run(cmd: &mut Command, what: &str) -> Result<Vec<u8>, String> {
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        return Ok(out.stdout);
    }
    let said = String::from_utf8_lossy(&out.stderr).lines().rev().find(|l| !l.trim().is_empty()).map(|l| l.trim().to_owned());
    Err(format!("{what}: {}", said.unwrap_or_else(|| out.status.to_string())))
}

/// The SHA-256 nodejs.org gives for `file` in its list of sums (`sums`).
fn sum_of<'a>(sums: &'a str, file: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (sum, name) = line.split_once(char::is_whitespace)?;
        (name.trim() == file).then_some(sum.trim())
    })
}

/// Downloads `url` to `path`, saying how far it is of `total` bytes.
fn download(url: &str, path: &Path, total: u64, progress: &impl Fn(Step)) -> Result<(), String> {
    let mut child = Command::new("/usr/bin/curl")
        .args(["-fsSL", "--retry", "3", "--connect-timeout", "20", "-o"])
        .arg(path)
        .arg(url)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl: {e}"))?;
    loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => return Ok(()),
            Some(_) => {
                let mut said = String::new();
                if let Some(mut err) = child.stderr.take() {
                    use std::io::Read;
                    let _ = err.read_to_string(&mut said);
                }
                return Err(format!("the download failed: {}", said.trim()));
            }
            None => {
                let have = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                if total > 0 {
                    progress(Step::Downloading((have as f32 / total as f32).min(1.0)));
                }
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
        }
    }
}

/// Installs the service under `~/.sophon/service` (blocking: a download of
/// some 50 MB and an `npm install`; about 400 MB once installed), telling `progress` how far it is. The
/// command line to start it with.
pub(crate) fn install(progress: impl Fn(Step)) -> Result<PathBuf, String> {
    let dir = dir();
    let node = dir.join("node");
    // Made beside its place and moved there whole: a start that finds
    // `node/bin/paseo` finds a complete install.
    let making = dir.join("node.installing");
    let _ = std::fs::remove_dir_all(&making);
    std::fs::create_dir_all(&making).map_err(|e| format!("{}: {e}", making.display()))?;

    let file = format!("node-{NODE}-darwin-{}.tar.gz", arch());
    let base = format!("https://nodejs.org/dist/{NODE}");
    progress(Step::Downloading(0.0));
    let sums = String::from_utf8_lossy(&run(Command::new("/usr/bin/curl").args(["-fsSL", "--retry", "3", "--connect-timeout", "20", &format!("{base}/SHASUMS256.txt")]), "the list of Node downloads")?).into_owned();
    let want = sum_of(&sums, &file).ok_or_else(|| format!("nodejs.org lists no {file}"))?.to_owned();
    let archive = dir.join(&file);
    // Its size, to say how far the download is (a missing header: no bar).
    let head = run(Command::new("/usr/bin/curl").args(["-fsSLI", "--connect-timeout", "20", &format!("{base}/{file}")]), "Node's download").unwrap_or_default();
    let total = String::from_utf8_lossy(&head)
        .lines()
        .filter_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<u64>().ok()))
        .next_back()
        .unwrap_or(0);
    download(&format!("{base}/{file}"), &archive, total, &progress)?;
    let got = String::from_utf8_lossy(&run(Command::new("/usr/bin/shasum").args(["-a", "256"]).arg(&archive), "checking Node's download")?).split_whitespace().next().unwrap_or_default().to_owned();
    if got != want {
        let _ = std::fs::remove_file(&archive);
        return Err("Node's download does not match its checksum".into());
    }

    progress(Step::Installing);
    run(Command::new("/usr/bin/tar").arg("-xzf").arg(&archive).arg("-C").arg(&making).args(["--strip-components", "1"]), "unpacking Node")?;
    let _ = std::fs::remove_file(&archive);
    // npm is a script run by the Node beside it: that folder first on the
    // PATH, then the system's (git and the like for packages that ask).
    let bin = making.join("bin");
    let path = std::env::join_paths([bin.clone(), "/usr/bin".into(), "/bin".into(), "/usr/sbin".into(), "/sbin".into()]).map_err(|e| e.to_string())?;
    run(
        Command::new(bin.join("npm"))
            .args(["install", "-g", "--no-audit", "--no-fund", "--loglevel", "error", "--prefix"])
            .arg(&making)
            .arg(format!("@getpaseo/cli@{SERVICE}"))
            .env("PATH", &path)
            .current_dir(&making),
        "installing the service",
    )?;
    if !bin.join("paseo").is_file() {
        return Err("the service's package installed no command line".into());
    }
    slim(&making);
    let _ = std::fs::remove_dir_all(&node);
    std::fs::rename(&making, &node).map_err(|e| format!("{}: {e}", node.display()))?;
    Ok(node.join("bin/paseo"))
}

/// Takes out of a fresh install what the service never runs, some 330 of
/// its 730 MB: what Node ships for building things (its headers, npm), the
/// copy of Claude Code the agent SDK carries (the service runs the user's
/// own, which it is pointed at), and the speech library (Sophon listens
/// and speaks with the system's).
fn slim(node: &Path) {
    for gone in ["include", "share", "lib/node_modules/npm", "lib/node_modules/corepack", "bin/npm", "bin/npx", "bin/corepack", "CHANGELOG.md", "README.md"] {
        let path = node.join(gone);
        let _ = if path.is_dir() && !path.is_symlink() { std::fs::remove_dir_all(&path) } else { std::fs::remove_file(&path) };
    }
    let modules = node.join("lib/node_modules/@getpaseo/cli/node_modules");
    // Per platform: `claude-agent-sdk-darwin-arm64`, `sherpa-onnx-darwin-arm64`.
    for (dir, prefix) in [(modules.join("@anthropic-ai"), "claude-agent-sdk-"), (modules.clone(), "sherpa-onnx-")] {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            if entry.file_name().to_string_lossy().starts_with(prefix) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_checksum_is_the_one_listed_for_the_file() {
        let sums = "aa11  node-v22.23.5-darwin-arm64.tar.gz\nbb22  node-v22.23.5-darwin-x64.tar.gz\ncc33  node-v22.23.5.pkg\n";
        assert_eq!(sum_of(sums, "node-v22.23.5-darwin-arm64.tar.gz"), Some("aa11"));
        assert_eq!(sum_of(sums, "node-v22.23.5-darwin-x64.tar.gz"), Some("bb22"));
        assert_eq!(sum_of(sums, "node-v22.23.5-linux-x64.tar.gz"), None);
    }

    /// The whole install, for real: `SOPHON_DATA_DIR=<empty folder>
    /// cargo test --lib the_service_installs_live -- --ignored --nocapture`
    /// (a download of some 50 MB; about 400 MB once installed).
    #[test]
    #[ignore]
    fn the_service_installs_live() {
        assert!(std::env::var_os("SOPHON_DATA_DIR").is_some(), "SOPHON_DATA_DIR: not the user's own folder");
        assert_eq!(cli(), None);
        let steps = std::cell::RefCell::new(Vec::new());
        let installed = install(|step| steps.borrow_mut().push(step)).expect("it installs");
        assert_eq!(cli(), Some(installed.clone()));
        let steps = steps.into_inner();
        assert!(steps.iter().any(|s| matches!(s, Step::Downloading(p) if *p > 0.5)), "the download said how far it was");
        assert_eq!(steps.last(), Some(&Step::Installing));
        let version = Command::new(&installed).arg("--version").env("PATH", crate::store::service_path(&installed)).output().expect("it runs");
        let version = String::from_utf8_lossy(&version.stdout).trim().to_owned();
        println!("installed: {} says {version}", installed.display());
        assert_eq!(version, SERVICE);
        // Slimmed: none of what the service never runs.
        let node = installed.parent().and_then(Path::parent).unwrap();
        assert!(!node.join("include").exists() && !node.join("bin/npm").exists());
        assert!(!node.join("lib/node_modules/@getpaseo/cli/node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64").exists());
        // The SDK itself (the JavaScript the service imports) stays.
        assert!(node.join("lib/node_modules/@getpaseo/cli/node_modules/@anthropic-ai/claude-agent-sdk").is_dir());
    }
}
