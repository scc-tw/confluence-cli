use serde::Serialize;

use crate::application::models::{AttachmentSummary, CommentSummary, ContentProperty, PageSummary};
use crate::application::pages::PageExportResult;
use crate::application::runtime::RuntimeConfig;
use crate::support::Result;

pub fn render_json<T: Serialize>(value: &T) -> Result<String> {
    Ok(serde_json::to_string_pretty(value)?)
}

pub fn render_profiles_human(runtime: &RuntimeConfig) -> String {
    if runtime.profiles.profiles.is_empty() {
        return "No profiles configured.".to_owned();
    }

    runtime
        .profiles
        .profiles
        .iter()
        .map(|name| {
            let marker = if runtime.profiles.active_profile.as_deref() == Some(name.as_str()) {
                "*"
            } else {
                " "
            };
            format!("{marker} {name}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_page_summary_human(summary: &PageSummary) -> String {
    let mut lines = vec![format!("{} [{}]", summary.title, summary.id)];
    if let Some(status) = &summary.status {
        lines.push(format!("status: {status}"));
    }
    if let Some(version) = summary.version {
        lines.push(format!("version: {version}"));
    }
    lines.join("\n")
}

pub fn render_page_summaries_human(summaries: &[PageSummary], empty_message: &str) -> String {
    if summaries.is_empty() {
        empty_message.to_owned()
    } else {
        summaries
            .iter()
            .map(|summary| format!("- {} [{}]", summary.title, summary.id))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn render_export_result_human(result: &PageExportResult) -> String {
    [
        format!("Exported to {}", result.directory.display()),
        format!("content: {}", result.content_path.display()),
        format!("attachments: {}", result.attachment_count),
    ]
    .join("\n")
}

pub fn render_paths_human(paths: &[std::path::PathBuf], empty_message: &str) -> String {
    if paths.is_empty() {
        empty_message.to_owned()
    } else {
        paths
            .iter()
            .map(|path| format!("- {}", path.display()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn render_attachments_human(attachments: &[AttachmentSummary]) -> String {
    if attachments.is_empty() {
        return "No attachments found.".to_owned();
    }

    attachments
        .iter()
        .map(|attachment| format!("- {} [{}]", attachment.title, attachment.id))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_property_human(property: &ContentProperty) -> String {
    [
        format!("key: {}", property.key),
        format!("version: {}", property.version),
        format!(
            "value: {}",
            serde_json::to_string_pretty(&property.value)
                .unwrap_or_else(|_| property.value.to_string())
        ),
    ]
    .join("\n")
}

pub fn render_properties_human(properties: &[ContentProperty]) -> String {
    if properties.is_empty() {
        return "No properties found.".to_owned();
    }

    properties
        .iter()
        .map(|property| format!("- {} (v{})", property.key, property.version))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_comments_human(comments: &[CommentSummary]) -> String {
    if comments.is_empty() {
        return "No comments found.".to_owned();
    }

    comments
        .iter()
        .map(|comment| {
            let mut lines = vec![format!("- {}", comment.id)];
            if let Some(author) = &comment.author {
                lines.push(format!("  author: {author}"));
            }
            if let Some(location) = comment.location {
                lines.push(format!("  location: {location:?}"));
            }
            if let Some(resolution) = &comment.resolution {
                lines.push(format!("  resolution: {resolution}"));
            }
            if let Some(marker_ref) = &comment.inline_marker_ref {
                lines.push(format!("  marker ref: {marker_ref}"));
            }
            if let Some(selection) = &comment.inline_original_selection {
                lines.push(format!("  original selection: {selection}"));
            }
            lines.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
