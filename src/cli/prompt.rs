use std::io::{BufRead, Write};

use crate::support::{ConfluenceCliError, Result};

use super::args::CliAuthKind;

pub(super) fn prompt_required<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
) -> Result<String> {
    loop {
        let value = prompt_line(reader, writer, label)?;
        if !value.is_empty() {
            return Ok(value);
        }
        writeln!(writer, "Please enter a value.")?;
    }
}

pub(super) fn prompt_optional<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
) -> Result<Option<String>> {
    let value = prompt_line(reader, writer, label)?;
    Ok((!value.is_empty()).then_some(value))
}

pub(super) fn prompt_auth_kind<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
) -> Result<CliAuthKind> {
    loop {
        let value = prompt_line(reader, writer, "Auth type [basic/bearer/mtls] (basic): ")?;
        match value.to_ascii_lowercase().as_str() {
            "" | "basic" => return Ok(CliAuthKind::Basic),
            "bearer" => return Ok(CliAuthKind::Bearer),
            "mtls" => return Ok(CliAuthKind::Mtls),
            _ => writeln!(writer, "Enter basic, bearer, or mtls.")?,
        }
    }
}

pub(super) fn prompt_bool<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
    default: bool,
) -> Result<bool> {
    loop {
        let suffix = if default { "Y/n" } else { "y/N" };
        let value = prompt_line(reader, writer, &format!("{label} [{suffix}]: "))?;
        match value.to_ascii_lowercase().as_str() {
            "" => return Ok(default),
            "y" | "yes" | "true" | "1" => return Ok(true),
            "n" | "no" | "false" | "0" => return Ok(false),
            _ => writeln!(writer, "Enter yes or no.")?,
        }
    }
}

fn prompt_line<R: BufRead, W: Write>(
    reader: &mut R,
    writer: &mut W,
    label: &str,
) -> Result<String> {
    write!(writer, "{label}")?;
    writer.flush()?;

    let mut buffer = String::new();
    let read = reader.read_line(&mut buffer)?;
    if read == 0 {
        return Err(ConfluenceCliError::Config(
            "interactive login requires stdin input".to_owned(),
        ));
    }

    Ok(buffer.trim().to_owned())
}
