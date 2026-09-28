//! Which window a renumbering is recorded in (#172).
//!
//! A public law states a renumbering once. With three release points the corpus
//! holds two windows of each work, and a statement can resolve in both: a shift
//! run leaves every path of the run present on every date. Only one window is
//! where the law acted. The rule is `docs/adr/0013`'s window rule, the one the
//! evidence matcher uses:
//!
//! 1. The window ends after the law's enactment date.
//! 2. The text under the statement's container changed between the window's
//!    two ends.
//!
//! The earliest window that meets both is recorded. A later one is named for
//! review and never written as a link.
//!
//! Every case here is read out of the committed corpus: the public law
//! `119-hr-1` and the three committed release points of title 7.

use std::collections::HashMap;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, adjacent_expressions, work_roots,
};
use words_to_data::diff::TreeDiff;
use words_to_data::inspect;
use words_to_data::legislature::redesignation_window::place;
use words_to_data::link::{LinkKind, Target};
use words_to_data::review::{Review, Verdict};
use words_to_data::storage::{InMemoryStorage, LinkReader};
use words_to_data::uslm::bill_redesignation::redesignations_stated_in;
use words_to_data::uslm::parser::parse;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";

/// The three committed release points, oldest first. `119-hr-1` was enacted on
/// 2025-07-04, so both windows end after it.
const DATES: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];

/// 7 U.S.C. 2015(o). Its text is the same on 2025-07-30 and 2025-08-14, and a
/// playtest found `diff` over that window reporting paragraph (8) "changed" and
/// paragraph (7) "added" all the same.
const SUBSECTION_2015_O: &str = "uscode/title_7/chapter_51/section_2015/subsection_o";

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

/// Title 7 at each committed release point, then the bill.
fn title_7_and_the_bill() -> Dataset<InMemoryStorage> {
    title_7_and_the_bill_at(&DATES)
}

/// Title 7 at the release points named, then the bill.
fn title_7_and_the_bill_at(dates: &[&str]) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for date in dates {
        add_title_7_at(&mut dataset, date);
    }
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    dataset
}

/// Put title 7 as it read on one date into a dataset, as a release point.
fn add_title_7_at(dataset: &mut Dataset<InMemoryStorage>, date: &str) {
    let parsed = parse(&format!("tests/test_data/usc/{date}/usc07.xml"), date)
        .expect("title 7 should parse");
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

/// The ids of every renumbering link a dataset holds, sorted.
fn renumbering_link_ids(dataset: &Dataset<InMemoryStorage>) -> Vec<String> {
    let mut ids: Vec<String> = dataset
        .links_by_kind(LinkKind::REDESIGNATED_AS)
        .expect("the links should read")
        .iter()
        .map(|link| link.id())
        .collect();
    ids.sort();
    ids
}

/// The step as `build-dataset` runs it: every window the dataset holds.
fn record_over_every_window(dataset: &mut Dataset<InMemoryStorage>) {
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let windows = adjacent_expressions(dataset).expect("the windows should list");
    dataset
        .record_redesignations_over(BILL_ID, &bill, &windows)
        .expect("the step should run");
}

/// The path of the provision a path sits directly under.
fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(parent, _)| parent)
}

#[test]
fn should_record_no_renumbering_in_a_window_when_the_text_under_its_container_is_identical() {
    let mut dataset = title_7_and_the_bill();
    record_over_every_window(&mut dataset);

    let links = dataset
        .links_by_kind(LinkKind::REDESIGNATED_AS)
        .expect("the links should read");
    assert!(
        !links.is_empty(),
        "the bill renumbers provisions of title 7"
    );

    // Every link says its window. The container is the provision the moved
    // path sits under, and a window in which its text is identical at both
    // ends cannot hold a move.
    for link in &links {
        let Target::Change {
            work,
            path,
            from_date,
            to_date,
        } = &link.subject
        else {
            panic!("a renumbering's subject is a change");
        };
        let container = parent_of(path);
        let at = |date: &str| {
            dataset
                .get_expression(&ExpressionId::new(work.clone(), date))
                .expect("the expression should read")
                .expect("the window's expressions are held")
                .root
                .find(container)
                .cloned()
        };
        // A container held at one end only has changed, which is enough.
        let (Some(earlier), Some(later)) = (at(from_date), at(to_date)) else {
            continue;
        };
        assert!(
            !TreeDiff::from_nodes(&earlier, &later).is_empty(),
            "{path} -> {:?} is recorded over {from_date} -> {to_date}, a window in which \
             nothing under {container} changed",
            link.object
        );
    }
}

