//! Error types for dataset operations

use std::io;
use thiserror::Error;

use crate::dataset::{ExpressionId, WorkId};

#[derive(Error, Debug)]
pub enum DatasetError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Names the work as well as the date. "Not found: 2025-07-18" cannot say
    /// whether the date or the document is the thing this dataset lacks.
    #[error("This dataset holds no expression {0}")]
    ExpressionNotFound(ExpressionId),

    /// A diff needs two expressions of one work. Across two works it would
    /// compare unrelated documents and report the whole of each as changed.
    #[error("A diff needs two expressions of one work, and `{from}` and `{to}` are two works")]
    WorkMismatch { from: WorkId, to: WorkId },

    #[error("Failed to load folder '{0}': folder is empty or unreadable")]
    FolderLoadFailed(String),

    #[error(
        "This dataset was written with schema version {found}, and this build reads version \
         {expected}. Datasets are rebuilt rather than migrated, so regenerate it."
    )]
    SchemaVersionMismatch { found: i32, expected: i32 },

    #[error(
        "This dataset's search index was written before this build and covers only some of the \
         text fields, so a search over it would report law as absent. Regenerate the dataset: \
         `words_to_data convert-dataset <source> {path}`."
    )]
    StaleSearchIndex { path: String },
}
