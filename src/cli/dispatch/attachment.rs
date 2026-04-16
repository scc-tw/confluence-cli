use std::path::PathBuf;

use crate::app;
use crate::application::runtime::ensure_writable;
use crate::infrastructure::content_io;
use crate::support::Result;

use super::super::args::{AttachmentCommand, GlobalArgs};
use super::super::bootstrap::load_runtime_and_api;
use super::super::output::{
    print_attachments_human, print_json_or_human, print_paths_human, print_text,
};

pub(super) fn dispatch_attachment(global: &GlobalArgs, command: AttachmentCommand) -> Result<()> {
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
            let paths = content_io::download_attachments_to_dir(&api, &page, &dest)?;
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
            let attachments = content_io::upload_attachment_from_path(
                &api, &page, file, comment, minor_edit, replace,
            )?;
            print_json_or_human(global.output, &attachments, |attachments| {
                print_attachments_human(attachments)
            })
        }
        AttachmentCommand::Delete { page, attachment } => {
            ensure_writable(&runtime)?;
            app::attachment_delete(&api, &page, &attachment)?;
            let message = "Attachment deletion request accepted.".to_owned();
            print_json_or_human(global.output, &message, |message| print_text(message))
        }
    }
}