#[test]
fn should_place_nothing_in_a_window_when_it_ends_on_the_enactment_date() {
    // `119-hr-1` was enacted on 2025-07-04, before every committed release
    // point, so the corpus holds no window that ends too early. The rule is
    // asked here about a law enacted on the day the first window ends: that
    // window holds the Code as it read before the law, whatever changed in it.
    let dataset = title_7_and_the_bill();
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let stated = redesignations_stated_in(BILL_ID, &bill.root);
    let windows = adjacent_expressions(&dataset).expect("the windows should list");

    let placement = place(&dataset, &stated, DATES[1], &windows).expect("the rule should run");

    let (first, report) = &placement.windows[0];
    assert_eq!(
        first.1.at, DATES[1],
        "the first window ends on {}",
        DATES[1]
    );
    assert!(
        report.resolved.is_empty(),
        "a window that ends on the enactment date holds nothing, got {} renumbering(s)",
        report.resolved.len()
    );
    // And the rule still read something: with the law's real date, the same
    // window holds the renumberings of title 7.
    let real = place(&dataset, &stated, &bill.id.at, &windows).expect("the rule should run");
    assert!(!real.windows[0].1.resolved.is_empty());
}

#[test]
fn should_name_a_later_window_for_review_and_link_nothing_in_it_when_two_windows_show_the_change() {
    // Every title of the committed corpus is identical from 2025-07-30 to
    // 2025-08-14, so no adjacent pair shows a change twice. An operator's span
    // can: `--between 2025-07-18 2025-08-14` names a window that holds the
    // same change as 2025-07-18 -> 2025-07-30. The earlier-ending window is
    // the one recorded, and the other is named for a reviewer.
    let dataset = title_7_and_the_bill();
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let stated = redesignations_stated_in(BILL_ID, &bill.root);
    let work = WorkId::new("uscode/title_7");
    let at = |date: &str| ExpressionId::new(work.clone(), date);
    let first = (at(DATES[0]), at(DATES[1]));
    let wide = (at(DATES[0]), at(DATES[2]));

    let placement = place(
        &dataset,
        &stated,
        &bill.id.at,
        &[wide.clone(), first.clone()],
    )
    .expect("the rule should run");

    let in_window = |window: &(ExpressionId, ExpressionId)| {
        placement
            .windows
            .iter()
            .find(|(held, _)| held == window)
            .map(|(_, report)| report.resolved.len())
            .expect("every window named is reported")
    };
    assert!(
        in_window(&first) > 0,
        "the earlier-ending window is recorded"
    );
    assert_eq!(in_window(&wide), 0, "the later window holds no link");

    let report = placement.report();
    assert!(
        !report.later_windows.is_empty(),
        "the later window is named for review"
    );
    // Each is named for a statement the first window answered for, placed or
    // not: a statement the first window could not place is still the first
    // window's to explain, and the later one is where a reviewer looks next.
    let (_, answered) = placement
        .windows
        .iter()
        .find(|(held, _)| *held == first)
        .expect("the first window is reported");
    for later in &report.later_windows {
        assert_eq!((later.from.clone(), later.to.clone()), wide);
        let named = |amendment_id: &str, text: &str| {
            amendment_id == later.amendment_id && text == later.text
        };
        assert!(
            answered
                .resolved
                .iter()
                .any(|row| named(&row.amendment_id, &row.text))
                || answered
                    .unplaced
                    .iter()
                    .any(|row| named(&row.amendment_id, &row.text)),
            "a later window is named for a statement the first window answered for"
        );
    }
}

#[test]
fn should_ask_for_no_work_in_a_window_when_the_step_has_recorded_the_law_in_an_earlier_one() {
    // The step records title 7's renumberings in 2025-07-18 -> 2025-07-30 and
    // nothing in 2025-07-30 -> 2025-08-14, where the text is identical. The
    // work-list must read the same windows, or it asks for a step in a window
    // the law did not act in.
    let mut dataset = title_7_and_the_bill();
    record_over_every_window(&mut dataset);

    let report = inspect::validate(&dataset).expect("validate should run");

    assert!(
        report.unresolved_redesignations.is_empty(),
        "the step has run over every window, got {:?}",
        report.unresolved_redesignations
    );
}

#[test]
fn should_name_only_the_window_the_law_acted_in_when_no_step_has_run() {
    let dataset = title_7_and_the_bill();

    let report = inspect::validate(&dataset).expect("validate should run");

    let windows: Vec<(&str, &str)> = report
        .unresolved_redesignations
        .iter()
        .map(|row| (row.from.as_str(), row.to.as_str()))
        .collect();
    assert_eq!(
        windows,
        vec![(DATES[0], DATES[1])],
        "only the first window shows a change under the statements"
    );
}

