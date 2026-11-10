//! Updates of the agents' CLIs on this computer: how each was installed
//! (from where its binary really lives), the newest version that way, and the
//! command that updates it. Run only for this computer's own agents; a remote
//! host's are only reported.

// The phone updates nothing: only reading is left there.
#![cfg_attr(any(target_os = "ios", target_arch = "wasm32"), allow(dead_code))]

/// How an agent's CLI was installed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Install {
    /// Claude Code's own installer (`~/.local/share/claude/versions/…`):
    /// `claude update`.
    ClaudeNative,
    /// A Homebrew cask (`…/Caskroom/<token>/…`).
    BrewCask(String),
    /// A Homebrew formula (`…/Cellar/<name>/…`).
    BrewFormula(String),
    /// A global npm package (`…/node_modules/<package>/…`).
    Npm(String),
}

/// Claude Code's npm package: the newest version its own installer brings.
const CLAUDE_PACKAGE: &str = "@anthropic-ai/claude-code";

/// How the CLI at `real` (its path with links resolved) was installed.
pub(crate) fn install_of(real: &str) -> Option<Install> {
    if real.contains("/.local/share/claude/versions/") {
        return Some(Install::ClaudeNative);
    }
    let after = |marker: &str| real.split_once(marker).and_then(|(_, rest)| rest.split('/').next()).filter(|s| !s.is_empty()).map(str::to_owned);
    if let Some(token) = after("/Caskroom/") {
        return Some(Install::BrewCask(token));
    }
    if let Some(name) = after("/Cellar/") {
        return Some(Install::BrewFormula(name));
    }
    // The package's own folder: `@scope/name` or `name`.
    let (_, rest) = real.rsplit_once("/node_modules/")?;
    let mut parts = rest.split('/');
    let first = parts.next().filter(|s| !s.is_empty())?;
    let package = if first.starts_with('@') { format!("{first}/{}", parts.next()?) } else { first.to_owned() };
    Some(Install::Npm(package))
}

/// The version in a CLI's own words: `2.1.321 (Claude Code)`, `codex-cli
/// 0.163.0` give `2.1.321`, `0.163.0`.
pub(crate) fn version_of(text: &str) -> Option<String> {
    text.split(|c: char| c.is_whitespace() || c == '(' || c == ')' || c == 'v')
        .find(|w| w.split('.').count() >= 2 && w.split('.').all(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())))
        .map(str::to_owned)
}

/// Whether `latest` is newer than `installed` (dotted numbers).
pub(crate) fn newer(installed: &str, latest: &str) -> bool {
    let parse = |v: &str| v.split('.').map(|n| n.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    parse(latest) > parse(installed)
}

/// The program and arguments that update `install`.
pub(crate) fn update_command(install: &Install, binary: &str) -> (String, Vec<String>) {
    match install {
        Install::ClaudeNative => (binary.to_owned(), vec!["update".into()]),
        Install::BrewCask(token) => (brew(), vec!["upgrade".into(), "--cask".into(), token.clone()]),
        Install::BrewFormula(name) => (brew(), vec!["upgrade".into(), name.clone()]),
        Install::Npm(package) => (tool("npm"), vec!["install".into(), "-g".into(), format!("{package}@latest")]),
    }
}

/// The command line `update_command` runs, as a person would type it.
pub(crate) fn command_line(install: &Install) -> String {
    match install {
        Install::ClaudeNative => "claude update".into(),
        Install::BrewCask(token) => format!("brew upgrade --cask {token}"),
        Install::BrewFormula(name) => format!("brew upgrade {name}"),
        Install::Npm(package) => format!("npm install -g {package}@latest"),
    }
}

/// Homebrew, where it installs itself: an app opened from the Finder has no
/// shell's PATH.
fn brew() -> String {
    ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"].into_iter().find(|p| std::path::Path::new(p).exists()).unwrap_or("brew").to_owned()
}

/// `name` on the PATH, else in the usual places.
fn tool(name: &str) -> String {
    let mut dirs: Vec<std::path::PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.extend(["/opt/homebrew/bin", "/usr/local/bin"].map(std::path::PathBuf::from));
    dirs.into_iter().map(|d| d.join(name)).find(|p| p.exists()).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| name.to_owned())
}

