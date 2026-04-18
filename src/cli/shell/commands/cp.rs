use crate::application::models::PageContentKind;
use crate::support::{ConfluenceCliError, Result};
use crate::NodeHandle;

use super::super::state::ShellState;
use super::super::CommandOutput;
use super::mv::resolve_destination;

pub fn execute(
    state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    if input.is_some() {
        return Err(ConfluenceCliError::Config(
            "cp does not accept piped input".to_owned(),
        ));
    }
    if argv.len() != 3 {
        return Err(ConfluenceCliError::Config(
            "usage: cp <source> <destination>".to_owned(),
        ));
    }

    state.ensure_writable()?;
    let source_lineage = state.resolve_target_lineage(Some(&argv[1]))?;
    let source = source_lineage
        .last()
        .expect("source lineage always has a node");
    let NodeHandle::Page(page) = source else {
        return Err(ConfluenceCliError::Config(
            "cp only supports pages for now".to_owned(),
        ));
    };
    if matches!(page.content_kind, PageContentKind::Folder) {
        return Err(ConfluenceCliError::Config(
            "cp only supports pages for now".to_owned(),
        ));
    }

    let (parent_lineage, new_name) = resolve_destination(state, source, &argv[2])?;
    let parent = parent_lineage
        .last()
        .expect("destination lineage always has a parent");
    state.vfs().copy_node(source, parent, new_name.as_deref())?;
    Ok(CommandOutput::Empty)
}
