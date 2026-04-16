pub mod api;
pub mod app;
mod application;
mod cli;
pub mod config;
mod convert;
pub mod domain;
mod infrastructure;
mod profile;
mod render;
mod secret;
mod support;

pub use application::models::{
    ArchiveResult, AttachmentSummary, AttachmentUploadRequest, CommentCreateRequest,
    CommentSummary, ContentProperty, CreatePageRequest, MovePageRequest, PageBody, PageSummary,
    SpaceSummary, UpdatePageRequest,
};
pub use application::runtime::{ResolveOptions, ResolvedProfile, RuntimeConfig};
pub use config::ConfigSecretBackend;
pub use profile::AuthKind;
pub use secret::SecretBackend;
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
