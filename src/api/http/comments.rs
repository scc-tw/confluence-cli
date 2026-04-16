use reqwest::header::CONTENT_TYPE;

use crate::application::models::{CommentCreateRequest, CommentSummary};
use crate::application::ports::CommentsApi;
use crate::domain::{CommentLocation, PageRef};
use crate::support::Result;

use super::dto::{CommentListResponse, CommentV1, InlineCommentV2};
use super::{comment_location_name, HttpConfluenceApi};

impl CommentsApi for HttpConfluenceApi {
    fn list_comments(
        &self,
        page: &PageRef,
        location: Option<CommentLocation>,
    ) -> Result<Vec<CommentSummary>> {
        let page_id = self.resolve_page_id(page)?;
        let mut start = 0;
        let mut comments = Vec::new();
        loop {
            let mut query = vec![
                (
                    "expand",
                    "body.storage,history,version,ancestors".to_owned(),
                ),
                ("start", start.to_string()),
                ("limit", "100".to_owned()),
            ];
            if let Some(location) = location {
                query.push(("location", comment_location_name(location).to_owned()));
            }
            let request = self.authed(
                self.client
                    .get(self.v1_url(&format!("/content/{page_id}/child/comment")))
                    .query(&query),
            )?;
            let response: CommentListResponse = request.send()?.error_for_status()?.json()?;
            comments.extend(response.results.into_iter().map(CommentV1::into_summary));
            if let Some(next) = self.parse_next_start(response.links.next.as_deref()) {
                start = next;
            } else {
                break;
            }
        }
        Ok(comments)
    }

    fn create_comment(&self, request: CommentCreateRequest) -> Result<CommentSummary> {
        let page_id = self.resolve_page_id(&request.page)?;
        let mut payload = serde_json::json!({
            "type": "comment",
            "container": { "id": page_id.get().to_string(), "type": "page" },
            "body": { "storage": { "value": request.body_storage, "representation": "storage" } },
            "extensions": { "location": comment_location_name(request.location) }
        });
        if let Some(parent_id) = request.parent_id {
            payload["ancestors"] = serde_json::json!([{ "id": parent_id }]);
        }
        if let Some(inline_properties) = request.inline_properties {
            payload["extensions"]["inlineProperties"] = inline_properties;
        }
        let request = self.authed(
            self.client
                .post(self.v1_url("/content"))
                .header(CONTENT_TYPE, "application/json")
                .json(&payload),
        )?;
        let response: CommentV1 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_summary())
    }

    fn get_comment(&self, comment_id: &str) -> Result<CommentSummary> {
        let request = self.authed(
            self.client
                .get(self.v1_url(&format!("/content/{comment_id}")))
                .query(&[("expand", "body.storage,history,version,ancestors")]),
        )?;
        let response: CommentV1 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_summary())
    }

    fn set_inline_comment_resolution(
        &self,
        comment_id: &str,
        resolved: bool,
    ) -> Result<CommentSummary> {
        let request = self.authed(
            self.client
                .get(self.v2_url(&format!("/inline-comments/{comment_id}")))
                .query(&[("body-format", "storage")]),
        )?;
        let current: InlineCommentV2 = request.send()?.error_for_status()?.json()?;
        let version = current.version.number + 1;
        let payload = serde_json::json!({
            "version": { "number": version },
            "resolved": resolved
        });
        let request = self.authed(
            self.client
                .put(self.v2_url(&format!("/inline-comments/{comment_id}")))
                .header(CONTENT_TYPE, "application/json")
                .json(&payload),
        )?;
        let response: InlineCommentV2 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_summary())
    }

    fn delete_comment(&self, comment_id: &str) -> Result<()> {
        let request = self.authed(
            self.client
                .delete(self.v1_url(&format!("/content/{comment_id}"))),
        )?;
        request.send()?.error_for_status()?;
        Ok(())
    }
}
