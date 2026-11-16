//! Claude Code's own slash commands in Sophon. An agent runs headless
//! (the Agent SDK, through the service): the commands that print something
//! (`/compact`, `/context`, `/cost`, `/review`, the user's own commands
//! and skills) work as they are, sent to it; the ones that open Claude
//! Code's interactive screens (`/model`, `/mcp`, `/permissions`…) did
//! nothing. Those are Sophon's to do ([`native`]): its own screens where it
//! has them, else a terminal on the computer running `claude` with the
//! command; and the few with no place here leave the suggestions
//! ([`hidden`]).

/// What Sophon does for a command, in place of sending it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Native {
    /// The model picker.
    Model,
    /// The agent's mode (permissions) and settings.
    Permissions,
    /// A new conversation.
    Clear,
    /// The history of conversations.
    Resume,
    /// The usage page.
    Usage,
    /// The computer, the service and the agent, said.
    Status,
    /// Sophon's settings.
    Config,
    /// The project's `CLAUDE.md`.
    Memory,
    /// A terminal on the computer running `claude /<command>`.
    Terminal(&'static str),
}

/// The commands that open Claude Code's own screens, done in a terminal.
const IN_TERMINAL: [&str; 9] = ["mcp", "agents", "hooks", "plugin", "plugins", "login", "logout", "doctor", "install-github-app"];

/// The commands with no place in Sophon (its own terminal, editor and
/// window take care of them).
const HIDDEN: [&str; 9] = ["vim", "terminal-setup", "ide", "exit", "quit", "theme", "statusline", "output-style", "upgrade"];

/// The command a message is, alone: `/name` and nothing else but spaces
/// (`None` for a message that starts with one and says more).
fn command_of(text: &str) -> Option<&str> {
    let t = text.trim();
    let name = t.strip_prefix('/')?;
    (!name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b':')).then_some(name)
}

/// What Sophon does for message `text` when it is one of the commands it
/// takes; `None`: it goes to the agent as it is.
pub(crate) fn native(text: &str) -> Option<Native> {
    let name = command_of(text)?.to_ascii_lowercase();
    Some(match name.as_str() {
        "model" => Native::Model,
        "permissions" | "allowed-tools" => Native::Permissions,
        "clear" | "new" | "reset" => Native::Clear,
        "resume" | "continue" => Native::Resume,
        "usage" => Native::Usage,
        "status" => Native::Status,
        "config" | "settings" => Native::Config,
        "memory" => Native::Memory,
        other => Native::Terminal(IN_TERMINAL.iter().copied().find(|c| *c == other)?),
    })
}

/// Whether command `name` (without its slash) leaves the suggestions.
pub(crate) fn hidden(name: &str) -> bool {
    HIDDEN.contains(&name.to_ascii_lowercase().as_str())
}

/// What the suggestion of command `name` says it does here, when Sophon
/// does it: in Sophon, or in a terminal.
pub(crate) fn hint(name: &str) -> Option<&'static str> {
    match native(&format!("/{name}"))? {
        Native::Terminal(_) => Some(crate::i18n::t("Opens a terminal running claude")),
        _ => Some(crate::i18n::t("Opens in Sophon")),
    }
}

/// The line typed into the terminal for `command`.
pub(crate) fn terminal_line(command: &str) -> String {
    format!("claude /{command}\r")
}

/// The command a Terminal window runs for `command`, in the project
/// `dir` (whose own MCP servers and settings `claude` reads there).
pub(crate) fn terminal_command(dir: &str, command: &str) -> String {
    format!("cd '{}' && claude /{command}", dir.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interactive_commands_are_sophon_s_the_rest_go_to_the_agent() {
        assert_eq!(native("/model"), Some(Native::Model));
        assert_eq!(native("  /MCP  "), Some(Native::Terminal("mcp")));
        assert_eq!(native("/clear"), Some(Native::Clear));
        assert_eq!(native("/permissions"), Some(Native::Permissions));
        assert_eq!(native("/memory"), Some(Native::Memory));
        // Printing commands, the user's own and skills: the agent's.
        for passes in ["/compact", "/context", "/cost", "/review", "/init", "/my-command", "/frontend-design:frontend-design"] {
            assert_eq!(native(passes), None, "{passes}");
        }
        // A message that only starts with a command says more: the agent's.
        assert_eq!(native("/model is what I want to ask about"), None);
        assert_eq!(native("model"), None);
        assert_eq!(native("/"), None);
    }

    #[test]
    fn some_commands_leave_the_suggestions_and_others_say_where_they_open() {
        assert!(hidden("vim") && hidden("terminal-setup") && !hidden("compact"));
        assert!(hint("compact").is_none());
        assert!(hint("mcp").is_some() && hint("model").is_some());
        assert_eq!(terminal_line("mcp"), "claude /mcp\r");
        assert_eq!(terminal_command("/Users/me/it's", "mcp"), "cd '/Users/me/it'\\''s' && claude /mcp");
    }
}
