//! A change of case only still shows the amendment that made it (#462).
//!
//! The matcher compares words in lower case, because the bill and the Code
//! can print the same words in a different case. A change that changes only
//! the case of words is then no change of words at all. Where a quoted string
//! shows nothing in lower case, it is also compared with its case.
//!
//! Every case reads the committed public law `119-s-1071` (Pub. L. 119-60) and
//! title 10 of the Code at the release points around the window it landed in.
//! Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

/// Pub. L. 119-60, enacted on 2025-12-18.
const NDAA: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";
const NDAA_ID: &str = "119-s-1071";

/// The release point before Pub. L. 119-60 reached title 10, and the first
/// one after.
const RELEASE_POINTS: [&str; 2] = ["2026-01-23", "2026-07-12"];

/// Pub. L. 119-60 § 1701(a)(3): "In section 132a— (A) in the section heading,
/// by striking "improvement officer" and inserting "Improvement Officer"; and
/// (B) in subsection (c)(1), by striking "… Act of Fiscal Year 2008" and
/// inserting "… Act for Fiscal Year 2008"."
const AMENDS_SECTION_132A: &str = "e1eff77cb587";

const SECTION_132A: &str = "uscode/title_10/subtitle_A/part_I/chapter_4/section_132a";

/// What the matcher says about every amendment of the law that acts in title
/// 10, with the renumberings the law states recorded first, as
/// `build-dataset` records them.
fn title_10_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        let bill = parse_bill_amendments(NDAA_ID, NDAA).expect("the law should parse");
        dataset.add_bill(bill).expect("the bill should add");
        let xml = std::fs::read_to_string(NDAA).expect("the law should be committed");
        let document = roxmltree::Document::parse(&xml).expect("the law should parse");
        let (law, _) = bill_expression(&document, NDAA_ID).expect("the law should read");
        dataset.add_expression(law).expect("the law should add");
        for date in RELEASE_POINTS {
            common::add_uslm_xml(
                &mut dataset,
                &format!("tests/test_data/usc/{date}/usc10.xml"),
                date,
            );
        }
        let windows = adjacent_expressions(&dataset).expect("the windows should list");
        let bill = dataset
            .bill_document(NDAA_ID)
            .expect("the dataset should answer for the bill")
            .expect("the bill is held as a document");
        dataset
            .record_redesignations_over(NDAA_ID, &bill, &windows)
            .expect("the renumberings should record");
        match_by_evidence(&dataset)
            .expect("the matcher should run")
            .matches
    })
}

/// Each change the amendment was given, with why, from every answer for it.
fn caused_by(id_start: &str) -> Vec<(String, String)> {
    let found: Vec<&AmendmentMatch> = title_10_matches()
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
fn should_link_the_heading_when_the_amendment_changes_only_the_case_of_its_words() {
    // The heading read "Performance improvement officer" and now reads
    // "Performance Improvement Officer". In lower case the words are the same.
    let caused = caused_by(AMENDS_SECTION_132A);

    assert!(
        caused
            .iter()
            .any(|(path, why)| path == SECTION_132A
                && why.contains("inserted \"Improvement Officer\"")),
        "the inserted string shows in the heading of {SECTION_132A}: {caused:?}"
    );
}
