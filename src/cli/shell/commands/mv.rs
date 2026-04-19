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
            "mv does not accept piped input".to_owned(),
        ));
    }
    if argv.len() != 3 {
        return Err(ConfluenceCliError::Config(
            "usage: mv <source> <destination>".to_owned(),
        ));
    }

    state.ensure_writable()?;
    let source_lineage = state.resolve_target_lineage(Some(&argv[1]))?;
    let source = source_lineage
        .last()
        .expect("source lineage always has a node");
    let (parent_lineage, new_name) = resolve_destination(state, source, &argv[2])?;
    let parent = parent_lineage
        .last()
        .expect("destination lineage always has a parent");
    state.vfs().move_node(source, parent, new_name.as_deref())?;
    Ok(CommandOutput::Empty)
}

pub(super) fn resolve_destination(
    state: &ShellState,
    source: &crate::NodeHandle,
    destination: &str,
) -> Result<(Vec<crate::NodeHandle>, Option<String>)> {
    match state.resolve_target_lineage(Some(destination)) {
        Ok(lineage) => Ok((lineage, source_name(source))),
        Err(error) => {
            if !error.to_string().contains("not found") {
                return Err(error);
            }
            let (parent_lineage, leaf) = state.resolve_parent_for_create(destination)?;
            Ok((parent_lineage, Some(leaf)))
        }
    }
}

fn source_name(source: &crate::NodeHandle) -> Option<String> {
    match source {
        crate::NodeHandle::Root => None,
        crate::NodeHandle::Space(space) => Some(space.name.clone()),
        crate::NodeHandle::Page(page) => Some(page.title.clone()),
    }
}
