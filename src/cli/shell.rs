use std::io::{BufRead, IsTerminal, Write};

use crate::domain::PageRef;
use crate::support::{ConfluenceCliError, Result};

use super::{try_run_from, GlobalArgs, OutputFormat};

const SHELL_HELP: &str = "Confluence shell commands:\n  help [topic]         Show shell or command help\n  context              Show current shell context\n  pwd                  Alias for context\n  use profile <name>   Set the shell-local profile\n  use page <ref>       Set the current page id or URL\n  use space-key <key>  Set the current space key for page create\n  use space-id <id>    Set the current space id for page create\n  unset profile        Clear the shell-local profile\n  unset page           Clear the current page\n  unset space          Clear the current space key/id\n  back                 Clear the current page\n  exit | quit          Leave the shell\n\nForwarded commands keep the same grammar as the one-liner CLI:\n  page info\n  page search \"release notes\"\n  attachment list\n  property list\n  comment list";

#[derive(Debug, Clone)]
struct ShellSession {
    global: GlobalArgs,
    current_page: Option<String>,
    current_space_id: Option<String>,
    current_space_key: Option<String>,
}

enum ShellControl {
    Continue,
    Exit,
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
    let mut session = ShellSession::new(global.clone());

    if interactive {
        writeln!(
            writer,
            "Confluence shell. Type `help` for help, `exit` to quit."
        )?;
    }

    loop {
        if interactive {
            write!(writer, "{}", session.prompt())?;
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

        match session.run_line(line, writer) {
            Ok(ShellControl::Continue) => {}
            Ok(ShellControl::Exit) => break,
            Err(error) => writeln!(writer, "error: {error}")?,
        }
    }

    Ok(())
}

impl ShellSession {
    fn new(global: GlobalArgs) -> Self {
        Self {
            global,
            current_page: None,
            current_space_id: None,
            current_space_key: None,
        }
    }

    fn prompt(&self) -> String {
        let mut segments = Vec::new();
        if let Some(profile) = &self.global.profile {
            segments.push(format!("profile={profile}"));
        }
        if let Some(space_key) = &self.current_space_key {
            segments.push(format!("space={space_key}"));
        } else if let Some(space_id) = &self.current_space_id {
            segments.push(format!("space-id={space_id}"));
        }
        if let Some(page) = &self.current_page {
            segments.push(format!("page={page}"));
        }

        if segments.is_empty() {
            "confluence> ".to_owned()
        } else {
            format!("confluence[{}]> ", segments.join(", "))
        }
    }

    fn run_line<W: Write>(&mut self, line: &str, writer: &mut W) -> Result<ShellControl> {
        let tokens = shlex::split(line).ok_or_else(|| {
            ConfluenceCliError::Config("shell input has unmatched quotes".to_owned())
        })?;

        if tokens.is_empty() {
            return Ok(ShellControl::Continue);
        }

        match tokens[0].as_str() {
            "exit" | "quit" => return Ok(ShellControl::Exit),
            "help" => {
                self.write_help(&tokens[1..], writer)?;
                return Ok(ShellControl::Continue);
            }
            "context" | "pwd" => {
                self.write_context(writer)?;
                return Ok(ShellControl::Continue);
            }
            "use" => {
                self.apply_use(&tokens[1..])?;
                self.write_context(writer)?;
                return Ok(ShellControl::Continue);
            }
            "unset" => {
                self.apply_unset(&tokens[1..])?;
                self.write_context(writer)?;
                return Ok(ShellControl::Continue);
            }
            "back" => {
                self.current_page = None;
                self.write_context(writer)?;
                return Ok(ShellControl::Continue);
            }
            _ => {}
        }

        if matches!(tokens.first().map(String::as_str), Some("shell")) {
            return Err(ConfluenceCliError::Config(
                "nested `confluence shell` sessions are not supported".to_owned(),
            ));
        }

        if matches!(tokens.first().map(String::as_str), Some("login")) {
            return Err(ConfluenceCliError::Config(
                "run `confluence login` outside the shell; shell sessions reuse the active profile"
                    .to_owned(),
            ));
        }

        let argv = self.build_argv(&tokens)?;
        if let Err(error) = try_run_from(argv) {
            write!(writer, "{error}")?;
        }

        Ok(ShellControl::Continue)
    }

    fn build_argv(&self, tokens: &[String]) -> Result<Vec<String>> {
        let mut forwarded = tokens.to_vec();
        self.apply_context(&mut forwarded)?;

        let mut argv = vec!["confluence".to_owned()];
        if let Some(config_path) = &self.global.config_path {
            argv.push("--config-path".to_owned());
            argv.push(config_path.display().to_string());
        }
        if let Some(profile) = &self.global.profile {
            argv.push("--profile".to_owned());
            argv.push(profile.clone());
        }
        if !matches!(self.global.output, OutputFormat::Human) {
            argv.push("--output".to_owned());
            argv.push("json".to_owned());
        }
        argv.extend(forwarded);
        Ok(argv)
    }

