//! A section a public law repeals, linked to the change that removed it
//! (#434).
//!
//! A repeal says *is repealed* where an amendment says *is amended*, and names
//! its section the same way. Before #434 the address reader read only *is
//! amended*, so no repeal had an address, and the removal of the section was
//! linked to nothing.
//!
//! Every case reads the committed public law `119-s-1071` (Pub. L. 119-60)
//! and title 10 of the Code at the release points around the window it
//! landed in. Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

const NDAA: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";
const NDAA_ID: &str = "119-s-1071";

/// The release point before Pub. L. 119-60 reached title 10, and the first
/// one after.
const RELEASE_POINTS: [&str; 2] = ["2026-01-23", "2026-07-12"];

/// What the matcher says about every amendment of the law that acts in title
/// 10.
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
        match_by_evidence(&dataset)
            .expect("the matcher should run")
            .matches
    })
}

/// The paths an amendment was linked to, or a failure that says why not.
fn linked_paths(id_start: &str) -> Vec<String> {
    let found: Vec<&AmendmentMatch> = title_10_matches()
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
fn should_link_the_removal_of_a_section_to_the_amendment_that_repeals_it() {
    // Pub. L. 119-60 § 901(b): "Section 147 of title 10, United States Code,
    // is repealed." The Code after the window holds no § 147.
    let section_147 = "uscode/title_10/subtitle_A/part_I/chapter_4/section_147";

    assert_eq!(linked_paths("d82893752bc9"), vec![section_147.to_string()]);
}
