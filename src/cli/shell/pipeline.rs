use crate::support::{ConfluenceCliError, Result};

use super::builtins;
use super::commands;
use super::forward;
use super::parser::{ShellLine, SimpleCommand};
use super::state::ShellState;
use super::{CommandOutput, ShellControl};

const MAX_PIPELINE_STAGES: usize = 16;

pub struct ExecutionOutcome {
    pub control: ShellControl,
    pub output: Option<String>,
}

pub fn execute(state: &mut ShellState, line: ShellLine) -> Result<ExecutionOutcome> {
    let stages = line.pipeline.commands;
    if stages.len() > MAX_PIPELINE_STAGES {
        return Err(ConfluenceCliError::Config(format!(
            "pipeline depth exceeds maximum of {MAX_PIPELINE_STAGES} stages"
        )));
    }
    execute_recursive(state, &stages, 0, None)
}

fn execute_recursive(
    state: &mut ShellState,
    stages: &[SimpleCommand],
    index: usize,
    input: Option<String>,
) -> Result<ExecutionOutcome> {
    let stage = stages
        .get(index)
        .expect("pipeline recursion only visits valid stages");
    let in_pipeline = stages.len() > 1;

    let (control, output) = execute_stage(state, &stage.argv, input, in_pipeline)?;
    if matches!(control, ShellControl::Exit) || index + 1 == stages.len() {
        return Ok(ExecutionOutcome {
            control,
            output: into_text(output),
        });
    }

    execute_recursive(state, stages, index + 1, into_text(output))
}

fn execute_stage(
    state: &mut ShellState,
    argv: &[String],
    input: Option<String>,
    in_pipeline: bool,
) -> Result<(ShellControl, CommandOutput)> {
    let Some(name) = argv.first().map(String::as_str) else {
        return Err(ConfluenceCliError::Config("empty command stage".to_owned()));
    };

    if let Some(builtin) = builtins::resolve(name) {
        return builtins::execute(state, builtin, argv, in_pipeline && argv.len() > 0);
    }
    if commands::is_registered(name) {
        return Ok((
            ShellControl::Continue,
            commands::execute(state, argv, input)?,
        ));
    }
    if in_pipeline {
        return Err(ConfluenceCliError::Config(format!(
            "`{name}` is not pipe-aware yet"
        )));
    }

    forward::execute_single(state, argv.to_vec())
}

fn into_text(output: CommandOutput) -> Option<String> {
    match output {
        CommandOutput::Empty => None,
        CommandOutput::Text(text) => Some(text),
    }
}
