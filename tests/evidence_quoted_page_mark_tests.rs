//! A Stat. page mark inside a quoted string does not stop the string from
//! showing in the Code (#485).
//!
//! The Public Law print of a bill puts a page mark of the Statutes at Large,
//! such as "139 STAT. 245", where a page of the Statutes ends. The mark can
//! fall inside a string that the bill quotes (`<quotedText>`). The mark is not
//! a word of the law, so the matcher removes it from a quoted string before it
//! compares the string with the Code, as it does for an enacted block.
//!
//! Every case reads the committed public law `119-hr-1` (Pub. L. 119-21) and
//! title 26 of the Code at the two release points around the window it landed
//! in. Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

/// Pub. L. 119-21, enacted on 2025-07-04.
const LAW: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const LAW_ID: &str = "119-hr-1";

/// The release point before Pub. L. 119-21 reached title 26, and the first
/// one after.
const RELEASE_POINTS: [&str; 2] = ["2025-07-18", "2025-07-30"];

/// Pub. L. 119-21 § 70434(d): "Section 181(c)(1) is amended by striking
/// "qualified film or television production or any qualified live theatrical
/// 139 STAT. 245 production" and inserting …". The page mark falls inside the
/// struck string.
const AMENDS_SECTION_181: &str = "310a75a7af63";

const SECTION_181_C_1: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_181/subsection_c/paragraph_1";

/// What the matcher says about every amendment of the law that acts in title
/// 26, with the renumberings the law states recorded first, as
/// `build-dataset` records them.
fn title_26_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        let bill = parse_bill_amendments(LAW_ID, LAW).expect("the law should parse");
        dataset.add_bill(bill).expect("the bill should add");
        let xml = std::fs::read_to_string(LAW).expect("the law should be committed");
        let document = roxmltree::Document::parse(&xml).expect("the law should parse");
        let (law, _) = bill_expression(&document, LAW_ID).expect("the law should read");
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
            .bill_document(LAW_ID)
            .expect("the dataset should answer for the bill")
            .expect("the bill is held as a document");
        dataset
            .record_redesignations_over(LAW_ID, &bill, &windows)
            .expect("the renumberings should record");
        match_by_evidence(&dataset)
            .expect("the matcher should run")
            .matches
    })
}

/// Each change the amendment was given, with why, from every answer for it.
fn caused_by(id_start: &str) -> Vec<(String, String)> {
    let found: Vec<&AmendmentMatch> = title_26_matches()
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect();
    assert!(
        !found.is_empty(),
        "an amendment should start with {id_start}"
    );
    found
        .iter()
        .filter_map(|found| match &found.outcome {
            Outcome::Linked(linked) => Some(linked),
            _ => None,
        })
        .flat_map(|linked| &linked.changes)
        .map(|change| (change.path.clone(), change.why.clone()))
        .collect()
}

#[test]
fn should_show_the_struck_string_when_a_page_mark_falls_inside_it() {
    // 26 U.S.C. 181(c)(1) at 2025-07-18 reads "… qualified film or television
    // production or any qualified live theatrical production shall be made",
    // and at 2025-07-30 it does not.
    let caused = caused_by(AMENDS_SECTION_181);

    assert!(
        caused.iter().any(|(path, why)| path == SECTION_181_C_1
            && why.contains(
                "struck \"qualified film or television production or any qualified live \
                 theatrical production\""
            )),
        "the struck string shows in {SECTION_181_C_1}: {caused:?}"
    );
}

#[test]
fn should_not_name_the_page_mark_in_the_evidence() {
    // The mark is not a word of the law, so the evidence does not quote it.
    let caused = caused_by(AMENDS_SECTION_181);

    assert!(
        caused.iter().all(|(_, why)| !why.contains("STAT.")),
        "no evidence quotes the page mark: {caused:?}"
    );
}
