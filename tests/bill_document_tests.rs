//! A bill is a document: one read of its XML, one stored document.
//!
//! Every case here is read out of the committed public law, `119-hr-1`. A bill
//! used to enter a dataset as `Bill { bill_id, amendments }` alone — a flat map
//! keyed by content hash, with no structure and no path into it — while the
//! parser had held the bill's whole hierarchy since #114
//! (`docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md`).

use std::collections::HashMap;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, WorkId};

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";

/// How the dataset names the bill: the id `build-dataset` is given, and the id
/// every amendment hash and every link is minted under.
const BILL_ID: &str = "119-hr-1";

/// How the publisher names the same document, which is what its path is built
/// from.
const BILL_WORK: &str = "publiclawdocument_119-21";

/// The date the bill's own markup says it was approved.
const APPROVED: &str = "2025-07-04";

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "bill document".to_string(),
        description: "one public law, stored as a document".to_string(),
        author: "Words to Data LLC".to_string(),
        source_urls: vec![],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        declaration: None,
    }
}

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

fn dataset_holding_the_bill() -> Dataset<words_to_data::storage::InMemoryStorage> {
    let mut dataset = Dataset::new(metadata());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    dataset
}

#[test]
fn should_hold_the_bill_as_an_expression_when_a_bill_download_is_loaded() {
    let dataset = dataset_holding_the_bill();

    let work = WorkId::new(BILL_WORK);
    let held = dataset
        .expressions(&work)
        .expect("the dataset should answer for the bill's work");

    assert_eq!(held.len(), 1, "a bill is published once");
    assert_eq!(held[0].id.at, APPROVED);
}

#[test]
fn should_keep_the_structure_the_parser_found_when_the_bill_is_stored() {
    let dataset = dataset_holding_the_bill();

    let held = dataset
        .expressions(&WorkId::new(BILL_WORK))
        .expect("the dataset should answer for the bill's work");
    let bill = dataset
        .get_expression(&held[0].id)
        .expect("the expression should read")
        .expect("the expression should be there");

    assert_eq!(bill.root.data.node_type.as_str(), "bill.public_law");
    // The section and the ten titles below `<main>`. Before #114 a public law
    // parsed to its root and nothing else.
    assert_eq!(bill.root.children.len(), 11);
    assert!(
        bill.root
            .find("publiclawdocument_119-21/title_VII/subtitle_A")
            .is_some(),
        "the bill's own nesting should be there to walk"
    );
}
