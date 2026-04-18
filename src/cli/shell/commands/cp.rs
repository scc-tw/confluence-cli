use crate::application::models::PageContentKind;
use crate::support::{ConfluenceCliError, Result};
use crate::{NodeHandle, NodeKind};

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

    let (recursive, source, destination) = parse_args(argv)?;
    state.ensure_writable()?;

    let source_lineage = state.resolve_target_lineage(Some(&source))?;
    let source_handle = source_lineage
        .last()
        .expect("source lineage always has a node");

    match source_handle {
        NodeHandle::Page(page) if matches!(page.content_kind, PageContentKind::Page) => {
            let (parent_lineage, new_name) =
                resolve_destination(state, source_handle, &destination)?;
            let parent = parent_lineage
                .last()
                .expect("destination lineage always has a parent");
            state
                .vfs()
                .copy_node(source_handle, parent, new_name.as_deref())?;
        }
        NodeHandle::Page(_) | NodeHandle::Space(_) => {
            if !recursive {
                return Err(ConfluenceCliError::Config(
                    "cp -r is required for folders and spaces".to_owned(),
                ));
            }
            let (parent_lineage, new_name) =
                resolve_destination(state, source_handle, &destination)?;
            let parent = parent_lineage
                .last()
                .expect("destination lineage always has a parent");
            copy_recursive(state, source_handle, parent, new_name.as_deref())?;
        }
        NodeHandle::Root => {
            return Err(ConfluenceCliError::Config(
                "cp does not support copying the root".to_owned(),
            ))
        }
    }

    Ok(CommandOutput::Empty)
}

fn copy_recursive(
    state: &ShellState,
    source: &NodeHandle,
    destination_parent: &NodeHandle,
    new_name: Option<&str>,
) -> Result<NodeHandle> {
    match source {
        NodeHandle::Space(space) => {
            let created = state.vfs().create_child(
                destination_parent,
                new_name.unwrap_or(&space.name),
                NodeKind::Folder,
            )?;
            for entry in state.vfs().read_dir(source)? {
                copy_recursive(state, &entry.handle, &created, None)?;
            }
            Ok(created)
        }
        NodeHandle::Page(page) if matches!(page.content_kind, PageContentKind::Folder) => {
            let created = state.vfs().create_child(
                destination_parent,
                new_name.unwrap_or(&page.title),
                NodeKind::Folder,
            )?;
            for entry in state.vfs().read_dir(source)? {
                copy_recursive(state, &entry.handle, &created, None)?;
            }
            Ok(created)
        }
        NodeHandle::Page(_) => state.vfs().copy_node(source, destination_parent, new_name),
        NodeHandle::Root => Err(ConfluenceCliError::Config(
            "cp does not support copying the root".to_owned(),
        )),
    }
}

fn parse_args(argv: &[String]) -> Result<(bool, String, String)> {
    let mut recursive = false;
    let mut positional = Vec::new();

    for arg in argv.iter().skip(1) {
        match arg.as_str() {
            "-r" | "-R" | "--recursive" => recursive = true,
            _ => positional.push(arg.clone()),
        }
    }

    match positional.as_slice() {
        [source, destination] => Ok((recursive, source.clone(), destination.clone())),
        _ => Err(ConfluenceCliError::Config(
            "usage: cp [-r|--recursive] <source> <destination>".to_owned(),
        )),
    }
}
