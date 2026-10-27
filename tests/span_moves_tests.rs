//! A diff over a span of several windows pairs the moves of each window in it
//! (#423).
//!
//! A renumbering is recorded for one window of two adjacent release points.
//! `119-hr-1` moved `26 U.S.C. § 45X(c)(6)(R)` through (Z) down to (S) through
//! (AA) between 2025-07-18 and 2025-07-30, and the next window of the corpus,
//! 2025-07-30 to 2025-08-14, does not touch them. Over the span 2025-07-18 to
//! 2025-08-14 the provisions moved all the same. The diff over the span read
//! the links of that exact pair of dates only, found none, and paired by
//! position: each renumbering became a "changed" or a removal and an addition,
//! while `path` over the same span said "moved in from (R)".
//!
//! Every case here is read out of the committed corpus.

mod common;

use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::diff::TreeDiff;
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_redesignation::redesignations_stated_in_file;

const BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-hr-1";

/// The three committed release points of title 26, oldest first.
const DATES: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];

/// `26 U.S.C. § 45X(c)(6)`, the paragraph the bill renumbered through.
const PARAGRAPH_45X_C_6: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_45X/subsection_c/paragraph_6";

fn expression(date: &str) -> ExpressionId {
    ExpressionId::new(WorkId::new("uscode/title_26"), date)
}

/// Title 26 at the three release points, with the bill's renumberings recorded
/// over the first window, where the bill acted.
fn title_26_over_two_windows() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for date in DATES {
        common::add_uslm_xml(
            &mut dataset,
            &format!("tests/test_data/usc/{date}/usc26.xml"),
            date,
        );
    }
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    dataset
        .record_redesignations(
            BILL_ID,
            &stated,
            &expression(DATES[0]),
            &expression(DATES[1]),
        )
        .expect("the redesignations should record");
    dataset
}

/// The moves a diff reports at § 45X(c)(6), as pairs of old and new path.
fn moves_at_45x_c_6(diff: &TreeDiff) -> Vec<(String, String)> {
    diff.find(PARAGRAPH_45X_C_6)
        .expect("§ 45X(c)(6) should change")
        .moved
        .iter()
        .map(|m| (m.from.path.to_string(), m.to.path.to_string()))
        .collect()
}

#[test]
fn should_pair_the_moves_of_each_window_when_a_diff_spans_several_windows() {
    let dataset = title_26_over_two_windows();

    let first_window = dataset
        .compute_diff(&expression(DATES[0]), &expression(DATES[1]))
        .expect("the diff over the first window should compute");
    let span = dataset
        .compute_diff(&expression(DATES[0]), &expression(DATES[2]))
        .expect("the diff over the span should compute");

    let moved_in_the_window = moves_at_45x_c_6(&first_window);
    assert_eq!(
        moved_in_the_window.len(),
        9,
        "(R) to (Z) moved down one letter, got {moved_in_the_window:?}"
    );
    assert_eq!(
        moves_at_45x_c_6(&span),
        moved_in_the_window,
        "the later window does not touch § 45X(c)(6), so the span reads the same moves"
    );
}
