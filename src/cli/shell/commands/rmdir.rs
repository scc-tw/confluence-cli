use crate::application::models::PageContentKind;
use crate::support::{ConfluenceCliError, Result};
use crate::NodeHandle;

use super::super::state::ShellState;
use super::super::CommandOutput;

pub fn execute(
    state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    if input.is_some() {
        return Err(ConfluenceCliError::Config(
            "rmdir does not accept piped input".to_owned(),
        ));
    }
    if argv.len() != 2 {
        return Err(ConfluenceCliError::Config(
            "usage: rmdir <target>".to_owned(),
        ));
    }

    state.ensure_writable()?;
    let lineage = state.resolve_target_lineage(Some(&argv[1]))?;
    let handle = lineage.last().expect("lineage always has a node");
    let NodeHandle::Page(page) = handle else {
        return Err(ConfluenceCliError::Config(
            "rmdir only supports folders".to_owned(),
        ));
    };
    if !matches!(page.content_kind, PageContentKind::Folder) {
        return Err(ConfluenceCliError::Config(
            "rmdir only supports folders".to_owned(),
        ));
    }
    let children = state.vfs().read_dir(handle)?;
    if !children.is_empty() {
        return Err(ConfluenceCliError::Config(
            "rmdir only removes empty folders".to_owned(),
        ));
    }

    state.vfs().remove_node(handle)?;
    Ok(CommandOutput::Empty)
}
