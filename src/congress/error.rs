use thiserror::Error;

#[derive(Debug, Error)]
pub enum CongressError {
    #[error("Not found: {0}")]
    NotFound(String),

    /// An offline client was asked for an entry its cache has not got. It
    /// names the file it looked for, so the reader knows what to fetch.
    #[error("{0} is not in the cache, and this client reads only the cache")]
    NotCached(String),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
