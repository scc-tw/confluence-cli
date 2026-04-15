use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::api::{AttachmentSummary, CommentSummary, ContentProperty, HttpConfluenceApi};
use crate::app::{self, RuntimeContext};
use crate::config::{
    AuthKind, Profile, ResolveOptions, RuntimeConfig, default_config_path, init_config,
    load_config, remove_profile, set_active_profile, upsert_profile,
};
use crate::convert::{apply_unified_patch, convert_text};
use crate::domain::{BodyFormat, CommentLocation, DeleteMode, MoveTarget, PageRef};
use crate::secret::{KeyringSecretStore, SecretKind, SecretStore};
use crate::support::{ConfluenceCliError, Result};
use uuid::Uuid;

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
    pub auth_type: Option<AuthKind>,
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
        #[arg(long, value_enum, default_value_t = BodyFormat::Storage)]
        format: BodyFormat,
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
        #[arg(long, value_enum, default_value_t = BodyFormat::Markdown)]
        body_format: BodyFormat,
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
        #[arg(long, value_enum, default_value_t = BodyFormat::Markdown)]
        body_format: BodyFormat,
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
        #[arg(long, value_enum, default_value_t = DeleteMode::Archive)]
        mode: DeleteMode,
        #[arg(long)]
        yes_im_sure: bool,
    },
    Export {
        page: PageRef,
        #[arg(long)]
        dest: Option<PathBuf>,
        #[arg(long, value_enum, default_value_t = BodyFormat::Markdown)]
        format: BodyFormat,
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
        location: Option<CommentLocation>,
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
        #[arg(long, value_enum, default_value_t = BodyFormat::Markdown)]
        body_format: BodyFormat,
        #[arg(long, value_enum, default_value_t = CommentLocation::Footer)]
        location: CommentLocation,
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
        #[arg(long, value_enum, default_value_t = BodyFormat::Markdown)]
        body_format: BodyFormat,
        #[arg(long, value_enum, default_value_t = CommentLocation::Footer)]
        location: CommentLocation,
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
    pub from: BodyFormat,
    #[arg(long, value_enum)]
    pub to: BodyFormat,
    #[arg(long)]
    pub input: Option<String>,
    #[arg(long)]
    pub input_file: Option<PathBuf>,
}

pub fn run() -> Result<()> {
    run_from(std::env::args())
}

pub fn run_from<I, T>(args: I) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    dispatch(cli)
}

pub fn command() -> clap::Command {
    Cli::command()
}

fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Profile(ProfileCommand::List) => profile_list(&cli.global),
        Command::Config(ConfigCommand::Init { name, profile }) => {
            config_init(&cli.global, &name, profile)
        }
        Command::Profile(ProfileCommand::Use { name }) => profile_use(&cli.global, &name),
        Command::Profile(ProfileCommand::Add {
            name,
            profile,
            activate,
        }) => profile_add(&cli.global, &name, profile, activate),
        Command::Profile(ProfileCommand::Remove { name }) => profile_remove(&cli.global, &name),
        Command::Page(command) => dispatch_page(&cli.global, command),
        Command::Attachment(command) => dispatch_attachment(&cli.global, command),
        Command::Property(command) => dispatch_property(&cli.global, command),
        Command::Comment(command) => dispatch_comment(&cli.global, command),
        Command::Convert(command) => dispatch_convert(command),
    }
}

