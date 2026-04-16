use serde_json::Value;

use crate::domain::{CommentLocation, PageRef};
use crate::support::{ConfluenceCliError, Result};

use super::models::{CommentCreateRequest, CommentSummary};
use super::ports::CommentsApi;

pub fn comment_list<A: CommentsApi>(
    api: &A,
    page: &PageRef,
    location: Option<CommentLocation>,
) -> Result<Vec<CommentSummary>> {
    api.list_comments(page, location)
}

pub fn comment_info<A: CommentsApi>(api: &A, comment_id: &str) -> Result<CommentSummary> {
    if comment_id.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "comment info requires a non-empty comment id".to_owned(),
        ));
    }
    api.get_comment(comment_id)
}

pub fn comment_create<A: CommentsApi>(
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

pub fn comment_delete<A: CommentsApi>(api: &A, comment_id: &str) -> Result<()> {
    if comment_id.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "comment delete requires a non-empty comment id".to_owned(),
        ));
    }
    api.delete_comment(comment_id)
}

pub fn comment_resolve<A: CommentsApi>(api: &A, comment_id: &str) -> Result<CommentSummary> {
    if comment_id.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "comment resolve requires a non-empty comment id".to_owned(),
        ));
    }
    api.set_inline_comment_resolution(comment_id, true)
}

pub fn comment_reopen<A: CommentsApi>(api: &A, comment_id: &str) -> Result<CommentSummary> {
    if comment_id.trim().is_empty() {
        return Err(ConfluenceCliError::Config(
            "comment reopen requires a non-empty comment id".to_owned(),
        ));
    }
    api.set_inline_comment_resolution(comment_id, false)
}
