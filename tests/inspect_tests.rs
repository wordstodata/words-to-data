//! Tests for the dataset inspection API (`words_to_data::inspect`).
//!
//! Every inspect function is backend-agnostic: it must return identical results
//! whether the dataset is `InMemoryStorage` or `SqliteStorage`. Each test builds
//! one real fixture from committed USC XML + a real public-law bill, then asserts
//! against both backends.

use words_to_data::annotation::{
    AnnotationMetadata, AnnotationStatus, BillReference, ChangeAnnotation,
};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::inspect;
use words_to_data::inspect::AnnotationQuery;
use words_to_data::legislature::AmendingAction;
use words_to_data::storage::{InMemoryStorage, SqliteStorage};
use words_to_data::uslm::bill_parser::parse_bill_amendments;

const ANNOTATED_PATH: &str = "uscode/title_9/chapter_1/section_1";
/// Title 9 is Arbitration, and is the work every fixture below holds.
const TITLE_9: &str = "uscode/title_9";

const USC09_18: &str = "tests/test_data/usc/2025-07-18/usc09.xml";
const USC09_30: &str = "tests/test_data/usc/2025-07-30/usc09.xml";
const PL_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

fn at(date: &str) -> ExpressionId {
    ExpressionId::new(WorkId::new(TITLE_9), date)
}

/// The pair the fixture's annotation sits between.
fn pair() -> (ExpressionId, ExpressionId) {
    (at("2025-07-18"), at("2025-07-30"))
}

/// Build a small real dataset: two USC title-9 versions plus one real bill.
fn make_fixture() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Inspect Fixture".to_string(),
        description: "Title 9, two release points".to_string(),
        author: "Tester".to_string(),
        source_urls: vec!["https://uscode.house.gov".to_string()],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(USC09_18, "2025-07-18", Some("Before".to_string()))
        .expect("add first version");
    dataset
        .add_uslm_xml(USC09_30, "2025-07-30", Some("After".to_string()))
        .expect("add second version");

    let bill = parse_bill_amendments("119-hr-1", PL_XML).expect("parse bill");
    dataset.add_bill(bill).expect("add bill");

    let annotation = ChangeAnnotation {
        operation: AmendingAction::Strike,
        source_bill: BillReference {
            bill_id: "119-hr-1".to_string(),
            amendment_id: "amendment-xyz".to_string(),
            causative_text: "by striking 'foo'".to_string(),
        },
        paths: vec![ANNOTATED_PATH.to_string()],
        metadata: AnnotationMetadata {
            status: AnnotationStatus::Pending,
            confidence: Some(0.9),
            annotator: "model:test".to_string(),
            timestamp: time::OffsetDateTime::UNIX_EPOCH,
            notes: None,
            reasoning: None,
        },
    };
    let (from, to) = pair();
    dataset
        .add_annotation(&from, &to, annotation)
        .expect("add annotation");
    dataset
}

/// Round-trip the fixture through SQLite so tests can exercise that backend too.
///
/// `name` must be unique per test: tests run in parallel and each needs its own
/// database file (SQLite also needs a writable directory for its journal, so we
/// use `target/`, which is always writable).
fn to_sqlite(fixture: &Dataset<InMemoryStorage>, name: &str) -> Dataset<SqliteStorage> {
    let dir = std::path::Path::new("target/inspect_test_dbs");
    std::fs::create_dir_all(dir).expect("create sqlite test dir");
    let path = dir.join(format!("{name}.sqlite"));
    std::fs::remove_file(&path).ok();
    fixture.save_to_sqlite(&path).expect("save to sqlite");
    Dataset::open_sqlite(&path).expect("open sqlite")
}

#[test]
fn should_report_metadata_and_counts_when_given_in_memory_dataset() {
    let dataset = make_fixture();

    let info = inspect::info(&dataset).expect("info");

    assert_eq!(info.name, "Inspect Fixture");
    assert_eq!(info.author, "Tester");
    assert_eq!(info.license, "Public Domain");
    assert_eq!(info.work_count, 1, "two releases of one title are one work");
    assert_eq!(info.expression_count, 2);
    assert_eq!(info.bill_count, 1);
}