fn dispatch_page(global: &GlobalArgs, command: PageCommand) -> Result<()> {
    let (runtime, api) = load_runtime_and_api(global)?;

    match command {
        PageCommand::Read { page, format } => {
            if matches!(format, BodyFormat::Markdown) {
                let body = app::page_read(&api, &page, BodyFormat::Storage)?;
                println!(
                    "{}",
                    convert_text(&body.content, BodyFormat::Storage, BodyFormat::Markdown)?
                );
            } else {
                let body = app::page_read(&api, &page, format)?;
                println!("{}", body.content);
            }
            Ok(())
        }
        PageCommand::Info { page } => {
            let summary = app::page_info(&api, &page)?;
            print_json_or_human(global.output, &summary, |summary| {
                println!("{} [{}]", summary.title, summary.id);
                if let Some(status) = &summary.status {
                    println!("status: {status}");
                }
                if let Some(version) = summary.version {
                    println!("version: {version}");
                }
            })
        }
        PageCommand::Find { title } => {
            let summaries = app::page_search(&api, &title)?;
            print_json_or_human(global.output, &summaries, |summaries| {
                if summaries.is_empty() {
                    println!("No pages found.");
                } else {
                    for summary in summaries {
                        println!("- {} [{}]", summary.title, summary.id);
                    }
                }
            })
        }
        PageCommand::Search { query, cql } => {
            let summaries = if cql {
                app::page_search_cql(&api, &query)?
            } else {
                app::page_search(&api, &query)?
            };
            print_json_or_human(global.output, &summaries, |summaries| {
                if summaries.is_empty() {
                    println!("No pages found.");
                } else {
                    for summary in summaries {
                        println!("- {} [{}]", summary.title, summary.id);
                    }
                }
            })
        }
        PageCommand::Children { page } => {
            let summaries = app::page_children(&api, &page)?;
            print_json_or_human(global.output, &summaries, |summaries| {
                if summaries.is_empty() {
                    println!("No child pages found.");
                } else {
                    for summary in summaries {
                        println!("- {} [{}]", summary.title, summary.id);
                    }
                }
            })
        }
        PageCommand::Create {
            title,
            body,
            body_file,
            body_format,
            space_id,
            space_key,
        } => {
            app::ensure_writable(&runtime)?;
            let raw = read_command_input(
                body,
                body_file,
                "page create requires --body or --body-file",
            )?;
            let storage_body = convert_body_to_storage(raw, body_format)?;
            let summary = app::page_create(&api, title, storage_body, space_id, space_key, None)?;
            print_json_or_human(global.output, &summary, |summary| {
                println!("Created {} [{}]", summary.title, summary.id);
            })
        }
        PageCommand::CreateChild {
            parent,
            title,
            body,
            body_file,
            body_format,
        } => {
            app::ensure_writable(&runtime)?;
            let raw = read_command_input(
                body,
                body_file,
                "page create-child requires --body or --body-file",
            )?;
            let storage_body = convert_body_to_storage(raw, body_format)?;
            let summary = app::page_create(&api, title, storage_body, None, None, Some(parent))?;
            print_json_or_human(global.output, &summary, |summary| {
                println!("Created child page {} [{}]", summary.title, summary.id);
            })
        }
        PageCommand::Update {
            page,
            title,
            storage_body,
            version,
        } => {
            app::ensure_writable(&runtime)?;
            let summary = app::page_update(&api, &page, title, storage_body, version)?;
            print_json_or_human(global.output, &summary, |summary| {
                println!("Updated {} [{}]", summary.title, summary.id);
            })
        }
        PageCommand::Move {
            page,
            to_parent,
            before,
            after,
            title,
        } => {
            app::ensure_writable(&runtime)?;
            let target = parse_move_target(to_parent, before, after)?;
            let summary = app::page_move(&api, &page, target, title)?;
            print_json_or_human(global.output, &summary, |summary| {
                println!("Moved {} [{}]", summary.title, summary.id);
            })
        }
        PageCommand::Archive { page } => {
            app::ensure_writable(&runtime)?;
            let result = app::page_archive(&api, &page)?;
            print_json_or_human(global.output, &result, |result| {
                println!("Archive task queued: {}", result.task_id);
            })
        }
        PageCommand::Delete {
            page,
            mode,
            yes_im_sure,
        } => {
            app::ensure_writable(&runtime)?;
            app::page_delete(&api, &page, mode, yes_im_sure)?;
            println!("Page deletion request accepted.");
            Ok(())
        }
        PageCommand::Patch {
            page,
            patch_file,
            base_file,
            dry_run,
        } => {
            let base = fs::read_to_string(base_file)?;
            let patch = fs::read_to_string(patch_file)?;
            let updated = apply_unified_patch(&base, &patch)?;
            if dry_run {
                println!("{}", updated);
                Ok(())
            } else {
                app::ensure_writable(&runtime)?;
                let summary = app::page_patch(&api, &page, &base, &patch)?;
                print_json_or_human(global.output, &summary, |summary| {
                    println!("Patched {} [{}]", summary.title, summary.id);
                })
            }
        }
        PageCommand::Export {
            page,
            dest,
            format,
            skip_attachments,
        } => {
            let dest = dest.unwrap_or_else(|| default_export_dir(&page));
            let result = app::page_export(&api, &page, &dest, format, !skip_attachments)?;
            print_json_or_human(global.output, &result, |result| {
                println!("Exported to {}", result.directory.display());
                println!("content: {}", result.content_path.display());
                println!("attachments: {}", result.attachment_count);
            })
        }
    }
}

