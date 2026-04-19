mod builtins;
mod commands;
mod format;
mod forward;
mod parser;
mod pipeline;
mod state;

use std::io::{BufRead, IsTerminal, Write};

use crate::support::Result;

use super::GlobalArgs;
use super::bootstrap::load_runtime_and_vfs;
use parser::ShellParser;
use state::ShellState;

const SHELL_HELP: &str = "Confluence shell commands:\n  help [topic]         Show shell or command help\n  pwd                  Print the current shell path\n  ls [target]          List spaces or child pages in the current directory\n  cd <target>          Enter a space or child page\n  cd ..                Go to the parent directory\n  cd /                 Go to the root space listing\n  clear | cls          Clear the visible terminal buffer\n  stat [target]        Show metadata for a node\n  mkdir <target>       Create a folder under a space or page parent\n  mv <source> <destination>  Move or rename a page or folder\n  cp [-r] <source> <destination>  Copy a page or recursively copy a folder-like subtree\n  rm <target>          Remove a page\n  rmdir <target>       Remove an empty folder\n  cat [--raw|--text|--markdown|--html] [target]  Read page content (default: markdown)\n  tail [-n <count>|-n +<start>] [target]  Print the last lines of piped input or page text\n  grep <pattern> [target]  Search shell text input or recursively search a subtree\n  find [target] [--name <pattern>]  Walk a subtree recursively\n  whoami | id          Show the active shell identity\n  seq ...              Print a numeric sequence\n  sleep <duration>     Delay for a duration like 1s or 250ms\n  use profile <name>   Switch profile and reset the shell to /\n  exit | quit          Leave the shell\n\nPipelines:\n  ls SPACE | grep Guide\n  cat 12345 | grep keyword\n\nInside shell, keep using the same one-liner commands without the binary name:\n  page info\n  page read\n  page create --title \"Draft\" --body \"# Hello\"\n  page create-child --title \"Child\" --body \"# Hello\"\n  attachment list\n  property list\n  comment list";

enum ShellControl {
    Continue,
    Exit,
}

pub(crate) enum CommandOutput {
    Empty,
    Text(String),
}

pub(super) fn run(global: &GlobalArgs) -> Result<()> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let interactive = stdin.is_terminal() && stdout.is_terminal();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    run_with_io(global, &mut reader, &mut writer, interactive)
}

pub(super) fn run_with_io<R: BufRead, W: Write>(
    global: &GlobalArgs,
    reader: &mut R,
    writer: &mut W,
    interactive: bool,
) -> Result<()> {
    let (runtime, vfs) = load_runtime_and_vfs(global)?;
    let mut state = ShellState::new(global.clone(), runtime, vfs);

    if interactive {
        writeln!(
            writer,
            "Confluence shell. You are at /. Use `ls` to list spaces, `cd <space>` to enter one, `pwd` to show location, and `exit` to quit."
        )?;
    }

    loop {
        if interactive {
            write!(writer, "{}", state.prompt())?;
            writer.flush()?;
        }

        let mut line = String::new();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            if interactive {
                writeln!(writer)?;
            }
            break;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match run_line(&mut state, line, writer) {
            Ok(ShellControl::Continue) => {}
            Ok(ShellControl::Exit) => break,
            Err(error) => writeln!(writer, "error: {error}")?,
        }
    }

    Ok(())
}

fn run_line<W: Write>(state: &mut ShellState, line: &str, writer: &mut W) -> Result<ShellControl> {
    let parsed = ShellParser::parse(line)?;
    let outcome = pipeline::execute(state, parsed)?;
    if let Some(text) = outcome.output {
        write!(writer, "{text}")?;
        if !text.ends_with('\n') {
            writeln!(writer)?;
        }
    }
    Ok(outcome.control)
}
