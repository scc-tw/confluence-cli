use serde_json::Value;
use std::fs;
use std::path::PathBuf;

use crate::app;
use crate::application::runtime::ensure_writable;
use crate::convert::{apply_unified_patch, convert_text};
use crate::domain::{BodyFormat, CommentLocation};
use crate::support::{ConfluenceCliError, Result};

use super::args::{
    AttachmentCommand, Cli, Command, CommentCommand, ConvertCommand, GlobalArgs, PageCommand,
    PropertyCommand,
};
use super::bootstrap::{
    config_init, load_runtime_and_api, profile_add, profile_list, profile_remove, profile_use,
};
use super::input::{
    convert_body_to_storage, default_export_dir, parse_move_target, read_command_input,
    read_optional_json,
};
use super::output::{
    print_archive_task, print_attachments_human, print_comment_action, print_comments_human,
    print_export_result_human, print_json_or_human, print_page_action, print_page_summaries_human,
    print_page_summary_human, print_paths_human, print_properties_human, print_property_human,
    print_simple_ack, print_text,
};

pub(super) fn dispatch(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Profile(super::args::ProfileCommand::List) => profile_list(&cli.global),
        Command::Config(super::args::ConfigCommand::Init { name, profile }) => {
            config_init(&cli.global, &name, profile)
        }
        Command::Profile(super::args::ProfileCommand::Use { name }) => {
            profile_use(&cli.global, &name)
        }
        Command::Profile(super::args::ProfileCommand::Add {
            name,
            profile,
            activate,
        }) => profile_add(&cli.global, &name, profile, activate),
        Command::Profile(super::args::ProfileCommand::Remove { name }) => {
            profile_remove(&cli.global, &name)
        }
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
            let format: BodyFormat = format.into();
            if matches!(format, BodyFormat::Markdown) {
                let body = app::page_read(&api, &page, BodyFormat::Storage)?;
                print_text(&convert_text(
                    &body.content,
                    BodyFormat::Storage,
                    BodyFormat::Markdown,
                )?);
            } else {
                let body = app::page_read(&api, &page, format)?;
                print_text(&body.content);
            }
            Ok(())
        }
        PageCommand::Info { page } => {
            let summary = app::page_info(&api, &page)?;
            print_json_or_human(global.output, &summary, print_page_summary_human)
        }
        PageCommand::Find { title } => {
            let summaries = app::page_search(&api, &title)?;
            print_json_or_human(global.output, &summaries, |summaries| {
                print_page_summaries_human(summaries, "No pages found.")
            })
        }
        PageCommand::Search { query, cql } => {
            let summaries = if cql {
                app::page_search_cql(&api, &query)?
            } else {
                app::page_search(&api, &query)?
            };
            print_json_or_human(global.output, &summaries, |summaries| {
                print_page_summaries_human(summaries, "No pages found.")
            })
        }
        PageCommand::Children { page } => {
            let summaries = app::page_children(&api, &page)?;
            print_json_or_human(global.output, &summaries, |summaries| {
                print_page_summaries_human(summaries, "No child pages found.")
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
            ensure_writable(&runtime)?;
            let raw = read_command_input(
                body,
                body_file,
                "page create requires --body or --body-file",
            )?;
            let storage_body = convert_body_to_storage(raw, body_format)?;
            let summary = app::page_create(&api, title, storage_body, space_id, space_key, None)?;
            print_json_or_human(global.output, &summary, |summary| {
                print_page_action("Created", summary)
            })
        }
        PageCommand::CreateChild {
            parent,
            title,
            body,
            body_file,
            body_format,
        } => {
            ensure_writable(&runtime)?;
            let raw = read_command_input(
                body,
                body_file,
                "page create-child requires --body or --body-file",
            )?;
            let storage_body = convert_body_to_storage(raw, body_format)?;
            let summary = app::page_create(&api, title, storage_body, None, None, Some(parent))?;
            print_json_or_human(global.output, &summary, |summary| {
                print_page_action("Created child page", summary)
            })
        }
        PageCommand::Update {
            page,
            title,
            storage_body,
            version,
        } => {
            ensure_writable(&runtime)?;
            let summary = app::page_update(&api, &page, title, storage_body, version)?;
            print_json_or_human(global.output, &summary, |summary| {
                print_page_action("Updated", summary)
            })
        }
        PageCommand::Move {
            page,
            to_parent,
            before,
            after,
            title,
        } => {
            ensure_writable(&runtime)?;
            let target = parse_move_target(to_parent, before, after)?;
            let summary = app::page_move(&api, &page, target, title)?;
            print_json_or_human(global.output, &summary, |summary| {
                print_page_action("Moved", summary)
            })
        }
        PageCommand::Archive { page } => {
            ensure_writable(&runtime)?;
            let result = app::page_archive(&api, &page)?;
            print_json_or_human(global.output, &result, |result| {
                print_archive_task(&result.task_id)
            })
        }
        PageCommand::Delete {
            page,
            mode,
            yes_im_sure,
        } => {
            ensure_writable(&runtime)?;
            app::page_delete(&api, &page, mode.into(), yes_im_sure)?;
            print_simple_ack("Page deletion request accepted.");
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
                print_text(&updated);
                Ok(())
            } else {
                ensure_writable(&runtime)?;
                let summary = app::page_patch(&api, &page, &base, &patch)?;
                print_json_or_human(global.output, &summary, |summary| {
                    print_page_action("Patched", summary)
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
            let result = app::page_export(&api, &page, &dest, format.into(), !skip_attachments)?;
            print_json_or_human(global.output, &result, print_export_result_human)
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
                print_paths_human(paths, "No attachments downloaded.")
            })
        }
        AttachmentCommand::Upload {
            page,
            file,
            comment,
            replace,
            minor_edit,
        } => {
            ensure_writable(&runtime)?;
            let attachments =
                app::attachment_upload(&api, &page, file, comment, minor_edit, replace)?;
            print_json_or_human(global.output, &attachments, |attachments| {
                print_attachments_human(attachments)
            })
        }
        AttachmentCommand::Delete { page, attachment } => {
            ensure_writable(&runtime)?;
            app::attachment_delete(&api, &page, &attachment)?;
            print_simple_ack("Attachment deletion request accepted.");
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
            ensure_writable(&runtime)?;
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
            ensure_writable(&runtime)?;
            app::property_delete(&api, &page, &key)?;
            print_simple_ack("Property deletion request accepted.");
            Ok(())
        }
    }
}