fn dispatch_attachment(global: &GlobalArgs, command: AttachmentCommand) -> Result<()> {
    let (runtime, api) = load_runtime_and_api(global)?;
    match command {
        AttachmentCommand::List { page } => {
            let attachments = app::attachment_list(&api, &page)?;
            print_json_or_human(global.output, &attachments, |attachments| {
                print_attachments_human(attachments)
            })
        }
        AttachmentCommand::Download { page, dest } => {
            let dest = dest.unwrap_or_else(|| PathBuf::from("attachments"));
            let paths = app::attachment_download_all(&api, &page, &dest)?;
            print_json_or_human(global.output, &paths, |paths| {
                if paths.is_empty() {
                    println!("No attachments downloaded.");
                } else {
                    for path in paths {
                        println!("- {}", path.display());
                    }
                }
            })
        }
        AttachmentCommand::Upload {
            page,
            file,
            comment,
            replace,
            minor_edit,
        } => {
            app::ensure_writable(&runtime)?;
            let attachments =
                app::attachment_upload(&api, &page, file, comment, minor_edit, replace)?;
            print_json_or_human(global.output, &attachments, |attachments| {
                print_attachments_human(attachments)
            })
        }
        AttachmentCommand::Delete { page, attachment } => {
            app::ensure_writable(&runtime)?;
            app::attachment_delete(&api, &page, &attachment)?;
            println!("Attachment deletion request accepted.");
            Ok(())
        }
    }
}

fn dispatch_property(global: &GlobalArgs, command: PropertyCommand) -> Result<()> {
    let (runtime, api) = load_runtime_and_api(global)?;
    match command {
        PropertyCommand::List { page } => {
            let properties = app::property_list(&api, &page)?;
            print_json_or_human(global.output, &properties, |properties| {
                print_properties_human(properties)
            })
        }
        PropertyCommand::Get { page, key } => {
            let property = app::property_get(&api, &page, &key)?;
            print_json_or_human(global.output, &property, print_property_human)
        }
        PropertyCommand::Set {
            page,
            key,
            value,
            value_file,
        } => {
            app::ensure_writable(&runtime)?;
            let input = read_command_input(
                value,
                value_file,
                "property set requires --value or --value-file",
            )?;
            let json: Value = serde_json::from_str(&input).map_err(|error| {
                ConfluenceCliError::Config(format!("property value must be valid JSON: {error}"))
            })?;
            let property = app::property_set(&api, &page, &key, json)?;
            print_json_or_human(global.output, &property, print_property_human)
        }
        PropertyCommand::Delete { page, key } => {
            app::ensure_writable(&runtime)?;
            app::property_delete(&api, &page, &key)?;
            println!("Property deletion request accepted.");
            Ok(())
        }
    }
}

fn dispatch_comment(global: &GlobalArgs, command: CommentCommand) -> Result<()> {
    let (runtime, api) = load_runtime_and_api(global)?;
    match command {
        CommentCommand::List { page, location } => {
            let comments = app::comment_list(&api, &page, location)?;
            print_json_or_human(global.output, &comments, |comments| {
                print_comments_human(comments)
            })
        }
        CommentCommand::Info { comment } => {
            let comment = app::comment_info(&api, &comment)?;
            print_json_or_human(global.output, &comment, |comment| {
                print_comments_human(std::slice::from_ref(comment))
            })
        }
        CommentCommand::Create {
            page,
            body,
            body_file,
            body_format,
            location,
            parent_id,
            inline_properties,
            inline_properties_file,
        } => {
            app::ensure_writable(&runtime)?;
            let input = read_command_input(
                body,
                body_file,
                "comment create requires --body or --body-file",
            )?;
            let body_storage = if matches!(body_format, BodyFormat::Storage) {
                input
            } else {
                convert_text(&input, body_format, BodyFormat::Storage)?
            };
            let inline_properties = if matches!(location, CommentLocation::Inline) {
                read_optional_json(
                    inline_properties,
                    inline_properties_file,
                    "inline comment creation requires --inline-properties or --inline-properties-file",
                )?
            } else {
                read_optional_json(inline_properties, inline_properties_file, "")?
            };

            let comment = app::comment_create(
                &api,
                &page,
                body_storage,
                location,
                parent_id,
                inline_properties,
            )?;
            print_json_or_human(global.output, &comment, |comment| {
                println!("Created comment {}", comment.id);
            })
        }
        CommentCommand::Reply {
            page,
            parent_id,
            body,
            body_file,
            body_format,
            location,
            inline_properties,
            inline_properties_file,
        } => {
            app::ensure_writable(&runtime)?;
            let input = read_command_input(
                body,
                body_file,
                "comment reply requires --body or --body-file",
            )?;
            let body_storage = convert_body_to_storage(input, body_format)?;
            let inline_properties = if matches!(location, CommentLocation::Inline) {
                read_optional_json(
                    inline_properties,
                    inline_properties_file,
                    "inline comment reply requires --inline-properties or --inline-properties-file",
                )?
            } else {
                read_optional_json(inline_properties, inline_properties_file, "")?
            };

            let comment = app::comment_create(
                &api,
                &page,
                body_storage,
                location,
                Some(parent_id),
                inline_properties,
            )?;
            print_json_or_human(global.output, &comment, |comment| {
                println!("Created reply {}", comment.id);
            })
        }
        CommentCommand::Delete { comment } => {
            app::ensure_writable(&runtime)?;
            app::comment_delete(&api, &comment)?;
            println!("Comment deletion request accepted.");
            Ok(())
        }
        CommentCommand::Resolve { comment } => {
            app::ensure_writable(&runtime)?;
            let comment = app::comment_resolve(&api, &comment)?;
            print_json_or_human(global.output, &comment, |comment| {
                println!("Resolved comment {}", comment.id);
            })
        }
        CommentCommand::Reopen { comment } => {
            app::ensure_writable(&runtime)?;
            let comment = app::comment_reopen(&api, &comment)?;
            print_json_or_human(global.output, &comment, |comment| {
                println!("Reopened comment {}", comment.id);
            })
        }
    }
}

