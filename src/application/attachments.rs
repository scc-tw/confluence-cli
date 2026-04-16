use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::PageRef;
use crate::support::{ConfluenceCliError, Result};

use super::models::{AttachmentSummary, AttachmentUploadRequest};
use super::ports::AttachmentsApi;

pub fn attachment_list<A: AttachmentsApi>(
    api: &A,
    page: &PageRef,
) -> Result<Vec<AttachmentSummary>> {
    api.list_attachments(page)
}

pub fn attachment_download_all<A: AttachmentsApi>(
    api: &A,
    page: &PageRef,
    directory: &Path,
) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(directory)?;
    let attachments = api.list_attachments(page)?;
    let mut written = Vec::with_capacity(attachments.len());
    for attachment in attachments {
        let path = unique_path_for(directory, &attachment.title);
        let bytes = api.download_attachment(page, &attachment.id)?;
        fs::write(&path, bytes)?;
        written.push(path);
    }
    Ok(written)
}

pub fn attachment_upload<A: AttachmentsApi>(
    api: &A,
    page: &PageRef,
    file_path: PathBuf,
    comment: Option<String>,
    minor_edit: bool,
    replace: bool,
) -> Result<Vec<AttachmentSummary>> {
    if !file_path.exists() {
        return Err(ConfluenceCliError::Config(format!(
            "attachment file '{}' does not exist",
            file_path.display()
        )));
    }

    let file_name = file_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            ConfluenceCliError::Config(format!(
                "attachment file '{}' must have a valid UTF-8 file name",
                file_path.display()
            ))
        })?
        .to_owned();
    let content = fs::read(&file_path)?;

    api.upload_attachment(AttachmentUploadRequest {
        page: page.clone(),
        file_name,
        content,
        comment,
        minor_edit,
        replace,
    })
}

pub fn attachment_delete<A: AttachmentsApi>(
    api: &A,
    page: &PageRef,
    attachment_id: &str,
) -> Result<()> {
    if attachment_id.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "attachment delete requires a non-empty attachment id".to_owned(),
        ));
    }
    api.delete_attachment(page, attachment_id)
}

fn unique_path_for(directory: &Path, file_name: &str) -> PathBuf {
    let candidate = directory.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let source = Path::new(file_name);
    let stem = source
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("attachment");
    let extension = source.extension().and_then(|value| value.to_str());

    for index in 1.. {
        let next_name = match extension {
            Some(extension) => format!("{stem} ({index}).{extension}"),
            None => format!("{stem} ({index})"),
        };
        let next_path = directory.join(next_name);
        if !next_path.exists() {
            return next_path;
        }
    }

    unreachable!("attachment path generation should always terminate")
}
