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
            "cat [target]\n  Read a page as shell text. Without a target, reads the current page or piped input.",
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
