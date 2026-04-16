use reqwest::header::CONTENT_TYPE;

use crate::application::models::{
    ArchiveResult, CreatePageRequest, MovePageRequest, PageBody, PageSummary, SpaceSummary,
    UpdatePageRequest,
};
use crate::application::ports::PagesApi;
use crate::domain::{BodyFormat, DeleteMode, MoveTarget, PageRef};
use crate::support::{ConfluenceCliError, Result};

use super::dto::{ArchiveResponse, PageChildrenResponse, PageV1, PageV2, SpacesResponse};
use super::{HttpConfluenceApi, validate_same_space};

impl PagesApi for HttpConfluenceApi {
    fn list_spaces(&self) -> Result<Vec<SpaceSummary>> {
        let request = self.authed(self.client.get(self.v2_url("/spaces")))?;
        let response: SpacesResponse = request.send()?.error_for_status()?.json()?;
        Ok(response
            .results
            .into_iter()
            .map(|space| SpaceSummary {
                id: space.id,
                key: space.key,
                name: space.name,
            })
            .collect())
    }

    fn create_page(&self, request: CreatePageRequest) -> Result<PageSummary> {
        let mut payload = serde_json::json!({
            "spaceId": request.space_id,
            "title": request.title,
            "status": "current",
            "body": { "representation": "storage", "value": request.storage_body }
        });

        if let Some(parent_id) = request.parent_id {
            payload["parentId"] = serde_json::json!(parent_id.get().to_string());
        }

        let request = self.authed(
            self.client
                .post(self.v2_url("/pages"))
                .header(CONTENT_TYPE, "application/json")
                .json(&payload),
        )?;
        let response: PageV2 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_summary())
    }

    fn list_child_pages(&self, page: &PageRef) -> Result<Vec<PageSummary>> {
        let page_id = self.resolve_page_id(page)?;
        let mut next_url = Some(self.v2_url(&format!("/pages/{page_id}/children?limit=100")));
        let mut children = Vec::new();

        while let Some(url) = next_url.take() {
            let request = self.authed(self.client.get(url))?;
            let response: PageChildrenResponse = request.send()?.error_for_status()?.json()?;
            children.extend(response.results.into_iter().map(PageV2::into_summary));
            next_url = response
                .links
                .and_then(|links| links.next)
                .map(|next| self.absolute_url(&next));
        }

        Ok(children)
    }

    fn get_page_info(&self, page: &PageRef) -> Result<PageSummary> {
        let page_id = self.resolve_page_id(page)?;
        let request = self.authed(self.client.get(self.v2_url(&format!("/pages/{page_id}"))))?;
        let response: PageV2 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_summary())
    }

    fn read_page(&self, page: &PageRef, format: BodyFormat) -> Result<PageBody> {
        let page_id = self.resolve_page_id(page)?;
        let format_name = match format {
            BodyFormat::Storage => "storage",
            BodyFormat::Markdown => {
                return Err(ConfluenceCliError::NotImplemented(
                    "server-side markdown reads are not supported; use local conversion".to_owned(),
                ));
            }
            BodyFormat::Html => "view",
            BodyFormat::Text => "export_view",
        };

        let response = self.get_page_v1(page_id, &format!("body.{format_name},version,space"))?;
        Ok(PageBody {
            page: response.clone().into_summary(),
            format,
            content: response.body_value(format_name).unwrap_or_default(),
        })
    }

    fn search_pages(&self, query: &str) -> Result<Vec<PageSummary>> {
        let cql = format!("type=page and text~\"{}\"", query.replace('"', "\\\""));
        self.search_pages_cql(&cql)
    }

    fn search_pages_cql(&self, query: &str) -> Result<Vec<PageSummary>> {
        let mut next_url = Some(format!(
            "{}?cql={}&limit=25",
            self.v1_url("/content/search"),
            urlencoding::encode(query)
        ));
        let mut pages = Vec::new();

        while let Some(url) = next_url.take() {
            let request = self.authed(self.client.get(url))?;
            let response: super::dto::SearchResponse =
                request.send()?.error_for_status()?.json()?;
            pages.extend(response.results.into_iter().map(PageV1::into_summary));
            next_url = response.links.next.map(|next| self.absolute_url(&next));
        }

        Ok(pages)
    }

    fn archive_page(&self, page: &PageRef) -> Result<ArchiveResult> {
        let page_id = self.resolve_page_id(page)?;
        let payload = serde_json::json!({ "pages": [{ "id": page_id.get().to_string() }] });
        let request = self.authed(
            self.client
                .post(self.v1_url("/content/archive"))
                .header(CONTENT_TYPE, "application/json")
                .json(&payload),
        )?;
        let response: ArchiveResponse = request.send()?.error_for_status()?.json()?;
        Ok(ArchiveResult {
            task_id: response.id,
            state: response.state,
        })
    }

    fn delete_page(&self, page: &PageRef, mode: DeleteMode) -> Result<()> {
        let page_id = self.resolve_page_id(page)?;
        let mut request = self
            .client
            .delete(self.v2_url(&format!("/pages/{page_id}")));
        if matches!(mode, DeleteMode::Purge) {
            request = request.query(&[("purge", "true")]);
        }
        let request = self.authed(request)?;
        request.send()?.error_for_status()?;
        Ok(())
    }

    fn update_page(&self, request: UpdatePageRequest) -> Result<PageSummary> {
        let page_id = self.resolve_page_id(&request.page)?;
        let payload = serde_json::json!({
            "id": page_id.get().to_string(),
            "type": "page",
            "title": request.title,
            "status": "current",
            "body": { "storage": { "value": request.storage_body, "representation": "storage" } },
            "version": { "number": request.version }
        });
        let request = self.authed(
            self.client
                .put(self.v1_url(&format!("/content/{page_id}")))
                .header(CONTENT_TYPE, "application/json")
                .json(&payload),
        )?;
        let response: PageV1 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_summary())
    }

    fn move_page(&self, request: MovePageRequest) -> Result<PageSummary> {
        let page_id = self.resolve_page_id(&request.page)?;
        let current = self.get_page_v1(page_id, "version,space,body.storage")?;

        match request.target {
            MoveTarget::Parent(ref parent) => {
                let target_parent = self.resolve_page_id(parent)?;
                let parent = self.get_page_v1(target_parent, "space")?;
                validate_same_space(&current, &parent)?;
                let version = current
                    .version
                    .as_ref()
                    .map(|version| version.number + 1)
                    .ok_or_else(|| {
                        ConfluenceCliError::Config(
                            "page move requires a current version".to_owned(),
                        )
                    })?;
                let payload = serde_json::json!({
                    "id": page_id.get().to_string(),
                    "type": "page",
                    "title": request.title.unwrap_or(current.title.clone()),
                    "status": current.status.clone().unwrap_or_else(|| "current".to_owned()),
                    "ancestors": [{ "id": target_parent.get().to_string() }],
                    "body": { "storage": { "value": current.body_value("storage").unwrap_or_default(), "representation": "storage" } },
                    "version": { "number": version }
                });
                let request = self.authed(
                    self.client
                        .put(self.v1_url(&format!("/content/{page_id}")))
                        .header(CONTENT_TYPE, "application/json")
                        .json(&payload),
                )?;
                let response: PageV1 = request.send()?.error_for_status()?.json()?;
                Ok(response.into_summary())
            }
            MoveTarget::Before(ref target) | MoveTarget::After(ref target) => {
                if request.title.is_some() {
                    return Err(ConfluenceCliError::NotImplemented(
                        "renaming during before/after move is not implemented yet".to_owned(),
                    ));
                }
                let target_id = self.resolve_page_id(target)?;
                let target_page = self.get_page_v1(target_id, "space,ancestors")?;
                validate_same_space(&current, &target_page)?;
                if target_page.ancestors.is_empty() {
                    return Err(ConfluenceCliError::Config(
                        "before/after move against a top-level target is blocked".to_owned(),
                    ));
                }
                let position = match request.target {
                    MoveTarget::Before(_) => "before",
                    MoveTarget::After(_) => "after",
                    MoveTarget::Parent(_) => unreachable!("handled above"),
                };
                let request = self.authed(self.client.put(
                    self.v1_url(&format!("/content/{page_id}/move/{position}/{target_id}")),
                ))?;
                let response: PageV1 = request.send()?.error_for_status()?.json()?;
                Ok(response.into_summary())
            }
        }
    }
}
