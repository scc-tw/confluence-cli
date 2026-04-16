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
pub use application::vfs::{
    DirEntry, NodeHandle, NodeKind, NodeStat, PageNode, SpaceNode, VirtualFileSystem,
};
pub use application::pages::PageExportResult;
pub use application::runtime::{ResolveOptions, ResolvedProfile, RuntimeConfig};
pub use support::Result;
pub use cli::{GlobalArgs, OutputFormat};
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

#[doc(hidden)]
pub mod test_support {
    use std::io::{BufRead, Write};
    use std::path::Path;

    use crate::application::runtime::RuntimeContext;
    use crate::secret::MemorySecretStore;
    use crate::support::Result;
    use crate::{GlobalArgs, OutputFormat, ResolveOptions};

    #[derive(Debug, Default)]
    pub struct CliHarness {
        store: MemorySecretStore,
    }

    impl CliHarness {
        pub fn new() -> Self {
            Self {
                store: MemorySecretStore::new(),
            }
        }

        pub fn login<R: BufRead, W: Write>(
            &self,
            config_path: &Path,
            input: &mut R,
            output: &mut W,
            output_format: OutputFormat,
        ) -> Result<()> {
            let global = GlobalArgs {
                config_path: Some(config_path.to_path_buf()),
                profile: None,
                output: output_format,
            };

            crate::cli::login_with_store_and_io(&global, input, output, &self.store)
        }

        pub fn profile_show<W: Write>(
            &self,
            config_path: &Path,
            selected_profile: Option<&str>,
            output: OutputFormat,
            writer: &mut W,
        ) -> Result<()> {
            let global = GlobalArgs {
                config_path: Some(config_path.to_path_buf()),
                profile: selected_profile.map(ToOwned::to_owned),
                output,
            };

            crate::cli::profile_show_with_store(&global, &self.store, writer)
        }

        pub fn load_runtime_context(
            &self,
            config_path: &Path,
            selected_profile: Option<&str>,
        ) -> Result<RuntimeContext> {
            let options = ResolveOptions::new(
                Some(config_path.to_path_buf()),
                selected_profile.map(ToOwned::to_owned),
            );
            crate::cli::load_runtime_context_with_store(&options, &self.store)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_runs_helpfully() {
        assert!(super::run_from(["confluence", "--help"]).is_ok());
    }
}
