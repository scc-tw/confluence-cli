mod attachments;
mod comments;
mod dto;
mod pages;
mod properties;

use reqwest::blocking::{Client, RequestBuilder};
use reqwest::Url;

use crate::domain::{CommentLocation, PageId, PageRef};
use crate::profile::AuthKind;
use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone)]
pub struct HttpApiConfig {
    pub domain: String,
    pub protocol: String,
    pub api_path: String,
    pub auth_type: AuthKind,
    pub email: Option<String>,
    pub username: Option<String>,
    pub api_token: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HttpConfluenceApi {
    client: Client,
    profile: HttpApiConfig,
}

impl HttpConfluenceApi {
    pub fn new(profile: HttpApiConfig) -> Result<Self> {
        let client = Client::builder().build().map_err(|error| {
            ConfluenceCliError::Config(format!("failed to build HTTP client: {error}"))
        })?;
        Ok(Self { client, profile })
    }

    fn v1_url(&self, path: &str) -> String {
        format!(
            "{}://{}{}{}",
            self.profile.protocol, self.profile.domain, self.profile.api_path, path
        )
    }

    fn v2_url(&self, path: &str) -> String {
        let prefix = if self.profile.domain.ends_with(".atlassian.net") {
            "/wiki/api/v2"
        } else {
            "/api/v2"
        };

        format!(
            "{}://{}{}{}",
            self.profile.protocol, self.profile.domain, prefix, path
        )
    }

    fn absolute_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            path.to_owned()
        } else {
            format!(
                "{}://{}{}",
                self.profile.protocol, self.profile.domain, path
            )
        }
    }

    fn authed(&self, request: RequestBuilder) -> Result<RequestBuilder> {
        match self.profile.auth_type {
            AuthKind::Basic => {
                let identity = self
                    .profile
                    .email
                    .clone()
                    .or_else(|| self.profile.username.clone())
                    .ok_or_else(|| {
                        ConfluenceCliError::Config(
                            "basic auth requires email or username".to_owned(),
                        )
                    })?;

                let secret = self
                    .profile
                    .api_token
                    .clone()
                    .or_else(|| self.profile.password.clone())
                    .ok_or_else(|| {
                        ConfluenceCliError::Config(
                            "basic auth requires api token or password".to_owned(),
                        )
                    })?;

                Ok(request.basic_auth(identity, Some(secret)))
            }
            AuthKind::Bearer => {
                let token = self.profile.api_token.clone().ok_or_else(|| {
                    ConfluenceCliError::Config("bearer auth requires api token".to_owned())
                })?;
                Ok(request.bearer_auth(token))
            }
            AuthKind::Mtls => Err(ConfluenceCliError::NotImplemented(
                "mTLS HTTP client setup".to_owned(),
            )),
        }
    }

    fn resolve_page_id(&self, page: &PageRef) -> Result<PageId> {
        match page {
            PageRef::Id(page_id) => Ok(*page_id),
            PageRef::Url(url) => {
                self.validate_page_url_domain(url)?;

                if let Some(page_id) = extract_page_id_from_url(url) {
                    return Ok(page_id);
                }

                if let Some(space_key) = extract_space_key_from_overview_url(url) {
                    return self.resolve_space_homepage_id(&space_key).map_err(|error| {
                        ConfluenceCliError::Config(format!(
                            "space overview URL did not resolve to a home page: {url} ({error})"
                        ))
                    });
                }

                Err(ConfluenceCliError::Config(format!(
                    "page URL must include a page id (`/pages/<id>/...` or `?pageId=<id>`): {url}"
                )))
            }
        }
    }

    fn get_page_v1(&self, page_id: PageId, expand: &str) -> Result<dto::PageV1> {
        let request = self.authed(
            self.client
                .get(self.v1_url(&format!("/content/{page_id}")))
                .query(&[("expand", expand)]),
        )?;
        Ok(request.send()?.error_for_status()?.json()?)
    }

    fn get_page_v2(&self, page_id: PageId, body_format: Option<&str>) -> Result<dto::PageV2> {
        let mut request = self.client.get(self.v2_url(&format!("/pages/{page_id}")));
        if let Some(body_format) = body_format {
            request = request.query(&[("body-format", body_format)]);
        }
        let request = self.authed(request)?;
        Ok(request.send()?.error_for_status()?.json()?)
    }

    fn parse_next_start(&self, next: Option<&str>) -> Option<u64> {
        let value = next?;
        let (_, query) = value.split_once('?')?;
        query.split('&').find_map(|pair| {
            let (key, raw) = pair.split_once('=')?;
            if key == "start" {
                raw.parse::<u64>().ok()
            } else {
                None
            }
        })
    }

    fn encode_property_key(&self, key: &str) -> String {
        urlencoding::encode(key).into_owned()
    }

    fn validate_page_url_domain(&self, url: &str) -> Result<()> {
        let parsed =
            Url::parse(url).map_err(|_| ConfluenceCliError::InvalidPageRef(url.to_owned()))?;
        let authority = match parsed.port() {
            Some(port) => format!("{}:{port}", parsed.host_str().unwrap_or_default()),
            None => parsed.host_str().unwrap_or_default().to_owned(),
        };

        if authority.eq_ignore_ascii_case(&self.profile.domain) {
            Ok(())
        } else {
            Err(ConfluenceCliError::Config(format!(
                "page URL domain '{authority}' does not match the active profile domain '{}'",
                self.profile.domain
            )))
        }
    }

    fn resolve_space_homepage_id(&self, space_key: &str) -> Result<PageId> {
        let request = self.authed(
            self.client
                .get(self.v2_url("/spaces"))
                .query(&[("keys", space_key), ("limit", "1")]),
        )?;
        let response: dto::SpacesResponse = request.send()?.error_for_status()?.json()?;
        let homepage_id = response
            .results
            .into_iter()
            .find(|space| space.key == space_key)
            .and_then(|space| space.homepage_id)
            .ok_or_else(|| {
                ConfluenceCliError::Config(format!(
                    "space '{space_key}' does not expose a home page id"
                ))
            })?;

        homepage_id.parse::<u64>().map(PageId::new).map_err(|_| {
            ConfluenceCliError::Config(format!(
                "space '{space_key}' returned an invalid home page id"
            ))
        })
    }
}

