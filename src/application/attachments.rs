use crate::domain::PageRef;
use crate::support::{ConfluenceCliError, Result};

use super::models::AttachmentSummary;
use super::ports::AttachmentsApi;

pub fn attachment_list<A: AttachmentsApi>(
    api: &A,
    page: &PageRef,
) -> Result<Vec<AttachmentSummary>> {
    api.list_attachments(page)
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