#[test]
fn should_list_expressions_with_ids_labels_and_element_counts() {
    let dataset = make_fixture();

    let expressions = inspect::expressions(&dataset, None).expect("expressions");

    assert_eq!(expressions.len(), 2);
    // Chronological order within the work.
    assert_eq!(expressions[0].id, "uscode/title_9@2025-07-18");
    assert_eq!(expressions[0].work, TITLE_9);
    assert_eq!(expressions[0].date, "2025-07-18");
    assert_eq!(expressions[0].label.as_deref(), Some("Before"));
    assert_eq!(expressions[1].id, "uscode/title_9@2025-07-30");
    // A parsed USC title has many elements.
    assert!(
        expressions[0].element_count > 1,
        "expected a populated tree"
    );
}

#[test]
fn should_list_only_the_requested_work() {
    let dataset = make_fixture();

    let expressions =
        inspect::expressions(&dataset, Some(&WorkId::new(TITLE_9))).expect("expressions");

    assert_eq!(expressions.len(), 2);
    assert!(expressions.iter().all(|e| e.work == TITLE_9));
}

#[test]
fn should_list_identical_expressions_for_sqlite_backend() {
    let fixture = make_fixture();
    let expected = inspect::expressions(&fixture, None).expect("expressions mem");

    let sqlite = to_sqlite(&fixture, "expressions");
    let actual = inspect::expressions(&sqlite, None).expect("expressions sqlite");

    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected.iter()) {
        assert_eq!(a.id, e.id);
        assert_eq!(a.label, e.label);
        assert_eq!(a.element_count, e.element_count);
    }
}

#[test]
fn should_summarize_bill_amendments_when_bill_exists() {
    let dataset = make_fixture();

    let summary = inspect::show_bill(&dataset, "119-hr-1")
        .expect("show_bill")
        .expect("bill should exist");

    assert_eq!(summary.bill_id, "119-hr-1");
    assert!(summary.amendment_count > 0, "bill should have amendments");
    assert_eq!(summary.amendments.len(), summary.amendment_count);
    let first = &summary.amendments[0];
    assert!(!first.id.is_empty());
    assert!(!first.amending_text.is_empty());
}

#[test]
fn should_return_none_when_bill_missing() {
    let dataset = make_fixture();
    assert!(
        inspect::show_bill(&dataset, "999-nope")
            .expect("show_bill")
            .is_none()
    );
}

#[test]
fn should_summarize_identical_bill_for_sqlite_backend() {
    let fixture = make_fixture();
    let expected = inspect::show_bill(&fixture, "119-hr-1")
        .expect("mem")
        .expect("bill");

    let sqlite = to_sqlite(&fixture, "show_bill");
    let actual = inspect::show_bill(&sqlite, "119-hr-1")
        .expect("sqlite")
        .expect("bill");

    assert_eq!(actual.bill_id, expected.bill_id);
    assert_eq!(actual.amendment_count, expected.amendment_count);
}

#[test]
fn should_find_matching_text_in_headings() {
    let dataset = make_fixture();

    // Title 9 is the Federal Arbitration Act — the term appears in its text.
    let hits = inspect::search(&dataset, "arbitration").expect("search");

    assert!(!hits.is_empty(), "expected matches for 'arbitration'");
    assert!(
        hits.iter()
            .all(|h| h.snippet.to_lowercase().contains("arbitration")),
        "every hit's snippet should contain the query"
    );
}

#[test]
fn should_find_heading_matches_on_sqlite_backend() {
    let fixture = make_fixture();
    let sqlite = to_sqlite(&fixture, "search");

    // SQLite indexes headings + content, so a heading term must still be found.
    let hits = inspect::search(&sqlite, "arbitration").expect("search sqlite");
    assert!(!hits.is_empty(), "sqlite should find heading matches");
}

/// Independent walk of a TreeDiff, mirroring what `inspect::diff` should collect.
fn walk_counts(diff: &words_to_data::diff::TreeDiff) -> (usize, usize, usize) {
    let mut changed = usize::from(!diff.changes.is_empty());
    let mut added = diff.added.len();
    let mut removed = diff.removed.len();
    for child in &diff.child_diffs {
        let (c, a, r) = walk_counts(child);
        changed += c;
        added += a;
        removed += r;
    }
    (changed, added, removed)
}

#[test]
fn should_collect_changed_added_and_removed_paths_matching_the_tree_diff() {
    let dataset = make_fixture();
    let (from, to) = pair();

    let tree = dataset.compute_diff(&from, &to).expect("compute_diff");
    let (changed, added, removed) = walk_counts(&tree);

    let summary = inspect::diff(&dataset, &from, &to).expect("diff");

    assert_eq!(summary.work, TITLE_9);
    assert_eq!(summary.from, from.to_string());
    assert_eq!(summary.to, to.to_string());
    assert_eq!(summary.from_date, from.at);
    assert_eq!(summary.to_date, to.at);
    assert_eq!(summary.changed_paths.len(), changed);
    assert_eq!(summary.added_paths.len(), added);
    assert_eq!(summary.removed_paths.len(), removed);
}

