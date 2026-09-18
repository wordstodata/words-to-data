//! `words_to_data build-dataset` — build a dataset from mirrored US Code
//! release points.

use clap::Args as ClapArgs;
use words_to_data::congress::CongressClient;
use words_to_data::dataset::{Dataset, DatasetMetadata, Declaration, Format};

use crate::release_points::{self, DEFAULT_MIRROR_INDEX, Missing, ReleaseSource};

#[derive(ClapArgs)]
pub struct Args {
    /// Release-point dates to include (YYYY-MM-DD), comma-separated
    #[arg(long, value_delimiter = ',', required = true)]
    pub uslm_dates: Vec<String>,

    /// Output path for the dataset
    pub output: String,

    /// Congress bills to include (e.g. `119-hr-1,119-hr-42`); requires CONGRESS_API_KEY
    #[arg(long, value_delimiter = ',')]
    pub bills: Vec<String>,

    /// Mirror manifest URL
    #[arg(long, default_value = DEFAULT_MIRROR_INDEX)]
    pub mirror_index: String,

    /// Cache directory for downloaded release points
    /// (default: the shared `<user cache dir>/words_to_data`)
    #[arg(long)]
    pub cache_dir: Option<String>,

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
    let source = crate::fail::or_exit(
        ReleaseSource::from_mirror(&args.mirror_index, args.cache_dir.as_deref()),
        "Error fetching mirror manifest",
    );

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "US Code".to_string(),
        description: "US Code release points".to_string(),
        author: "Words to Data LLC".to_string(),
        source_urls: vec![args.mirror_index.clone()],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        declaration: read_declaration(args.declaration.as_ref()),
    });

    // The same path `add-release-points` takes, so a dataset that grew holds
    // what a dataset that was built holds. A date the mirror does not carry is
    // reported and skipped: this command builds what it can, and a run that
    // grows a dataset instead stops.
    crate::fail::or_exit(
        release_points::add_all(&mut dataset, &source, &args.uslm_dates, Missing::Skip),
        "Error adding release points",
    );

    if !args.bills.is_empty() {
        let api_key = std::env::var("CONGRESS_API_KEY").expect(
            "Including bills requires CONGRESS_API_KEY. Get it here: https://api.congress.gov/sign-up/",
        );
        let client = CongressClient::new(api_key, None);
        for bill in &args.bills {
            println!("Downloading bill {bill}...");
            let download = client
                .download_bill(bill)
                .unwrap_or_else(|e| panic!("Error downloading bill {bill}: {e}"));
            dataset
                .load_bill_download(&download)
                .unwrap_or_else(|e| panic!("Error loading bill {bill}: {e}"));
        }
    }

    dataset
        .save(&args.output, Format::Compact)
        .expect("Error saving dataset");
    println!("Wrote {}", args.output);
}
