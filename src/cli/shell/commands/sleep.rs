use std::thread;
use std::time::Duration;

use crate::support::{ConfluenceCliError, Result};

use super::super::CommandOutput;
use super::super::state::ShellState;

pub fn execute(
    _state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    if input.is_some() {
        return Err(ConfluenceCliError::Config(
            "sleep does not accept piped input".to_owned(),
        ));
    }
    if argv.len() != 2 {
        return Err(ConfluenceCliError::Config(
            "usage: sleep <duration>".to_owned(),
        ));
    }

    thread::sleep(parse_duration(&argv[1])?);
    Ok(CommandOutput::Empty)
}

fn parse_duration(raw: &str) -> Result<Duration> {
    let (number, unit) = if let Some(value) = raw.strip_suffix("ms") {
        (value, "ms")
    } else if let Some(value) = raw.strip_suffix('s') {
        (value, "s")
    } else if let Some(value) = raw.strip_suffix('m') {
        (value, "m")
    } else if let Some(value) = raw.strip_suffix('h') {
        (value, "h")
    } else {
        (raw, "s")
    };

    let value = number
        .parse::<u64>()
        .map_err(|_| ConfluenceCliError::Config(format!("invalid duration: {raw}")))?;

    Ok(match unit {
        "ms" => Duration::from_millis(value),
        "s" => Duration::from_secs(value),
        "m" => Duration::from_secs(value * 60),
        "h" => Duration::from_secs(value * 3600),
        _ => unreachable!("duration unit already normalized"),
    })
}
