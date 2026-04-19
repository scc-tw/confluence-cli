use crate::NodeKind;
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
            "mkdir does not accept piped input".to_owned(),
        ));
    }
    if argv.len() != 2 {
        return Err(ConfluenceCliError::Config(
            "usage: mkdir <target>".to_owned(),
        ));
    }

    state.ensure_writable()?;
    let (parent_lineage, leaf) = state.resolve_parent_for_create(&argv[1])?;
    let parent = parent_lineage
        .last()
        .expect("lineage always has a parent node");
    state.vfs().create_child(parent, &leaf, NodeKind::Folder)?;
    Ok(CommandOutput::Empty)
}