#[test]
fn should_say_nothing_changed_when_the_report_reads_a_statement_whose_text_no_window_changed() {
    // 20 U.S.C. 1087tt(b)(1)(B) reads the same on all three dates, though
    // `119-hr-1` renumbers its clauses (vi) and (vii). The step records no link,
    // and the report must say why, rather than say the step has not run.
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for date in DATES {
        let parsed = parse(&format!("tests/test_data/usc/{date}/usc20.xml"), date)
            .expect("title 20 should parse");
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
    record_over_every_window(&mut dataset);

    let report = inspect::redesignation_report(&dataset, Some(BILL_ID))
        .expect("the report should read the dataset");

    let row = report
        .rows
        .iter()
        .find(|row| {
            row.clause
                .contains("redesignating clauses (vi) and (vii) as clauses (v) and (vi)")
        })
        .expect("the statement has a row");
    assert!(!row.placed, "no link is recorded for it");
    assert_eq!(
        row.reason.as_deref(),
        Some(
            "nothing under uscode/title_20/chapter_28/subchapter_IV/part_F/section_1087tt/\
             subsection_b/paragraph_1/subparagraph_B changed in any window after 2025-07-04"
        )
    );
}

#[test]
fn should_report_no_change_under_2015_o_when_its_text_is_identical_at_both_ends_of_the_window() {
    // The playtest's harm: `diff` over 2025-07-30 -> 2025-08-14 said paragraph
    // (8) "changed" and paragraph (7) was "added", in a window where the text of
    // 7 U.S.C. 2015(o) is the same at both ends.
    let mut dataset = title_7_and_the_bill();
    record_over_every_window(&mut dataset);

    let work = WorkId::new("uscode/title_7");
    let diff = dataset
        .compute_diff(
            &ExpressionId::new(work.clone(), DATES[1]),
            &ExpressionId::new(work, DATES[2]),
        )
        .expect("the window should diff");

    assert!(
        diff.find(SUBSECTION_2015_O).is_none_or(TreeDiff::is_empty),
        "nothing under {SUBSECTION_2015_O} changed from {} to {}, and the diff says: {:?}",
        DATES[1],
        DATES[2],
        diff.find(SUBSECTION_2015_O)
    );
}

#[test]
fn should_record_no_second_placement_when_a_grown_dataset_is_read_over_a_later_window_only() {
    // A dataset built over two release points, with the step run over the
    // window it holds. Then it grows to the third, and the step runs again as
    // `add-release-points` says: over the new windows only (#273).
    let mut grown = title_7_and_the_bill_at(&DATES[..2]);
    record_over_every_window(&mut grown);
    add_title_7_at(&mut grown, DATES[2]);

    let bill = grown
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let work = WorkId::new("uscode/title_7");
    let at = |date: &str| ExpressionId::new(work.clone(), date);
    // Title 7 reads the same on 2025-07-30 and 2025-08-14, so the adjacent new
    // window cannot hold a move. An operator's span from the first date to the
    // new one shows the law's change again: it is a later window that
    // qualifies, and the statements are already placed in an earlier one.
    for window in [(at(DATES[1]), at(DATES[2])), (at(DATES[0]), at(DATES[2]))] {
        grown
            .record_redesignations_over(BILL_ID, &bill, &[window])
            .expect("the step should run");
    }

    let mut rebuilt = title_7_and_the_bill();
    record_over_every_window(&mut rebuilt);

    assert_eq!(
        renumbering_link_ids(&grown),
        renumbering_link_ids(&rebuilt),
        "a grown dataset holds the renumbering links a build over all three dates holds"
    );
}

#[test]
fn should_place_a_statement_again_when_every_link_of_it_in_the_earlier_window_is_refuted() {
    // A refuted link was checked and found wrong, so it is no placement. A
    // statement whose every link is refuted is work again, as an amendment is
    // (#268), and a later window that qualifies can hold it.
    let mut grown = title_7_and_the_bill_at(&DATES[..2]);
    record_over_every_window(&mut grown);
    let links = grown
        .links_by_kind(LinkKind::REDESIGNATED_AS)
        .expect("the links should read");
    let statement_of = |link: &words_to_data::link::Link| {
        let amendment = link
            .payload
            .as_ref()
            .expect("a renumbering has a payload")
            .value["amendment_id"]
            .as_str()
            .expect("the payload names the amendment")
            .to_string();
        let words = link
            .provenance
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reasoning.clone())
            .expect("a renumbering carries the words it was read out of");
        (amendment, words)
    };
    let refuted = statement_of(&links[0]);
    let refutation = Review {
        verdict: Verdict::Refuted,
        reviewer: "human:test".to_string(),
        reasoning: Some("The move is not in this window.".to_string()),
        at: time::OffsetDateTime::UNIX_EPOCH,
    };
    for link in links.iter().filter(|link| statement_of(link) == refuted) {
        grown
            .add_link(refutation.about(link))
            .expect("the review should record");
    }
    add_title_7_at(&mut grown, DATES[2]);

    let bill = grown
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let work = WorkId::new("uscode/title_7");
    let wide = (
        ExpressionId::new(work.clone(), DATES[0]),
        ExpressionId::new(work, DATES[2]),
    );
    grown
        .record_redesignations_over(BILL_ID, &bill, &[wide])
        .expect("the step should run");

    let placed_again = grown
        .links_by_kind(LinkKind::REDESIGNATED_AS)
        .expect("the links should read")
        .iter()
        .filter(|link| statement_of(link) == refuted)
        .any(|link| {
            matches!(&link.subject, Target::Change { from_date, to_date, .. }
                if from_date == DATES[0] && to_date == DATES[2])
        });
    assert!(
        placed_again,
        "a statement whose every earlier link is refuted is placed in the later window: {}",
        refuted.1
    );
}

