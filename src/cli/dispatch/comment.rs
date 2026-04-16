use crate::app;
use crate::application::runtime::ensure_writable;
use crate::convert::convert_text;
use crate::domain::{BodyFormat, CommentLocation};
use crate::support::Result;

use super::super::args::{CommentCommand, GlobalArgs};
use super::super::bootstrap::load_runtime_and_api;
use super::super::input::{convert_body_to_storage, read_command_input, read_optional_json};
use super::super::output::{
    print_comment_action, print_comments_human, print_json_or_human, print_text,
};

pub(super) fn dispatch_comment(global: &GlobalArgs, command: CommentCommand) -> Result<()> {
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
            let location: CommentLocation = location.into();
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
            let location: CommentLocation = location.into();
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
            let message = "Comment deletion request accepted.".to_owned();
            print_json_or_human(global.output, &message, |message| print_text(message))
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
