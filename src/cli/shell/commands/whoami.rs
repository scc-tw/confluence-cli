use crate::support::{ConfluenceCliError, Result};

use super::super::state::ShellState;
use super::super::CommandOutput;

pub fn execute(
    state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    if input.is_some() {
        return Err(ConfluenceCliError::Config(
            "whoami does not accept piped input".to_owned(),
        ));
    }
    if argv.len() != 1 {
        return Err(ConfluenceCliError::Config("usage: whoami".to_owned()));
    }

    let identity = state
        .resolved_profile()
        .and_then(|profile| {
            profile
                .email
                .clone()
                .or(profile.username.clone())
                .or(profile.name.clone())
        })
        .or_else(|| state.global().profile.clone())
        .unwrap_or_else(|| "unknown".to_owned());
    Ok(CommandOutput::Text(format!("{identity}\n")))
}
