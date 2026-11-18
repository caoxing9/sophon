//! The service's own environment, cleared of what is not the host's.
//!
//! Whatever starts the service hands it an environment, and everything
//! the service starts (agents, terminals, account queries) gets it in
//! turn. Started from inside an agent's session (a terminal an agent
//! runs, an app launched by one), that environment carries the session's
//! own routing: its endpoint and token, its sandbox flags, its ids. An
//! agent started with those would answer to that session's account, not
//! the one the host's `claude` or `codex` is signed in to.
//!
//! So an agent runs on what its CLI is signed in to and configured with
//! in its own files, and another endpoint is a provider's `env` in the
//! service's config, named there on purpose.

use std::env;

/// Whether `name` routes, credentials or marks an agent session: all of
/// `ANTHROPIC_*`, `CLAUDE_CODE_*`, `CLAUDE_AGENT_*` and `CODEX_*`, and
/// the session's own `CLAUDECODE`, `CLAUDE_EFFORT` and `CLAUDE_PID`.
/// `CLAUDE_CONFIG_DIR` and `CODEX_HOME` stay: they say where the host
/// keeps a CLI's files, which is the host's to say.
pub fn foreign(name: &str) -> bool {
    if matches!(name, "CODEX_HOME" | "CLAUDE_CONFIG_DIR") {
        return false;
    }
    matches!(name, "CLAUDECODE" | "CLAUDE_EFFORT" | "CLAUDE_PID")
        || ["ANTHROPIC_", "CLAUDE_CODE_", "CLAUDE_AGENT_", "CODEX_"]
            .iter()
            .any(|prefix| name.starts_with(prefix))
}

/// Removes them from this process, so nothing started from it has them.
/// Call before the runtime starts: the environment is not safe to change
/// once another thread may read it.
pub fn scrub() {
    let names: Vec<_> = env::vars_os()
        .map(|(name, _)| name)
        .filter(|name| name.to_str().is_some_and(foreign))
        .collect();
    for name in names {
        // SAFETY: called from `main` before the runtime, so no other
        // thread exists to read the environment while it changes.
        unsafe { env::remove_var(name) }
    }
}

#[cfg(test)]
mod tests {
    use super::foreign;

    #[test]
    fn a_session_s_routing_goes_and_the_host_s_locations_stay() {
        for name in [
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_AUTH_TOKEN",
            "ANTHROPIC_MODEL",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CODE_CHILD_SESSION",
            "CLAUDECODE",
            "CLAUDE_EFFORT",
            "CODEX_SANDBOX_NETWORK_DISABLED",
            "CODEX_THREAD_ID",
        ] {
            assert!(foreign(name), "{name}");
        }
        for name in [
            "CLAUDE_CONFIG_DIR",
            "CODEX_HOME",
            "OPENAI_API_KEY",
            "PATH",
            "HOME",
        ] {
            assert!(!foreign(name), "{name}");
        }
    }
}
