use serde_json::Value;

use crate::application::models::{
    ArchiveResult, AttachmentSummary, AttachmentUploadRequest, CommentCreateRequest,
    CommentSummary, ContentProperty, CreateFolderRequest, CreatePageRequest, MovePageRequest,
    PageBody, PageContentKind, PageSummary, SpaceSummary, UpdatePageRequest,
};
use crate::domain::{BodyFormat, CommentLocation, DeleteMode, PageRef};
use crate::support::Result;

use super::profiles::{ProfileDraft, ProfileSecrets};
use super::runtime::RuntimeConfig;

pub trait PagesApi {
    fn list_spaces(&self) -> Result<Vec<SpaceSummary>>;
    fn create_page(&self, request: CreatePageRequest) -> Result<PageSummary>;
    fn create_folder(&self, request: CreateFolderRequest) -> Result<PageSummary>;
    fn list_child_pages(&self, page: &PageRef) -> Result<Vec<PageSummary>>;
    fn list_child_content(
        &self,
        parent: &PageRef,
        parent_kind: PageContentKind,
    ) -> Result<Vec<PageSummary>>;
    fn get_page_info(&self, page: &PageRef) -> Result<PageSummary>;
    fn read_page(&self, page: &PageRef, format: BodyFormat) -> Result<PageBody>;
    fn search_pages(&self, query: &str) -> Result<Vec<PageSummary>>;
    fn search_pages_cql(&self, query: &str) -> Result<Vec<PageSummary>>;
    fn archive_page(&self, page: &PageRef) -> Result<ArchiveResult>;
    fn delete_page(&self, page: &PageRef, mode: DeleteMode) -> Result<()>;
    fn delete_folder(&self, folder: &PageRef) -> Result<()>;
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

pub trait ProfilesStore {
    fn init_profile(
        &self,
        name: &str,
        draft: ProfileDraft,
        secrets: &ProfileSecrets,
    ) -> Result<RuntimeConfig>;

    fn add_or_update_profile(
        &self,
        name: &str,
        draft: ProfileDraft,
        secrets: &ProfileSecrets,
        activate: bool,
    ) -> Result<RuntimeConfig>;

    fn use_profile(&self, name: &str) -> Result<RuntimeConfig>;

    fn remove_profile(&self, name: &str) -> Result<RuntimeConfig>;

    fn list_profiles(&self) -> Result<RuntimeConfig>;
}
