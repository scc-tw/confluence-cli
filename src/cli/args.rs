use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::domain::{BodyFormat, CommentLocation, DeleteMode, PageRef};
use crate::profile::AuthKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliAuthKind {
    Basic,
    Bearer,
    Mtls,
}

impl From<CliAuthKind> for AuthKind {
    fn from(value: CliAuthKind) -> Self {
        match value {
            CliAuthKind::Basic => AuthKind::Basic,
            CliAuthKind::Bearer => AuthKind::Bearer,
            CliAuthKind::Mtls => AuthKind::Mtls,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliBodyFormat {
    Storage,
    Markdown,
    Html,
    Text,
}

impl From<CliBodyFormat> for BodyFormat {
    fn from(value: CliBodyFormat) -> Self {
        match value {
            CliBodyFormat::Storage => BodyFormat::Storage,
            CliBodyFormat::Markdown => BodyFormat::Markdown,
            CliBodyFormat::Html => BodyFormat::Html,
            CliBodyFormat::Text => BodyFormat::Text,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliDeleteMode {
    Archive,
    Trash,
    Purge,
}

impl From<CliDeleteMode> for DeleteMode {
    fn from(value: CliDeleteMode) -> Self {
        match value {
            CliDeleteMode::Archive => DeleteMode::Archive,
            CliDeleteMode::Trash => DeleteMode::Trash,
            CliDeleteMode::Purge => DeleteMode::Purge,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum CliCommentLocation {
    Footer,
    Inline,
    Resolved,
}

impl From<CliCommentLocation> for CommentLocation {
    fn from(value: CliCommentLocation) -> Self {
        match value {
            CliCommentLocation::Footer => CommentLocation::Footer,
            CliCommentLocation::Inline => CommentLocation::Inline,
            CliCommentLocation::Resolved => CommentLocation::Resolved,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "confluence", version, about = "Rust-first Confluence CLI")]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, Args)]
pub struct GlobalArgs {
    #[arg(long)]
    pub config_path: Option<PathBuf>,

    #[arg(long)]
    pub profile: Option<String>,

    #[arg(long, value_enum, default_value_t = OutputFormat::Human)]
    pub output: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(subcommand)]
    Config(ConfigCommand),
    #[command(subcommand)]
    Profile(ProfileCommand),
    #[command(subcommand)]
    Page(PageCommand),
    #[command(subcommand)]
    Attachment(AttachmentCommand),
    #[command(subcommand)]
    Property(PropertyCommand),
    #[command(subcommand)]
    Comment(CommentCommand),
    Convert(ConvertCommand),
}

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Init {
        #[arg(long, default_value = "default")]
        name: String,
        #[command(flatten)]
        profile: ProfileArgs,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    List,
    Use {
        name: String,
    },
    Add {
        name: String,
        #[command(flatten)]
        profile: ProfileArgs,
        #[arg(long)]
        activate: bool,
    },
    Remove {
        name: String,
    },
}

#[derive(Debug, Clone, Args, Default)]
pub struct ProfileArgs {
    #[arg(long)]
    pub domain: Option<String>,
    #[arg(long)]
    pub protocol: Option<String>,
    #[arg(long)]
    pub api_path: Option<String>,
    #[arg(long, value_enum)]
    pub auth_type: Option<CliAuthKind>,
    #[arg(long)]
    pub email: Option<String>,
    #[arg(long)]
    pub username: Option<String>,
    #[arg(long)]
    pub api_token: Option<String>,
    #[arg(long)]
    pub password: Option<String>,
    #[arg(long)]
    pub read_only: bool,
}

#[derive(Debug, Subcommand)]
pub enum PageCommand {
    Read {
        page: PageRef,
        #[arg(long, value_enum, default_value_t = CliBodyFormat::Storage)]
        format: CliBodyFormat,
    },
    Info {
        page: PageRef,
    },
    Find {
        title: String,
    },
    Search {
        query: String,
        #[arg(long)]
        cql: bool,
    },
    Children {
        page: PageRef,
    },
    Create {
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        body_file: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CliBodyFormat::Markdown)]
        body_format: CliBodyFormat,
        #[arg(long)]
        space_id: Option<String>,
        #[arg(long)]
        space_key: Option<String>,
    },
    CreateChild {
        parent: PageRef,
        #[arg(long)]
        title: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        body_file: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CliBodyFormat::Markdown)]
        body_format: CliBodyFormat,
    },
    Update {
        page: PageRef,
        #[arg(long)]
        title: String,
        #[arg(long)]
        storage_body: String,
        #[arg(long)]
        version: u32,
    },
    Patch {
        page: PageRef,
        #[arg(long)]
        base_file: PathBuf,
        #[arg(long)]
        patch_file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Move {
        page: PageRef,
        #[arg(long)]
        to_parent: Option<PageRef>,
        #[arg(long)]
        before: Option<PageRef>,
        #[arg(long)]
        after: Option<PageRef>,
        #[arg(long)]
        title: Option<String>,
    },
    Archive {
        page: PageRef,
    },
    Delete {
        page: PageRef,
        #[arg(long, value_enum, default_value_t = CliDeleteMode::Archive)]
        mode: CliDeleteMode,
        #[arg(long)]
        yes_im_sure: bool,
    },
    Export {
        page: PageRef,
        #[arg(long)]
        dest: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CliBodyFormat::Markdown)]
        format: CliBodyFormat,
        #[arg(long)]
        skip_attachments: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum AttachmentCommand {
    List {
        page: PageRef,
    },
    Download {
        page: PageRef,
        #[arg(long)]
        dest: Option<PathBuf>,
    },
    Upload {
        page: PageRef,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        comment: Option<String>,
        #[arg(long)]
        replace: bool,
        #[arg(long)]
        minor_edit: bool,
    },
    Delete {
        page: PageRef,
        attachment: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum PropertyCommand {
    List {
        page: PageRef,
    },
    Get {
        page: PageRef,
        key: String,
    },
    Set {
        page: PageRef,
        key: String,
        #[arg(long)]
        value: Option<String>,
        #[arg(long)]
        value_file: Option<PathBuf>,
    },
    Delete {
        page: PageRef,
        key: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum CommentCommand {
    List {
        page: PageRef,
        #[arg(long, value_enum)]
        location: Option<CliCommentLocation>,
    },
    Info {
        comment: String,
    },
    Create {
        page: PageRef,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        body_file: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CliBodyFormat::Markdown)]
        body_format: CliBodyFormat,
        #[arg(long, value_enum, default_value_t = CliCommentLocation::Footer)]
        location: CliCommentLocation,
        #[arg(long)]
        parent_id: Option<String>,
        #[arg(long)]
        inline_properties: Option<String>,
        #[arg(long)]
        inline_properties_file: Option<PathBuf>,
    },
    Reply {
        page: PageRef,
        parent_id: String,
        #[arg(long)]
        body: Option<String>,
        #[arg(long)]
        body_file: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = CliBodyFormat::Markdown)]
        body_format: CliBodyFormat,
        #[arg(long, value_enum, default_value_t = CliCommentLocation::Footer)]
        location: CliCommentLocation,
        #[arg(long)]
        inline_properties: Option<String>,
        #[arg(long)]
        inline_properties_file: Option<PathBuf>,
    },
    Delete {
        comment: String,
    },
    Resolve {
        comment: String,
    },
    Reopen {
        comment: String,
    },
}

#[derive(Debug, Args)]
pub struct ConvertCommand {
    #[arg(long, value_enum)]
    pub from: CliBodyFormat,
    #[arg(long, value_enum)]
    pub to: CliBodyFormat,
    #[arg(long)]
    pub input: Option<String>,
    #[arg(long)]
    pub input_file: Option<PathBuf>,
}
