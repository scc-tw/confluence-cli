use std::fs;

use crate::app;
use crate::application::runtime::ensure_writable;
use crate::convert::{apply_unified_patch, convert_text};
use crate::domain::BodyFormat;
use crate::infrastructure::content_io;
use crate::support::Result;

use super::super::args::{GlobalArgs, PageCommand};
use super::super::bootstrap::load_runtime_and_api;
use super::super::input::{
    convert_body_to_storage, default_export_dir, parse_move_target, read_command_input,
};
use super::super::output::{
    print_archive_task, print_export_result_human, print_json_or_human, print_page_action,
    print_page_summaries_human, print_page_summary_human, print_text,
};

pub(super) fn dispatch_page(global: &GlobalArgs, command: PageCommand) -> Result<()> {
    let (runtime, api) = load_runtime_and_api(global)?;

    match command {
        PageCommand::Read { page, format } => {
            let format: BodyFormat = format.into();
            if matches!(format, BodyFormat::Markdown) {
                let body = app::page_read(&api, &page, BodyFormat::Storage)?;
                let output =
                    convert_text(&body.content, BodyFormat::Storage, BodyFormat::Markdown)?;
                print_json_or_human(global.output, &output, |output| print_text(output))
            } else {
                let body = app::page_read(&api, &page, format)?;
                print_json_or_human(global.output, &body.content, |output| print_text(output))
            }
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
            let message = "Page deletion request accepted.".to_owned();
            print_json_or_human(global.output, &message, |message| print_text(message))
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
                print_json_or_human(global.output, &updated, |output| print_text(output))
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
            let result = content_io::export_page_to_dir(
                &api,
                &page,
                &dest,
                format.into(),
                !skip_attachments,
            )?;
            print_json_or_human(global.output, &result, print_export_result_human)
        }
    }
}
