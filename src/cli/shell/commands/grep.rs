use crate::NodeHandle;
use crate::convert::convert_text;
use crate::domain::BodyFormat;
use crate::support::{ConfluenceCliError, Result};

use super::super::CommandOutput;
use super::super::state::ShellState;

pub fn execute(
    state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    let (pattern, target) = parse_args(argv)?;

    if let Some(input) = input {
        if target.is_some() {
            return Err(ConfluenceCliError::Config(
                "grep does not accept both piped input and an explicit target".to_owned(),
            ));
        }
        let output = input
            .lines()
            .filter(|line| line.contains(&pattern))
            .map(str::to_owned)
            .collect::<Vec<_>>()
            .join("\n");
        return Ok(CommandOutput::Text(if output.is_empty() {
            output
        } else {
            format!("{output}\n")
        }));
    }

    let lineage = state.resolve_target_lineage(target.as_deref())?;
    let mut matches = Vec::new();
    grep_lineage(state, &lineage, &pattern, &mut matches)?;
    Ok(CommandOutput::Text(if matches.is_empty() {
        String::new()
    } else {
        format!("{}\n", matches.join("\n"))
    }))
}

fn parse_args(argv: &[String]) -> Result<(String, Option<String>)> {
    match argv {
        [cmd, pattern] if cmd == "grep" => Ok((pattern.clone(), None)),
        [cmd, pattern, target] if cmd == "grep" => Ok((pattern.clone(), Some(target.clone()))),
        _ => Err(ConfluenceCliError::Config(
            "usage: grep <pattern> [target]".to_owned(),
        )),
    }
}

fn grep_lineage(
    state: &ShellState,
    lineage: &[NodeHandle],
    pattern: &str,
    matches: &mut Vec<String>,
) -> Result<()> {
    let current = lineage.last().expect("lineage always has a node");
    if matches!(current, NodeHandle::Page(_)) {
        let body = state.vfs().read(current)?;
        let text = convert_text(&body, BodyFormat::Storage, BodyFormat::Text)?;
        let prefix = format_entry(lineage, &state.render_lineage(lineage));
        for (index, line) in text.lines().enumerate() {
            if line.contains(pattern) {
                matches.push(format!("{prefix}:{}:{}", index + 1, line));
            }
        }
    }

    if !matches!(
        current,
        NodeHandle::Page(_) | NodeHandle::Root | NodeHandle::Space(_)
    ) {
        return Ok(());
    }

    for entry in state.vfs().read_dir(current)? {
        let mut next = lineage.to_vec();
        next.push(entry.handle);
        grep_lineage(state, &next, pattern, matches)?;
    }
    Ok(())
}

fn format_entry(lineage: &[NodeHandle], display: &str) -> String {
    match lineage.last().expect("lineage always has a node") {
        NodeHandle::Root => "/".to_owned(),
        NodeHandle::Space(space) => format!("{display} [{}]", space.id),
        NodeHandle::Page(page) => format!("{display} [{}]", page.id),
    }
}
