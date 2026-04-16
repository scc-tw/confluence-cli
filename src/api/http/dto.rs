use serde::Deserialize;
use serde_json::Value;

use crate::application::models::{AttachmentSummary, CommentSummary, ContentProperty, PageSummary};
use crate::domain::CommentLocation;

use super::{parse_comment_location, HttpConfluenceApi};

#[derive(Debug, Deserialize)]
pub(super) struct SpacesResponse {
    pub(super) results: Vec<SpaceV2>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SpaceV2 {
    pub(super) id: String,
    pub(super) key: String,
    pub(super) name: String,
    #[serde(rename = "homepageId")]
    pub(super) homepage_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PageV2 {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) status: Option<String>,
    #[serde(rename = "spaceId")]
    pub(super) space_id: Option<String>,
    pub(super) body: Option<PageBodyContainer>,
    pub(super) version: Option<PageVersion>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PageChildrenResponse {
    pub(super) results: Vec<PageV2>,
    #[serde(rename = "_links")]
    pub(super) links: Option<V2Links>,
}

#[derive(Debug, Deserialize)]
pub(super) struct V2Links {
    pub(super) next: Option<String>,
}

impl PageV2 {
    pub(super) fn into_summary(self) -> PageSummary {
        PageSummary {
            id: self.id.parse().unwrap_or_default(),
            title: self.title,
            status: self.status,
            space_id: self.space_id,
            version: self.version.map(|version| version.number),
        }
    }

    pub(super) fn body_value(&self, format: &str) -> Option<String> {
        let body = self.body.as_ref()?;
        body.section(format)
            .and_then(|section| section.value.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PageVersion {
    pub(super) number: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PageV1 {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) status: Option<String>,
    pub(super) version: Option<PageVersion>,
    pub(super) space: Option<PageSpace>,
    pub(super) body: Option<PageBodyContainer>,
    #[serde(default)]
    pub(super) ancestors: Vec<PageAncestor>,
}

impl PageV1 {
    pub(super) fn into_summary(self) -> PageSummary {
        PageSummary {
            id: self.id.parse().unwrap_or_default(),
            title: self.title,
            status: self.status,
            space_id: self.space.map(|space| space.id),
            version: self.version.map(|version| version.number),
        }
    }

    pub(super) fn body_value(&self, format: &str) -> Option<String> {
        let body = self.body.as_ref()?;
        body.section(format)
            .and_then(|section| section.value.clone())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PageSpace {
    pub(super) id: String,
    pub(super) key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PageAncestor {
    #[allow(dead_code)]
    pub(super) id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PageBodyContainer {
    pub(super) storage: Option<PageBodySection>,
    pub(super) view: Option<PageBodySection>,
    pub(super) export_view: Option<PageBodySection>,
    pub(super) atlas_doc_format: Option<PageBodySection>,
}

impl PageBodyContainer {
    pub(super) fn section(&self, format: &str) -> Option<&PageBodySection> {
        match format {
            "storage" => self.storage.as_ref(),
            "view" => self.view.as_ref(),
            "export_view" => self.export_view.as_ref(),
            "atlas_doc_format" => self.atlas_doc_format.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PageBodySection {
    pub(super) value: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchResponse {
    #[serde(default)]
    pub(super) results: Vec<PageV1>,
    #[serde(rename = "_links", default)]
    pub(super) links: ResponseLinks,
}

#[derive(Debug, Deserialize)]
pub(super) struct ArchiveResponse {
    pub(super) id: String,
    pub(super) state: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct ResponseLinks {
    pub(super) next: Option<String>,
    pub(super) download: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AttachmentListResponse {
    #[serde(default)]
    pub(super) results: Vec<AttachmentV1>,
    #[serde(rename = "_links", default)]
    pub(super) links: ResponseLinks,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct AttachmentV1 {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) metadata: Option<AttachmentMetadata>,
    pub(super) extensions: Option<AttachmentExtensions>,
    pub(super) version: Option<PageVersion>,
    #[serde(rename = "_links", default)]
    pub(super) links: ResponseLinks,
}

impl AttachmentV1 {
    pub(super) fn into_summary(self, api: &HttpConfluenceApi) -> AttachmentSummary {
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
pub(super) struct AttachmentMetadata {
    #[serde(rename = "mediaType")]
    pub(super) media_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct AttachmentExtensions {
    #[serde(rename = "fileSize")]
    pub(super) file_size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(super) struct PropertyListResponse {
    #[serde(default)]
    pub(super) results: Vec<PropertyV1>,
    #[serde(rename = "_links", default)]
    pub(super) links: ResponseLinks,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PropertyV1 {
    pub(super) key: String,
    pub(super) value: Value,
    pub(super) version: Option<PageVersion>,
}

impl PropertyV1 {
    pub(super) fn into_property(self) -> ContentProperty {
        ContentProperty {
            key: self.key,
            value: self.value,
            version: self.version.map(|version| version.number).unwrap_or(1),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct CommentListResponse {
    #[serde(default)]
    pub(super) results: Vec<CommentV1>,
    #[serde(rename = "_links", default)]
    pub(super) links: ResponseLinks,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CommentV1 {
    pub(super) id: String,
    pub(super) status: Option<String>,
    pub(super) body: Option<PageBodyContainer>,
    pub(super) history: Option<CommentHistory>,
    pub(super) version: Option<PageVersion>,
    pub(super) ancestors: Option<Vec<CommentAncestor>>,
    pub(super) extensions: Option<CommentExtensions>,
}

impl CommentV1 {
    pub(super) fn into_summary(self) -> CommentSummary {
        let resolution = self.extensions.as_ref().and_then(|extensions| {
            extensions
                .resolution
                .as_ref()
                .and_then(|resolution| resolution.status.clone())
        });
        let inline_properties = self
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.inline_properties.clone());

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
            resolution,
            inline_properties,
            inline_marker_ref: None,
            inline_original_selection: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CommentHistory {
    #[serde(rename = "createdBy")]
    pub(super) created_by: Option<CommentUser>,
    #[serde(rename = "createdDate")]
    pub(super) created_date: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CommentUser {
    #[serde(rename = "displayName")]
    pub(super) display_name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CommentAncestor {
    pub(super) id: String,
    #[serde(rename = "type")]
    pub(super) item_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CommentExtensions {
    pub(super) location: Option<String>,
    pub(super) resolution: Option<CommentResolution>,
    #[serde(rename = "inlineProperties")]
    pub(super) inline_properties: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct CommentResolution {
    pub(super) status: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct InlineCommentV2 {
    pub(super) id: String,
    pub(super) status: Option<String>,
    pub(super) version: PageVersion,
    #[serde(rename = "resolutionStatus")]
    pub(super) resolution_status: Option<String>,
    pub(super) body: Option<InlineCommentBody>,
    pub(super) properties: Option<InlineCommentProperties>,
}

impl InlineCommentV2 {
    pub(super) fn into_summary(self) -> CommentSummary {
        CommentSummary {
            id: self.id,
            status: self.status,
            body_storage: self
                .body
                .and_then(|body| body.storage)
                .and_then(|body| body.value)
                .unwrap_or_default(),
            location: Some(CommentLocation::Inline),
            parent_id: None,
            author: None,
            created_at: None,
            version: Some(self.version.number),
            resolution: self.resolution_status,
            inline_properties: None,
            inline_marker_ref: self
                .properties
                .as_ref()
                .and_then(|properties| properties.inline_marker_ref.clone()),
            inline_original_selection: self
                .properties
                .and_then(|properties| properties.inline_original_selection),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct InlineCommentBody {
    pub(super) storage: Option<InlineCommentBodyStorage>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct InlineCommentBodyStorage {
    pub(super) value: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct InlineCommentProperties {
    #[serde(rename = "inlineMarkerRef")]
    pub(super) inline_marker_ref: Option<String>,
    #[serde(rename = "inlineOriginalSelection")]
    pub(super) inline_original_selection: Option<String>,
}