    fn apply_context(&self, tokens: &mut Vec<String>) -> Result<()> {
        let Some(group) = tokens.first().cloned() else {
            return Ok(());
        };
        let command = tokens.get(1).cloned();

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
                    self.inject_page_ref(tokens, false)?;
                }

                if command == "create"
                    && !tokens
                        .iter()
                        .any(|token| token == "--space-key" || token == "--space-id")
                {
                    if let Some(space_key) = &self.current_space_key {
                        tokens.push("--space-key".to_owned());
                        tokens.push(space_key.clone());
                    } else if let Some(space_id) = &self.current_space_id {
                        tokens.push("--space-id".to_owned());
                        tokens.push(space_id.clone());
                    }
                }
            }
            ("attachment", Some(command)) => {
                self.inject_page_ref(tokens, command == "delete")?;
            }
            ("property", Some(command)) => {
                self.inject_page_ref(tokens, matches!(command, "get" | "set" | "delete"))?;
            }
            ("comment", Some(command)) => {
                if matches!(command, "list" | "create") {
                    self.inject_page_ref(tokens, false)?;
                } else if command == "reply" {
                    self.inject_page_ref(tokens, true)?;
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn inject_page_ref(
        &self,
        tokens: &mut Vec<String>,
        allow_second_positional: bool,
    ) -> Result<()> {
        let Some(current_page) = &self.current_page else {
            return Ok(());
        };

        let should_insert = match tokens.get(2) {
            None => true,
            Some(token) if token.starts_with('-') => true,
            Some(token) if allow_second_positional => PageRef::parse(token).is_err(),
            Some(_) => false,
        };

        if should_insert {
            tokens.insert(2, current_page.clone());
        }

        Ok(())
    }

    fn write_help<W: Write>(&self, args: &[String], writer: &mut W) -> Result<()> {
        if args.is_empty() || matches!(args.first().map(String::as_str), Some("shell")) {
            writeln!(writer, "{SHELL_HELP}")?;
            return Ok(());
        }

        let mut argv = vec!["confluence".to_owned()];
        argv.extend(args.iter().cloned());
        if !argv.iter().any(|token| token == "--help" || token == "-h") {
            argv.push("--help".to_owned());
        }

        if let Err(error) = try_run_from(argv) {
            write!(writer, "{error}")?;
        }

        Ok(())
    }

    fn write_context<W: Write>(&self, writer: &mut W) -> Result<()> {
        writeln!(
            writer,
            "profile: {}",
            self.global
                .profile
                .as_deref()
                .unwrap_or("(resolver default)")
        )?;
        writeln!(
            writer,
            "space-key: {}",
            self.current_space_key.as_deref().unwrap_or("(none)")
        )?;
        writeln!(
            writer,
            "space-id: {}",
            self.current_space_id.as_deref().unwrap_or("(none)")
        )?;
        writeln!(
            writer,
            "page: {}",
            self.current_page.as_deref().unwrap_or("(none)")
        )?;
        Ok(())
    }

    fn apply_use(&mut self, args: &[String]) -> Result<()> {
        match args {
            [kind, value] if kind == "profile" => {
                self.global.profile = Some(value.clone());
                Ok(())
            }
            [kind, value] if kind == "page" => {
                PageRef::parse(value)?;
                self.current_page = Some(value.clone());
                Ok(())
            }
            [kind, value] if kind == "space-key" => {
                self.current_space_key = Some(value.clone());
                self.current_space_id = None;
                Ok(())
            }
            [kind, value] if kind == "space-id" => {
                self.current_space_id = Some(value.clone());
                self.current_space_key = None;
                Ok(())
            }
            _ => Err(ConfluenceCliError::Config(
                "usage: use profile <name> | use page <page-ref> | use space-key <key> | use space-id <id>"
                    .to_owned(),
            )),
        }
    }

    fn apply_unset(&mut self, args: &[String]) -> Result<()> {
        match args {
            [kind] if kind == "profile" => {
                self.global.profile = None;
                Ok(())
            }
            [kind] if kind == "page" => {
                self.current_page = None;
                Ok(())
            }
            [kind] if *kind == "space" || *kind == "space-key" || *kind == "space-id" => {
                self.current_space_key = None;
                self.current_space_id = None;
                Ok(())
            }
            _ => Err(ConfluenceCliError::Config(
                "usage: unset profile | unset page | unset space".to_owned(),
            )),
        }
    }
}
