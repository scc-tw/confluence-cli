mod api;
mod app;
mod application;
mod cli;
mod config;
mod convert;
pub mod domain;
mod infrastructure;
mod profile;
mod render;
mod secret;
mod support;

pub use api::contracts::{AttachmentsApi, CommentsApi, PagesApi, PropertiesApi};
pub use api::{HttpApiConfig, HttpConfluenceApi};
pub use app::{
    attachment_delete, attachment_list, comment_create, comment_delete, comment_info, comment_list,
    comment_reopen, comment_resolve, list_spaces, page_archive, page_children, page_create,
    page_delete, page_info, page_move, page_patch, page_read, page_search, page_search_cql,
    page_update, property_delete, property_get, property_list, property_set,
};
pub use application::models::{
    ArchiveResult, AttachmentSummary, AttachmentUploadRequest, CommentCreateRequest,
    CommentSummary, ContentProperty, CreatePageRequest, MovePageRequest, PageBody, PageSummary,
    SpaceSummary, UpdatePageRequest,
};
pub use application::pages::PageExportResult;
pub use application::runtime::{ResolveOptions, ResolvedProfile, RuntimeConfig};
pub use config::ConfigSecretBackend;
pub use infrastructure::content_io::{
    download_attachments_to_dir, export_page_to_dir, upload_attachment_from_path,
};
pub use profile::AuthKind;
pub use support::ConfluenceCliError;

pub fn run() -> support::Result<()> {
    cli::run()
}

pub fn run_from<I, T>(args: I) -> support::Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    cli::run_from(args)
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_runs_helpfully() {
        assert!(super::run_from(["confluence", "--help"]).is_ok());
    }
}
