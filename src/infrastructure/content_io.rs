use std::fs;
use std::path::{Path, PathBuf};

use crate::application::models::{AttachmentSummary, AttachmentUploadRequest};
use crate::application::pages::PageExportResult;
use crate::application::ports::{AttachmentsApi, PagesApi};
use crate::convert::{build_bundle_metadata, convert_text, export_bundle_file};
use crate::domain::{BodyFormat, PageRef};
use crate::support::{ConfluenceCliError, Result};

pub fn download_attachments_to_dir<A: AttachmentsApi>(
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

pub fn upload_attachment_from_path<A: AttachmentsApi>(
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

pub fn export_page_to_dir<A: PagesApi + AttachmentsApi>(
    api: &A,
    page: &PageRef,
    directory: &Path,
    format: BodyFormat,
    include_attachments: bool,
) -> Result<PageExportResult> {
    let summary = api.get_page_info(page)?;
    let storage = api.read_page(page, BodyFormat::Storage)?;
    let (file_name, content) = match format {
        BodyFormat::Markdown => (
            "page.md",
            convert_text(&storage.content, BodyFormat::Storage, BodyFormat::Markdown)?,
        ),
        BodyFormat::Storage => ("page.storage", storage.content.clone()),
        BodyFormat::Text => (
            "page.txt",
            convert_text(&storage.content, BodyFormat::Storage, BodyFormat::Text)?,
        ),
        BodyFormat::Html => ("page.html", api.read_page(page, BodyFormat::Html)?.content),
    };

    let metadata = build_bundle_metadata(
        Some(summary.id),
        Some(summary.title.clone()),
        summary.version,
        &storage.content,
    );
    export_bundle_file(directory, &metadata, file_name, &content)?;

    let attachment_count = if include_attachments {
        let attachments_dir = directory.join("attachments");
        download_attachments_to_dir(api, page, &attachments_dir)?.len()
    } else {
        0
    };

    Ok(PageExportResult {
        directory: directory.to_path_buf(),
        content_path: directory.join(file_name),
        attachment_count,
    })
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
