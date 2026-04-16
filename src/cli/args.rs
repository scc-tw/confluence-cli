use clap::{Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::domain::{BodyFormat, CommentLocation, DeleteMode, PageRef};
use crate::profile::AuthKind;

const ROOT_AFTER_HELP: &str = "Quick start:\n  confluence login\n  confluence profile show\n  confluence page search \"release notes\"\n  confluence page info 12345\n  confluence page create --space-key SPACE --title \"Draft\" --body \"# Hello\"\n  confluence shell\n\nDrill down:\n  confluence page --help\n  confluence page create --help\n  confluence profile --help\n  confluence shell --help";

const SHELL_AFTER_HELP: &str = "Shell basics:\n  pwd\n  ls\n  cd SPACE\n  cd ..\n  cat [target]\n  grep <pattern> [target]\n  find [target] [--name <pattern>]\n  ls SPACE | grep Guide\n  use profile work\n\nInside shell, keep using the same one-liner commands without the binary name:\n  page info\n  page read\n  page create-child --title \"Child\" --body \"# Hello\"\n  attachment list\n  property list\n  comment list\n\nType `help page` or `help page create` for command help.";

const PROFILE_AFTER_HELP: &str = "Common profile flows:\n  confluence login\n  confluence profile list\n  confluence profile show\n  confluence profile use work\n  confluence profile add work --domain example.atlassian.net --auth-type bearer --api-token <token>";

const PAGE_AFTER_HELP: &str = "Common page flows:\n  confluence page search \"release notes\"\n  confluence page search 'type=page and space=SPACE' --cql\n  confluence page info 12345\n  confluence page read 12345 --format markdown\n  confluence page read https://your-site.atlassian.net/wiki/spaces/SPACE/pages/12345/Page+Title\n  confluence page create --space-key SPACE --title \"Draft\" --body \"# Hello\"\n  confluence page create-child 12345 --title \"Child\" --body \"# Hello\"\n  confluence page export 12345";

const ATTACHMENT_AFTER_HELP: &str = "Common attachment flows:\n  confluence attachment list 12345\n  confluence attachment upload 12345 --file diagram.png\n  confluence attachment download 12345\n  confluence attachment delete 12345 diagram.png";

const PROPERTY_AFTER_HELP: &str = "Common property flows:\n  confluence property list 12345\n  confluence property get 12345 owner\n  confluence property set 12345 owner --value '{\"team\":\"eng\"}'\n  confluence property delete 12345 owner";

const COMMENT_AFTER_HELP: &str = "Common comment flows:\n  confluence comment list 12345\n  confluence comment create 12345 --body \"Looks good\"\n  confluence comment reply 12345 c-1 --body \"Ack\"\n  confluence comment resolve c-1\n  confluence comment reopen c-1";

const CONVERT_AFTER_HELP: &str = "Local conversion examples:\n  confluence convert --from markdown --to storage --input \"# Hello\"\n  confluence convert --from storage --to markdown --input-file page.storage";

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
#[command(
    name = "confluence",
    version,
    about = "Confluence CLI for one-liners and interactive shell workflows",
    after_help = ROOT_AFTER_HELP
)]
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
    Login,
    Shell(ShellCommand),
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
#[command(after_help = PROFILE_AFTER_HELP)]
pub enum ProfileCommand {
    List,
    Show,
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
#[command(
    about = "Start an interactive Confluence shell",
    long_about = "Start an interactive Confluence shell with Unix-like navigation over spaces and pages while keeping one-liner CRUD commands available inside the shell.",
    after_help = SHELL_AFTER_HELP
)]
pub struct ShellCommand {}

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
    pub read_only: Option<bool>,
}

#[derive(Debug, Subcommand)]
#[command(after_help = PAGE_AFTER_HELP)]
pub enum PageCommand {
    Read {
        page: PageRef,
        #[arg(long, value_enum, default_value_t = CliBodyFormat::Storage)]
        format: CliBodyFormat,
    },
    Info {
        page: PageRef,
    },
    #[command(about = "Quick page lookup by title or text")]
    Find {
        title: String,
    },
    #[command(about = "Search pages by text, or pass raw CQL with --cql")]
    Search {
        query: String,
        #[arg(long)]
        cql: bool,
    },
    Children {
        page: PageRef,
    },
    #[command(about = "Create a page in a space")]
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
    #[command(about = "Create a child page under an existing page")]
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
    #[command(about = "Update a page using explicit storage body and version")]
    Update {
        page: PageRef,
        #[arg(long)]
        title: String,
        #[arg(long)]
        storage_body: String,
        #[arg(long)]
        version: u32,
    },
    #[command(about = "Apply a unified patch to a page using a saved base file")]
    Patch {
        page: PageRef,
        #[arg(long)]
        base_file: PathBuf,
        #[arg(long)]
        patch_file: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    #[command(about = "Move a page to a new parent or reorder it before/after another page")]
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
    #[command(about = "Delete a page using archive, trash, or purge mode")]
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
#[command(after_help = ATTACHMENT_AFTER_HELP)]
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
#[command(after_help = PROPERTY_AFTER_HELP)]
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
#[command(after_help = COMMENT_AFTER_HELP)]
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
#[command(after_help = CONVERT_AFTER_HELP)]
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