fn comment_location_name(location: CommentLocation) -> &'static str {
    match location {
        CommentLocation::Footer => "footer",
        CommentLocation::Inline => "inline",
        CommentLocation::Resolved => "resolved",
    }
}

fn parse_comment_location(value: Option<&str>) -> Option<CommentLocation> {
    match value? {
        "footer" => Some(CommentLocation::Footer),
        "inline" => Some(CommentLocation::Inline),
        "resolved" => Some(CommentLocation::Resolved),
        _ => None,
    }
}

fn validate_same_space(source: &dto::PageV1, target: &dto::PageV1) -> Result<()> {
    let source_space = source.space.as_ref();
    let target_space = target.space.as_ref();
    if let (Some(source_space), Some(target_space)) = (source_space, target_space) {
        let same_id = source_space.id == target_space.id;
        let same_key = source_space.key.is_some()
            && target_space.key.is_some()
            && source_space.key == target_space.key;
        if !same_id && !same_key {
            return Err(ConfluenceCliError::Config(
                "page move across spaces is not supported".to_owned(),
            ));
        }
    }
    Ok(())
}

fn extract_page_id_from_url(url: &str) -> Option<PageId> {
    let parsed = Url::parse(url).ok()?;

    if let Some((_, value)) = parsed.query_pairs().find(|(key, _)| key == "pageId") {
        if let Ok(page_id) = value.parse::<u64>() {
            return Some(PageId::new(page_id));
        }
    }

    let segments: Vec<_> = parsed.path_segments()?.collect();
    for window in segments.windows(2) {
        if window[0] == "pages" {
            if let Ok(page_id) = window[1].parse::<u64>() {
                return Some(PageId::new(page_id));
            }
        }
    }

    None
}

fn extract_space_key_from_overview_url(url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let segments: Vec<_> = parsed.path_segments()?.collect();
    for window in segments.windows(3) {
        if window[0] == "spaces" && window[2] == "overview" {
            return Some(window[1].to_owned());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_page_id_from_query_parameter() {
        let page_id = extract_page_id_from_url(
            "https://example.atlassian.net/wiki/pages/viewpage.action?pageId=12345",
        )
        .expect("page id should be extracted");

        assert_eq!(page_id.get(), 12345);
    }

    #[test]
    fn extracts_page_id_from_pretty_url() {
        let page_id = extract_page_id_from_url(
            "https://example.atlassian.net/wiki/spaces/ENG/pages/99887/some-page",
        )
        .expect("page id should be extracted");

        assert_eq!(page_id.get(), 99887);
    }

    #[test]
    fn extracts_space_key_from_overview_url() {
        let space_key = extract_space_key_from_overview_url(
            "https://example.atlassian.net/wiki/spaces/~abc123/overview",
        )
        .expect("space key should be extracted");

        assert_eq!(space_key, "~abc123");
    }

    #[test]
    fn extracts_space_key_from_overview_url_with_trailing_slash() {
        let space_key = extract_space_key_from_overview_url(
            "https://example.atlassian.net/wiki/spaces/~abc123/overview/",
        )
        .expect("space key should be extracted");

        assert_eq!(space_key, "~abc123");
    }

    #[test]
    fn parses_next_start_from_paged_link() {
        let api = HttpConfluenceApi::new(HttpApiConfig {
            domain: "example.atlassian.net".to_owned(),
            protocol: "https".to_owned(),
            api_path: "/wiki/rest/api".to_owned(),
            auth_type: AuthKind::Bearer,
            email: None,
            username: None,
            api_token: Some("token".to_owned()),
            password: None,
        })
        .expect("api should initialize");

        let start = api.parse_next_start(Some(
            "/wiki/rest/api/content/123/child/attachment?start=100&limit=100",
        ));
        assert_eq!(start, Some(100));
    }
}
