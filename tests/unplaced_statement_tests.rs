//! An unplaced statement: a redesignation the corpus states and no reader could
//! place, kept where a party reading the dataset can find it.
//!
//! `CONTEXT.md` defines the concept. It carries the words, the reason, the
//! reader that failed, and the path in the source document where the words sit,
//! so a reviewer can open them. Until #196 the dataset did not hold the bill, so
//! there was no path to give. It holds the bill now.
//!
//! Every case here is read out of the committed public law, `119-hr-1`, and the
//! committed release points.

use std::collections::HashMap;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_redesignation::redesignations_stated_in;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// The bill as the Congress client would hand it over, read from the committed
/// cache, so a test exercises the path a build really takes.
fn committed_bill_download() -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: BILL_ID.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: HashMap::new(),
    }
}

/// A dataset holding the bill and nothing else.
fn dataset_holding_the_bill() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    dataset
}

#[test]
fn should_name_the_path_in_the_bill_when_a_statement_is_read_from_the_stored_bill() {
    let dataset = dataset_holding_the_bill();
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");

    let stated = redesignations_stated_in(BILL_ID, &bill.root);
    assert_eq!(stated.len(), 57, "119-hr-1 states 57 redesignations");

    // Each statement says where in the bill its words sit, and the path leads
    // back to a node the dataset holds. That is what a reviewer opens.
    for statement in &stated {
        let path = statement
            .path
            .as_deref()
            .expect("a statement read from the stored bill knows where it sat");
        assert!(
            bill.root.find(path).is_some(),
            "the bill should hold a node at {path}"
        );
    }
}
