//! A block shows a provision that the Code prints with a space lost between
//! two words (#452).
//!
//! The Code prints a table cell of two lines as two paragraphs with no space
//! between them: 26 U.S.C. 1202(a)(5) prints "Applicablepercentage:" where
//! the bill prints "Applicable percentage:". The words are the same, so the
//! block still shows the provision.
//!
//! The case reads a committed public law and title 26 of the Code at the
//! committed release points around the window it landed in. Nothing is
//! mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

/// Pub. L. 119-21, enacted on 2025-07-04.
const ONE_BIG_BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const ONE_BIG_BILL_ID: &str = "119-hr-1";

/// What the matcher says about every amendment of Pub. L. 119-21 in title 26,
/// in the window it landed in, with the renumberings the law states recorded
/// first, as `build-dataset` records them.
fn title_26_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        let bill =
            parse_bill_amendments(ONE_BIG_BILL_ID, ONE_BIG_BILL).expect("the law should parse");
        dataset.add_bill(bill).expect("the bill should add");
        let xml = std::fs::read_to_string(ONE_BIG_BILL).expect("the law should be committed");
        let document = roxmltree::Document::parse(&xml).expect("the law should parse");
        let (expression, _) =
            bill_expression(&document, ONE_BIG_BILL_ID).expect("the law should read");
        dataset
            .add_expression(expression)
            .expect("the law should add");
        for date in ["2025-07-18", "2025-07-30"] {
            common::add_uslm_xml(
                &mut dataset,
                &format!("tests/test_data/usc/{date}/usc26.xml"),
                date,
            );
        }
        let windows = adjacent_expressions(&dataset).expect("the windows should list");
        let document = dataset
            .bill_document(ONE_BIG_BILL_ID)
            .expect("the dataset should answer for the bill")
            .expect("the bill is held as a document");
        dataset
            .record_redesignations_over(ONE_BIG_BILL_ID, &document, &windows)
            .expect("the renumberings should record");
        match_by_evidence(&dataset)
            .expect("the matcher should run")
            .matches
    })
}

/// Each change the amendment was given, with why. The test fails with the
/// reason when the amendment was given none.
fn caused_by(id_start: &str) -> Vec<(String, String)> {
    let found: Vec<&AmendmentMatch> = title_26_matches()
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect();
    let [found] = found[..] else {
        panic!("one amendment should start with {id_start}");
    };
    match &found.outcome {
        Outcome::Linked(linked) => linked
            .changes
            .iter()
            .map(|change| (change.path.clone(), change.why.clone()))
            .collect(),
        other => panic!("{id_start} should be linked: {other:?}"),
    }
}

#[test]
fn should_link_an_added_table_when_the_code_prints_a_cell_of_two_lines_without_a_space() {
    // Pub. L. 119-21 § 70431(a)(2): "Section 1202(a) is amended by adding at
    // the end the following new paragraph: "(5) Applicable percentage.—The
    // applicable percentage under paragraph (1) shall be determined under the
    // following table: …"". The Code prints the second heading of the table
    // as "Applicablepercentage:".
    let paragraph_5 = "uscode/title_26/subtitle_A/chapter_1/subchapter_P/part_I/section_1202/subsection_a/\
         paragraph_5";

    let caused = caused_by("de65643fcd3a");

    assert_eq!(
        caused
            .iter()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>(),
        vec![paragraph_5],
        "§ 70431(a)(2) adds 1202(a)(5), and nothing else: {caused:?}"
    );
    assert!(
        caused[0].1.contains("enacted text"),
        "the block it enacts shows the paragraph: {caused:?}"
    );
}
