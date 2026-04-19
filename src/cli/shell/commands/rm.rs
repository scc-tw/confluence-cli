use crate::NodeHandle;
use crate::application::models::PageContentKind;
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
            "rm does not accept piped input".to_owned(),
        ));
    }
    if argv.len() != 2 {
        return Err(ConfluenceCliError::Config("usage: rm <target>".to_owned()));
    }

    state.ensure_writable()?;
    let lineage = state.resolve_target_lineage(Some(&argv[1]))?;
    let handle = lineage.last().expect("lineage always has a node");
    let NodeHandle::Page(page) = handle else {
        return Err(ConfluenceCliError::Config(
            "rm only supports pages; use rmdir for folders".to_owned(),
        ));
    };
    if matches!(page.content_kind, PageContentKind::Folder) {
        return Err(ConfluenceCliError::Config(
            "rm only supports pages; use rmdir for folders".to_owned(),
        ));
    }

    state.vfs().remove_node(handle)?;
    Ok(CommandOutput::Empty)
}