fn dispatch_comment(global: &GlobalArgs, command: CommentCommand) -> Result<()> {
    let (runtime, api) = load_runtime_and_api(global)?;
    match command {
        CommentCommand::List { page, location } => {
            let location = location.map(Into::into);
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
            ensure_writable(&runtime)?;
            let input = read_command_input(
                body,
                body_file,
                "comment create requires --body or --body-file",
            )?;
            let body_format: BodyFormat = body_format.into();
            let location: crate::domain::CommentLocation = location.into();
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
                print_comment_action("Created comment", comment)
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
            ensure_writable(&runtime)?;
            let input = read_command_input(
                body,
                body_file,
                "comment reply requires --body or --body-file",
            )?;
            let location: crate::domain::CommentLocation = location.into();
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
                print_comment_action("Created reply", comment)
            })
        }
        CommentCommand::Delete { comment } => {
            ensure_writable(&runtime)?;
            app::comment_delete(&api, &comment)?;
            print_simple_ack("Comment deletion request accepted.");
            Ok(())
        }
        CommentCommand::Resolve { comment } => {
            ensure_writable(&runtime)?;
            let comment = app::comment_resolve(&api, &comment)?;
            print_json_or_human(global.output, &comment, |comment| {
                print_comment_action("Resolved comment", comment)
            })
        }
        CommentCommand::Reopen { comment } => {
            ensure_writable(&runtime)?;
            let comment = app::comment_reopen(&api, &comment)?;
            print_json_or_human(global.output, &comment, |comment| {
                print_comment_action("Reopened comment", comment)
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
    let output = convert_text(&input, command.from.into(), command.to.into())?;
    print_text(&output);
    Ok(())
}
