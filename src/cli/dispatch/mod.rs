mod attachment;
mod comment;
mod convert;
mod page;
mod property;

use crate::support::Result;

use super::args::{Cli, Command};
use super::bootstrap::{config_init, profile_add, profile_list, profile_remove, profile_use};

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
        Command::Page(command) => page::dispatch_page(&cli.global, command),
        Command::Attachment(command) => attachment::dispatch_attachment(&cli.global, command),
        Command::Property(command) => property::dispatch_property(&cli.global, command),
        Command::Comment(command) => comment::dispatch_comment(&cli.global, command),
        Command::Convert(command) => convert::dispatch_convert(&cli.global, command),
    }
}
