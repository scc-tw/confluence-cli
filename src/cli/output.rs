use serde::Serialize;

use crate::application::models::{AttachmentSummary, CommentSummary, ContentProperty, PageSummary};
use crate::application::pages::PageExportResult;
use crate::application::runtime::RuntimeConfig;
use crate::render;
use crate::support::Result;

use super::OutputFormat;

pub(super) fn print_profiles_human(runtime: &RuntimeConfig) {
    println!("{}", render::render_profiles_human(runtime));
}

pub(super) fn print_profiles_json(runtime: &RuntimeConfig) -> Result<()> {
    #[derive(serde::Serialize)]
    struct ProfileEntry<'a> {
        name: &'a str,
        active: bool,
    }

    let entries: Vec<_> = runtime
        .profiles
        .profiles
        .iter()
        .map(|name| ProfileEntry {
            name: name.as_str(),
            active: runtime.profiles.active_profile.as_deref() == Some(name.as_str()),
        })
        .collect();

    println!("{}", render::render_json(&entries)?);
    Ok(())
}

pub(super) fn print_json_or_human<T, F>(output: OutputFormat, value: &T, human: F) -> Result<()>
where
    T: Serialize,
    F: FnOnce(&T),
{
    match output {
        OutputFormat::Human => human(value),
        OutputFormat::Json => println!("{}", render::render_json(value)?),
    }
    Ok(())
}

pub(super) fn print_attachments_human(attachments: &[AttachmentSummary]) {
    println!("{}", render::render_attachments_human(attachments));
}

pub(super) fn print_text(text: &str) {
    println!("{text}");
}

pub(super) fn print_page_action(label: &str, summary: &PageSummary) {
    println!("{label} {} [{}]", summary.title, summary.id);
}

pub(super) fn print_archive_task(task_id: &str) {
    println!("Archive task queued: {task_id}");
}

pub(super) fn print_simple_ack(message: &str) {
    println!("{message}");
}

pub(super) fn print_comment_action(label: &str, comment: &CommentSummary) {
    println!("{label} {}", comment.id);
}

pub(super) fn print_page_summary_human(summary: &PageSummary) {
    println!("{}", render::render_page_summary_human(summary));
}

pub(super) fn print_page_summaries_human(summaries: &[PageSummary], empty_message: &str) {
    println!(
        "{}",
        render::render_page_summaries_human(summaries, empty_message)
    );
}

pub(super) fn print_export_result_human(result: &PageExportResult) {
    println!("{}", render::render_export_result_human(result));
}

pub(super) fn print_paths_human(paths: &[std::path::PathBuf], empty_message: &str) {
    println!("{}", render::render_paths_human(paths, empty_message));
}

pub(super) fn print_property_human(property: &ContentProperty) {
    println!("{}", render::render_property_human(property));
}

pub(super) fn print_properties_human(properties: &[ContentProperty]) {
    println!("{}", render::render_properties_human(properties));
}

pub(super) fn print_comments_human(comments: &[CommentSummary]) {
    println!("{}", render::render_comments_human(comments));
}
