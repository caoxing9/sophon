//! Runs the Rust application directly on the machine's native backend.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

#[cfg(target_os = "linux")]
use nsh_platform_linux as platform;
#[cfg(target_os = "macos")]
use nsh_platform_macos as platform;
#[cfg(target_os = "windows")]
use nsh_platform_windows as platform;

#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn main() -> impl std::process::Termination {
    leave_the_session_that_started_this();
    sophon::applog::init();
    platform::run_app(
        platform::AppSource::Rust(platform::Root::of::<sophon::App>()),
        platform::AppOptions {
            title: "Sophon".to_owned(),
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            app_id: "dev.sophon.app".to_owned(),
            size: (1100, 760),
            ..Default::default()
        },
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
compile_error!(
    "Run this binary on macOS, Windows or Linux; use the TSX entry with nsh run for iOS and Android."
);

/// Clears from this process what an agent's session gives the commands
/// it runs: its endpoint and token, its sandbox flags, its ids. An app
/// started from inside one (a terminal an agent runs) would otherwise
/// hand them to the host service and to each `claude` and `codex` it
/// asks something of, as if they were the host's own. The service does
/// the same for itself (`sophon-service`'s `env`); where a CLI keeps
/// its files (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`) is the host's to say
/// and stays.
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
fn leave_the_session_that_started_this() {
    let foreign = |name: &str| {
        !matches!(name, "CODEX_HOME" | "CLAUDE_CONFIG_DIR")
            && (matches!(name, "CLAUDECODE" | "CLAUDE_EFFORT" | "CLAUDE_PID")
                || ["ANTHROPIC_", "CLAUDE_CODE_", "CLAUDE_AGENT_", "CODEX_"].iter().any(|prefix| name.starts_with(prefix)))
    };
    let names: Vec<_> =
        std::env::vars_os().map(|(name, _)| name).filter(|name| name.to_str().is_some_and(foreign)).collect();
    for name in names {
        // SAFETY: the first thing `main` does, before any other thread
        // exists to read the environment.
        unsafe { std::env::remove_var(name) }
    }
}
