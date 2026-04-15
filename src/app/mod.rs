use crate::api::{
    ArchiveResult, AttachmentSummary, AttachmentUploadRequest, CommentCreateRequest,
    CommentSummary, ConfluenceApi, ContentProperty, MovePageRequest, PageBody, PageSummary,
    SpaceSummary, UpdatePageRequest,
};
use crate::config::{ResolveOptions, RuntimeConfig, load_runtime};
use crate::convert::{build_bundle_metadata, convert_text, export_bundle_file};
use crate::domain::{BodyFormat, CommentLocation, DeleteMode, MoveTarget, PageRef};
use crate::support::{ConfluenceCliError, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RuntimeContext {
    pub runtime_config: RuntimeConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PageExportResult {
    pub directory: PathBuf,
    pub content_path: PathBuf,
    pub attachment_count: usize,
}

impl RuntimeContext {
    pub fn load(options: &ResolveOptions) -> Result<Self> {
        Ok(Self {
            runtime_config: load_runtime(options)?,
        })
    }
}

pub fn list_spaces<A: ConfluenceApi>(api: &A) -> Result<Vec<SpaceSummary>> {
    api.list_spaces()
}

pub fn page_info<A: ConfluenceApi>(api: &A, page: &PageRef) -> Result<PageSummary> {
    api.get_page_info(page)
}

pub fn page_read<A: ConfluenceApi>(
    api: &A,
    page: &PageRef,
    format: BodyFormat,
) -> Result<PageBody> {
    api.read_page(page, format)
}

pub fn page_search<A: ConfluenceApi>(api: &A, query: &str) -> Result<Vec<PageSummary>> {
    if query.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "search query must not be empty".to_owned(),
        ));
    }

    api.search_pages(query)
}

pub fn page_archive<A: ConfluenceApi>(api: &A, page: &PageRef) -> Result<ArchiveResult> {
    api.archive_page(page)
}

pub fn page_delete<A: ConfluenceApi>(
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

pub fn page_update<A: ConfluenceApi>(
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

pub fn page_move<A: ConfluenceApi>(
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

pub fn page_export<A: ConfluenceApi>(
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

pub fn attachment_list<A: ConfluenceApi>(
    api: &A,
    page: &PageRef,
) -> Result<Vec<AttachmentSummary>> {
    api.list_attachments(page)
}

pub fn attachment_download_all<A: ConfluenceApi>(
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

pub fn attachment_upload<A: ConfluenceApi>(
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

    api.upload_attachment(AttachmentUploadRequest {
        page: page.clone(),
        file_path,
        comment,
        minor_edit,
        replace,
    })
}

pub fn attachment_delete<A: ConfluenceApi>(
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

pub fn property_list<A: ConfluenceApi>(api: &A, page: &PageRef) -> Result<Vec<ContentProperty>> {
    api.list_properties(page)
}

pub fn property_get<A: ConfluenceApi>(
    api: &A,
    page: &PageRef,
    key: &str,
) -> Result<ContentProperty> {
    require_property_key(key)?;
    api.get_property(page, key)
}

pub fn property_set<A: ConfluenceApi>(
    api: &A,
    page: &PageRef,
    key: &str,
    value: Value,
) -> Result<ContentProperty> {
    require_property_key(key)?;
    api.set_property(page, key, value)
}

pub fn property_delete<A: ConfluenceApi>(api: &A, page: &PageRef, key: &str) -> Result<()> {
    require_property_key(key)?;
    api.delete_property(page, key)
}

pub fn comment_list<A: ConfluenceApi>(
    api: &A,
    page: &PageRef,
    location: Option<CommentLocation>,
) -> Result<Vec<CommentSummary>> {
    api.list_comments(page, location)
}

pub fn comment_create<A: ConfluenceApi>(
    api: &A,
    page: &PageRef,
    body_storage: String,
    location: CommentLocation,
    parent_id: Option<String>,
    inline_properties: Option<Value>,
) -> Result<CommentSummary> {
    if body_storage.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "comment create requires a non-empty body".to_owned(),
        ));
    }

    if matches!(location, CommentLocation::Inline) && inline_properties.is_none() {
        return Err(ConfluenceCliError::NotImplemented(
            "inline comment creation requires explicit inline properties".to_owned(),
        ));
    }

    api.create_comment(CommentCreateRequest {
        page: page.clone(),
        body_storage,
        parent_id,
        location,
        inline_properties,
    })
}

pub fn comment_delete<A: ConfluenceApi>(api: &A, comment_id: &str) -> Result<()> {
    if comment_id.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "comment delete requires a non-empty comment id".to_owned(),
        ));
    }

    api.delete_comment(comment_id)
}

