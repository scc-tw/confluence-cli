use crate::application::vfs::DirEntry;
use crate::support::{ConfluenceCliError, Result};
use crate::NodeHandle;

use super::commands;
use super::state::ShellState;
use super::{CommandOutput, ShellControl, SHELL_HELP};

pub fn resolve(name: &str) -> Option<Builtin> {
    match name {
        "help" => Some(Builtin::Help),
        "pwd" => Some(Builtin::Pwd),
        "ls" => Some(Builtin::Ls),
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
            require_range("ls", argv, 1, 2)?;
            let handle = state.resolve_listing_target(argv.get(1).map(String::as_str))?;
            Ok((
                ShellControl::Continue,
                CommandOutput::Text(render_listing(state, &handle)?),
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

fn render_listing(state: &ShellState, handle: &NodeHandle) -> Result<String> {
    let entries = state.vfs().read_dir(handle)?;
    if entries.is_empty() {
        let empty = match handle {
            NodeHandle::Root => "No spaces found.",
            NodeHandle::Space(_) | NodeHandle::Page(_) => "No child pages found.",
        };
        return Ok(format!("{empty}\n"));
    }

    let mut out = String::new();
    for entry in entries {
        out.push_str(&render_entry(&entry));
        out.push('\n');
    }
    Ok(out)
}

fn render_entry(entry: &DirEntry) -> String {
    match &entry.handle {
        NodeHandle::Space(space) => format!("- {}/  {} [{}]", space.key, space.name, space.id),
        NodeHandle::Page(page) => {
            let suffix = if entry.stat.has_children == Some(true) {
                "/"
            } else {
                ""
            };
            format!("- {}{} [{}]", page.title, suffix, page.id)
        }
        NodeHandle::Root => "- /".to_owned(),
    }
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
        "cd" => "cd <space|page|..|/>".to_owned(),
        "help" => "help [topic]".to_owned(),
        "exit" => "exit".to_owned(),
        _ => command.to_owned(),
    }
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
