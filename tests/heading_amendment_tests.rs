//! An amendment of "the heading for section X" acts on the heading of that
//! provision alone (#451).
//!
//! Its address is the whole provision, so every change below it is under the
//! address. A subsection heading that prints the same struck words is not a
//! change its words can have made.
//!
//! The case reads the committed public law `119-hr-1` (Pub. L. 119-21) and
//! title 26 of the Code at the release points around the window it landed
//! in. Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

const ONE_BIG_BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const ONE_BIG_BILL_ID: &str = "119-hr-1";

/// The release point before Pub. L. 119-21 reached title 26, and the first
/// one after.
const RELEASE_POINTS: [&str; 2] = ["2025-07-18", "2025-07-30"];

/// Pub. L. 119-21 § 70323(a)(3)(E)(i): "The heading for section 951A is amended
/// by striking "global intangible low-taxed income" and inserting "net cfc
/// tested income"."
const AMENDS_HEADING_OF_951A: &str = "1bd7a424ec78";

const SECTION_951A: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_III/subpart_F/section_951A";

/// What the matcher says about every amendment of the law that acts in title
/// 26, with the renumberings the law states recorded first, as
/// `build-dataset` records them.
fn title_26_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        let bill =
            parse_bill_amendments(ONE_BIG_BILL_ID, ONE_BIG_BILL).expect("the law should parse");
        dataset.add_bill(bill).expect("the bill should add");
        let xml = std::fs::read_to_string(ONE_BIG_BILL).expect("the law should be committed");
        let document = roxmltree::Document::parse(&xml).expect("the law should parse");
        let (law, _) = bill_expression(&document, ONE_BIG_BILL_ID).expect("the law should read");
        dataset.add_expression(law).expect("the law should add");
        for date in RELEASE_POINTS {
            common::add_uslm_xml(
                &mut dataset,
                &format!("tests/test_data/usc/{date}/usc26.xml"),
                date,
            );
        }
        let windows = adjacent_expressions(&dataset).expect("the windows should list");
        let bill = dataset
            .bill_document(ONE_BIG_BILL_ID)
            .expect("the dataset should answer for the bill")
            .expect("the bill is held as a document");
        dataset
            .record_redesignations_over(ONE_BIG_BILL_ID, &bill, &windows)
            .expect("the renumberings should record");
        match_by_evidence(&dataset)
            .expect("the matcher should run")
            .matches
    })
}

/// The paths an amendment was linked to, or a failure that says why not.
fn linked_paths(id_start: &str) -> Vec<String> {
    let found: Vec<&AmendmentMatch> = title_26_matches()
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect();
    let [found] = found[..] else {
        panic!("one amendment should start with {id_start}");
    };
    match &found.outcome {
        Outcome::Linked(linked) => linked.paths(),
        other => panic!("{id_start} should be linked: {other:?}"),
    }
}

#[test]
fn should_link_only_the_section_heading_when_the_amendment_names_the_heading_for_the_section() {
    // The heading of 951A(b) also lost "global intangible low-taxed income" in
    // the window, by another amendment of the law. The words "the heading for
    // section 951A" name the section's own heading, not a subsection's.
    let of_heading = linked_paths(AMENDS_HEADING_OF_951A);

    assert_eq!(of_heading, [SECTION_951A]);
}
