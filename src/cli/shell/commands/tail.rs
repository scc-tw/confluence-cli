use crate::NodeHandle;
use crate::convert::convert_text;
use crate::domain::BodyFormat;
use crate::support::{ConfluenceCliError, Result};

use super::super::CommandOutput;
use super::super::state::ShellState;

pub fn execute(
    state: &ShellState,
    argv: &[String],
    input: Option<String>,
) -> Result<CommandOutput> {
    let (mode, target) = parse_args(argv)?;

    let text = if let Some(input) = input {
        if target.is_some() {
            return Err(ConfluenceCliError::Config(
                "tail does not accept both piped input and an explicit target".to_owned(),
            ));
        }
        input
    } else {
        let lineage = state.resolve_target_lineage(target.as_deref())?;
        let handle = lineage.last().expect("lineage always has a node");
        if !matches!(handle, NodeHandle::Page(_)) {
            return Err(ConfluenceCliError::Config(
                "tail requires a page target or a current page".to_owned(),
            ));
        }
        let body = state.vfs().read(handle)?;
        convert_text(&body, BodyFormat::Storage, BodyFormat::Text)?
    };

    let lines = text.lines().collect::<Vec<_>>();
    let selected = match mode {
        TailMode::Last(count) => {
            let keep = count.min(lines.len());
            lines[lines.len().saturating_sub(keep)..].to_vec()
        }
        TailMode::From(start) => {
            if start == 0 {
                return Err(ConfluenceCliError::Config(
                    "tail starting line must be >= 1".to_owned(),
                ));
            }
            lines.into_iter().skip(start - 1).collect::<Vec<_>>()
        }
    };

    Ok(CommandOutput::Text(if selected.is_empty() {
        String::new()
    } else {
        format!("{}\n", selected.join("\n"))
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TailMode {
    Last(usize),
    From(usize),
}

fn parse_args(argv: &[String]) -> Result<(TailMode, Option<String>)> {
    let mut mode = TailMode::Last(10);
    let mut target = None;
    let mut iter = argv.iter().skip(1);

    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "-n" => {
                let Some(value) = iter.next() else {
                    return Err(ConfluenceCliError::Config(
                        "usage: tail [-n <count>|-n +<start>] [target]".to_owned(),
                    ));
                };
                mode = parse_n(value)?;
            }
            other if target.is_none() => target = Some(other.to_owned()),
            _ => {
                return Err(ConfluenceCliError::Config(
                    "usage: tail [-n <count>|-n +<start>] [target]".to_owned(),
                ));
            }
        }
    }

    Ok((mode, target))
}

fn parse_n(raw: &str) -> Result<TailMode> {
    if let Some(rest) = raw.strip_prefix('+') {
        let start = rest
            .parse::<usize>()
            .map_err(|_| ConfluenceCliError::Config(format!("invalid tail line count: {raw}")))?;
        Ok(TailMode::From(start))
    } else {
        let count = raw
            .parse::<usize>()
            .map_err(|_| ConfluenceCliError::Config(format!("invalid tail line count: {raw}")))?;
        Ok(TailMode::Last(count))
    }
}

#[cfg(test)]
mod tests {
    use super::{TailMode, parse_n};

    #[test]
    fn parses_last_n_mode() {
        assert_eq!(parse_n("5").unwrap(), TailMode::Last(5));
    }

    #[test]
    fn parses_from_n_mode() {
        assert_eq!(parse_n("+3").unwrap(), TailMode::From(3));
    }
}