fn dispatch_convert(command: ConvertCommand) -> Result<()> {
    let input = read_command_input(
        command.input,
        command.input_file,
        "convert requires --input or --input-file",
    )?;
    let output = convert_text(&input, command.from, command.to)?;
    println!("{output}");
    Ok(())
}

fn config_init(global: &GlobalArgs, name: &str, profile_args: ProfileArgs) -> Result<()> {
    let path = config_path(global);
    let existing = load_config(&path)?;
    if !existing.profiles.is_empty() {
        return Err(ConfluenceCliError::Config(
            "config already exists; use profile add or profile use instead".to_owned(),
        ));
    }

    let store = KeyringSecretStore;
    let (profile, secrets) = profile_from_args(profile_args)?;
    write_profile_secrets(&store, &profile, &secrets)?;
    let config = init_config(&path, name, profile)?;
    print_profiles_human(&RuntimeConfig {
        config,
        resolved_profile: None,
    });
    Ok(())
}

fn profile_add(
    global: &GlobalArgs,
    name: &str,
    profile_args: ProfileArgs,
    activate: bool,
) -> Result<()> {
    let path = config_path(global);
    let store = KeyringSecretStore;
    let (profile, secrets) = profile_from_args(profile_args)?;
    write_profile_secrets(&store, &profile, &secrets)?;
    let config = upsert_profile(&path, name, profile, activate)?;
    print_profiles_human(&RuntimeConfig {
        config,
        resolved_profile: None,
    });
    Ok(())
}

fn profile_use(global: &GlobalArgs, name: &str) -> Result<()> {
    let path = config_path(global);
    let config = set_active_profile(&path, name)?;
    print_profiles_human(&RuntimeConfig {
        config,
        resolved_profile: None,
    });
    Ok(())
}

fn profile_remove(global: &GlobalArgs, name: &str) -> Result<()> {
    let path = config_path(global);
    if let Some(profile) = load_config(&path)?.profiles.get(name).cloned()
        && profile.secret_backend.is_some()
    {
        let store = KeyringSecretStore;
        let profile_id = profile.id.as_deref().unwrap_or(name);
        store.delete(profile_id, SecretKind::ApiToken)?;
        store.delete(profile_id, SecretKind::Password)?;
    }
    let config = remove_profile(&path, name)?;
    print_profiles_human(&RuntimeConfig {
        config,
        resolved_profile: None,
    });
    Ok(())
}

#[derive(Debug, Clone, Default)]
struct ProfileSecrets {
    api_token: Option<String>,
    password: Option<String>,
}