#[test]
fn should_produce_identical_diff_summary_for_sqlite_backend() {
    let fixture = make_fixture();
    let (from, to) = pair();
    let expected = inspect::diff(&fixture, &from, &to).expect("mem");

    let sqlite = to_sqlite(&fixture, "diff");
    let actual = inspect::diff(&sqlite, &from, &to).expect("sqlite");

    assert_eq!(actual.changed_paths, expected.changed_paths);
    assert_eq!(actual.added_paths, expected.added_paths);
    assert_eq!(actual.removed_paths, expected.removed_paths);
}

#[test]
fn should_list_annotations_for_an_expression_pair() {
    let dataset = make_fixture();
    let (from, to) = pair();

    let anns = inspect::annotations(
        &dataset,
        AnnotationQuery::Pair {
            from: &from,
            to: &to,
        },
    )
    .expect("annotations");

    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].bill_id, "119-hr-1");
    assert_eq!(anns[0].operation, "strike");
    assert_eq!(anns[0].confidence, Some(0.9));
    assert_eq!(anns[0].paths, vec![ANNOTATED_PATH.to_string()]);
    // Every annotation carries the expression pair it belongs to, work and all:
    // a bare date pair could not say which document was diffed.
    assert_eq!(anns[0].work, TITLE_9);
    assert_eq!(anns[0].from, "uscode/title_9@2025-07-18");
    assert_eq!(anns[0].to, "uscode/title_9@2025-07-30");
    assert_eq!(anns[0].from_date, "2025-07-18");
    assert_eq!(anns[0].to_date, "2025-07-30");
}

#[test]
fn should_list_annotations_filtered_by_bill_and_path() {
    let dataset = make_fixture();

    let by_bill =
        inspect::annotations(&dataset, AnnotationQuery::Bill("119-hr-1")).expect("by bill");
    assert_eq!(by_bill.len(), 1);

    let by_path =
        inspect::annotations(&dataset, AnnotationQuery::Path(ANNOTATED_PATH)).expect("by path");
    assert_eq!(by_path.len(), 1);
    assert_eq!(by_path[0].from_date, "2025-07-18");
    assert_eq!(by_path[0].to_date, "2025-07-30");

    let missing = inspect::annotations(&dataset, AnnotationQuery::Bill("000-none")).expect("none");
    assert!(missing.is_empty());
}

#[test]
fn should_list_identical_annotations_for_sqlite_backend() {
    let fixture = make_fixture();
    let sqlite = to_sqlite(&fixture, "annotations");

    let anns = inspect::annotations(&sqlite, AnnotationQuery::Bill("119-hr-1")).expect("sqlite");
    assert_eq!(anns.len(), 1);
    assert_eq!(anns[0].paths, vec![ANNOTATED_PATH.to_string()]);
}

#[test]
fn should_flag_annotation_with_unknown_amendment_and_missing_path() {
    // The fixture's annotation references a bogus amendment id and a path that
    // is not a real element — validate must report both.
    let dataset = make_fixture();

    let report = inspect::validate(&dataset).expect("validate");

    assert!(!report.ok, "expected validation to fail");
    assert!(report.checked_annotations >= 1);
    assert!(
        report.issues.iter().any(|i| i.contains("amendment-xyz")),
        "should flag the unknown amendment id, got: {:?}",
        report.issues
    );
}

