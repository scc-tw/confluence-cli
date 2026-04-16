use crate::convert::convert_text;
use crate::support::Result;

use super::super::args::{ConvertCommand, GlobalArgs};
use super::super::input::read_command_input;
use super::super::output::{print_json_or_human, print_text};

pub(super) fn dispatch_convert(global: &GlobalArgs, command: ConvertCommand) -> Result<()> {
    let input = read_command_input(
        command.input,
        command.input_file,
        "convert requires --input or --input-file",
    )?;
    let output = convert_text(&input, command.from.into(), command.to.into())?;
    print_json_or_human(global.output, &output, |output| print_text(output))
}
