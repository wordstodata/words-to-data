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
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, work_roots,
};
use words_to_data::inspect;
use words_to_data::legislature::redesignation::Reader;
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_redesignation::redesignations_stated_in;
use words_to_data::uslm::parser::parse;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// Title 26 before and after `119-hr-1` reached the Code.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

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

/// Title 26 at both release points, and then the bill, which is the order a
/// build takes: the windows exist before a bill is swept against them.
fn dataset_holding_title_26_and_the_bill() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (path, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
        let parsed = parse(path, date).expect("title 26 should parse");
        for root in work_roots(parsed) {
            let work = WorkId::new(root.data.path.to_string());
            dataset
                .add_expression(Expression {
                    id: ExpressionId::new(work, date),
                    label: None,
                    root,
                })
                .expect("the expression should store");
        }
    }
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    dataset
}

#[test]
fn should_name_the_reader_and_the_path_when_a_statement_cannot_be_placed() {
    let mut dataset = dataset_holding_title_26_and_the_bill();
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");

    let report = dataset
        .record_redesignations_stated_in(BILL_ID, &bill.root)
        .expect("the sweep should run");

    assert!(
        !report.unplaced.is_empty(),
        "119-hr-1 states more than title 26 alone can place"
    );
    for statement in &report.unplaced {
        // The rule reader is the only one that reads today. The model reader
        // comes with #154 (ADR 0010).
        assert_eq!(statement.reader, Reader::Rule);

        // The words, and where they sit in the bill, so a reviewer can open
        // them. Eleven of the thirteen the full corpus leaves unplaced have no
        // US Code path at all, so this is the only path they carry.
        assert!(!statement.text.is_empty());
        let path = statement
            .path
            .as_deref()
            .expect("an unplaced statement says where its words sit");
        assert!(
            bill.root.find(path).is_some(),
            "the bill should hold a node at {path}"
        );
    }
}

#[test]
fn should_put_the_unplaced_statements_first_when_it_reports_a_bill() {
    let dataset = dataset_holding_title_26_and_the_bill();

    // The whole report, from the dataset alone: no XML, and no model call.
    let report =
        inspect::redesignation_report(&dataset, None).expect("the report should read the dataset");

    // One row for each link, and one row for each statement no reader placed.
    // The two counts measure different things and are never added (#166).
    assert!(report.totals.statements > 0);
    assert!(report.totals.links > 0);
    assert!(report.totals.unplaced > 0);
    assert_eq!(
        report.rows.len(),
        report.totals.links + report.totals.unplaced
    );

    // Weakest first: nothing placed at all comes before anything placed.
    let first_placed = report
        .rows
        .iter()
        .position(|row| row.placed)
        .expect("title 26 places some of what the bill states");
    assert!(
        report.rows[..first_placed].iter().all(|row| !row.placed),
        "every unplaced row comes before the first placed one"
    );
    assert_eq!(first_placed, report.totals.unplaced);

    // Then the placed rows run from the least corroborated upwards, so a
    // reviewer reads the doubtful handful first (ADR 0010).
    let figures: Vec<f32> = report.rows[first_placed..]
        .iter()
        .map(|row| row.corroboration.expect("a placed row carries a figure"))
        .collect();
    assert!(
        figures.windows(2).all(|pair| pair[0] <= pair[1]),
        "placed rows run from the weakest figure upwards: {figures:?}"
    );

    // Each unplaced row says which bill, where in it, which amendment, the
    // words, which reader failed, and why.
    for row in &report.rows[..first_placed] {
        assert_eq!(row.bill_id, BILL_ID);
        assert!(row.bill_path.is_some());
        assert!(!row.amendment_id.is_empty());
        assert!(!row.clause.is_empty());
        assert_eq!(row.reader, Reader::Rule);
        assert!(row.reason.is_some());
        assert_eq!(row.from_path, None);
        assert_eq!(row.to_path, None);
    }

    // Each placed row names the two paths the provision moved between, and
    // states no reason, because nothing failed.
    for row in &report.rows[first_placed..] {
        assert!(row.from_path.is_some());
        assert!(row.to_path.is_some());
        assert_eq!(row.reason, None);
    }
}
