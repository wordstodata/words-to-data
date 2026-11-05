//! What document classes a dataset holds, as its node types say.
//!
//! A class is a projection (`docs/adr/0007`): each work's root node says it, and
//! nothing stores it a second time. No test here reads a class from a work id.

use std::collections::{BTreeMap, HashMap};
use std::process::Command;

use words_to_data::congress::BillDownload;
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId, works_between};
use words_to_data::document::NodeType;
use words_to_data::storage::InMemoryStorage;

const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

const BILL_ID: &str = "119-hr-1";
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
/// The work the committed bill is stored under.
const BILL_WORK: &str = "publiclawdocument_119-21";
const CACHE: &str = "tests/test_data/courtlistener";
/// Boeing Co. v. United States, 537 U.S. 437 (2003).
const BOEING: u64 = 122262;
const TITLE_9: &str = "uscode/title_9";
const TITLE_51: &str = "uscode/title_51";

fn committed_bill() -> BillDownload {
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

/// Title 9 at one release point, and the committed bill.
fn the_code_and_a_bill() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .add_uslm_xml(
            &format!("tests/test_data/usc/{EARLY}/usc09.xml"),
            EARLY,
            None,
        )
        .expect("title 9 should load");
    dataset
        .load_bill_download(&committed_bill())
        .expect("the committed bill should load");
    dataset
}

fn classes_of_the_code_and_a_bill() -> BTreeMap<&'static str, Vec<WorkId>> {
    BTreeMap::from([
        (NodeType::BILL, vec![WorkId::new(BILL_WORK)]),
        (NodeType::USCODE, vec![WorkId::new(TITLE_9)]),
    ])
}

#[test]
fn should_name_each_class_and_its_works_when_a_dataset_holds_the_code_and_a_bill() {
    let scope = the_code_and_a_bill()
        .scope()
        .expect("the scope should derive");

    assert_eq!(scope.classes(), classes_of_the_code_and_a_bill());
}

/// The SQLite backend reads the root type from its index and not from the
/// tree, so this test holds the two backends to one answer.
#[test]
fn should_name_the_same_classes_when_the_dataset_is_read_from_sqlite() {
    let path = format!("{}/document_classes.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    the_code_and_a_bill()
        .save_to_sqlite(&path)
        .expect("the dataset should save");

    let reopened = Dataset::open_sqlite(&path).expect("the dataset should open");
    let scope = reopened.scope().expect("the scope should derive");

    assert_eq!(scope.classes(), classes_of_the_code_and_a_bill());
}

/// Title 9 at both dates, title 51 at the first date only, and the bill.
fn the_code_at_two_dates_and_a_bill() -> Dataset<InMemoryStorage> {
    let mut dataset = the_code_and_a_bill();
    for (title, date) in [("usc09", LATE), ("usc51", EARLY)] {
        dataset
            .add_uslm_xml(
                &format!("tests/test_data/usc/{date}/{title}.xml"),
                date,
                None,
            )
            .expect("the title should load");
    }
    dataset
}

#[test]
fn should_skip_only_the_code_work_when_a_span_runs_over_a_dataset_with_a_bill() {
    let dataset = the_code_at_two_dates_and_a_bill();

    let between = works_between(&dataset, EARLY, LATE).expect("works should pair");

    assert_eq!(between.skipped, vec![WorkId::new(TITLE_51)]);
    assert_eq!(between.pairs.len(), 1, "title 9 is held on both dates");
}

/// A case is published once, so a span has nothing to say about it, on the same
/// rule as a bill.
#[test]
fn should_leave_a_case_out_when_a_span_runs_over_a_dataset_with_a_case() {
    let read = |name: String| {
        std::fs::read_to_string(format!("{CACHE}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    let cluster = ClusterRecord::from_json(&read(format!("cluster_{BOEING}.json")))
        .expect("the cluster record should read");
    let opinions: Vec<OpinionRecord> = cluster
        .opinion_ids()
        .into_iter()
        .map(|id| {
            OpinionRecord::from_json(&read(format!("opinion_{id}.json")))
                .expect("the opinion record should read")
        })
        .collect();
    let case = case_expression(&cluster, &opinions).expect("the case should build");
    let mut dataset = the_code_at_two_dates_and_a_bill();
    dataset
        .add_expression(case.expression)
        .expect("the case should add");

    let between = works_between(&dataset, EARLY, LATE).expect("works should pair");

    assert_eq!(between.skipped, vec![WorkId::new(TITLE_51)]);
    assert!(
        dataset
            .scope()
            .expect("the scope should derive")
            .classes()
            .contains_key(NodeType::JUDICIAL),
        "the dataset holds the case"
    );
}

/// The message an operator reads. `redesignations` is the command that takes a
/// span, and the bill is in the dataset because that command needs it.
#[test]
fn should_name_the_code_work_and_not_the_bill_when_a_between_run_reports_what_it_skipped() {
    let path = format!("{}/between_skipped.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    the_code_at_two_dates_and_a_bill()
        .save_to_sqlite(&path)
        .expect("the dataset should save");

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["redesignations", &path, "--bill-id", BILL_ID])
        .args(["--between", EARLY, LATE])
        .output()
        .expect("the binary should run");

    let printed = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "the run should exit zero: {printed}"
    );
    assert!(
        printed.contains(&format!(
            "Skipping 1 work(s) not held at both {EARLY} and {LATE}: {TITLE_51}"
        )),
        "the run should name the Code work, got:\n{printed}"
    );
    assert!(
        !printed.contains(BILL_WORK),
        "the run should not name the bill, got:\n{printed}"
    );
}

/// A document is the bill's own because of what it says and its node type, so
/// the name of its work does not matter.
#[test]
fn should_find_a_bill_document_when_its_work_id_follows_another_convention() {
    let dataset = the_code_and_a_bill();
    let mut law = dataset
        .bill_document(BILL_ID)
        .expect("the lookup should read")
        .expect("the bill document should be held");
    law.id = ExpressionId::new(WorkId::new("bill/119-hr-1"), law.id.at.clone());

    let bill = dataset
        .get_bill(BILL_ID)
        .expect("the bill should read")
        .expect("the bill should be held");

    let mut renamed = Dataset::new(DatasetMetadata::default());
    renamed.add_bill(bill).expect("the bill should add");
    renamed
        .add_expression(law)
        .expect("the document should add");

    let found = renamed
        .bill_document(BILL_ID)
        .expect("the lookup should read")
        .expect("the bill document should be found under its new work id");

    assert_eq!(found.id.work, WorkId::new("bill/119-hr-1"));
}