fn profile_from_args(args: ProfileArgs) -> Result<(Profile, ProfileSecrets)> {
    let domain = args.domain.ok_or_else(|| {
        ConfluenceCliError::Config("profile configuration requires --domain".to_owned())
    })?;

    let auth_type = args.auth_type.or_else(|| {
        if args.email.is_some() {
            Some(AuthKind::Basic)
        } else if args.api_token.is_some() {
            Some(AuthKind::Bearer)
        } else {
            None
        }
    });

    let secrets = ProfileSecrets {
        api_token: args.api_token,
        password: args.password,
    };

    Ok((
        Profile {
            id: Some(Uuid::new_v4().to_string()),
            domain: Some(domain),
            protocol: args.protocol,
            api_path: args.api_path,
            auth_type,
            email: args.email,
            username: args.username,
            api_token: None,
            password: None,
            read_only: args.read_only.then_some(true),
            secret_backend: if secrets.api_token.is_some() || secrets.password.is_some() {
                Some(crate::config::SecretBackend::Keyring)
            } else {
                None
            },
        },
        secrets,
    ))
}

fn write_profile_secrets(
    store: &dyn SecretStore,
    profile: &Profile,
    secrets: &ProfileSecrets,
) -> Result<()> {
    let profile_id = profile
        .id
        .as_deref()
        .ok_or_else(|| ConfluenceCliError::Config("profile id missing".to_owned()))?;

    if let Some(api_token) = secrets.api_token.as_deref() {
        store.set(profile_id, SecretKind::ApiToken, api_token)?;
    }

    if let Some(password) = secrets.password.as_deref() {
        store.set(profile_id, SecretKind::Password, password)?;
    }

    Ok(())
}

fn config_path(global: &GlobalArgs) -> PathBuf {
    global
        .config_path
        .clone()
        .unwrap_or_else(default_config_path)
}

fn read_command_input(
    inline: Option<String>,
    file: Option<PathBuf>,
    missing_message: &str,
) -> Result<String> {
    match (inline, file) {
        (Some(input), None) => Ok(input),
        (None, Some(path)) => Ok(fs::read_to_string(path)?),
        (Some(_), Some(_)) => Err(ConfluenceCliError::Config(
            "use either the inline value or file input, not both".to_owned(),
        )),
        (None, None) => Err(ConfluenceCliError::Config(missing_message.to_owned())),
    }
}

fn read_optional_json(
    inline: Option<String>,
    file: Option<PathBuf>,
    missing_message: &str,
) -> Result<Option<Value>> {
    match (inline, file) {
        (None, None) => {
            if missing_message.is_empty() {
                Ok(None)
            } else {
                Err(ConfluenceCliError::Config(missing_message.to_owned()))
            }
        }
        (inline, file) => {
            let raw = read_command_input(inline, file, missing_message)?;
            let json = serde_json::from_str(&raw).map_err(|error| {
                ConfluenceCliError::Config(format!("JSON input must be valid JSON: {error}"))
            })?;
            Ok(Some(json))
        }
    }
}

fn profile_list(global: &GlobalArgs) -> Result<()> {
    let runtime = load_runtime_context(global)?;
    match global.output {
        OutputFormat::Human => print_profiles_human(&runtime.runtime_config),
        OutputFormat::Json => print_profiles_json(&runtime.runtime_config)?,
    }

    Ok(())
}

fn load_runtime_context(global: &GlobalArgs) -> Result<RuntimeContext> {
    let options = ResolveOptions::new(global.config_path.clone(), global.profile.clone());
    RuntimeContext::load(&options)
}

fn load_runtime_and_api(global: &GlobalArgs) -> Result<(RuntimeContext, HttpConfluenceApi)> {
    let runtime = load_runtime_context(global)?;
    let profile = runtime
        .runtime_config
        .resolved_profile
        .clone()
        .ok_or_else(|| ConfluenceCliError::Config("no active or selected profile".to_owned()))?;
    Ok((runtime, HttpConfluenceApi::new(profile)?))
}

fn convert_body_to_storage(body: String, format: BodyFormat) -> Result<String> {
    if matches!(format, BodyFormat::Storage) {
        Ok(body)
    } else {
        convert_text(&body, format, BodyFormat::Storage)
    }
}

fn parse_move_target(
    to_parent: Option<PageRef>,
    before: Option<PageRef>,
    after: Option<PageRef>,
) -> Result<MoveTarget> {
    let count = usize::from(to_parent.is_some())
        + usize::from(before.is_some())
        + usize::from(after.is_some());
    if count != 1 {
        return Err(ConfluenceCliError::Config(
            "page move requires exactly one of --to-parent, --before, or --after".to_owned(),
        ));
    }

    if let Some(parent) = to_parent {
        Ok(MoveTarget::Parent(parent))
    } else if let Some(before) = before {
        Ok(MoveTarget::Before(before))
    } else if let Some(after) = after {
        Ok(MoveTarget::After(after))
    } else {
        unreachable!("validated move target should exist")
    }
}

