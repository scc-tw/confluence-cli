use std::path::{Path, PathBuf};

use crate::convert::{
    apply_unified_patch, build_bundle_metadata, convert_text, export_bundle_file,
};
use crate::domain::{BodyFormat, DeleteMode, MoveTarget, PageId, PageRef};
use crate::support::{ConfluenceCliError, Result};

use super::models::{
    ArchiveResult, CreatePageRequest, MovePageRequest, PageBody, PageSummary, SpaceSummary,
    UpdatePageRequest,
};
use super::ports::{AttachmentsApi, PagesApi};

use super::attachments::attachment_download_all;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PageExportResult {
    pub directory: PathBuf,
    pub content_path: PathBuf,
    pub attachment_count: usize,
}

pub fn list_spaces<A: PagesApi>(api: &A) -> Result<Vec<SpaceSummary>> {
    api.list_spaces()
}

pub fn page_info<A: PagesApi>(api: &A, page: &PageRef) -> Result<PageSummary> {
    api.get_page_info(page)
}

pub fn page_children<A: PagesApi>(api: &A, page: &PageRef) -> Result<Vec<PageSummary>> {
    api.list_child_pages(page)
}

pub fn page_read<A: PagesApi>(api: &A, page: &PageRef, format: BodyFormat) -> Result<PageBody> {
    api.read_page(page, format)
}

pub fn page_search<A: PagesApi>(api: &A, query: &str) -> Result<Vec<PageSummary>> {
    if query.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "search query must not be empty".to_owned(),
        ));
    }
    api.search_pages(query)
}

pub fn page_search_cql<A: PagesApi>(api: &A, query: &str) -> Result<Vec<PageSummary>> {
    if query.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "search query must not be empty".to_owned(),
        ));
    }
    api.search_pages_cql(query)
}

pub fn page_archive<A: PagesApi>(api: &A, page: &PageRef) -> Result<ArchiveResult> {
    api.archive_page(page)
}

pub fn page_delete<A: PagesApi>(
    api: &A,
    page: &PageRef,
    mode: DeleteMode,
    yes_im_sure: bool,
) -> Result<()> {
    if matches!(mode, DeleteMode::Purge) && !yes_im_sure {
        return Err(ConfluenceCliError::Config(
            "purge requires --yes-im-sure".to_owned(),
        ));
    }
    api.delete_page(page, mode)
}

pub fn page_update<A: PagesApi>(
    api: &A,
    page: &PageRef,
    title: String,
    storage_body: String,
    version: u32,
) -> Result<PageSummary> {
    if title.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "page update requires a non-empty title".to_owned(),
        ));
    }
    if storage_body.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "page update requires a non-empty storage body".to_owned(),
        ));
    }
    api.update_page(UpdatePageRequest {
        page: page.clone(),
        title,
        storage_body,
        version,
    })
}

pub fn page_create<A: PagesApi>(
    api: &A,
    title: String,
    storage_body: String,
    space_id: Option<String>,
    space_key: Option<String>,
    parent: Option<PageRef>,
) -> Result<PageSummary> {
    if title.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "page create requires a non-empty title".to_owned(),
        ));
    }
    if storage_body.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "page create requires a non-empty storage body".to_owned(),
        ));
    }
    if space_id.is_some() && space_key.is_some() {
        return Err(ConfluenceCliError::Config(
            "page create accepts either space id or space key, not both".to_owned(),
        ));
    }

    let resolved_parent_id = match parent.as_ref() {
        Some(PageRef::Id(page_id)) => Some(*page_id),
        Some(PageRef::Url(_)) => Some(PageId::new(
            api.get_page_info(parent.as_ref().expect("parent exists"))?
                .id,
        )),
        None => None,
    };

    let resolved_space_id = if let Some(space_id) = space_id {
        space_id
    } else if let Some(space_key) = space_key {
        api.list_spaces()?
            .into_iter()
            .find(|space| space.key.eq_ignore_ascii_case(&space_key))
            .map(|space| space.id)
            .ok_or_else(|| {
                ConfluenceCliError::Config(format!("space key '{space_key}' not found"))
            })?
    } else if let Some(parent) = parent.as_ref() {
        api.get_page_info(parent)?.space_id.ok_or_else(|| {
            ConfluenceCliError::Config("parent page did not expose a space id".to_owned())
        })?
    } else {
        return Err(ConfluenceCliError::Config(
            "page create requires either --space-id, --space-key, or --parent".to_owned(),
        ));
    };

    api.create_page(CreatePageRequest {
        title,
        storage_body,
        space_id: resolved_space_id,
        parent_id: resolved_parent_id,
    })
}

pub fn page_patch<A: PagesApi>(
    api: &A,
    page: &PageRef,
    base: &str,
    patch: &str,
) -> Result<PageSummary> {
    let current = api.read_page(page, BodyFormat::Storage)?;
    if current.content != base {
        return Err(ConfluenceCliError::Config(
            "page patch base file does not match the current remote storage body".to_owned(),
        ));
    }

    let version = current.page.version.ok_or_else(|| {
        ConfluenceCliError::Config("page patch requires a current version".to_owned())
    })?;
    let updated_body = apply_unified_patch(base, patch)?;

    api.update_page(UpdatePageRequest {
        page: page.clone(),
        title: current.page.title,
        storage_body: updated_body,
        version: version + 1,
    })
}

pub fn page_move<A: PagesApi>(
    api: &A,
    page: &PageRef,
    target: MoveTarget,
    title: Option<String>,
) -> Result<PageSummary> {
    api.move_page(MovePageRequest {
        page: page.clone(),
        target,
        title,
    })
}

pub fn page_export<A: PagesApi + AttachmentsApi>(
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
        attachment_download_all(api, page, &attachments_dir)?.len()
    } else {
        0
    };

    Ok(PageExportResult {
        directory: directory.to_path_buf(),
        content_path: directory.join(file_name),
        attachment_count,
    })
}
