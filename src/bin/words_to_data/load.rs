//! Open a dataset file by path, auto-detecting the backend from its extension.
//!
//! `.sqlite`/`.db` open lazily as a SQLite-backed dataset; anything else loads
//! as a compact-JSON dataset. Inspect commands then run over either backend via
//! the [`with_dataset!`] macro, which expands the same body for both variants.
//!
//! **Every command takes either form now.** There was a `refuse_sqlite` here
//! that stopped a command which could only write into a compact JSON file. Its
//! last caller was `extract-changes`, and it went when that command learned to
//! write into a database (#180, #195, #199). That command is gone too (#252).

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

/// Where a command that grows a compact JSON dataset writes its result, or a
/// stop when no `--output` was named.
///
/// A compact JSON dataset is written whole: `Dataset::save` calls `fs::write`,
/// which empties the target before it writes. A command that defaulted to its
/// input would therefore destroy that dataset if the write stopped part way,
/// and the dataset can hold hundreds of model calls that cost money to make
/// again. Durability belongs to the store rather than to a serializer (#186),
/// so the compact JSON path refuses instead of trying to be safe.
///
/// SQLite never comes here. A SQLite dataset grows in place, under a
/// transaction, which is the store giving the guarantee.
///
/// Call this **before** the work starts. A run that cannot save its result must
/// not first spend a model call or an API quota.
pub fn output_or_refuse<'a>(dataset: &str, output: Option<&'a str>, command: &str) -> &'a str {
    match output {
        Some(path) if !is_the_same_file(dataset, path) => path,
        _ => {
            eprintln!(
                "{command} grows the dataset, and it will not write back over {dataset}.\n\
                 Name where the result goes:\n    \
                 words_to_data {command} {dataset} ... --output <new file>\n\
                 A compact JSON dataset is written whole, so a write over the input \
                 that stopped part way would destroy it.\n\
                 To grow a dataset in place, convert it to SQLite first:\n    \
                 words_to_data convert-dataset {dataset}"
            );
            std::process::exit(1);
        }
    }
}

/// Whether two paths name one file.
///
/// Compared as the file system resolves them, so `--output` cannot reach the
/// input by another spelling of the same place. A path that does not resolve is
/// compared as it was written: it names no file yet, so it cannot be the input.
fn is_the_same_file(one: &str, other: &str) -> bool {
    match (std::fs::canonicalize(one), std::fs::canonicalize(other)) {
        (Ok(one), Ok(other)) => one == other,
        _ => one == other,
    }
}

/// Open a dataset, choosing the backend from the file extension.
pub fn open(path: &str) -> Result<OpenDataset, DatasetError> {
    if is_sqlite(path) {
        Ok(OpenDataset::Sql(Dataset::open_sqlite(path)?))
    } else {
        Ok(OpenDataset::Mem(load_compact(path)?))
    }
}

/// Read a compact JSON dataset, with a spinner while it reads.
///
/// A real dataset is a gigabyte of JSON and takes many seconds to read, which
/// without a word looks like a command that hangs.
pub fn load_compact(path: &str) -> Result<Dataset<InMemoryStorage>, DatasetError> {
    crate::ui::step(&format!("Open {path}"), || {
        Dataset::load(path, Format::Compact)
    })
}

/// Write a compact JSON dataset, with a spinner while it writes.
pub fn save_compact(dataset: &Dataset<InMemoryStorage>, path: &str) -> Result<(), DatasetError> {
    crate::ui::step(&format!("Save {path}"), || {
        dataset.save(path, Format::Compact)
    })
}

/// Write a dataset in the format its file name asks for: SQLite for `.sqlite`
/// or `.db`, compact JSON for anything else (#288).
///
/// A SQLite output starts from an empty file. Writing into an existing database
/// keeps whatever it already held: rows of a different dataset that share no
/// key survive, and a table from an older build keeps its narrower shape. The
/// command names an output, so producing that output rather than a mixture of
/// it and its predecessor is the whole job. Writing JSON truncates already.
pub fn save(dataset: &Dataset<InMemoryStorage>, path: &str) -> Result<(), DatasetError> {
    if !is_sqlite(path) {
        return save_compact(dataset, path);
    }
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
        _ => {}
    }
    crate::ui::step(&format!("Write {path}"), || dataset.save_to_sqlite(path))
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
