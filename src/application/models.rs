use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::domain::{BodyFormat, CommentLocation, MoveTarget, PageId, PageRef};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceSummary {
    pub id: String,
    pub key: String,
    pub name: String,
    pub homepage_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PageContentKind {
    Page,
    Folder,
}

impl Default for PageContentKind {
    fn default() -> Self {
        Self::Page
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSummary {
    pub id: u64,
    pub title: String,
    pub status: Option<String>,
    pub space_id: Option<String>,
    pub version: Option<u32>,
    #[serde(default)]
    pub content_kind: PageContentKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageBody {
    pub page: PageSummary,
    pub format: BodyFormat,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveResult {
    pub task_id: String,
    pub state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePageRequest {
    pub title: String,
    pub storage_body: String,
    pub space_id: String,
    pub parent_id: Option<PageId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateFolderRequest {
    pub title: String,
    pub space_id: String,
    pub parent_id: Option<PageId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePageRequest {
    pub page: PageRef,
    pub title: String,
    pub storage_body: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovePageRequest {
    pub page: PageRef,
    pub target: MoveTarget,
    pub title: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentSummary {
    pub id: String,
    pub title: String,
    pub media_type: String,
    pub file_size: u64,
    pub version: Option<u32>,
    pub download_link: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentUploadRequest {
    pub page: PageRef,
    pub file_name: String,
    pub content: Vec<u8>,
    pub comment: Option<String>,
    pub minor_edit: bool,
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContentProperty {
    pub key: String,
    pub value: Value,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommentSummary {
    pub id: String,
    pub status: Option<String>,
    pub body_storage: String,
    pub location: Option<CommentLocation>,
    pub parent_id: Option<String>,
    pub author: Option<String>,
    pub created_at: Option<String>,
    pub version: Option<u32>,
    pub resolution: Option<String>,
    pub inline_properties: Option<Value>,
    pub inline_marker_ref: Option<String>,
    pub inline_original_selection: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentCreateRequest {
    pub page: PageRef,
    pub body_storage: String,
    pub parent_id: Option<String>,
    pub location: CommentLocation,
    pub inline_properties: Option<Value>,
}
