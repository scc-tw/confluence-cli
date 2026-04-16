mod cat;
mod find;
mod grep;

use crate::support::Result;

use super::state::ShellState;
use super::CommandOutput;

pub fn is_registered(name: &str) -> bool {
    matches!(name, "cat" | "grep" | "find")
}

pub fn help_for(name: &str) -> Option<&'static str> {
    match name {
        "cat" => Some(
            "cat [--raw|--text|--markdown|--html] [target]\n  Read page content. Without a target, reads the current page or piped input. Default output is markdown.",
        ),
        "grep" => Some(
            "grep <pattern> [target]\n  Search shell text input or recursively search page text under the target subtree.",
        ),
        "find" => Some(
            "find [target] [--name <pattern>]\n  Recursively walk spaces/pages under the target subtree.",
        ),
        _ => None,
    }
}

pub fn execute(
    state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    match argv.first().map(String::as_str) {
        Some("cat") => cat::execute(state, argv, input),
        Some("grep") => grep::execute(state, argv, input),
        Some("find") => find::execute(state, argv, input),
        _ => unreachable!("checked by registry before dispatch"),
    }
}

#[cfg(test)]
mod tests {
    use super::{help_for, is_registered};

    #[test]
    fn registry_knows_shell_native_commands() {
        assert!(is_registered("cat"));
        assert!(is_registered("grep"));
        assert!(is_registered("find"));
        assert!(!is_registered("page"));
    }

    #[test]
    fn help_entries_exist_for_registered_commands() {
        assert!(help_for("cat").unwrap().contains("markdown"));
        assert!(help_for("grep").unwrap().contains("pattern"));
        assert!(help_for("find").unwrap().contains("--name"));
    }
}
