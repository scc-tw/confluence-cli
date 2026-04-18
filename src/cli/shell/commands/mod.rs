mod cat;
mod cp;
mod find;
mod grep;
mod mkdir;
mod mv;
mod rm;
mod rmdir;
mod seq;
mod sleep;
mod tail;
mod whoami;

use crate::support::Result;

use super::state::ShellState;
use super::CommandOutput;

pub fn is_registered(name: &str) -> bool {
    matches!(
        name,
        "cat"
            | "grep"
            | "find"
            | "mkdir"
            | "mv"
            | "cp"
            | "rm"
            | "rmdir"
            | "whoami"
            | "id"
            | "seq"
            | "sleep"
            | "tail"
    )
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
        "mkdir" => Some(
            "mkdir <target>\n  Create a folder under a space or page parent.",
        ),
        "mv" => Some(
            "mv <source> <destination>\n  Move or rename a page or folder.",
        ),
        "cp" => Some(
            "cp <source> <destination>\n  Copy a page to a new destination. Folder copy is not supported yet.",
        ),
        "rm" => Some(
            "rm <target>\n  Remove a page by moving it to the archive/trash path.",
        ),
        "rmdir" => Some(
            "rmdir <target>\n  Remove an empty folder.",
        ),
        "tail" => Some(
            "tail [-n <count>|-n +<start>] [target]\n  Print the last lines of piped input or the current/target page rendered as text.",
        ),
        "whoami" | "id" => Some(
            "whoami\n  Show the active shell identity derived from the resolved Confluence profile.",
        ),
        "seq" => Some(
            "seq <end> | seq <start> <end> | seq <start> <step> <end>\n  Print a numeric sequence.",
        ),
        "sleep" => Some(
            "sleep <duration>\n  Delay for a duration like 1s, 250ms, 2m, or 1h.",
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
        Some("mkdir") => mkdir::execute(state, argv, input),
        Some("mv") => mv::execute(state, argv, input),
        Some("cp") => cp::execute(state, argv, input),
        Some("rm") => rm::execute(state, argv, input),
        Some("rmdir") => rmdir::execute(state, argv, input),
        Some("tail") => tail::execute(state, argv, input),
        Some("whoami") | Some("id") => whoami::execute(state, argv, input),
        Some("seq") => seq::execute(state, argv, input),
        Some("sleep") => sleep::execute(state, argv, input),
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
        assert!(is_registered("mkdir"));
        assert!(is_registered("mv"));
        assert!(is_registered("cp"));
        assert!(is_registered("rm"));
        assert!(is_registered("rmdir"));
        assert!(is_registered("whoami"));
        assert!(is_registered("id"));
        assert!(is_registered("seq"));
        assert!(is_registered("sleep"));
        assert!(is_registered("tail"));
        assert!(!is_registered("page"));
    }

    #[test]
    fn help_entries_exist_for_registered_commands() {
        assert!(help_for("cat").unwrap().contains("markdown"));
        assert!(help_for("grep").unwrap().contains("pattern"));
        assert!(help_for("find").unwrap().contains("--name"));
        assert!(help_for("mkdir").unwrap().contains("folder"));
        assert!(help_for("mv").unwrap().contains("Move or rename"));
        assert!(help_for("cp").unwrap().contains("Copy a page"));
        assert!(help_for("rm").unwrap().contains("Remove a page"));
        assert!(help_for("rmdir").unwrap().contains("empty folder"));
        assert!(help_for("whoami").unwrap().contains("identity"));
        assert!(help_for("id").unwrap().contains("identity"));
        assert!(help_for("seq").unwrap().contains("sequence"));
        assert!(help_for("sleep").unwrap().contains("duration"));
        assert!(help_for("tail").unwrap().contains("last lines"));
    }
}
