use crate::NodeHandle;
use crate::support::{ConfluenceCliError, Result};

use super::super::CommandOutput;
use super::super::state::ShellState;

pub fn execute(
    state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    if input.is_some() {
        return Err(ConfluenceCliError::Config(
            "find does not accept piped input".to_owned(),
        ));
    }

    let (target, name_filter) = parse_args(argv)?;
    let lineage = state.resolve_target_lineage(target.as_deref())?;
    let mut lines = Vec::new();
    walk(state, &lineage, name_filter.as_deref(), &mut lines)?;
    Ok(CommandOutput::Text(if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    }))
}

fn parse_args(argv: &[String]) -> Result<(Option<String>, Option<String>)> {
    let mut target = None;
    let mut name_filter = None;
    let mut iter = argv.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == "--name" {
            let Some(pattern) = iter.next() else {
                return Err(ConfluenceCliError::Config(
                    "usage: find [target] [--name <pattern>]".to_owned(),
                ));
            };
            name_filter = Some(pattern.clone());
        } else if target.is_none() {
            target = Some(arg.clone());
        } else {
            return Err(ConfluenceCliError::Config(
                "usage: find [target] [--name <pattern>]".to_owned(),
            ));
        }
    }
    Ok((target, name_filter))
}

fn walk(
    state: &ShellState,
    lineage: &[NodeHandle],
    name_filter: Option<&str>,
    lines: &mut Vec<String>,
) -> Result<()> {
    let current = lineage.last().expect("lineage always has a node");
    let display = state.render_lineage(lineage);
    let name = match current {
        NodeHandle::Root => "/",
        NodeHandle::Space(space) => &space.key,
        NodeHandle::Page(page) => &page.title,
    };
    if name_filter
        .map(|pattern| wildcard_match(pattern, name))
        .unwrap_or(true)
    {
        lines.push(format_entry(lineage, &display));
    }

    for entry in state.vfs().read_dir(current)? {
        let mut next = lineage.to_vec();
        next.push(entry.handle);
        walk(state, &next, name_filter, lines)?;
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

fn wildcard_match(pattern: &str, text: &str) -> bool {
    wildcard_match_inner(pattern.as_bytes(), text.as_bytes())
}

fn wildcard_match_inner(pattern: &[u8], text: &[u8]) -> bool {
    match (pattern.first(), text.first()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(b'*'), _) => {
            wildcard_match_inner(&pattern[1..], text)
                || (!text.is_empty() && wildcard_match_inner(pattern, &text[1..]))
        }
        (Some(b'?'), Some(_)) => wildcard_match_inner(&pattern[1..], &text[1..]),
        (Some(ch), Some(text_ch)) if ch.eq_ignore_ascii_case(text_ch) => {
            wildcard_match_inner(&pattern[1..], &text[1..])
        }
        _ => false,
    }
}