fn default_export_dir(page: &PageRef) -> PathBuf {
    match page {
        PageRef::Id(id) => PathBuf::from(format!("page-export-{}", id.get())),
        PageRef::Url(_) => PathBuf::from("page-export"),
    }
}

fn print_profiles_human(runtime: &RuntimeConfig) {
    if runtime.config.profiles.is_empty() {
        println!("No profiles configured.");
        return;
    }

    for name in runtime.config.profiles.keys() {
        let marker = if runtime.config.active_profile.as_deref() == Some(name.as_str()) {
            "*"
        } else {
            " "
        };
        println!("{marker} {name}");
    }
}

fn print_profiles_json(runtime: &RuntimeConfig) -> Result<()> {
    #[derive(serde::Serialize)]
    struct ProfileEntry<'a> {
        name: &'a str,
        active: bool,
    }

    let entries: Vec<_> = runtime
        .config
        .profiles
        .keys()
        .map(|name| ProfileEntry {
            name,
            active: runtime.config.active_profile.as_deref() == Some(name.as_str()),
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&entries)?);
    Ok(())
}

fn print_json_or_human<T, F>(output: OutputFormat, value: &T, human: F) -> Result<()>
where
    T: Serialize,
    F: FnOnce(&T),
{
    match output {
        OutputFormat::Human => human(value),
        OutputFormat::Json => println!("{}", serde_json::to_string_pretty(value)?),
    }
    Ok(())
}

fn print_attachments_human(attachments: &[AttachmentSummary]) {
    if attachments.is_empty() {
        println!("No attachments found.");
        return;
    }

    for attachment in attachments {
        println!("- {} [{}]", attachment.title, attachment.id);
    }
}

fn print_property_human(property: &ContentProperty) {
    println!("key: {}", property.key);
    println!("version: {}", property.version);
    println!(
        "value: {}",
        serde_json::to_string_pretty(&property.value)
            .unwrap_or_else(|_| property.value.to_string())
    );
}

fn print_properties_human(properties: &[ContentProperty]) {
    if properties.is_empty() {
        println!("No properties found.");
        return;
    }

    for property in properties {
        println!("- {} (v{})", property.key, property.version);
    }
}

fn print_comments_human(comments: &[CommentSummary]) {
    if comments.is_empty() {
        println!("No comments found.");
        return;
    }

    for comment in comments {
        println!("- {}", comment.id);
        if let Some(author) = &comment.author {
            println!("  author: {author}");
        }
        if let Some(location) = comment.location {
            println!("  location: {location:?}");
        }
        if let Some(resolution) = &comment.resolution {
            println!("  resolution: {resolution}");
        }
        if let Some(marker_ref) = &comment.inline_marker_ref {
            println!("  marker ref: {marker_ref}");
        }
        if let Some(selection) = &comment.inline_original_selection {
            println!("  original selection: {selection}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_command_tree_is_valid() {
        command().debug_assert();
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
                assert_eq!(page, PageRef::Id(crate::domain::PageId::new(123)));
                assert_eq!(mode, DeleteMode::Purge);
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
                assert_eq!(page, PageRef::Id(crate::domain::PageId::new(123)));
                assert!(replace);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn convert_command_requires_a_single_input_source() {
        let error = read_command_input(
            Some("a".to_owned()),
            Some(PathBuf::from("page.md")),
            "convert requires input",
        )
        .expect_err("duplicate input sources should fail");
        assert!(matches!(error, ConfluenceCliError::Config(_)));
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
            "--body-format",
            "markdown",
            "--location",
            "footer",
        ]);

        match cli.command {
            Command::Comment(CommentCommand::Create {
                page,
                body,
                body_format,
                location,
                ..
            }) => {
                assert_eq!(page, PageRef::Id(crate::domain::PageId::new(123)));
                assert_eq!(body.as_deref(), Some("hello"));
                assert_eq!(body_format, BodyFormat::Markdown);
                assert_eq!(location, CommentLocation::Footer);
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
                assert_eq!(page, PageRef::Id(crate::domain::PageId::new(123)));
                assert_eq!(parent_id, "c-1");
                assert_eq!(body.as_deref(), Some("hello"));
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
                assert_eq!(parent, PageRef::Id(crate::domain::PageId::new(123)));
                assert_eq!(title, "Child");
            }
            other => panic!("unexpected command: {other:?}"),
        }
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
}
