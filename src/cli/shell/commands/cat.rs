use crate::convert::convert_text;
use crate::domain::BodyFormat;
use crate::support::{ConfluenceCliError, Result};
use crate::NodeHandle;

use super::super::state::ShellState;
use super::super::CommandOutput;

pub fn execute(
    state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    let (format, target) = parse_args(argv)?;

    if input.is_some() && target.is_some() {
        return Err(ConfluenceCliError::Config(
            "cat does not accept both piped input and an explicit target".to_owned(),
        ));
    }

    if input.is_some() && target.is_none() {
        return Ok(CommandOutput::Text(input.expect("checked above")));
    }

    let lineage = state.resolve_target_lineage(target.as_deref())?;
    let handle = lineage.last().expect("lineage always has a node");
    if !matches!(handle, NodeHandle::Page(_)) {
        return Err(ConfluenceCliError::Config(
            "cat requires a page target or a current page".to_owned(),
        ));
    }

    let body = state.vfs().read(handle)?;
    let output = match format {
        BodyFormat::Storage => body,
        BodyFormat::Markdown => convert_text(&body, BodyFormat::Storage, BodyFormat::Markdown)?,
        BodyFormat::Text => convert_text(&body, BodyFormat::Storage, BodyFormat::Text)?,
        BodyFormat::Html => convert_text(&body, BodyFormat::Storage, BodyFormat::Html)?,
    };
    Ok(CommandOutput::Text(format!("{output}\n")))
}

fn parse_args(argv: &[String]) -> Result<(BodyFormat, Option<String>)> {
    let mut format = BodyFormat::Markdown;
    let mut target = None;
    let mut iter = argv.iter().skip(1);

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--raw" | "--storage" => format = BodyFormat::Storage,
            "--text" => format = BodyFormat::Text,
            "--markdown" => format = BodyFormat::Markdown,
            "--html" => format = BodyFormat::Html,
            "--format" => {
                let Some(value) = iter.next() else {
                    return Err(ConfluenceCliError::Config(
                        "usage: cat [--format <markdown|text|storage|html>] [target]".to_owned(),
                    ));
                };
                format = match value.as_str() {
                    "markdown" => BodyFormat::Markdown,
                    "text" => BodyFormat::Text,
                    "storage" => BodyFormat::Storage,
                    "html" => BodyFormat::Html,
                    _ => {
                        return Err(ConfluenceCliError::Config(
                            "cat format must be one of: markdown, text, storage, html".to_owned(),
                        ))
                    }
                };
            }
            other if target.is_none() => target = Some(other.to_owned()),
            _ => {
                return Err(ConfluenceCliError::Config(
                    "usage: cat [--format <markdown|text|storage|html>] [target]".to_owned(),
                ))
            }
        }
    }

    Ok((format, target))
}
