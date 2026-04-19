use reqwest::StatusCode;
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;

use crate::application::models::ContentProperty;
use crate::application::ports::PropertiesApi;
use crate::domain::PageRef;
use crate::support::{ConfluenceCliError, Result};

use super::HttpConfluenceApi;
use super::dto::{PropertyListResponse, PropertyV1};

impl PropertiesApi for HttpConfluenceApi {
    fn list_properties(&self, page: &PageRef) -> Result<Vec<ContentProperty>> {
        let page_id = self.resolve_page_id(page)?;
        let mut start = 0;
        let mut properties = Vec::new();
        loop {
            let request = self.authed(
                self.client
                    .get(self.v1_url(&format!("/content/{page_id}/property")))
                    .query(&[("start", start), ("limit", 100_u64)]),
            )?;
            let response: PropertyListResponse = request.send()?.error_for_status()?.json()?;
            properties.extend(response.results.into_iter().map(PropertyV1::into_property));
            if let Some(next) = self.parse_next_start(response.links.next.as_deref()) {
                start = next;
            } else {
                break;
            }
        }
        Ok(properties)
    }

    fn get_property(&self, page: &PageRef, key: &str) -> Result<ContentProperty> {
        let page_id = self.resolve_page_id(page)?;
        let key = self.encode_property_key(key);
        let request = self.authed(
            self.client
                .get(self.v1_url(&format!("/content/{page_id}/property/{key}"))),
        )?;
        let response: PropertyV1 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_property())
    }

    fn set_property(&self, page: &PageRef, key: &str, value: Value) -> Result<ContentProperty> {
        let page_id = self.resolve_page_id(page)?;
        let next_version = match self.get_property(page, key) {
            Ok(existing) => existing.version + 1,
            Err(ConfluenceCliError::Http(error))
                if error.status() == Some(StatusCode::NOT_FOUND) =>
            {
                1
            }
            Err(error) => return Err(error),
        };
        let payload = serde_json::json!({
            "key": key,
            "value": value,
            "version": { "number": next_version }
        });
        let key = self.encode_property_key(key);
        let request = self.authed(
            self.client
                .put(self.v1_url(&format!("/content/{page_id}/property/{key}")))
                .header(CONTENT_TYPE, "application/json")
                .json(&payload),
        )?;
        let response: PropertyV1 = request.send()?.error_for_status()?.json()?;
        Ok(response.into_property())
    }

    fn delete_property(&self, page: &PageRef, key: &str) -> Result<()> {
        let page_id = self.resolve_page_id(page)?;
        let key = self.encode_property_key(key);
        let request = self.authed(
            self.client
                .delete(self.v1_url(&format!("/content/{page_id}/property/{key}"))),
        )?;
        request.send()?.error_for_status()?;
        Ok(())
    }
}
