//! Error types for dataset operations

use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DatasetError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("Version not found: {0}")]
    VersionNotFound(String),

    #[error("Failed to load folder '{0}': folder is empty or unreadable")]
    FolderLoadFailed(String),

    #[error(
        "This dataset was written with schema version {found}, and this build reads version \
         {expected}. Datasets are rebuilt rather than migrated, so regenerate it."
    )]
    SchemaVersionMismatch { found: i32, expected: i32 },
}
