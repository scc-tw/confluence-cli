use serde_json::Value;

use crate::app;
use crate::application::runtime::ensure_writable;
use crate::support::{ConfluenceCliError, Result};

use super::super::args::{GlobalArgs, PropertyCommand};
use super::super::bootstrap::load_runtime_and_api;
use super::super::input::read_command_input;
use super::super::output::{
    print_json_or_human, print_properties_human, print_property_human, print_text,
};

pub(super) fn dispatch_property(global: &GlobalArgs, command: PropertyCommand) -> Result<()> {
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
            let message = "Property deletion request accepted.".to_owned();
            print_json_or_human(global.output, &message, |message| print_text(message))
        }
    }
}
