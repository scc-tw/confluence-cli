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
    if argv.len() > 2 {
        return Err(ConfluenceCliError::Config("usage: cat [target]".to_owned()));
    }

    if input.is_some() && argv.len() == 1 {
        return Ok(CommandOutput::Text(input.expect("checked above")));
    }

    let lineage = state.resolve_target_lineage(argv.get(1).map(String::as_str))?;
    let handle = lineage.last().expect("lineage always has a node");
    if !matches!(handle, NodeHandle::Page(_)) {
        return Err(ConfluenceCliError::Config(
            "cat requires a page target or a current page".to_owned(),
        ));
    }

    let body = state.vfs().read(handle)?;
    let text = convert_text(&body, BodyFormat::Storage, BodyFormat::Text)?;
    Ok(CommandOutput::Text(format!("{text}\n")))
}