/// The newest version `install` would bring (blocking: a network or brew
/// call). Homebrew's own, which can trail npm's, since that is what its
/// update gets.
#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
pub(crate) fn latest(install: &Install) -> Result<String, String> {
    let run = |program: &str, args: &[&str]| -> Result<String, String> {
        let out = std::process::Command::new(program).args(args).output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("failed").to_owned());
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    };
    match install {
        Install::BrewCask(token) | Install::BrewFormula(token) => {
            let flag = if matches!(install, Install::BrewCask(_)) { "--cask" } else { "--formula" };
            let json: serde_json::Value = serde_json::from_str(&run(&brew(), &["info", "--json=v2", flag, token])?).map_err(|e| e.to_string())?;
            json.pointer("/casks/0/version")
                .or_else(|| json.pointer("/formulae/0/versions/stable"))
                .and_then(serde_json::Value::as_str)
                .and_then(version_of)
                .ok_or_else(|| "no version in brew info".to_owned())
        }
        Install::ClaudeNative | Install::Npm(_) => {
            let package = match install {
                Install::Npm(p) => p.as_str(),
                _ => CLAUDE_PACKAGE,
            };
            let url = format!("https://registry.npmjs.org/{package}/latest");
            let json: serde_json::Value = serde_json::from_str(&run("/usr/bin/curl", &["-fsSL", "-m", "10", &url])?).map_err(|e| e.to_string())?;
            json.get("version").and_then(serde_json::Value::as_str).map(str::to_owned).ok_or_else(|| "no version from npm".to_owned())
        }
    }
}

/// Runs `install`'s update (blocking); its last line of output on failure.
#[cfg(not(any(target_os = "ios", target_arch = "wasm32")))]
pub(crate) fn run_update(install: &Install, binary: &str) -> Result<(), String> {
    let (program, args) = update_command(install, binary);
    let out = std::process::Command::new(&program)
        .args(&args)
        // Homebrew asks nothing and leaves its other updates for later.
        .env("HOMEBREW_NO_AUTO_UPDATE", "1")
        .env("HOMEBREW_NO_INSTALL_CLEANUP", "1")
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Err(text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("failed").trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_install_is_told_by_where_the_binary_lives() {
        assert_eq!(install_of("/Users/me/.local/share/claude/versions/2.1.321"), Some(Install::ClaudeNative));
        assert_eq!(install_of("/opt/homebrew/Caskroom/codex/0.163.0/bin/codex"), Some(Install::BrewCask("codex".into())));
        assert_eq!(install_of("/usr/local/Cellar/codex/0.1/bin/codex"), Some(Install::BrewFormula("codex".into())));
        assert_eq!(
            install_of("/Users/me/.nvm/versions/node/v22/lib/node_modules/@openai/codex/bin/codex.js"),
            Some(Install::Npm("@openai/codex".into()))
        );
        assert_eq!(install_of("/usr/local/bin/pi"), None);
    }

    #[test]
    fn versions_read_and_compare() {
        assert_eq!(version_of("2.1.321 (Claude Code)").as_deref(), Some("2.1.321"));
        assert_eq!(version_of("codex-cli 0.163.0").as_deref(), Some("0.163.0"));
        assert_eq!(version_of("v1.2.3").as_deref(), Some("1.2.3"));
        assert!(newer("0.163.0", "0.166.0"));
        assert!(newer("2.1.321", "2.1.322"));
        assert!(!newer("2.1.322", "2.1.322"));
        assert!(!newer("0.10.0", "0.9.9"));
    }

    #[test]
    fn each_install_updates_its_own_way() {
        assert_eq!(command_line(&Install::ClaudeNative), "claude update");
        assert_eq!(command_line(&Install::BrewCask("codex".into())), "brew upgrade --cask codex");
        assert_eq!(update_command(&Install::ClaudeNative, "/x/claude"), ("/x/claude".to_owned(), vec!["update".to_owned()]));
    }
}

/// This computer's claude and codex: how each was installed and its newest
/// version. Reads only; nothing is updated.
#[cfg(all(test, not(any(target_os = "ios", target_arch = "wasm32"))))]
#[test]
#[ignore]
fn latest_live() {
    for bin in ["claude", "codex"] {
        let Ok(out) = std::process::Command::new("/usr/bin/which").arg(bin).output() else { continue };
        let path = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if path.is_empty() {
            continue;
        }
        let real = std::fs::canonicalize(&path).unwrap().to_string_lossy().into_owned();
        let install = install_of(&real);
        eprintln!("{bin}: {real} -> {install:?}");
        if let Some(i) = install {
            eprintln!("  latest {:?}, by `{}`", latest(&i), command_line(&i));
        }
    }
}
