use thiserror::Error;

pub type Result<T> = std::result::Result<T, ConfluenceCliError>;

#[derive(Debug, Error)]
pub enum ConfluenceCliError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("invalid page reference: {0}")]
    InvalidPageRef(String),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("command not implemented yet: {0}")]
    NotImplemented(String),
}
