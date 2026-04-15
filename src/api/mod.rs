use reqwest::StatusCode;
use reqwest::blocking::{Client, RequestBuilder, multipart};
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use crate::config::{AuthKind, ResolvedProfile};
use crate::domain::{BodyFormat, CommentLocation, DeleteMode, MoveTarget, PageId, PageRef};
use crate::support::{ConfluenceCliError, Result};

pub trait ConfluenceApi {
    fn list_spaces(&self) -> Result<Vec<SpaceSummary>>;
    fn get_page_info(&self, page: &PageRef) -> Result<PageSummary>;
    fn read_page(&self, page: &PageRef, format: BodyFormat) -> Result<PageBody>;
    fn search_pages(&self, query: &str) -> Result<Vec<PageSummary>>;
    fn archive_page(&self, page: &PageRef) -> Result<ArchiveResult>;
    fn delete_page(&self, page: &PageRef, mode: DeleteMode) -> Result<()>;
    fn update_page(&self, request: UpdatePageRequest) -> Result<PageSummary>;
    fn move_page(&self, request: MovePageRequest) -> Result<PageSummary>;
    fn list_attachments(&self, page: &PageRef) -> Result<Vec<AttachmentSummary>>;
    fn download_attachment(&self, page: &PageRef, attachment_id: &str) -> Result<Vec<u8>>;
    fn upload_attachment(&self, request: AttachmentUploadRequest)
    -> Result<Vec<AttachmentSummary>>;
    fn delete_attachment(&self, page: &PageRef, attachment_id: &str) -> Result<()>;
    fn list_properties(&self, page: &PageRef) -> Result<Vec<ContentProperty>>;
    fn get_property(&self, page: &PageRef, key: &str) -> Result<ContentProperty>;
    fn set_property(&self, page: &PageRef, key: &str, value: Value) -> Result<ContentProperty>;
    fn delete_property(&self, page: &PageRef, key: &str) -> Result<()>;
    fn list_comments(
        &self,
        page: &PageRef,
        location: Option<CommentLocation>,
    ) -> Result<Vec<CommentSummary>>;
    fn create_comment(&self, request: CommentCreateRequest) -> Result<CommentSummary>;
    fn delete_comment(&self, comment_id: &str) -> Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpaceSummary {
    pub id: String,
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageSummary {
    pub id: u64,
    pub title: String,
    pub status: Option<String>,
    pub space_id: Option<String>,
    pub version: Option<u32>,
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
    pub file_path: PathBuf,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommentCreateRequest {
    pub page: PageRef,
    pub body_storage: String,
    pub parent_id: Option<String>,
    pub location: CommentLocation,
    pub inline_properties: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct HttpConfluenceApi {
    client: Client,
    profile: ResolvedProfile,
}

impl HttpConfluenceApi {
    pub fn new(profile: ResolvedProfile) -> Result<Self> {
        let client = Client::builder().build().map_err(|error| {
            ConfluenceCliError::Config(format!("failed to build HTTP client: {error}"))
        })?;

        Ok(Self { client, profile })
    }

    fn v1_url(&self, path: &str) -> String {
        format!(
            "{}://{}{}{}",
            self.profile.protocol, self.profile.domain, self.profile.api_path, path
        )
    }

    fn v2_url(&self, path: &str) -> String {
        let prefix = if self.profile.domain.ends_with(".atlassian.net") {
            "/wiki/api/v2"
        } else {
            "/api/v2"
        };

        format!(
            "{}://{}{}{}",
            self.profile.protocol, self.profile.domain, prefix, path
        )
    }

    fn absolute_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_owned()
        } else {
            format!(
                "{}://{}{}",
                self.profile.protocol, self.profile.domain, path
            )
        }
    }

    fn authed(&self, request: RequestBuilder) -> Result<RequestBuilder> {
        match self.profile.auth_type {
            AuthKind::Basic => {
                let identity = self
                    .profile
                    .email
                    .clone()
                    .or_else(|| self.profile.username.clone())
                    .ok_or_else(|| {
                        ConfluenceCliError::Config(
                            "basic auth requires email or username".to_owned(),
                        )
                    })?;

                let secret = self
                    .profile
                    .api_token
                    .clone()
                    .or_else(|| self.profile.password.clone())
                    .ok_or_else(|| {
                        ConfluenceCliError::Config(
                            "basic auth requires api token or password".to_owned(),
                        )
                    })?;

                Ok(request.basic_auth(identity, Some(secret)))
            }
            AuthKind::Bearer => {
                let token = self.profile.api_token.clone().ok_or_else(|| {
                    ConfluenceCliError::Config("bearer auth requires api token".to_owned())
                })?;
                Ok(request.bearer_auth(token))
            }
            AuthKind::Mtls => Err(ConfluenceCliError::NotImplemented(
                "mTLS HTTP client setup".to_owned(),
            )),
        }
    }

    fn resolve_page_id(&self, page: &PageRef) -> Result<PageId> {
        match page {
            PageRef::Id(page_id) => Ok(*page_id),
            PageRef::Url(url) => extract_page_id_from_url(url)
                .ok_or_else(|| ConfluenceCliError::InvalidPageRef(url.clone())),
        }
    }

    fn get_page_v1(&self, page_id: PageId, expand: &str) -> Result<PageV1> {
        let request = self.authed(
            self.client
                .get(self.v1_url(&format!("/content/{page_id}")))
                .query(&[("expand", expand)]),
        )?;
        Ok(request.send()?.error_for_status()?.json()?)
    }

    fn parse_next_start(&self, next: Option<&str>) -> Option<u64> {
        let value = next?;
        let (_, query) = value.split_once('?')?;
        query.split('&').find_map(|pair| {
            let (key, raw) = pair.split_once('=')?;
            if key == "start" {
                raw.parse::<u64>().ok()
            } else {
                None
            }
        })
    }

    fn encode_property_key(&self, key: &str) -> String {
        urlencoding::encode(key).into_owned()
    }
}

impl ConfluenceApi for HttpConfluenceApi {
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
        let request = self.authed(
            self.client
                .get(self.v1_url("/content/search"))
                .query(&[("cql", cql), ("limit", "25".to_owned())]),
        )?;
        let response: SearchResponse = request.send()?.error_for_status()?.json()?;
        Ok(response
            .results
            .into_iter()
            .map(PageV1::into_summary)
            .collect())
    }

    fn archive_page(&self, page: &PageRef) -> Result<ArchiveResult> {
        let page_id = self.resolve_page_id(page)?;
        let payload = serde_json::json!({
            "pages": [{ "id": page_id.get().to_string() }]
        });

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
            "body": {
                "storage": {
                    "value": request.storage_body,
                    "representation": "storage"
                }
            },
            "version": {
                "number": request.version
            }
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
        let target_parent = match request.target {
            MoveTarget::Parent(ref parent) => self.resolve_page_id(parent)?,
            MoveTarget::Before(_) | MoveTarget::After(_) => {
                return Err(ConfluenceCliError::NotImplemented(
                    "page move before/after positioning is not implemented yet".to_owned(),
                ));
            }
        };

        let current = self.get_page_v1(page_id, "version,space,body.storage")?;
        let parent = self.get_page_v1(target_parent, "space")?;

        let current_space = current.space.as_ref();
        let parent_space = parent.space.as_ref();
        if let (Some(current_space), Some(parent_space)) = (current_space, parent_space) {
            let same_id = current_space.id == parent_space.id;
            let same_key = current_space.key.is_some()
                && parent_space.key.is_some()
                && current_space.key == parent_space.key;
            if !same_id && !same_key {
                return Err(ConfluenceCliError::Config(
                    "page move across spaces is not supported".to_owned(),
                ));
            }
        }

        let version = current
            .version
            .as_ref()
            .map(|version| version.number + 1)
            .ok_or_else(|| {
                ConfluenceCliError::Config("page move requires a current version".to_owned())
            })?;

        let payload = serde_json::json!({
            "id": page_id.get().to_string(),
            "type": "page",
            "title": request.title.unwrap_or(current.title.clone()),
            "status": current.status.clone().unwrap_or_else(|| "current".to_owned()),
            "ancestors": [{ "id": target_parent.get().to_string() }],
            "body": {
                "storage": {
                    "value": current.body_value("storage").unwrap_or_default(),
                    "representation": "storage"
                }
            },
            "version": {
                "number": version
            }
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
        let mut form = multipart::Form::new().file("file", &request.file_path)?;
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

    fn list_properties(&self, page: &PageRef) -> Result<Vec<ContentProperty>> {
        let page_id = self.resolve_page_id(page)?;
        let mut start = 0;
        let mut properties = Vec::new();

        loop {
            let request = self.authed(
                self.client
                    .get(self.v1_url(&format!("/content/{page_id}/property")))
                    .query(&[("start", start), ("limit", 100_u64)]),
            )?;
            let response: PropertyListResponse = request.send()?.error_for_status()?.json()?;
            properties.extend(response.results.into_iter().map(PropertyV1::into_property));
            if let Some(next) = self.parse_next_start(response.links.next.as_deref()) {
                start = next;
            } else {
                break;
            }
        }

        Ok(properties)
    }

    fn get_property(&self, page: &PageRef, key: &str) -> Result<ContentProperty> {
        let page_id = self.resolve_page_id(page)?;
        let key = self.encode_property_key(key);
        let request = self.authed(
            self.client
                .get(self.v1_url(&format!("/content/{page_id}/property/{key}"))),
        )?;
        let response: PropertyV1 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_property())
    }

    fn set_property(&self, page: &PageRef, key: &str, value: Value) -> Result<ContentProperty> {
        let page_id = self.resolve_page_id(page)?;
        let next_version = match self.get_property(page, key) {
            Ok(existing) => existing.version + 1,
            Err(ConfluenceCliError::Http(error))
                if error.status() == Some(StatusCode::NOT_FOUND) =>
            {
                1
            }
            Err(error) => return Err(error),
        };

        let payload = serde_json::json!({
            "key": key,
            "value": value,
            "version": { "number": next_version }
        });
        let key = self.encode_property_key(key);
        let request = self.authed(
            self.client
                .put(self.v1_url(&format!("/content/{page_id}/property/{key}")))
                .header(CONTENT_TYPE, "application/json")
                .json(&payload),
        )?;
        let response: PropertyV1 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_property())
    }

    fn delete_property(&self, page: &PageRef, key: &str) -> Result<()> {
        let page_id = self.resolve_page_id(page)?;
        let key = self.encode_property_key(key);
        let request = self.authed(
            self.client
                .delete(self.v1_url(&format!("/content/{page_id}/property/{key}"))),
        )?;
        request.send()?.error_for_status()?;
        Ok(())
    }

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
            "body": {
                "storage": {
                    "value": request.body_storage,
                    "representation": "storage"
                }
            },
            "extensions": {
                "location": comment_location_name(request.location)
            }
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

    fn delete_comment(&self, comment_id: &str) -> Result<()> {
        let request = self.authed(
            self.client
                .delete(self.v1_url(&format!("/content/{comment_id}"))),
        )?;
        request.send()?.error_for_status()?;
        Ok(())
    }
}

fn comment_location_name(location: CommentLocation) -> &'static str {
    match location {
        CommentLocation::Footer => "footer",
        CommentLocation::Inline => "inline",
        CommentLocation::Resolved => "resolved",
    }
}

fn parse_comment_location(value: Option<&str>) -> Option<CommentLocation> {
    match value? {
        "footer" => Some(CommentLocation::Footer),
        "inline" => Some(CommentLocation::Inline),
        "resolved" => Some(CommentLocation::Resolved),
        _ => None,
    }
}

fn extract_page_id_from_url(url: &str) -> Option<PageId> {
    if let Some(index) = url.find("pageId=") {
        let value = &url[index + 7..];
        let digits: String = value.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        if let Ok(page_id) = digits.parse::<u64>() {
            return Some(PageId::new(page_id));
        }
    }

    if let Some(index) = url.find("/pages/") {
        let value = &url[index + 7..];
        let digits: String = value.chars().take_while(|ch| ch.is_ascii_digit()).collect();
        if let Ok(page_id) = digits.parse::<u64>() {
            return Some(PageId::new(page_id));
        }
    }

    None
}

#[derive(Debug, Deserialize)]
struct SpacesResponse {
    results: Vec<SpaceV2>,
}

#[derive(Debug, Deserialize)]
struct SpaceV2 {
    id: String,
    key: String,
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct PageV2 {
    id: String,
    title: String,
    status: Option<String>,
    #[serde(rename = "spaceId")]
    space_id: Option<String>,
    version: Option<PageVersion>,
}

impl PageV2 {
    fn into_summary(self) -> PageSummary {
        PageSummary {
            id: self.id.parse().unwrap_or_default(),
            title: self.title,
            status: self.status,
            space_id: self.space_id,
            version: self.version.map(|version| version.number),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PageVersion {
    number: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct PageV1 {
    id: String,
    title: String,
    status: Option<String>,
    version: Option<PageVersion>,
    space: Option<PageSpace>,
    body: Option<PageBodyContainer>,
}

impl PageV1 {
    fn into_summary(self) -> PageSummary {
        PageSummary {
            id: self.id.parse().unwrap_or_default(),
            title: self.title,
            status: self.status,
            space_id: self.space.map(|space| space.id),
            version: self.version.map(|version| version.number),
        }
    }

    fn body_value(&self, format: &str) -> Option<String> {
        let body = self.body.as_ref()?;
        body.section(format)
            .and_then(|section| section.value.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PageSpace {
    id: String,
    key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PageBodyContainer {
    storage: Option<PageBodySection>,
    view: Option<PageBodySection>,
    export_view: Option<PageBodySection>,
}

impl PageBodyContainer {
    fn section(&self, format: &str) -> Option<&PageBodySection> {
        match format {
            "storage" => self.storage.as_ref(),
            "view" => self.view.as_ref(),
            "export_view" => self.export_view.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PageBodySection {
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    results: Vec<PageV1>,
}

#[derive(Debug, Deserialize)]
struct ArchiveResponse {
    id: String,
    state: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ResponseLinks {
    next: Option<String>,
    download: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AttachmentListResponse {
    #[serde(default)]
    results: Vec<AttachmentV1>,
    #[serde(rename = "_links", default)]
    links: ResponseLinks,
}

#[derive(Debug, Clone, Deserialize)]
struct AttachmentV1 {
    id: String,
    title: String,
    metadata: Option<AttachmentMetadata>,
    extensions: Option<AttachmentExtensions>,
    version: Option<PageVersion>,
    #[serde(rename = "_links", default)]
    links: ResponseLinks,
}

impl AttachmentV1 {
    fn into_summary(self, api: &HttpConfluenceApi) -> AttachmentSummary {
        AttachmentSummary {
            id: self.id,
            title: self.title,
            media_type: self
                .metadata
                .and_then(|meta| meta.media_type)
                .unwrap_or_default(),
            file_size: self
                .extensions
                .and_then(|extensions| extensions.file_size)
                .unwrap_or_default(),
            version: self.version.map(|version| version.number),
            download_link: self.links.download.map(|path| api.absolute_url(&path)),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AttachmentMetadata {
    #[serde(rename = "mediaType")]
    media_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct AttachmentExtensions {
    #[serde(rename = "fileSize")]
    file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PropertyListResponse {
    #[serde(default)]
    results: Vec<PropertyV1>,
    #[serde(rename = "_links", default)]
    links: ResponseLinks,
}

#[derive(Debug, Clone, Deserialize)]
struct PropertyV1 {
    key: String,
    value: Value,
    version: Option<PageVersion>,
}

impl PropertyV1 {
    fn into_property(self) -> ContentProperty {
        ContentProperty {
            key: self.key,
            value: self.value,
            version: self.version.map(|version| version.number).unwrap_or(1),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CommentListResponse {
    #[serde(default)]
    results: Vec<CommentV1>,
    #[serde(rename = "_links", default)]
    links: ResponseLinks,
}

#[derive(Debug, Clone, Deserialize)]
struct CommentV1 {
    id: String,
    status: Option<String>,
    body: Option<PageBodyContainer>,
    history: Option<CommentHistory>,
    version: Option<PageVersion>,
    ancestors: Option<Vec<CommentAncestor>>,
    extensions: Option<CommentExtensions>,
}

impl CommentV1 {
    fn into_summary(self) -> CommentSummary {
        CommentSummary {
            id: self.id,
            status: self.status,
            body_storage: self
                .body
                .as_ref()
                .and_then(|body| body.section("storage"))
                .and_then(|section| section.value.clone())
                .unwrap_or_default(),
            location: parse_comment_location(
                self.extensions
                    .as_ref()
                    .and_then(|extensions| extensions.location.as_deref()),
            ),
            parent_id: self.ancestors.and_then(|ancestors| {
                ancestors
                    .into_iter()
                    .rev()
                    .find(|ancestor| ancestor.item_type.as_deref() == Some("comment"))
                    .map(|ancestor| ancestor.id)
            }),
            author: self
                .history
                .as_ref()
                .and_then(|history| history.created_by.as_ref())
                .and_then(|user| user.display_name.clone()),
            created_at: self.history.and_then(|history| history.created_date),
            version: self.version.map(|version| version.number),
            resolution: self.extensions.and_then(|extensions| {
                extensions
                    .resolution
                    .and_then(|resolution| resolution.status)
            }),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct CommentHistory {
    #[serde(rename = "createdBy")]
    created_by: Option<CommentUser>,
    #[serde(rename = "createdDate")]
    created_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CommentUser {
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CommentAncestor {
    id: String,
    #[serde(rename = "type")]
    item_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct CommentExtensions {
    location: Option<String>,
    resolution: Option<CommentResolution>,
}

#[derive(Debug, Clone, Deserialize)]
struct CommentResolution {
    status: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_page_id_from_query_parameter() {
        let page_id = extract_page_id_from_url(
            "https://example.atlassian.net/wiki/pages/viewpage.action?pageId=12345",
        )
        .expect("page id should be extracted");

        assert_eq!(page_id.get(), 12345);
    }

    #[test]
    fn extracts_page_id_from_pretty_url() {
        let page_id = extract_page_id_from_url(
            "https://example.atlassian.net/wiki/spaces/ENG/pages/99887/some-page",
        )
        .expect("page id should be extracted");

        assert_eq!(page_id.get(), 99887);
    }

    #[test]
    fn parses_next_start_from_paged_link() {
        let api = HttpConfluenceApi::new(ResolvedProfile {
            name: None,
            domain: "example.atlassian.net".to_owned(),
            protocol: "https".to_owned(),
            api_path: "/wiki/rest/api".to_owned(),
            auth_type: AuthKind::Bearer,
            email: None,
            username: None,
            api_token: Some("token".to_owned()),
            password: None,
            read_only: false,
        })
        .expect("api should construct");

        assert_eq!(
            api.parse_next_start(Some("/rest/api/content/123/property?start=5&limit=25")),
            Some(5)
        );
        assert_eq!(
            api.parse_next_start(Some("/rest/api/content/123/property?limit=25")),
            None
        );
    }
}