fn require_property_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        Err(ConfluenceCliError::Config(
            "property key must not be empty".to_owned(),
        ))
    } else {
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{ConfluenceApi, UpdatePageRequest};
    use crate::domain::PageId;
    use std::cell::RefCell;
    use tempfile::tempdir;

    #[derive(Default)]
    struct FakeApi {
        deleted: RefCell<Vec<(String, DeleteMode)>>,
        updates: RefCell<Vec<UpdatePageRequest>>,
        moved: RefCell<Vec<MovePageRequest>>,
        comments: RefCell<Vec<CommentCreateRequest>>,
    }

    impl ConfluenceApi for FakeApi {
        fn list_spaces(&self) -> Result<Vec<SpaceSummary>> {
            Ok(vec![SpaceSummary {
                id: "100".to_owned(),
                key: "ENG".to_owned(),
                name: "Engineering".to_owned(),
            }])
        }

        fn get_page_info(&self, _page: &PageRef) -> Result<PageSummary> {
            Ok(PageSummary {
                id: 1,
                title: "Design Doc".to_owned(),
                status: Some("current".to_owned()),
                space_id: Some("100".to_owned()),
                version: Some(3),
            })
        }

        fn read_page(&self, _page: &PageRef, format: BodyFormat) -> Result<PageBody> {
            let content = match format {
                BodyFormat::Html => "<div>Hello</div>",
                _ => "<p>Hello</p>",
            };

            Ok(PageBody {
                page: self.get_page_info(&PageRef::Id(PageId::new(1)))?,
                format,
                content: content.to_owned(),
            })
        }

        fn search_pages(&self, _query: &str) -> Result<Vec<PageSummary>> {
            Ok(vec![self.get_page_info(&PageRef::Id(PageId::new(1)))?])
        }

        fn archive_page(&self, _page: &PageRef) -> Result<ArchiveResult> {
            Ok(ArchiveResult {
                task_id: "task-1".to_owned(),
                state: Some("RUNNING".to_owned()),
            })
        }

        fn delete_page(&self, page: &PageRef, mode: DeleteMode) -> Result<()> {
            self.deleted.borrow_mut().push((format!("{page:?}"), mode));
            Ok(())
        }

        fn update_page(&self, request: UpdatePageRequest) -> Result<PageSummary> {
            self.updates.borrow_mut().push(request.clone());
            Ok(PageSummary {
                id: 1,
                title: request.title,
                status: Some("current".to_owned()),
                space_id: Some("100".to_owned()),
                version: Some(request.version),
            })
        }

        fn move_page(&self, request: MovePageRequest) -> Result<PageSummary> {
            self.moved.borrow_mut().push(request.clone());
            Ok(PageSummary {
                id: 1,
                title: request.title.unwrap_or_else(|| "Design Doc".to_owned()),
                status: Some("current".to_owned()),
                space_id: Some("100".to_owned()),
                version: Some(4),
            })
        }

        fn list_attachments(&self, _page: &PageRef) -> Result<Vec<AttachmentSummary>> {
            Ok(vec![
                AttachmentSummary {
                    id: "a-1".to_owned(),
                    title: "diagram.png".to_owned(),
                    media_type: "image/png".to_owned(),
                    file_size: 12,
                    version: Some(1),
                    download_link: Some("https://example.test/file".to_owned()),
                },
                AttachmentSummary {
                    id: "a-2".to_owned(),
                    title: "diagram.png".to_owned(),
                    media_type: "image/png".to_owned(),
                    file_size: 15,
                    version: Some(1),
                    download_link: Some("https://example.test/file2".to_owned()),
                },
            ])
        }

        fn download_attachment(&self, _page: &PageRef, attachment_id: &str) -> Result<Vec<u8>> {
            Ok(format!("bytes:{attachment_id}").into_bytes())
        }

        fn upload_attachment(
            &self,
            request: AttachmentUploadRequest,
        ) -> Result<Vec<AttachmentSummary>> {
            Ok(vec![AttachmentSummary {
                id: "uploaded".to_owned(),
                title: request
                    .file_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("file.bin")
                    .to_owned(),
                media_type: "application/octet-stream".to_owned(),
                file_size: 1,
                version: Some(if request.replace { 2 } else { 1 }),
                download_link: None,
            }])
        }

        fn delete_attachment(&self, _page: &PageRef, _attachment_id: &str) -> Result<()> {
            Ok(())
        }

        fn list_properties(&self, _page: &PageRef) -> Result<Vec<ContentProperty>> {
            Ok(vec![ContentProperty {
                key: "color".to_owned(),
                value: serde_json::json!({ "hex": "#fff" }),
                version: 1,
            }])
        }

        fn get_property(&self, _page: &PageRef, key: &str) -> Result<ContentProperty> {
            Ok(ContentProperty {
                key: key.to_owned(),
                value: serde_json::json!(true),
                version: 2,
            })
        }

        fn set_property(
            &self,
            _page: &PageRef,
            key: &str,
            value: Value,
        ) -> Result<ContentProperty> {
            Ok(ContentProperty {
                key: key.to_owned(),
                value,
                version: 3,
            })
        }

        fn delete_property(&self, _page: &PageRef, _key: &str) -> Result<()> {
            Ok(())
        }

        fn list_comments(
            &self,
            _page: &PageRef,
            location: Option<CommentLocation>,
        ) -> Result<Vec<CommentSummary>> {
            Ok(vec![CommentSummary {
                id: "c-1".to_owned(),
                status: Some("current".to_owned()),
                body_storage: "<p>Hello</p>".to_owned(),
                location,
                parent_id: None,
                author: Some("Ada".to_owned()),
                created_at: Some("2025-01-01".to_owned()),
                version: Some(1),
                resolution: None,
            }])
        }

        fn create_comment(&self, request: CommentCreateRequest) -> Result<CommentSummary> {
            self.comments.borrow_mut().push(request.clone());
            Ok(CommentSummary {
                id: "c-2".to_owned(),
                status: Some("current".to_owned()),
                body_storage: request.body_storage,
                location: Some(request.location),
                parent_id: request.parent_id,
                author: Some("Ada".to_owned()),
                created_at: Some("2025-01-01".to_owned()),
                version: Some(1),
                resolution: None,
            })
        }

        fn delete_comment(&self, _comment_id: &str) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn rejects_empty_search_queries() {
        let api = FakeApi::default();
        let error = page_search(&api, "   ").expect_err("empty queries should fail");
        assert!(matches!(error, ConfluenceCliError::Config(_)));
    }

    #[test]
    fn requires_yes_im_sure_for_purge() {
        let api = FakeApi::default();
        let error = page_delete(&api, &PageRef::Id(PageId::new(1)), DeleteMode::Purge, false)
            .expect_err("purge should require explicit confirmation");
        assert!(matches!(error, ConfluenceCliError::Config(_)));
    }

    #[test]
    fn updates_require_title_and_body() {
        let api = FakeApi::default();
        let error = page_update(
            &api,
            &PageRef::Id(PageId::new(1)),
            " ".to_owned(),
            "<p>x</p>".to_owned(),
            2,
        )
        .expect_err("blank titles should fail");
        assert!(matches!(error, ConfluenceCliError::Config(_)));
    }

    #[test]
    fn passes_update_request_to_api() {
        let api = FakeApi::default();
        let summary = page_update(
            &api,
            &PageRef::Id(PageId::new(1)),
            "New Title".to_owned(),
            "<p>Updated</p>".to_owned(),
            5,
        )
        .expect("update should succeed");

        assert_eq!(summary.title, "New Title");
        assert_eq!(summary.version, Some(5));
        assert_eq!(api.updates.borrow().len(), 1);
    }

    #[test]
    fn move_requests_are_forwarded() {
        let api = FakeApi::default();
        let summary = page_move(
            &api,
            &PageRef::Id(PageId::new(1)),
            MoveTarget::Parent(PageRef::Id(PageId::new(2))),
            Some("Moved".to_owned()),
        )
        .expect("move should succeed");

        assert_eq!(summary.title, "Moved");
        assert_eq!(api.moved.borrow().len(), 1);
    }

    #[test]
    fn export_writes_content_metadata_and_attachments() {
        let api = FakeApi::default();
        let dir = tempdir().expect("tempdir should exist");

        let result = page_export(
            &api,
            &PageRef::Id(PageId::new(1)),
            dir.path(),
            BodyFormat::Markdown,
            true,
        )
        .expect("export should succeed");

        let content =
            fs::read_to_string(dir.path().join("page.md")).expect("exported markdown should exist");
        let attachment_1 = fs::read(dir.path().join("attachments").join("diagram.png"))
            .expect("first attachment should exist");
        let attachment_2 = fs::read(dir.path().join("attachments").join("diagram (1).png"))
            .expect("second attachment should exist");

        assert!(content.contains("Hello"));
        assert_eq!(result.attachment_count, 2);
        assert_eq!(attachment_1, b"bytes:a-1");
        assert_eq!(attachment_2, b"bytes:a-2");
    }

    #[test]
    fn inline_comments_require_inline_properties() {
        let api = FakeApi::default();
        let error = comment_create(
            &api,
            &PageRef::Id(PageId::new(1)),
            "<p>Hello</p>".to_owned(),
            CommentLocation::Inline,
            None,
            None,
        )
        .expect_err("inline comments should stay explicit");

        assert!(matches!(error, ConfluenceCliError::NotImplemented(_)));
    }

    #[test]
    fn property_keys_must_not_be_blank() {
        let api = FakeApi::default();
        let error = property_get(&api, &PageRef::Id(PageId::new(1)), "  ")
            .expect_err("blank property keys should fail");
        assert!(matches!(error, ConfluenceCliError::Config(_)));
    }
}
