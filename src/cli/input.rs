use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use super::args::CliBodyFormat;
use crate::convert::convert_text;
use crate::domain::{BodyFormat, MoveTarget, PageRef};
use crate::support::{ConfluenceCliError, Result};

pub(super) fn read_command_input(
    inline: Option<String>,
    file: Option<PathBuf>,
    missing_message: &str,
) -> Result<String> {
    match (inline, file) {
        (Some(input), None) => Ok(input),
        (None, Some(path)) => Ok(fs::read_to_string(path)?),
        (Some(_), Some(_)) => Err(ConfluenceCliError::Config(
            "use either the inline value or file input, not both".to_owned(),
        )),
        (None, None) => Err(ConfluenceCliError::Config(missing_message.to_owned())),
    }
}

pub(super) fn read_optional_json(
    inline: Option<String>,
    file: Option<PathBuf>,
    missing_message: &str,
) -> Result<Option<Value>> {
    match (inline, file) {
        (None, None) => {
            if missing_message.is_empty() {
                Ok(None)
            } else {
                Err(ConfluenceCliError::Config(missing_message.to_owned()))
            }
        }
        (inline, file) => {
            let raw = read_command_input(inline, file, missing_message)?;
            let json = serde_json::from_str(&raw).map_err(|error| {
                ConfluenceCliError::Config(format!("JSON input must be valid JSON: {error}"))
            })?;
            Ok(Some(json))
        }
    }
}

pub(super) fn convert_body_to_storage(body: String, format: CliBodyFormat) -> Result<String> {
    let format: BodyFormat = format.into();
    if matches!(format, BodyFormat::Storage) {
        Ok(body)
    } else {
        convert_text(&body, format, BodyFormat::Storage)
    }
}

pub(super) fn parse_move_target(
    to_parent: Option<PageRef>,
    before: Option<PageRef>,
    after: Option<PageRef>,
) -> Result<MoveTarget> {
    let count = usize::from(to_parent.is_some())
        + usize::from(before.is_some())
        + usize::from(after.is_some());
    if count != 1 {
        return Err(ConfluenceCliError::Config(
            "page move requires exactly one of --to-parent, --before, or --after".to_owned(),
        ));
    }

    if let Some(parent) = to_parent {
        Ok(MoveTarget::Parent(parent))
    } else if let Some(before) = before {
        Ok(MoveTarget::Before(before))
    } else if let Some(after) = after {
        Ok(MoveTarget::After(after))
    } else {
        unreachable!("validated move target should exist")
    }
}

pub(super) fn default_export_dir(page: &PageRef) -> PathBuf {
    match page {
        PageRef::Id(id) => PathBuf::from(format!("page-export-{}", id.get())),
        PageRef::Url(_) => PathBuf::from("page-export"),
    }
}
