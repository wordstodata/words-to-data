//! What `validate` says about a bill nobody has run the redesignation step for.
//!
//! #183. Recording is an explicit step over a named window (#181), so a dataset
//! can hold a bill, hold a window that could carry its statements, and hold no
//! link — and before this, `validate` exited zero and printed "OK — no issues".
//! An absence nobody can count is the failure this project refuses everywhere
//! else (`Exclusion`, `ParseReport`, the `unplaced` count of #166).
//!
//! The unit is a pair: a bill and a window. The list is derived from the links
//! the dataset holds and never stored
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//!
//! Every case here is read out of the committed corpus: the public law
//! `119-hr-1` and the two committed release points.

use std::collections::HashMap;
use std::process::Command;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, Format, WorkId, adjacent_expressions,
    work_roots,
};
use words_to_data::inspect;
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::parser::parse;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// Title 26 before and after `119-hr-1` reached the Code.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// Title 9 before and after the same bill: the smallest title in the corpus,
/// and one `119-hr-1` renumbers nothing in.
const TITLE_9_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc09.xml";
const TITLE_9_AFTER: &str = "tests/test_data/usc/2025-07-30/usc09.xml";

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

/// Put one release-point file, as it read on one date, into the dataset.
fn add_release_point(dataset: &mut Dataset<InMemoryStorage>, file: &str, date: &str) {
    let parsed = parse(file, date).expect("the release point should parse");
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

/// A dataset grown in the organic order: a release point, the bill, and then
/// the release point that closes the window.
///
/// The order #180 measured. Nothing re-records a bill that is already loaded,
/// so the dataset holds a window, holds the statements, and holds no link.
fn grown_dataset() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    add_release_point(&mut dataset, TITLE_26_BEFORE, BEFORE);
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    add_release_point(&mut dataset, TITLE_26_AFTER, AFTER);
    dataset
}

/// Run the explicit step over every window the dataset holds, as
/// `build-dataset` does after it has loaded everything (#181).
fn record_over_every_window(dataset: &mut Dataset<InMemoryStorage>) {
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let windows = adjacent_expressions(dataset).expect("the windows should list");
    dataset
        .record_redesignations_over(BILL_ID, &bill.root, &windows)
        .expect("the step should run");
}

#[test]
fn should_exit_non_zero_and_name_the_step_when_the_validate_command_runs() {
    // What #180 measured and #183 exists to repair: this dataset held no link
    // at all, and `validate` exited zero and printed "OK — no issues". A silent
    // absence is bad; an absence the tool certifies as fine is worse.
    let path = format!("{}/unresolved_grown.json", env!("CARGO_TARGET_TMPDIR"));
    grown_dataset()
        .save(&path, Format::Compact)
        .expect("the fixture should save");

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["validate", &path])
        .output()
        .expect("the binary should run");

    assert!(
        !output.status.success(),
        "a dataset with a step outstanding does not pass"
    );
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        said.contains(BILL_ID),
        "the run should name the bill, got:\n{said}"
    );
    assert!(
        said.contains(&format!("--between {BEFORE} {AFTER}")),
        "and the command that closes the gap, got:\n{said}"
    );
}

#[test]
fn should_say_nothing_when_no_statement_could_be_placed_in_the_window() {
    // Title 9 is the smallest title the corpus holds, and `119-hr-1` renumbers
    // nothing in it: every statement is refused, with a reason. That is
    // finished work, not a step waiting to be run, and a report that could not
    // tell the two apart would cry wolf and be ignored.
    let mut dataset = Dataset::new(DatasetMetadata::default());
    add_release_point(&mut dataset, TITLE_9_BEFORE, BEFORE);
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    add_release_point(&mut dataset, TITLE_9_AFTER, AFTER);

    let report = inspect::validate(&dataset).expect("validate should run");

    assert!(
        report.unresolved_redesignations.is_empty(),
        "an honest refusal is not a gap, got {:?}",
        report.unresolved_redesignations
    );
    assert!(report.ok, "and nothing is reported: {:?}", report.issues);
}

#[test]
fn should_say_nothing_when_the_step_has_run_over_the_window() {
    // The repair #180 named: the grown dataset gets the step by hand, and it
    // then holds what a rebuild holds. Finished work is reported as finished.
    let mut dataset = grown_dataset();
    record_over_every_window(&mut dataset);

    let report = inspect::validate(&dataset).expect("validate should run");

    assert!(
        report.unresolved_redesignations.is_empty(),
        "the step has run, got {:?}",
        report.unresolved_redesignations
    );
    assert!(report.ok, "and nothing is reported: {:?}", report.issues);
}

#[test]
fn should_name_the_bill_and_the_window_when_no_step_has_resolved_them() {
    let dataset = grown_dataset();

    let report = inspect::validate(&dataset).expect("validate should run");

    assert!(
        !report.ok,
        "a dataset whose bill was never resolved is not finished, issues: {:?}",
        report.issues
    );
    assert_eq!(
        report.unresolved_redesignations.len(),
        1,
        "one bill and one window, got {:?}",
        report.unresolved_redesignations
    );
    let gap = &report.unresolved_redesignations[0];
    assert_eq!(gap.bill_id, BILL_ID);
    assert_eq!(gap.work.to_string(), "uscode/title_26");
    assert_eq!(gap.from, BEFORE);
    assert_eq!(gap.to, AFTER);
    // The size of the work waiting: 26 of the bill's 57 statements are ones
    // title 26 can place. The other 31 cannot be placed against title 26 at
    // all, and they keep their own reasons rather than being counted here.
    assert_eq!(gap.statements, 26);

    // The line a reader acts on names the bill, the window and the command
    // that closes the gap.
    let line = gap.to_string();
    assert!(
        line.contains(BILL_ID)
            && line.contains("uscode/title_26")
            && line.contains(BEFORE)
            && line.contains(AFTER),
        "the line should name the bill and the window: {line}"
    );

    // And the dataset's own faults are a different list. Nothing is wrong with
    // what this dataset holds; a step has not been run over it.
    assert!(
        report.issues.is_empty(),
        "work outstanding is not a fault in the file, got {:?}",
        report.issues
    );
}
