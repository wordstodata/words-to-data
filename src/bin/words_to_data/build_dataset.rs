//! `words_to_data build-dataset` — build a dataset from mirrored US Code
//! release points.

use clap::Args as ClapArgs;
use words_to_data::dataset::{Dataset, DatasetMetadata, Declaration, Format};

use crate::release_points::{self, DEFAULT_MIRROR_INDEX, Missing, ReleaseSource};

#[derive(ClapArgs)]
pub struct Args {
    /// Release-point dates to include (YYYY-MM-DD), comma-separated
    #[arg(long, value_delimiter = ',', required = true)]
    pub uslm_dates: Vec<String>,

    /// Output path for the dataset
    pub output: String,

    /// Congress bills to include (e.g. `119-hr-1,119-hr-42`); requires
    /// CONGRESS_API_KEY unless the run is offline
    #[arg(long, value_delimiter = ',')]
    pub bills: Vec<String>,

    /// Mirror manifest URL
    #[arg(long, default_value = DEFAULT_MIRROR_INDEX)]
    pub mirror_index: String,

    /// Cache directory for downloaded release points and Congress responses
    /// (default: the shared `<user cache dir>/words_to_data`)
    #[arg(long)]
    pub cache_dir: Option<String>,

    /// Read only the cache, and never reach the network
    ///
    /// A release point the cache has not got is skipped, as a date the mirror
    /// does not carry is. A bill the cache has not got stops the run.
    #[arg(long)]
    pub offline: bool,

    /// JSON file declaring what this dataset is meant to cover
    ///
    /// A declaration is a statement someone should have to look at, so it is a
    /// file that can be committed and reviewed rather than a flag. Without one
    /// the dataset reports only what it holds.
    #[arg(long)]
    pub declaration: Option<String>,
}

/// Read a declaration file, or `None` when none was given.
///
/// A declaration that will not parse is fatal rather than skipped. Building a
/// dataset that silently declares nothing is how a gap goes unreported, which
/// is the failure this whole feature exists to prevent.
fn read_declaration(path: Option<&String>) -> Option<Declaration> {
    let path = path?;
    let text = crate::fail::or_exit(std::fs::read_to_string(path), "Error reading declaration");
    Some(crate::fail::or_exit(
        serde_json::from_str(&text),
        "Error parsing declaration",
    ))
}

pub fn run(args: Args) {
    let source = if args.offline {
        ReleaseSource::cached_only(args.cache_dir.as_deref())
    } else {
        crate::fail::or_exit(
            ReleaseSource::from_mirror(&args.mirror_index, args.cache_dir.as_deref()),
            "Error fetching mirror manifest",
        )
    };

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "US Code".to_string(),
        description: "US Code release points".to_string(),
        author: "Words to Data LLC".to_string(),
        source_urls: vec![args.mirror_index.clone()],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        declaration: read_declaration(args.declaration.as_ref()),
        // Nothing has run over a dataset that is being created. Each step
        // records itself as it runs.
        method_runs: Vec::new(),
    });

    // The same path `add-release-points` takes, so a dataset that grew holds
    // what a dataset that was built holds. A date the mirror does not carry is
    // reported and skipped: this command builds what it can, and a run that
    // grows a dataset instead stops.
    crate::fail::or_exit(
        release_points::add_all(&mut dataset, &source, &args.uslm_dates, Missing::Skip),
        "Error adding release points",
    );

    // The same path `add-bills` takes, so a dataset that took a bill
    // afterwards holds what a dataset built with it holds (#272).
    if !args.bills.is_empty() {
        let client = crate::congress_bills::client(args.cache_dir.as_deref(), args.offline);
        crate::fail::or_exit(
            crate::congress_bills::add_all(&mut dataset, &client, &args.bills),
            "Error adding bills",
        );
    }

    dataset
        .save(&args.output, Format::Compact)
        .expect("Error saving dataset");
    println!("Wrote {}", args.output);
}
