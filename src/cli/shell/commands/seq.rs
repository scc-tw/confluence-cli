use crate::support::{ConfluenceCliError, Result};

use super::super::state::ShellState;
use super::super::CommandOutput;

pub fn execute(
    _state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    if input.is_some() {
        return Err(ConfluenceCliError::Config(
            "seq does not accept piped input".to_owned(),
        ));
    }

    let (start, step, end) = match argv {
        [_cmd, end] => (1.0, 1.0, parse_number(end)?),
        [_cmd, start, end] => (parse_number(start)?, 1.0, parse_number(end)?),
        [_cmd, start, step, end] => (
            parse_number(start)?,
            parse_number(step)?,
            parse_number(end)?,
        ),
        _ => {
            return Err(ConfluenceCliError::Config(
                "usage: seq <end> | seq <start> <end> | seq <start> <step> <end>".to_owned(),
            ))
        }
    };

    if step == 0.0 {
        return Err(ConfluenceCliError::Config(
            "seq step must not be zero".to_owned(),
        ));
    }

    let mut values = Vec::new();
    let mut current = start;
    if step > 0.0 {
        while current <= end {
            values.push(format_number(current));
            current += step;
        }
    } else {
        while current >= end {
            values.push(format_number(current));
            current += step;
        }
    }

    Ok(CommandOutput::Text(format!("{}\n", values.join("\n"))))
}

fn parse_number(raw: &str) -> Result<f64> {
    raw.parse::<f64>()
        .map_err(|_| ConfluenceCliError::Config(format!("invalid number: {raw}")))
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        value.to_string()
    }
}
