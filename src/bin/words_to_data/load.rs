//! Open a dataset file by path, auto-detecting the backend from its extension.
//!
//! `.sqlite`/`.db` open lazily as a SQLite-backed dataset; anything else loads
//! as a compact-JSON dataset. Inspect commands then run over either backend via
//! the [`with_dataset!`] macro, which expands the same body for both variants.

use words_to_data::dataset::{Dataset, DatasetError, Format};
use words_to_data::storage::{InMemoryStorage, SqliteStorage};

/// A dataset opened from disk, tagged with its backend.
pub enum OpenDataset {
    Mem(Dataset<InMemoryStorage>),
    Sql(Dataset<SqliteStorage>),
}

/// True when the path names a SQLite database rather than a JSON dataset.
pub fn is_sqlite(path: &str) -> bool {
    path.ends_with(".sqlite") || path.ends_with(".db")
}

/// Open a dataset, choosing the backend from the file extension.
pub fn open(path: &str) -> Result<OpenDataset, DatasetError> {
    if is_sqlite(path) {
        Ok(OpenDataset::Sql(Dataset::open_sqlite(path)?))
    } else {
        Ok(OpenDataset::Mem(Dataset::load(path, Format::Compact)?))
    }
}

/// Run the same expression against whichever backend was opened.
///
/// ```ignore
/// let ds = load::open(path)?;
/// let info = with_dataset!(ds, d => inspect::info(d))?;
/// ```
macro_rules! with_dataset {
    ($open:expr, $d:ident => $body:expr) => {
        match $open {
            $crate::load::OpenDataset::Mem($d) => $body,
            $crate::load::OpenDataset::Sql($d) => $body,
        }
    };
}

pub(crate) use with_dataset;
