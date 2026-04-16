use serde_json::Value;

use crate::application::models::{
    ArchiveResult, AttachmentSummary, AttachmentUploadRequest, CommentCreateRequest,
    CommentSummary, ContentProperty, CreatePageRequest, MovePageRequest, PageBody, PageSummary,
    SpaceSummary, UpdatePageRequest,
};
use crate::domain::{BodyFormat, CommentLocation, DeleteMode, PageRef};
use crate::support::Result;

pub trait PagesApi {
    fn list_spaces(&self) -> Result<Vec<SpaceSummary>>;
    fn create_page(&self, request: CreatePageRequest) -> Result<PageSummary>;
    fn list_child_pages(&self, page: &PageRef) -> Result<Vec<PageSummary>>;
    fn get_page_info(&self, page: &PageRef) -> Result<PageSummary>;
    fn read_page(&self, page: &PageRef, format: BodyFormat) -> Result<PageBody>;
    fn search_pages(&self, query: &str) -> Result<Vec<PageSummary>>;
    fn search_pages_cql(&self, query: &str) -> Result<Vec<PageSummary>>;
    fn archive_page(&self, page: &PageRef) -> Result<ArchiveResult>;
    fn delete_page(&self, page: &PageRef, mode: DeleteMode) -> Result<()>;
    fn update_page(&self, request: UpdatePageRequest) -> Result<PageSummary>;
    fn move_page(&self, request: MovePageRequest) -> Result<PageSummary>;
}

pub trait AttachmentsApi {
    fn list_attachments(&self, page: &PageRef) -> Result<Vec<AttachmentSummary>>;
    fn download_attachment(&self, page: &PageRef, attachment_id: &str) -> Result<Vec<u8>>;
    fn upload_attachment(&self, request: AttachmentUploadRequest)
    -> Result<Vec<AttachmentSummary>>;
    fn delete_attachment(&self, page: &PageRef, attachment_id: &str) -> Result<()>;
}

pub trait PropertiesApi {
    fn list_properties(&self, page: &PageRef) -> Result<Vec<ContentProperty>>;
    fn get_property(&self, page: &PageRef, key: &str) -> Result<ContentProperty>;
    fn set_property(&self, page: &PageRef, key: &str, value: Value) -> Result<ContentProperty>;
    fn delete_property(&self, page: &PageRef, key: &str) -> Result<()>;
}

pub trait CommentsApi {
    fn list_comments(
        &self,
        page: &PageRef,
        location: Option<CommentLocation>,
    ) -> Result<Vec<CommentSummary>>;
    fn get_comment(&self, comment_id: &str) -> Result<CommentSummary>;
    fn create_comment(&self, request: CommentCreateRequest) -> Result<CommentSummary>;
    fn set_inline_comment_resolution(
        &self,
        comment_id: &str,
        resolved: bool,
    ) -> Result<CommentSummary>;
    fn delete_comment(&self, comment_id: &str) -> Result<()>;
}
