use crate::support::{ConfluenceCliError, Result};
use crate::NodeHandle;
use terminal_size::{terminal_size, Width};

use super::commands;
use super::format::{render_file, render_listing, ListingStyle};
use super::state::ShellState;
use super::{CommandOutput, ShellControl, SHELL_HELP};

pub fn resolve(name: &str) -> Option<Builtin> {
    match name {
        "help" => Some(Builtin::Help),
        "pwd" => Some(Builtin::Pwd),
        "ls" => Some(Builtin::Ls),
        "file" => Some(Builtin::File),
        "clear" | "cls" => Some(Builtin::Clear),
        "cd" => Some(Builtin::Cd),
        "use" => Some(Builtin::UseProfile),
        "exit" | "quit" => Some(Builtin::Exit),
        "context" => Some(Builtin::CompatPwd),
        "back" => Some(Builtin::CompatBack),
        "unset" => Some(Builtin::CompatUnset),
        _ => None,
    }
}

pub enum Builtin {
    Help,
    Pwd,
    Ls,
    File,
    Clear,
    Cd,
    UseProfile,
    Exit,
    CompatPwd,
    CompatBack,
    CompatUnset,
}

pub fn execute(
    state: &mut ShellState,
    builtin: Builtin,
    argv: &[String],
    in_pipeline: bool,
) -> Result<(ShellControl, CommandOutput)> {
    match builtin {
        Builtin::Help => {
            require_range("help", argv, 1, usize::MAX)?;
            let topic = argv[1..].to_vec();
            Ok((
                ShellControl::Continue,
                CommandOutput::Text(render_help(topic)),
            ))
        }
        Builtin::Pwd => {
            require_arity("pwd", argv, 1)?;
            Ok((
                ShellControl::Continue,
                CommandOutput::Text(format!("{}\n", state.cwd_display())),
            ))
        }
        Builtin::Ls => {
            let (style, target) = parse_ls_args(argv)?;
            let handle = state.resolve_listing_target(target.as_deref())?;
            Ok((
                ShellControl::Continue,
                CommandOutput::Text(render_listing_output(state, &handle, style)?),
            ))
        }
        Builtin::File => {
            require_range("file", argv, 1, 2)?;
            let lineage = state.resolve_target_lineage(argv.get(1).map(String::as_str))?;
            let handle = lineage.last().expect("lineage always has a node");
            let stat = state.vfs().stat(handle)?;
            Ok((
                ShellControl::Continue,
                CommandOutput::Text(render_file(&state.render_lineage(&lineage), handle, &stat)),
            ))
        }
        Builtin::Clear => {
            reject_in_pipeline("clear", in_pipeline)?;
            require_arity("clear", argv, 1)?;
            Ok((
                ShellControl::Continue,
                CommandOutput::Text("\u{1b}[2J\u{1b}[H".to_owned()),
            ))
        }
        Builtin::Cd => {
            reject_in_pipeline("cd", in_pipeline)?;
            require_arity("cd", argv, 2)?;
            state.change_directory(&argv[1])?;
            Ok((ShellControl::Continue, CommandOutput::Empty))
        }
        Builtin::UseProfile => {
            reject_in_pipeline("use profile", in_pipeline)?;
            match argv {
                [cmd, kind, profile] if cmd == "use" && kind == "profile" => {
                    state.use_profile(profile.clone())?;
                    Ok((ShellControl::Continue, CommandOutput::Empty))
                }
                [cmd, ..] if cmd == "use" => Err(ConfluenceCliError::Config(
                    "filesystem shell navigation uses `cd`; only `use profile <name>` remains supported"
                        .to_owned(),
                )),
                _ => Err(ConfluenceCliError::Config("usage: use profile <name>".to_owned())),
            }
        }
        Builtin::Exit => {
            reject_in_pipeline("exit", in_pipeline)?;
            require_arity("exit", argv, 1)?;
            Ok((ShellControl::Exit, CommandOutput::Empty))
        }
        Builtin::CompatPwd => Err(ConfluenceCliError::Config(
            "`context` was replaced by `pwd`".to_owned(),
        )),
        Builtin::CompatBack => Err(ConfluenceCliError::Config(
            "`back` was replaced by `cd ..`".to_owned(),
        )),
        Builtin::CompatUnset => Err(ConfluenceCliError::Config(
            "filesystem shell navigation uses `cd` and `pwd`; `unset` is no longer supported"
                .to_owned(),
        )),
    }
}

fn render_help(topic: Vec<String>) -> String {
    if topic.is_empty() || matches!(topic.first().map(String::as_str), Some("shell")) {
        return format!("{SHELL_HELP}\n");
    }
    if topic.len() == 1 {
        if let Some(help) = commands::help_for(&topic[0]) {
            return format!("{help}\n");
        }
    }
    format!(
        "help for shell-native command topics is not implemented yet; use `confluence {} --help`\n",
        topic.join(" ")
    )
}

fn render_listing_output(
    state: &ShellState,
    handle: &NodeHandle,
    style: ListingStyle,
) -> Result<String> {
    let entries = state.vfs().read_dir(handle)?;
    if entries.is_empty() {
        let empty = match handle {
            NodeHandle::Root => "No spaces found.",
            NodeHandle::Space(_) | NodeHandle::Page(_) => "No child pages found.",
        };
        return Ok(format!("{empty}\n"));
    }
    Ok(render_listing(&entries, style, terminal_width()))
}

fn require_arity(command: &str, argv: &[String], expected: usize) -> Result<()> {
    if argv.len() == expected {
        Ok(())
    } else {
        Err(ConfluenceCliError::Config(format!(
            "usage: {}",
            usage_for(command)
        )))
    }
}

fn require_range(command: &str, argv: &[String], min: usize, max: usize) -> Result<()> {
    if argv.len() >= min && argv.len() <= max {
        Ok(())
    } else {
        Err(ConfluenceCliError::Config(format!(
            "usage: {}",
            usage_for(command)
        )))
    }
}

fn usage_for(command: &str) -> String {
    match command {
        "pwd" => "pwd".to_owned(),
        "ls" => "ls [target]".to_owned(),
        "file" => "file [target]".to_owned(),
        "clear" => "clear".to_owned(),
        "cd" => "cd <space|page|..|/>".to_owned(),
        "help" => "help [topic]".to_owned(),
        "exit" => "exit".to_owned(),
        _ => command.to_owned(),
    }
}

fn terminal_width() -> Option<usize> {
    terminal_size().map(|(Width(width), _)| width as usize)
}

fn parse_ls_args(argv: &[String]) -> Result<(ListingStyle, Option<String>)> {
    let mut style = ListingStyle::Simple;
    let mut target = None;
    for arg in argv.iter().skip(1) {
        match arg.as_str() {
            "-l" | "--long" => style = ListingStyle::Long,
            other if target.is_none() => target = Some(other.to_owned()),
            _ => {
                return Err(ConfluenceCliError::Config(
                    "usage: ls [-l|--long] [target]".to_owned(),
                ))
            }
        }
    }
    Ok((style, target))
}

fn reject_in_pipeline(command: &str, in_pipeline: bool) -> Result<()> {
    if in_pipeline {
        Err(ConfluenceCliError::Config(format!(
            "`{command}` cannot be used in a pipeline"
        )))
    } else {
        Ok(())
    }
}
