use crate::OutputFormat;
use crate::domain::PageRef;
use crate::support::Result;

use super::super::try_run_from;
use super::state::ShellState;
use super::{CommandOutput, ShellControl};

pub fn build_forwarded_argv(state: &ShellState, tokens: Vec<String>) -> Result<Vec<String>> {
    let mut forwarded = tokens;
    apply_context(state, &mut forwarded)?;

    let mut argv = vec!["confluence".to_owned()];
    if let Some(config_path) = &state.global().config_path {
        argv.push("--config-path".to_owned());
        argv.push(config_path.display().to_string());
    }
    if let Some(profile) = &state.global().profile {
        argv.push("--profile".to_owned());
        argv.push(profile.clone());
    }
    if !matches!(state.global().output, OutputFormat::Human) {
        argv.push("--output".to_owned());
        argv.push("json".to_owned());
    }
    argv.extend(forwarded);
    Ok(argv)
}

pub fn execute_single(
    state: &ShellState,
    tokens: Vec<String>,
) -> Result<(ShellControl, CommandOutput)> {
    let argv = build_forwarded_argv(state, tokens)?;
    if let Err(error) = try_run_from(argv) {
        return Ok((
            ShellControl::Continue,
            CommandOutput::Text(error.to_string()),
        ));
    }
    Ok((ShellControl::Continue, CommandOutput::Empty))
}

fn apply_context(state: &ShellState, tokens: &mut Vec<String>) -> Result<()> {
    let Some(group) = tokens.first().cloned() else {
        return Ok(());
    };
    let command = tokens.get(1).cloned();
    let current_page = state.current_page_ref();
    let current_space = state.current().as_space().cloned();

    match (group.as_str(), command.as_deref()) {
        ("page", Some(command)) => {
            if matches!(
                command,
                "read"
                    | "info"
                    | "children"
                    | "create-child"
                    | "update"
                    | "patch"
                    | "move"
                    | "archive"
                    | "delete"
                    | "export"
            ) {
                inject_page_ref(tokens, false, current_page.clone())?;
            }

            if command == "create"
                && !tokens
                    .iter()
                    .any(|token| token == "--space-key" || token == "--space-id")
            {
                if let Some(space) = &current_space {
                    tokens.push("--space-id".to_owned());
                    tokens.push(space.id.clone());
                }
            }
        }
        ("attachment", Some(command)) => {
            inject_page_ref(tokens, command == "delete", current_page.clone())?;
        }
        ("property", Some(command)) => {
            inject_page_ref(
                tokens,
                matches!(command, "get" | "set" | "delete"),
                current_page.clone(),
            )?;
        }
        ("comment", Some(command)) => {
            if matches!(command, "list" | "create") {
                inject_page_ref(tokens, false, current_page.clone())?;
            } else if command == "reply" {
                inject_page_ref(tokens, true, current_page.clone())?;
            }
        }
        _ => {}
    }

    Ok(())
}

fn inject_page_ref(
    tokens: &mut Vec<String>,
    allow_second_positional: bool,
    current_page: Option<PageRef>,
) -> Result<()> {
    let Some(current_page) = current_page else {
        return Ok(());
    };

    let current_page = match current_page {
        PageRef::Id(page_id) => page_id.get().to_string(),
        PageRef::Url(url) => url,
    };

    let should_insert = match tokens.get(2) {
        None => true,
        Some(token) if token.starts_with('-') => true,
        Some(token) if allow_second_positional => PageRef::parse(token).is_err(),
        Some(_) => false,
    };

    if should_insert {
        tokens.insert(2, current_page);
    }
    Ok(())
}
