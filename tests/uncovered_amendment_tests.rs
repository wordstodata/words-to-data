//! What `validate` says about a window nobody has run the amendment step over.
//!
//! #210, which is #183 one link kind over. A dataset can hold a bill with
//! hundreds of amendments, hold a window those amendments could have reached,
//! and hold no run of a matching method — and before this, `validate` printed
//! "OK — 0 annotation(s) checked, no issues" and exited zero. A half-built
//! dataset and a finished one read the same, which is the fault #183 exists to
//! prevent for redesignations.
//!
//! **The question is whether a run happened, not what it found.** A window the
//! model answered "no match" for has been worked on. On a real corpus four
//! bills of five gave nothing while the run covered all of them, so a report
//! that counted annotations would name four finished bills and be ignored. The
//! answer comes from [`words_to_data::method::MethodRun`], which says which
//! method ran over which window, and from nothing else.
//!
//! Derived, never stored
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//!
//! Every case here is read out of the committed corpus: the public law
//! `119-hr-1` and the three committed release points of title 26.

mod common;

use std::collections::HashMap;
use std::process::Command;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, ExpressionId, Format, WorkId, adjacent_expressions,
};
use words_to_data::inspect;
use words_to_data::legislature::evidence_matching::evidence_method;
use words_to_data::method::Method;
use words_to_data::storage::InMemoryStorage;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// Title 26 at the three committed release points, which make two windows.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const TITLE_26_LATEST: &str = "tests/test_data/usc/2025-08-14/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";
const LATEST: &str = "2025-08-14";
const TITLE_26: &str = "uscode/title_26";

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
    common::add_uslm_xml(dataset, file, date);
}

/// A dataset holding one window of title 26 and the bill, and no run at all.
///
/// What a build leaves behind after `add-release-points` and a bill load: the
/// matching step is explicit and nobody has called it.
fn dataset_with_one_window() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    add_release_point(&mut dataset, TITLE_26_BEFORE, BEFORE);
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    add_release_point(&mut dataset, TITLE_26_AFTER, AFTER);
    dataset
}

/// The same dataset with the third release point, which makes a second window.
///
/// A bill can be covered in one window and not the other, and the corpus holds
/// three release points, so the case is a real one rather than an invented one.
fn dataset_with_two_windows() -> Dataset<InMemoryStorage> {
    let mut dataset = dataset_with_one_window();
    add_release_point(&mut dataset, TITLE_26_LATEST, LATEST);
    dataset
}

/// One expression of title 26, as the dataset names it.
fn title_26_at(date: &str) -> ExpressionId {
    ExpressionId::new(WorkId::new(TITLE_26), date)
}

/// Record that the evidence method ran over every window the dataset holds,
/// which is what `link-by-evidence` records whatever it linked.
fn record_a_matching_run_over_every_window(dataset: &mut Dataset<InMemoryStorage>) {
    record_a_run_over_every_window(dataset, evidence_method());
}

/// Record that one method ran over every window the dataset holds.
fn record_a_run_over_every_window(dataset: &mut Dataset<InMemoryStorage>, method: Method) {
    for (from, to) in adjacent_expressions(dataset).expect("the windows should list") {
        dataset
            .record_method_run(method.clone(), &from, &to)
            .expect("the run should record");
    }
}

/// The method `match-amendments` recorded, before the command was removed
/// (#252). A dataset built then holds runs under this name, and they are still
/// runs.
fn the_removed_model_method() -> Method {
    Method::new("llm choice among scored candidates", 1)
}

/// Run the explicit redesignation step over every window, as `build-dataset`
/// does once it has loaded everything (#181).
///
/// The #183 finding is the other thing that holds the exit code down, so a
/// dataset that must exit zero needs both steps and not only this issue's.
fn record_the_redesignation_step(dataset: &mut Dataset<InMemoryStorage>) {
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let windows = adjacent_expressions(dataset).expect("the windows should list");
    dataset
        .record_redesignations_over(BILL_ID, &bill, &windows)
        .expect("the step should run");
}

#[test]
fn should_name_the_bill_and_the_window_when_no_matching_run_has_covered_them() {
    let dataset = dataset_with_one_window();

    let report = inspect::validate(&dataset).expect("validate should run");

    assert_eq!(
        report.uncovered_amendments.len(),
        1,
        "one bill and one window, got {:?}",
        report.uncovered_amendments
    );
    let gap = &report.uncovered_amendments[0];
    assert_eq!(gap.bill_id, BILL_ID);
    assert_eq!(gap.work.to_string(), "uscode/title_26");
    assert_eq!(gap.from, BEFORE);
    assert_eq!(gap.to, AFTER);
    // The size of the work waiting, and a count a reader can act on: every
    // amendment the bill states, because a run that never happened covered
    // none of them.
    // The same 603 the issue read off a real 63-work dataset, beside the `OK`
    // this check replaces.
    assert_eq!(gap.amendments, 603);

    // The line a reader acts on names the bill, the window, and the command
    // that closes the gap.
    let line = gap.to_string();
    assert!(
        line.contains(BILL_ID)
            && line.contains("uscode/title_26")
            && line.contains(BEFORE)
            && line.contains(AFTER)
            && line.contains("link-by-evidence"),
        "the line should name the bill, the window and the step: {line}"
    );

    assert!(!report.ok, "a dataset with a step outstanding is not ok");
    // And the dataset's own faults are a different list. Nothing is wrong with
    // what this dataset holds; a step has not been run over it.
    assert!(
        report.issues.is_empty(),
        "work outstanding is not a fault in the file, got {:?}",
        report.issues
    );
}