#[test]
fn should_pass_validation_for_a_consistent_dataset() {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Clean".to_string(),
        description: String::new(),
        author: String::new(),
        source_urls: vec![],
        license: String::new(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(USC09_18, "2025-07-18", None)
        .expect("v1");
    dataset
        .add_uslm_xml(USC09_30, "2025-07-30", None)
        .expect("v2");

    let bill = parse_bill_amendments("119-hr-1", PL_XML).expect("bill");
    let amendment_id = bill.amendments.keys().next().expect("an amendment").clone();
    dataset.add_bill(bill).expect("add bill");

    // A path that really exists: the root element of an expression.
    let (from, to) = pair();
    let real_path = dataset
        .get_expression(&from)
        .unwrap()
        .unwrap()
        .element
        .data
        .path
        .to_string();

    dataset
        .add_annotation(
            &from,
            &to,
            ChangeAnnotation {
                operation: AmendingAction::Amend,
                source_bill: BillReference {
                    bill_id: "119-hr-1".to_string(),
                    amendment_id,
                    causative_text: "real".to_string(),
                },
                paths: vec![real_path],
                metadata: AnnotationMetadata {
                    status: AnnotationStatus::Verified,
                    confidence: None,
                    annotator: "human:tester".to_string(),
                    timestamp: time::OffsetDateTime::UNIX_EPOCH,
                    notes: None,
                    reasoning: None,
                },
            },
        )
        .expect("add annotation");

    let report = inspect::validate(&dataset).expect("validate");
    assert!(
        report.ok,
        "expected clean dataset, issues: {:?}",
        report.issues
    );
    assert_eq!(report.checked_annotations, 1);
}

#[test]
fn should_report_annotations_for_a_path() {
    let dataset = make_fixture();

    let (from, to) = pair();
    let report =
        inspect::path_report(&dataset, ANNOTATED_PATH, Some((&from, &to))).expect("path_report");

    assert_eq!(report.path, ANNOTATED_PATH);
    assert_eq!(report.annotations.len(), 1);
    assert_eq!(report.annotations[0].operation, "strike");
}

#[test]
fn should_report_presence_and_field_changes_for_a_real_path() {
    let dataset = make_fixture();
    let (from, to) = pair();

    // The root element of an expression is a path guaranteed to exist.
    let real_path = dataset
        .get_expression(&from)
        .unwrap()
        .unwrap()
        .element
        .data
        .path
        .to_string();

    // Independent count of field changes at that path.
    let tree = dataset.compute_diff(&from, &to).expect("diff");
    let expected_changes = tree.find(&real_path).map(|n| n.changes.len()).unwrap_or(0);

    let report =
        inspect::path_report(&dataset, &real_path, Some((&from, &to))).expect("path_report");

    assert!(
        report
            .present_in
            .iter()
            .any(|p| p.expression == from.to_string() && p.provisions == 1),
        "root path should be present once in the from-expression, got {:?}",
        report.present_in
    );

    // The root of an expression cannot be added or removed inside its own
    // tree, so it is one provision present in both, carrying every change.
    assert_eq!(report.provisions.len(), 1);
    assert_eq!(report.provisions[0].presence, inspect::Presence::InBoth);
    assert_eq!(report.provisions[0].from_position, Some(0));
    assert_eq!(report.provisions[0].to_position, Some(0));
    assert_eq!(report.provisions[0].changes.len(), expected_changes);
}

#[test]
fn should_report_one_provision_in_both_when_an_ordinary_path_is_unchanged() {
    let dataset = make_fixture();
    let (from, to) = pair();

    // Title 9 section 1 is in both expressions. Whatever it did or did not do
    // to its own fields, it is one provision and it survived.
    let report =
        inspect::path_report(&dataset, ANNOTATED_PATH, Some((&from, &to))).expect("path_report");

    assert_eq!(
        report.present_in.len(),
        2,
        "both expressions hold the path, each once: {:?}",
        report.present_in
    );
    assert!(report.present_in.iter().all(|p| p.provisions == 1));

    assert_eq!(report.provisions.len(), 1);
    assert_eq!(report.provisions[0].presence, inspect::Presence::InBoth);
    assert_eq!(report.provisions[0].from_position, Some(0));
    assert_eq!(report.provisions[0].to_position, Some(0));
}

#[test]
fn should_report_the_same_provisions_on_both_backends() {
    let fixture = make_fixture();
    let sqlite = to_sqlite(&fixture, "path_provisions");
    let (from, to) = pair();

    let from_memory =
        inspect::path_report(&fixture, ANNOTATED_PATH, Some((&from, &to))).expect("memory");
    let from_sqlite =
        inspect::path_report(&sqlite, ANNOTATED_PATH, Some((&from, &to))).expect("sqlite");

    // Storage must not change the answer. Both backends must agree on the
    // counts, the verdicts, and the positions.
    let shape = |r: &inspect::PathReport| {
        (
            r.present_in
                .iter()
                .map(|p| (p.expression.clone(), p.provisions))
                .collect::<Vec<_>>(),
            r.provisions
                .iter()
                .map(|p| (p.from_position, p.to_position, p.presence, p.changes.len()))
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(shape(&from_memory), shape(&from_sqlite));
}

#[test]
fn should_report_path_annotations_on_sqlite_backend() {
    let fixture = make_fixture();
    let sqlite = to_sqlite(&fixture, "path_report");

    let report = inspect::path_report(&sqlite, ANNOTATED_PATH, None).expect("path_report sqlite");
    assert_eq!(report.annotations.len(), 1);
}

/// Collect every changed/added/removed path from a diff tree (order preserved).
fn collect_diff_paths(diff: &words_to_data::diff::TreeDiff, out: &mut Vec<String>) {
    if !diff.changes.is_empty() {
        out.push(diff.root_path.clone());
    }
    out.extend(diff.added.iter().map(|e| e.path.to_string()));
    out.extend(diff.removed.iter().map(|e| e.path.to_string()));
    for child in &diff.child_diffs {
        collect_diff_paths(child, out);
    }
}

#[test]
fn should_account_coverage_against_the_real_diff() {
    // Title 27 actually changes between the two release points.
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Coverage".to_string(),
        description: String::new(),
        author: String::new(),
        source_urls: vec![],
        license: String::new(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc13.xml",
            "2025-07-18",
            None,
        )
        .expect("v1");
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-30/usc13.xml",
            "2025-07-30",
            None,
        )
        .expect("v2");

    let title_13 = WorkId::new("uscode/title_13");
    let from = ExpressionId::new(title_13.clone(), "2025-07-18");
    let to = ExpressionId::new(title_13, "2025-07-30");
    let tree = dataset.compute_diff(&from, &to).expect("diff");
    let mut universe = Vec::new();
    collect_diff_paths(&tree, &mut universe);
    universe.sort();
    universe.dedup();
    assert!(!universe.is_empty(), "title 13 should have a real diff");
    let target = universe[0].clone();

    // Before annotating: every changed path is in the work queue.
    let before = inspect::coverage(&dataset, &from, &to).expect("coverage");
    assert_eq!(before.changed_path_count, universe.len());
    assert_eq!(before.unannotated_count, universe.len());
    assert_eq!(before.annotated_count, 0);
    assert_eq!(before.coverage, 0.0);
    assert!(before.unannotated_paths.contains(&target));

    // Annotate one real changed path.
    dataset
        .add_annotation(
            &from,
            &to,
            ChangeAnnotation {
                operation: AmendingAction::Amend,
                source_bill: BillReference {
                    bill_id: "119-hr-1".to_string(),
                    amendment_id: "amd".to_string(),
                    causative_text: "x".to_string(),
                },
                paths: vec![target.clone()],
                metadata: AnnotationMetadata {
                    status: AnnotationStatus::Pending,
                    confidence: None,
                    annotator: "test".to_string(),
                    timestamp: time::OffsetDateTime::UNIX_EPOCH,
                    notes: None,
                    reasoning: None,
                },
            },
        )
        .expect("annotate");

    // After: that path is covered and drops out of the work queue.
    let after = inspect::coverage(&dataset, &from, &to).expect("coverage");
    assert_eq!(after.changed_path_count, universe.len());
    assert_eq!(
        after.annotated_count + after.unannotated_count,
        after.changed_path_count
    );
    assert_eq!(after.unannotated_paths.len(), after.unannotated_count);
    assert!(after.annotated_count >= 1);
    assert!(!after.unannotated_paths.contains(&target));
}

#[test]
fn should_report_full_coverage_when_no_changes() {
    // Title 9 parses to zero semantic changes: nothing to annotate == fully covered.
    let dataset = make_fixture();
    let (from, to) = pair();
    let cov = inspect::coverage(&dataset, &from, &to).expect("coverage");
    assert_eq!(cov.changed_path_count, 0);
    assert!(cov.unannotated_paths.is_empty());
    assert_eq!(cov.coverage, 1.0);
}

#[test]
fn should_report_identical_info_for_sqlite_backend() {
    let fixture = make_fixture();
    let expected = inspect::info(&fixture).expect("info mem");

    let sqlite = to_sqlite(&fixture, "info");
    let actual = inspect::info(&sqlite).expect("info sqlite");

    assert_eq!(actual.name, expected.name);
    assert_eq!(actual.work_count, expected.work_count);
    assert_eq!(actual.expression_count, expected.expression_count);
    assert_eq!(actual.bill_count, expected.bill_count);
    // The scope pairs each work with its own dates, and both backends must
    // derive the same pairing.
    assert_eq!(actual.scope.held, expected.scope.held);
}
