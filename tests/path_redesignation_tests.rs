//! `path` pairs provisions over the redesignation links, not over the path
//! string (#165).
//!
//! `119-hr-1` put a new subparagraph at `26 U.S.C. § 45X(c)(6)(R)` and moved
//! (R) through (Z) down to (S) through (AA). Asked about (R), the command used
//! to answer "in both", and to report the heading changing from " Neodymium"
//! to " Metallurgical coal". Those are two different subparagraphs. Neodymium
//! is at (S), and no bill touched its words.
//!
//! Every case here is read out of the committed corpus.

use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, work_roots,
};
use words_to_data::inspect::{self, PathMatch, PathReport, Presence};
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_redesignation::redesignations_stated_in_file;
use words_to_data::uslm::parser::parse;

const BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-hr-1";

const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// `26 U.S.C. § 45X(c)(6)`, the paragraph the bill renumbered through.
const PARAGRAPH_45X_C_6: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_45X/subsection_c/paragraph_6";

/// One subparagraph of that paragraph, by its letter.
fn subparagraph(letter: &str) -> String {
    format!("{PARAGRAPH_45X_C_6}/subparagraph_{letter}")
}

/// Title 26 at both release points, with `119-hr-1`'s redesignations recorded
/// as links, and the pair of expressions they name.
fn title_26_with_redesignations() -> (Dataset<InMemoryStorage>, ExpressionId, ExpressionId) {
    let work = WorkId::new("uscode/title_26");
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (file, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
        let parsed = parse(file, date).expect("title 26 should parse");
        let root = work_roots(parsed).pop().expect("the file holds one title");
        dataset
            .add_expression(Expression {
                id: ExpressionId::new(work.clone(), date),
                label: None,
                root,
            })
            .expect("the expression should store");
    }
    let from = ExpressionId::new(work.clone(), BEFORE);
    let to = ExpressionId::new(work, AFTER);

    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    dataset
        .record_redesignations(BILL_ID, &stated, &from, &to)
        .expect("the redesignations should record");
    (dataset, from, to)
}

/// The report for one subparagraph of § 45X(c)(6), across the two release
/// points.
fn report_for(letter: &str) -> PathReport {
    let (dataset, from, to) = title_26_with_redesignations();
    inspect::path_report(
        &dataset,
        &subparagraph(letter),
        Some((&from, &to)),
        PathMatch::Subtree,
    )
    .expect("a path report should build")
}

#[test]
fn should_report_a_move_out_and_an_addition_when_a_bill_inserted_a_subparagraph() {
    let report = report_for("R");

    assert_eq!(
        report.provisions.len(),
        2,
        "two provisions touch (R): the one that left it and the one that took it, got {:?}",
        report.provisions
    );

    // The neodymium subparagraph left (R) for (S), and the bill changed no word
    // of it.
    assert_eq!(
        report.provisions[0].presence,
        Presence::MovedOut {
            to_path: subparagraph("S")
        }
    );
    assert_eq!(report.provisions[0].from_position, Some(0));
    assert!(
        report.provisions[0].changes.is_empty(),
        "the renumbering changed no words, got {:?}",
        report.provisions[0].changes
    );

    // The metallurgical coal subparagraph is new law at (R).
    assert_eq!(report.provisions[1].presence, Presence::Added);
    assert_eq!(report.provisions[1].to_position, Some(0));

    // And the false statement is gone.
    assert!(
        !report
            .provisions
            .iter()
            .any(|p| p.presence == Presence::InBoth),
        "nothing at (R) is one provision across the two dates, got {:?}",
        report.provisions
    );
    assert!(
        !report
            .provisions
            .iter()
            .flat_map(|p| &p.changes)
            .any(|c| c.old_value.contains("Neodymium") && c.new_value.contains("coal")),
        "neodymium never became metallurgical coal, got {:?}",
        report.provisions
    );
}

#[test]
fn should_report_a_move_out_and_a_move_in_when_the_letters_cascade() {
    // The letters cascade, so every letter after (R) carries a provision away
    // and receives another. (S) used to claim that nickel became neodymium and
    // (T) that niobium became nickel. Both statements are false: the bill moved
    // each subparagraph down one letter and changed no word of any of them.
    for (letter, left_for, came_from) in [("S", "T", "R"), ("T", "U", "S")] {
        let report = report_for(letter);

        assert_eq!(
            report.provisions.len(),
            2,
            "({letter}) holds one provision that left and one that arrived, got {:?}",
            report.provisions
        );
        assert_eq!(
            report.provisions[0].presence,
            Presence::MovedOut {
                to_path: subparagraph(left_for)
            }
        );
        assert!(
            report.provisions[0].changes.is_empty(),
            "({letter}) moved to ({left_for}) with its words untouched, got {:?}",
            report.provisions[0].changes
        );
        assert_eq!(
            report.provisions[1].presence,
            Presence::MovedIn {
                from_path: subparagraph(came_from)
            }
        );
        assert!(
            report.provisions[1].changes.is_empty(),
            "({came_from}) moved to ({letter}) with its words untouched, got {:?}",
            report.provisions[1].changes
        );
        assert!(
            !report
                .provisions
                .iter()
                .any(|p| p.presence == Presence::InBoth),
            "({letter}) names two different provisions across the two dates, got {:?}",
            report.provisions
        );
    }
}
