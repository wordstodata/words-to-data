//! Loading Congress bills into a dataset: the one path that `build-dataset`
//! and `add-bills` both take (#272).
//!
//! **One path, not two.** A dataset built with a bill and a dataset that took
//! the bill afterwards must be the same dataset. When each command had its own
//! loading, the two would drift apart as each was fixed, so the loading lives
//! here and both commands call it.
//!
//! For each bill it fetches the bill, its public-law text, its sponsors and its
//! House votes with their members, stores them, and then records the
//! renumbering statements of the bills it loaded over every window the dataset
//! holds. It runs no other step.

use words_to_data::congress::{BillDownload, CongressClient};
use words_to_data::dataset::{Dataset, adjacent_expressions};
use words_to_data::storage::{DocumentReader, LegislatureReader, LegislatureWriter, Storage};

/// The Congress client for a run: the network and the cache, or the cache only.
///
/// An offline client needs no API key, so a run over a cache that holds every
/// bill it names can happen with no key at all.
pub fn client(cache_dir: Option<&str>, offline: bool) -> CongressClient {
    let cache_dir = cache_dir.map(String::from);
    if offline {
        return CongressClient::cached_only(cache_dir);
    }
    let Ok(api_key) = std::env::var("CONGRESS_API_KEY") else {
        crate::fail::refuse(
            "Loading bills requires CONGRESS_API_KEY, or --offline to read only the \
             cache. Get a key here: https://api.congress.gov/sign-up/",
        );
    };
    CongressClient::new(api_key, cache_dir)
}

/// What one call did with the bills it was given.
pub struct Added {
    /// The bills loaded by this call, in the order given.
    pub loaded: Vec<String>,
    /// The bills the dataset held already. Nothing was fetched for them.
    pub already_held: Vec<String>,
}

/// Load each bill named that the dataset does not hold yet, then record what
/// the loaded bills renumbered.
///
/// **Every bill is fetched before any is stored.** A bill that cannot be
/// fetched stops the run before the dataset changes, so a failed run leaves
/// the dataset as it was.
///
/// A bill the dataset holds already is not fetched again and not loaded again,
/// so a second call with the same bills adds nothing.
pub fn add_all<S: Storage + LegislatureReader + LegislatureWriter>(
    dataset: &mut Dataset<S>,
    client: &CongressClient,
    bills: &[String],
) -> Result<Added, String> {
    let mut to_load: Vec<&String> = Vec::new();
    let mut already_held = Vec::new();
    for bill in bills {
        let held = dataset
            .get_bill(bill)
            .map_err(|e| format!("the dataset's bills could not be read: {e}"))?;
        if held.is_some() {
            already_held.push(bill.clone());
        } else if !to_load.contains(&bill) {
            to_load.push(bill);
        }
    }

    let mut downloads: Vec<BillDownload> = Vec::new();
    for bill in to_load {
        println!("Downloading bill {bill}...");
        let download = client
            .download_bill(bill)
            .map_err(|e| format!("bill {bill} could not be fetched: {e}"))?;
        downloads.push(download);
    }

    let mut loaded = Vec::new();
    for download in &downloads {
        let bill_id = dataset
            .load_bill_download(download)
            .map_err(|e| format!("bill {} could not be loaded: {e}", download.bill_id))?;
        loaded.push(bill_id);
    }

    record_redesignations(dataset, &loaded);

    Ok(Added {
        loaded,
        already_held,
    })
}

/// Record what each bill renumbered, after everything is loaded.
///
/// **The step runs here and not at load time (#181).** Loading a bill loads a
/// bill. Here the dataset holds every release point and every bill of the run,
/// which is the knowledge a load-time call did not have.
///
/// The windows are every window the dataset holds, and the step records each
/// statement in the one window the law acted in (#172). A window that ends
/// before the law's enactment cannot hold its statements, so to name them all
/// is correct for a bill added to a dataset and for a bill loaded in a build.
///
/// Every statement the run cannot place reaches stderr, because the tool's
/// silence must not read as the corpus's silence (#110).
fn record_redesignations<S: Storage + LegislatureReader>(
    dataset: &mut Dataset<S>,
    bills: &[String],
) {
    if bills.is_empty() {
        return;
    }
    let windows = crate::fail::or_exit(
        adjacent_expressions(dataset as &dyn DocumentReader),
        "Error listing the dataset's windows",
    );

    for bill in bills {
        let Some(document) = crate::fail::or_exit(
            dataset.bill_document(bill),
            "Error reading the dataset's bills",
        ) else {
            continue;
        };
        let report = crate::fail::or_exit(
            dataset.record_redesignations_over(bill, &document, &windows),
            "Error recording redesignations",
        );
        report.warn(bill);
    }
}
