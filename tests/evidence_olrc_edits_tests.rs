//! A block or a string that the OLRC printed with edits still shows the change
//! its amendment made (#433).
//!
//! The OLRC edits some words of a law when it prints them in the Code. Where
//! the bill's own words fix the edit, the matcher reads the words both ways.
//!
//! Every case reads a committed public law and the titles of the Code it
//! amends at the committed release points around the window it landed in.
//! Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

/// Pub. L. 119-21, enacted on 2025-07-04.
const ONE_BIG_BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const ONE_BIG_BILL_ID: &str = "119-hr-1";

/// What the matcher says about every amendment of `bill_id` that acts in
/// `titles` (`usc26`), with the Code at `release_points`, and the
/// renumberings the law states recorded first, as `build-dataset` records
/// them.
fn matches_of(
    law: &str,
    bill_id: &str,
    titles: &[&str],
    release_points: &[&str],
) -> Vec<AmendmentMatch> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    let bill = parse_bill_amendments(bill_id, law).expect("the law should parse");
    dataset.add_bill(bill).expect("the bill should add");
    let xml = std::fs::read_to_string(law).expect("the law should be committed");
    let document = roxmltree::Document::parse(&xml).expect("the law should parse");
    let (expression, _) = bill_expression(&document, bill_id).expect("the law should read");
    dataset
        .add_expression(expression)
        .expect("the law should add");
    for date in release_points {
        for title in titles {
            common::add_uslm_xml(
                &mut dataset,
                &format!("tests/test_data/usc/{date}/{title}.xml"),
                date,
            );
        }
    }
    let windows = adjacent_expressions(&dataset).expect("the windows should list");
    let document = dataset
        .bill_document(bill_id)
        .expect("the dataset should answer for the bill")
        .expect("the bill is held as a document");
    dataset
        .record_redesignations_over(bill_id, &document, &windows)
        .expect("the renumberings should record");
    match_by_evidence(&dataset)
        .expect("the matcher should run")
        .matches
}

/// Pub. L. 119-21 in title 26, in the window it landed in.
fn title_26_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        matches_of(
            ONE_BIG_BILL,
            ONE_BIG_BILL_ID,
            &["usc26"],
            &["2025-07-18", "2025-07-30"],
        )
    })
}

/// Each change an amendment was given, with why, or a failure that says why
/// it was given none.
fn caused_by(matches: &[AmendmentMatch], id_start: &str) -> Vec<(String, String)> {
    let found: Vec<&AmendmentMatch> = matches
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
fn should_show_a_quoted_string_whose_inner_quotes_the_code_prints_as_double_quotes() {
    // Pub. L. 119-21 § 70115(a)(1): in 26 U.S.C. 529A(b)(2)(B)(i), "by
    // inserting "(determined by substituting '1996' for '1997' in paragraph
    // (2)(B) thereof)" after "section 2503(b)"". The bill quotes inside the
    // string with single marks, and the Code prints ""1996" for "1997"".
    let clause_i = "uscode/title_26/subtitle_A/chapter_1/subchapter_F/part_VIII/section_529A/\
                    subsection_b/paragraph_2/subparagraph_B/clause_i";

    let caused = caused_by(title_26_matches(), "cc3e6707f226");

    assert!(
        caused.iter().any(|(path, why)| path == clause_i
            && why.contains("(determined by substituting '1996' for '1997'")),
        "the inserted string shows in {clause_i}: {caused:?}"
    );
}

/// Pub. L. 119-60, enacted on 2025-12-18.
const NDAA: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";
const NDAA_ID: &str = "119-s-1071";

/// Pub. L. 119-60 in titles 21 and 33, in the window it landed in.
fn ndaa_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        matches_of(
            NDAA,
            NDAA_ID,
            &["usc21", "usc33"],
            &["2026-01-23", "2026-07-12"],
        )
    })
}

#[test]
fn should_show_a_block_whose_references_the_code_prints_as_references_to_the_code() {
    // Pub. L. 119-60 § 7403 adds 33 U.S.C. 2717(g): "… no person may bring an
    // action under this Act, section 311 of the Federal Water Pollution
    // Control Act (33 U.S.C. 1321), or chapter 7 of title 5, United States
    // Code, challenging …". The Code prints "section 1321 of this title, or
    // chapter 7 of title 5 challenging". The bill's own citation gives the
    // section, so the block shows (g).
    let subsection_g = "uscode/title_33/chapter_40/subchapter_I/section_2717/subsection_g";

    let caused = caused_by(ndaa_matches(), "da57322e147b");

    assert!(
        caused
            .iter()
            .any(|(path, why)| path == subsection_g && why.contains("enacted text 1 of 1")),
        "the block of § 7403 shows {subsection_g}: {caused:?}"
    );
}

#[test]
fn should_show_a_block_whose_date_of_enactment_the_code_prints_as_the_date() {
    // Pub. L. 119-60 § 8316(3) adds 21 U.S.C. 2312(3): "… has knowingly
    // engaged in, on or after the date of the enactment of the BUST Fentanyl
    // Act, a pattern of significant activity …". The BUST Fentanyl Act is a
    // part of the law, and the Code prints "on or after December 18, 2025,".
    let paragraph_3 = "uscode/title_21/chapter_28/subchapter_I/section_2312/paragraph_3";

    let caused = caused_by(ndaa_matches(), "a391e5b77813");

    assert!(
        caused
            .iter()
            .any(|(path, why)| path == paragraph_3 && why.contains("enacted text 1 of 1")),
        "the block of § 8316 shows {paragraph_3}: {caused:?}"
    );
}