#[test]
fn should_say_how_many_statements_are_placed_earlier_when_the_command_runs_over_a_later_window() {
    // The command as `add-release-points` tells an operator to run it, over a
    // SQLite dataset that it changes in place.
    let path = format!(
        "{}/renumbering_placed_earlier.sqlite",
        env!("CARGO_TARGET_TMPDIR")
    );
    let _ = std::fs::remove_file(&path);
    let mut dataset = title_7_and_the_bill();
    record_over_every_window(&mut dataset);
    dataset
        .save_to_sqlite(&path)
        .expect("the dataset should save");
    let before = renumbering_link_ids(&dataset);

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "redesignations",
            &path,
            "--bill-id",
            BILL_ID,
            "--between",
            DATES[0],
            DATES[2],
        ])
        .output()
        .expect("the binary should run");

    let printed = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "redesignations should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        printed.contains("already placed in an earlier window"),
        "the run should say why it placed nothing again, got:\n{printed}"
    );
    let after = Dataset::open_sqlite(&path).expect("the dataset should open");
    let mut ids: Vec<String> = after
        .links_by_kind(LinkKind::REDESIGNATED_AS)
        .expect("the links should read")
        .iter()
        .map(|link| link.id())
        .collect();
    ids.sort();
    assert_eq!(ids, before, "the run records no second placement");
}

#[test]
fn should_report_a_statement_as_placed_and_not_as_unplaced_when_its_link_is_in_an_earlier_window() {
    let mut grown = title_7_and_the_bill_at(&DATES[..2]);
    record_over_every_window(&mut grown);
    add_title_7_at(&mut grown, DATES[2]);

    let mut rebuilt = title_7_and_the_bill();
    let bill = rebuilt
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the dataset should hold the bill as a document");
    let windows = adjacent_expressions(&rebuilt).expect("the windows should list");
    let every_window = rebuilt
        .record_redesignations_over(BILL_ID, &bill, &windows)
        .expect("the step should run");

    let work = WorkId::new("uscode/title_7");
    let at = |date: &str| ExpressionId::new(work.clone(), date);
    // The adjacent new window, in which title 7 is identical, and the span from
    // the first date to the new one, in which the law's change shows again.
    for window in [(at(DATES[1]), at(DATES[2])), (at(DATES[0]), at(DATES[2]))] {
        let report = grown
            .record_redesignations_over(BILL_ID, &bill, std::slice::from_ref(&window))
            .expect("the step should run");

        assert_eq!(
            report.statements(),
            every_window.statements(),
            "over {} -> {}, the report names every statement the bill makes",
            window.0,
            window.1.at
        );
        for unplaced in &report.unplaced {
            assert!(
                !every_window.resolved.iter().any(|row| {
                    row.amendment_id == unplaced.amendment_id && row.text == unplaced.text
                }),
                "over {} -> {}, a statement placed in an earlier window is reported as \
                 not placed: {}",
                window.0,
                window.1.at,
                unplaced.reason
            );
        }
    }
}
