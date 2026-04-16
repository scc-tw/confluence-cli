use reqwest::blocking::multipart;

use crate::application::models::{AttachmentSummary, AttachmentUploadRequest};
use crate::application::ports::AttachmentsApi;
use crate::domain::PageRef;
use crate::support::{ConfluenceCliError, Result};

use super::dto::{AttachmentListResponse, AttachmentV1};
use super::HttpConfluenceApi;

impl AttachmentsApi for HttpConfluenceApi {
    fn list_attachments(&self, page: &PageRef) -> Result<Vec<AttachmentSummary>> {
        let page_id = self.resolve_page_id(page)?;
        let mut start = 0;
        let mut attachments = Vec::new();
        loop {
            let request = self.authed(
                self.client
                    .get(self.v1_url(&format!("/content/{page_id}/child/attachment")))
                    .query(&[("start", start), ("limit", 100_u64)]),
            )?;
            let response: AttachmentListResponse = request.send()?.error_for_status()?.json()?;
            attachments.extend(
                response
                    .results
                    .into_iter()
                    .map(|item| item.into_summary(self)),
            );
            if let Some(next) = self.parse_next_start(response.links.next.as_deref()) {
                start = next;
            } else {
                break;
            }
        }
        Ok(attachments)
    }

    fn download_attachment(&self, page: &PageRef, attachment_id: &str) -> Result<Vec<u8>> {
        let page_id = self.resolve_page_id(page)?;
        let request = self.authed(self.client.get(self.v1_url(&format!(
            "/content/{page_id}/child/attachment/{attachment_id}"
        ))))?;
        let attachment: AttachmentV1 = request.send()?.error_for_status()?.json()?;
        let download_url = attachment
            .links
            .download
            .as_deref()
            .map(|path| self.absolute_url(path))
            .ok_or_else(|| {
                ConfluenceCliError::Config(format!(
                    "attachment '{attachment_id}' does not expose a download URL"
                ))
            })?;
        let request = self.authed(self.client.get(download_url))?;
        Ok(request.send()?.error_for_status()?.bytes()?.to_vec())
    }

    fn upload_attachment(
        &self,
        request: AttachmentUploadRequest,
    ) -> Result<Vec<AttachmentSummary>> {
        let page_id = self.resolve_page_id(&request.page)?;
        let part = multipart::Part::bytes(request.content).file_name(request.file_name);
        let mut form = multipart::Form::new().part("file", part);
        if let Some(comment) = &request.comment {
            form = form.text("comment", comment.clone());
        }
        if request.minor_edit {
            form = form.text("minorEdit", "true");
        }
        let method_request = if request.replace {
            self.client
                .put(self.v1_url(&format!("/content/{page_id}/child/attachment")))
        } else {
            self.client
                .post(self.v1_url(&format!("/content/{page_id}/child/attachment")))
        };
        let request = self.authed(
            method_request
                .header("X-Atlassian-Token", "nocheck")
                .multipart(form),
        )?;
        let response: AttachmentListResponse = request.send()?.error_for_status()?.json()?;
        Ok(response
            .results
            .into_iter()
            .map(|item| item.into_summary(self))
            .collect())
    }

    fn delete_attachment(&self, page: &PageRef, attachment_id: &str) -> Result<()> {
        let page_id = self.resolve_page_id(page)?;
        let request = self.authed(self.client.delete(self.v1_url(&format!(
            "/content/{page_id}/child/attachment/{attachment_id}"
        ))))?;
        request.send()?.error_for_status()?;
        Ok(())
    }
}