#[test]
fn should_say_nothing_when_a_matching_run_has_covered_the_window() {
    // The case that decides whether this report is worth reading. The run
    // covered the window and wrote no annotation at all — four bills of five
    // read like this on a real corpus. A check that counted annotations would
    // call every one of them unfinished.
    let mut dataset = dataset_with_one_window();
    record_a_matching_run_over_every_window(&mut dataset);

    let report = inspect::validate(&dataset).expect("validate should run");

    assert_eq!(
        report.checked_annotations, 0,
        "the run wrote nothing, which is the whole of the case"
    );
    assert!(
        report.uncovered_amendments.is_empty(),
        "a run that found nothing is still a run, got {:?}",
        report.uncovered_amendments
    );
}

#[test]
fn should_name_the_matching_step_when_the_validate_command_runs() {
    // What the issue measured: `validate` printed "OK — 433 annotation(s)
    // checked, no issues" over a bill of 603 amendments, and exited zero. A
    // reader of that output could not tell a complete dataset from a
    // half-built one.
    let path = format!("{}/uncovered_amendments.json", env!("CARGO_TARGET_TMPDIR"));
    dataset_with_one_window()
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
        said.contains("amendment(s) no matching run has covered"),
        "the run should say what is missing, got:\n{said}"
    );
    assert!(
        said.contains("words_to_data link-by-evidence <dataset>"),
        "and the command that closes the gap, got:\n{said}"
    );
}

#[test]
fn should_exit_zero_when_every_step_has_run_over_the_window() {
    // The other half of the answer. A check that only ever failed would be
    // turned off, so a dataset every step has run over must pass, and it must
    // pass while holding no annotation at all.
    let mut dataset = dataset_with_one_window();
    record_the_redesignation_step(&mut dataset);
    record_a_matching_run_over_every_window(&mut dataset);
    let path = format!("{}/covered_amendments.json", env!("CARGO_TARGET_TMPDIR"));
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["validate", &path])
        .output()
        .expect("the binary should run");

    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "every step has run, got:\n{said}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !said.contains("link-by-evidence"),
        "and no step is named, got:\n{said}"
    );
}

#[test]
fn should_name_only_the_uncovered_window_when_a_bill_is_covered_in_one_of_two() {
    // The answer is per window, not per bill. A run over the earlier window
    // says nothing about the later one, and a report that named the bill once
    // would hide half the work.
    let mut dataset = dataset_with_two_windows();
    dataset
        .record_method_run(evidence_method(), &title_26_at(BEFORE), &title_26_at(AFTER))
        .expect("the run should record");

    let report = inspect::validate(&dataset).expect("validate should run");

    assert_eq!(
        report.uncovered_amendments.len(),
        1,
        "one window is covered and one is not, got {:?}",
        report.uncovered_amendments
    );
    let gap = &report.uncovered_amendments[0];
    assert_eq!(gap.work.to_string(), TITLE_26);
    assert_eq!(gap.from, AFTER, "the covered window must not be named");
    assert_eq!(gap.to, LATEST);
}

#[test]
fn should_say_nothing_when_a_run_of_the_removed_model_method_has_covered_the_window() {
    // A dataset built before #252 holds the runs `match-amendments` recorded.
    // The command is gone and the record is not: the reasoning ran over the
    // window, so the window was worked on (`docs/adr/0005`).
    let mut dataset = dataset_with_one_window();
    record_a_run_over_every_window(&mut dataset, the_removed_model_method());

    let report = inspect::validate(&dataset).expect("validate should run");

    assert!(
        report.uncovered_amendments.is_empty(),
        "a run recorded by an older build is still a run, got {:?}",
        report.uncovered_amendments
    );
}

/// Title 7, which 82 amendments of the bill act in, and title 9, which no
/// amendment addresses, at two committed release points, with the bill.
fn dataset_with_an_addressed_and_an_unaddressed_work() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for date in [BEFORE, AFTER] {
        for title in ["usc07", "usc09"] {
            add_release_point(
                &mut dataset,
                &format!("tests/test_data/usc/{date}/{title}.xml"),
                date,
            );
        }
    }
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    dataset
}

#[test]
fn should_name_no_window_when_link_by_evidence_has_run_even_in_a_work_no_amendment_addresses() {
    // The matcher looks at every window of every work. In a work no amendment
    // addresses it finds nothing to link, and that is still a run over the
    // window. `validate` must not send the reader to run `link-by-evidence`
    // again for nothing (#252).
    let input = format!(
        "{}/link_by_evidence_every_window.json",
        env!("CARGO_TARGET_TMPDIR")
    );
    let linked = format!(
        "{}/link_by_evidence_every_window_linked.json",
        env!("CARGO_TARGET_TMPDIR")
    );
    dataset_with_an_addressed_and_an_unaddressed_work()
        .save(&input, Format::Compact)
        .expect("the fixture should save");

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["link-by-evidence", &input, "--output", &linked])
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "link-by-evidence should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let dataset = Dataset::load(&linked, Format::Compact).expect("the output should load");
    let report = inspect::validate(&dataset).expect("validate should run");

    assert!(
        report.uncovered_amendments.is_empty(),
        "every window was looked at, got {:?}",
        report.uncovered_amendments
    );
}
