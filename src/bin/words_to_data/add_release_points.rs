//! `words_to_data add-release-points` — put more US Code release points into a
//! dataset that is already there (#180).
//!
//! Until this existed a dataset could not grow: `build-dataset` created, and one
//! more printing of one title meant building everything again, which bought
//! every model call a second time. A release point is not a stored thing — an
//! expression is `(work, date)` — so growth is more keys and no format change
//! (`docs/adr/0003-storage-is-keyed-by-work.md`).
//!
//! **Where the result goes is not the same for the two forms.** A SQLite dataset
//! grows in place, under a transaction. A compact JSON dataset is written whole,
//! so it must be told where to write and is never written back over its input
//! (#186).
//!
//! **It adds release points and runs no step over them.** A window is made here
//! and resolved by `redesignations` and `match-amendments`, which is why the run
//! ends by naming the windows it made and what each one holds.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, Format};
use words_to_data::storage::Storage;

use crate::release_points::{self, DEFAULT_MIRROR_INDEX, Missing, ReleaseSource};

#[derive(ClapArgs)]
pub struct Args {
    /// Path to the dataset to add the release points to (compact JSON or SQLite)
    pub dataset: String,

    /// Release-point dates to add (YYYY-MM-DD), comma-separated
    #[arg(long, value_delimiter = ',', required = true)]
    pub uslm_dates: Vec<String>,

    /// Mirror manifest URL
    #[arg(long, default_value = DEFAULT_MIRROR_INDEX)]
    pub mirror_index: String,

    /// Cache directory for downloaded release points
    /// (default: the shared `<user cache dir>/words_to_data`)
    #[arg(long)]
    pub cache_dir: Option<String>,

    /// Read only the cache, and never reach the network
    ///
    /// A run that needs a release point it has not got fails by name instead of
    /// downloading hundreds of megabytes.
    #[arg(long)]
    pub offline: bool,

    /// Where to write a compact JSON dataset. Required for compact JSON, which
    /// is never written back over its input. Ignored for SQLite, which grows in
    /// place.
    #[arg(long)]
    pub output: Option<String>,
}

pub fn run(args: Args) {
    // Where the result goes: `None` is SQLite, which grows in place. Compact
    // JSON must be told, and it is asked before anything is downloaded.
    let output = if crate::load::is_sqlite(&args.dataset) {
        None
    } else {
        Some(crate::load::output_or_refuse(
            &args.dataset,
            args.output.as_deref(),
            "add-release-points",
        ))
    };

    let source = if args.offline {
        ReleaseSource::cached_only(args.cache_dir.as_deref())
    } else {
        crate::fail::or_exit(
            ReleaseSource::from_mirror(&args.mirror_index, args.cache_dir.as_deref()),
            "Error fetching mirror manifest",
        )
    };

    match output {
        None => {
            let mut dataset =
                crate::fail::or_exit(Dataset::open_sqlite(&args.dataset), "Error opening dataset");
            grow(&mut dataset, &source, &args.uslm_dates);
            println!("\nWrote {}", args.dataset);
        }
        Some(output) => {
            let mut dataset = crate::fail::or_exit(
                Dataset::load(&args.dataset, Format::Compact),
                "Error loading dataset",
            );
            grow(&mut dataset, &source, &args.uslm_dates);
            crate::fail::or_exit(
                dataset.save(output, Format::Compact),
                "Error saving dataset",
            );
            println!("\nWrote {output}");
        }
    }
}

/// Add the release points, and say which ones arrived.
fn grow<S: Storage>(dataset: &mut Dataset<S>, source: &ReleaseSource, dates: &[String]) {
    let added = crate::fail::or_exit(
        release_points::add_all(dataset, source, dates, Missing::Stop),
        "Error adding release points",
    );
    println!(
        "\nAdded {} release point(s): {}",
        added.len(),
        added.join(", ")
    );
}
