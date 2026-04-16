pub use crate::application::attachments::{
    attachment_delete, attachment_download_all, attachment_list, attachment_upload,
};
pub use crate::application::comments::{
    comment_create, comment_delete, comment_info, comment_list, comment_reopen, comment_resolve,
};
pub use crate::application::pages::{
    PageExportResult, list_spaces, page_archive, page_children, page_create, page_delete,
    page_export, page_info, page_move, page_patch, page_read, page_search, page_search_cql,
    page_update,
};
pub use crate::application::properties::{
    property_delete, property_get, property_list, property_set,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AttachmentsApi, CommentsApi, PagesApi, PropertiesApi};
    use crate::application::models::{
        ArchiveResult, AttachmentSummary, AttachmentUploadRequest, CommentCreateRequest,
        CommentSummary, ContentProperty, CreatePageRequest, MovePageRequest, PageBody, PageSummary,
        SpaceSummary, UpdatePageRequest,
    };
    use crate::application::runtime::{
        ResolvedProfile, RuntimeConfig, RuntimeContext, RuntimeProfiles, ensure_writable,
    };
    use crate::domain::{BodyFormat, CommentLocation, DeleteMode, MoveTarget, PageId, PageRef};
    use crate::profile::AuthKind;
    use crate::support::{ConfluenceCliError, Result};
    use serde_json::Value;
    use std::cell::RefCell;
    use std::fs;
    use tempfile::tempdir;

    #[derive(Default)]
    struct FakeApi {
        deleted: RefCell<Vec<(String, DeleteMode)>>,
        updates: RefCell<Vec<UpdatePageRequest>>,
        moved: RefCell<Vec<MovePageRequest>>,
        comments: RefCell<Vec<CommentCreateRequest>>,
    }

    impl PagesApi for FakeApi {
        fn list_spaces(&self) -> Result<Vec<SpaceSummary>> {
            Ok(vec![SpaceSummary {
                id: "100".to_owned(),
                key: "ENG".to_owned(),
                name: "Engineering".to_owned(),
            }])
        }

        fn create_page(&self, request: CreatePageRequest) -> Result<PageSummary> {
            Ok(PageSummary {
                id: request
                    .parent_id
                    .map_or(10, |parent: crate::domain::PageId| parent.get() + 1),
                title: request.title,
                status: Some("current".to_owned()),
                space_id: Some(request.space_id),
                version: Some(1),
            })
        }

        fn list_child_pages(&self, _page: &PageRef) -> Result<Vec<PageSummary>> {
            Ok(vec![PageSummary {
                id: 2,
                title: "Child Page".to_owned(),
                status: Some("current".to_owned()),
                space_id: Some("100".to_owned()),
                version: Some(1),
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

        fn search_pages_cql(&self, _query: &str) -> Result<Vec<PageSummary>> {
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
    }

    impl AttachmentsApi for FakeApi {
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
                title: request.file_name,
                media_type: "application/octet-stream".to_owned(),
                file_size: 1,
                version: Some(if request.replace { 2 } else { 1 }),
                download_link: None,
            }])
        }

        fn delete_attachment(&self, _page: &PageRef, _attachment_id: &str) -> Result<()> {
            Ok(())
        }
    }

    impl PropertiesApi for FakeApi {
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
    }

    impl CommentsApi for FakeApi {
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
                inline_properties: None,
                inline_marker_ref: None,
                inline_original_selection: None,
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
                inline_properties: request.inline_properties,
                inline_marker_ref: None,
                inline_original_selection: None,
            })
        }

        fn get_comment(&self, comment_id: &str) -> Result<CommentSummary> {
            Ok(CommentSummary {
                id: comment_id.to_owned(),
                status: Some("current".to_owned()),
                body_storage: "<p>Hello</p>".to_owned(),
                location: Some(CommentLocation::Inline),
                parent_id: None,
                author: Some("Ada".to_owned()),
                created_at: Some("2025-01-01".to_owned()),
                version: Some(2),
                resolution: Some("open".to_owned()),
                inline_properties: Some(serde_json::json!({ "markerRef": "m-1" })),
                inline_marker_ref: Some("m-1".to_owned()),
                inline_original_selection: Some("selected text".to_owned()),
            })
        }

        fn set_inline_comment_resolution(
            &self,
            comment_id: &str,
            resolved: bool,
        ) -> Result<CommentSummary> {
            Ok(CommentSummary {
                id: comment_id.to_owned(),
                status: Some("current".to_owned()),
                body_storage: "<p>Hello</p>".to_owned(),
                location: Some(CommentLocation::Inline),
                parent_id: None,
                author: Some("Ada".to_owned()),
                created_at: Some("2025-01-01".to_owned()),
                version: Some(3),
                resolution: Some(if resolved { "resolved" } else { "open" }.to_owned()),
                inline_properties: None,
                inline_marker_ref: Some("m-1".to_owned()),
                inline_original_selection: Some("selected text".to_owned()),
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

    #[test]
    fn create_page_can_resolve_space_key() {
        let api = FakeApi::default();
        let summary = page_create(
            &api,
            "New Page".to_owned(),
            "<p>Hello</p>".to_owned(),
            None,
            Some("ENG".to_owned()),
            None,
        )
        .expect("page create should succeed");

        assert_eq!(summary.title, "New Page");
        assert_eq!(summary.space_id.as_deref(), Some("100"));
    }

    #[test]
    fn patch_requires_matching_base() {
        let api = FakeApi::default();
        let error = page_patch(
            &api,
            &PageRef::Id(PageId::new(1)),
            "<p>Different</p>",
            "--- original\n+++ updated\n@@ -1 +1 @@\n-<p>Different</p>\n+<p>Hello</p>\n",
        )
        .expect_err("mismatched patch bases should fail");

        assert!(matches!(error, ConfluenceCliError::Config(_)));
    }

    #[test]
    fn read_only_runtime_rejects_mutations() {
        let runtime = RuntimeContext {
            runtime_config: RuntimeConfig {
                profiles: RuntimeProfiles {
                    active_profile: None,
                    profiles: Vec::new(),
                },
                resolved_profile: Some(ResolvedProfile {
                    id: "profile-1".to_owned(),
                    name: Some("work".to_owned()),
                    domain: "example.atlassian.net".to_owned(),
                    protocol: "https".to_owned(),
                    api_path: "/wiki/rest/api".to_owned(),
                    auth_type: AuthKind::Bearer,
                    email: None,
                    username: None,
                    api_token: Some("token".to_owned()),
                    password: None,
                    read_only: true,
                    secret_backend: Some(crate::secret::SecretBackend::Keyring),
                }),
            },
        };

        let error = ensure_writable(&runtime).expect_err("read-only runtime should fail");
        assert!(matches!(error, ConfluenceCliError::Config(_)));
    }

    #[test]
    fn comment_info_returns_richer_inline_metadata() {
        let api = FakeApi::default();
        let comment = comment_info(&api, "c-1").expect("comment info should succeed");
        assert_eq!(comment.inline_marker_ref.as_deref(), Some("m-1"));
        assert_eq!(
            comment.inline_original_selection.as_deref(),
            Some("selected text")
        );
    }

    #[test]
    fn comment_resolve_and_reopen_update_status() {
        let api = FakeApi::default();
        let resolved = comment_resolve(&api, "c-1").expect("resolve should succeed");
        let reopened = comment_reopen(&api, "c-1").expect("reopen should succeed");
        assert_eq!(resolved.resolution.as_deref(), Some("resolved"));
        assert_eq!(reopened.resolution.as_deref(), Some("open"));
    }
}
