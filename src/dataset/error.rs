//! Error types for dataset operations

use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatasetError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Version not found: {0}")]
    VersionNotFound(String),
}
