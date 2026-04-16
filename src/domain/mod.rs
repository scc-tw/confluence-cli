use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::support::{ConfluenceCliError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteMode {
    Archive,
    Trash,
    Purge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyFormat {
    Storage,
    Markdown,
    Html,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommentLocation {
    Footer,
    Inline,
    Resolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PageId(u64);

impl PageId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageRef {
    Id(PageId),
    Url(String),
}

impl PageRef {
    pub fn parse(input: &str) -> Result<Self> {
        if let Ok(value) = input.parse::<u64>() {
            return Ok(Self::Id(PageId::new(value)));
        }

        if input.starts_with("http://") || input.starts_with("https://") {
            return Ok(Self::Url(input.to_owned()));
        }

        Err(ConfluenceCliError::InvalidPageRef(input.to_owned()))
    }
}

impl FromStr for PageRef {
    type Err = ConfluenceCliError;

    fn from_str(s: &str) -> Result<Self> {
        Self::parse(s)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveTarget {
    Parent(PageRef),
    Before(PageRef),
    After(PageRef),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numeric_page_ref() {
        let page_ref = PageRef::parse("123").expect("page ref should parse");
        assert_eq!(page_ref, PageRef::Id(PageId::new(123)));
    }

    #[test]
    fn parses_url_page_ref() {
        let page_ref = PageRef::parse("https://example.atlassian.net/wiki/spaces/ENG/pages/123")
            .expect("page ref should parse");

        assert_eq!(
            page_ref,
            PageRef::Url("https://example.atlassian.net/wiki/spaces/ENG/pages/123".to_owned())
        );
    }

    #[test]
    fn rejects_invalid_page_ref() {
        let error = PageRef::parse("not-a-page-ref").expect_err("page ref should fail");
        assert!(matches!(error, ConfluenceCliError::InvalidPageRef(_)));
    }
}
