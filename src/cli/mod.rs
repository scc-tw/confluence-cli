use clap::Parser;
use std::io::{BufRead, Write};

use crate::support::Result;
use crate::{ResolveOptions, application::runtime::RuntimeContext, secret::SecretStore};

mod args;
mod bootstrap;
mod dispatch;
mod input;
mod output;
mod prompt;
mod shell;

pub use args::*;

pub fn run() -> Result<()> {
    run_from(std::env::args())
}

pub fn run_from<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    dispatch::dispatch(cli)
}

pub(crate) fn try_run_from<I, T>(args: I) -> std::result::Result<(), String>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    match Cli::try_parse_from(args) {
        Ok(cli) => dispatch::dispatch(cli).map_err(|error| error.to_string()),
        Err(error) => Err(error.to_string()),
    }
}

pub(crate) fn login_with_store_and_io<R: BufRead, W: Write>(
    global: &GlobalArgs,
    reader: &mut R,
    writer: &mut W,
    store: &dyn SecretStore,
) -> Result<()> {
    bootstrap::login_with_store_and_io(global, reader, writer, store)
}

pub(crate) fn profile_show_with_store<W: Write>(
    global: &GlobalArgs,
    store: &dyn SecretStore,
    writer: &mut W,
) -> Result<()> {
    bootstrap::profile_show_with_store(global, store, writer)
}

pub(crate) fn load_runtime_context_with_store(
    options: &ResolveOptions,
    store: &dyn SecretStore,
) -> Result<RuntimeContext> {
    bootstrap::load_runtime_context_with_store(options, store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_command_tree_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_profile_list_command() {
        let cli = Cli::parse_from(["confluence", "profile", "list"]);
        assert!(matches!(
            cli.command,
            Command::Profile(ProfileCommand::List)
        ));
    }

    #[test]
    fn parses_profile_show_command() {
        let cli = Cli::parse_from(["confluence", "profile", "show"]);
        assert!(matches!(
            cli.command,
            Command::Profile(ProfileCommand::Show)
        ));
    }

    #[test]
    fn parses_top_level_login_command() {
        let cli = Cli::parse_from(["confluence", "login"]);
        assert!(matches!(cli.command, Command::Login));
    }

    #[test]
    fn parses_top_level_shell_command() {
        let cli = Cli::parse_from(["confluence", "shell"]);
        assert!(matches!(cli.command, Command::Shell(_)));
    }

    #[test]
    fn parses_page_delete_with_mode() {
        let cli = Cli::parse_from([
            "confluence",
            "page",
            "delete",
            "123",
            "--mode",
            "purge",
            "--yes-im-sure",
        ]);

        match cli.command {
            Command::Page(PageCommand::Delete {
                page,
                mode,
                yes_im_sure,
            }) => {
                assert_eq!(
                    page,
                    crate::domain::PageRef::Id(crate::domain::PageId::new(123))
                );
                assert_eq!(mode, crate::cli::args::CliDeleteMode::Purge);
                assert!(yes_im_sure);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_attachment_upload_command() {
        let cli = Cli::parse_from([
            "confluence",
            "attachment",
            "upload",
            "123",
            "--file",
            "diagram.png",
            "--replace",
        ]);

        match cli.command {
            Command::Attachment(AttachmentCommand::Upload { page, replace, .. }) => {
                assert_eq!(
                    page,
                    crate::domain::PageRef::Id(crate::domain::PageId::new(123))
                );
                assert!(replace);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_comment_create_flags() {
        let cli = Cli::parse_from([
            "confluence",
            "comment",
            "create",
            "123",
            "--body",
            "hello",
            "--location",
            "inline",
            "--inline-properties",
            r#"{"markerRef":"m1"}"#,
        ]);

        match cli.command {
            Command::Comment(CommentCommand::Create {
                page,
                location,
                inline_properties,
                ..
            }) => {
                assert_eq!(
                    page,
                    crate::domain::PageRef::Id(crate::domain::PageId::new(123))
                );
                assert_eq!(location, crate::cli::args::CliCommentLocation::Inline);
                assert_eq!(inline_properties.as_deref(), Some(r#"{"markerRef":"m1"}"#));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_comment_reply_flags() {
        let cli = Cli::parse_from([
            "confluence",
            "comment",
            "reply",
            "123",
            "c-1",
            "--body",
            "hello",
        ]);

        match cli.command {
            Command::Comment(CommentCommand::Reply {
                page,
                parent_id,
                body,
                ..
            }) => {
                assert_eq!(
                    page,
                    crate::domain::PageRef::Id(crate::domain::PageId::new(123))
                );
                assert_eq!(parent_id, "c-1");
                assert_eq!(body.as_deref(), Some("hello"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_comment_info_command() {
        let cli = Cli::parse_from(["confluence", "comment", "info", "c-1"]);

        match cli.command {
            Command::Comment(CommentCommand::Info { comment }) => {
                assert_eq!(comment, "c-1");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_comment_resolution_commands() {
        let resolve = Cli::parse_from(["confluence", "comment", "resolve", "c-1"]);
        let reopen = Cli::parse_from(["confluence", "comment", "reopen", "c-1"]);

        assert!(matches!(
            resolve.command,
            Command::Comment(CommentCommand::Resolve { comment }) if comment == "c-1"
        ));
        assert!(matches!(
            reopen.command,
            Command::Comment(CommentCommand::Reopen { comment }) if comment == "c-1"
        ));
    }

    #[test]
    fn parses_page_search_cql_flag() {
        let cli = Cli::parse_from(["confluence", "page", "search", "type=page", "--cql"]);

        match cli.command {
            Command::Page(PageCommand::Search { query, cql }) => {
                assert_eq!(query, "type=page");
                assert!(cql);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_config_init_with_domain() {
        let cli = Cli::parse_from([
            "confluence",
            "config",
            "init",
            "--name",
            "work",
            "--domain",
            "example.atlassian.net",
        ]);

        match cli.command {
            Command::Config(ConfigCommand::Init { name, profile }) => {
                assert_eq!(name, "work");
                assert_eq!(profile.domain.as_deref(), Some("example.atlassian.net"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_page_create_child_command() {
        let cli = Cli::parse_from([
            "confluence",
            "page",
            "create-child",
            "123",
            "--title",
            "Child",
            "--body",
            "# hi",
        ]);

        match cli.command {
            Command::Page(PageCommand::CreateChild { parent, title, .. }) => {
                assert_eq!(
                    parent,
                    crate::domain::PageRef::Id(crate::domain::PageId::new(123))
                );
                assert_eq!(title, "Child");
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn convert_command_requires_a_single_input_source() {
        let error = super::input::read_command_input(
            Some("a".to_owned()),
            Some(std::path::PathBuf::from("page.md")),
            "missing",
        )
        .expect_err("duplicate input sources should fail");
        assert!(matches!(
            error,
            crate::support::ConfluenceCliError::Config(_)
        ));
    }
}
