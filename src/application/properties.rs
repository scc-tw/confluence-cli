use serde_json::Value;

use crate::domain::PageRef;
use crate::support::{ConfluenceCliError, Result};

use super::models::ContentProperty;
use super::ports::PropertiesApi;

pub fn property_list<A: PropertiesApi>(api: &A, page: &PageRef) -> Result<Vec<ContentProperty>> {
    api.list_properties(page)
}

pub fn property_get<A: PropertiesApi>(
    api: &A,
    page: &PageRef,
    key: &str,
) -> Result<ContentProperty> {
    require_property_key(key)?;
    api.get_property(page, key)
}

pub fn property_set<A: PropertiesApi>(
    api: &A,
    page: &PageRef,
    key: &str,
    value: Value,
) -> Result<ContentProperty> {
    require_property_key(key)?;
    api.set_property(page, key, value)
}

pub fn property_delete<A: PropertiesApi>(api: &A, page: &PageRef, key: &str) -> Result<()> {
    require_property_key(key)?;
    api.delete_property(page, key)
}

fn require_property_key(key: &str) -> Result<()> {
    if key.trim().is_empty() {
        Err(ConfluenceCliError::Config(
            "property key must not be empty".to_owned(),
        ))
    } else {
        Ok(())
    }
}
