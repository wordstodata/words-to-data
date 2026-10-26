//! A provision the window added whole, matched with every provision below it
//! (#432).
//!
//! The diff lists an added provision as one path, and nothing beneath it, so
//! the change holds every provision below it (#336). The words the bill
//! enacts are compared with the words of the provision and of everything
//! below it.
//!
//! Every case reads the committed public law `119-s-1071` (Pub. L. 119-60)
//! and title 10 of the Code at the release points around the window it
//! landed in. Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

const NDAA: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";
const NDAA_ID: &str = "119-s-1071";

/// The release point before Pub. L. 119-60 reached title 10, and the first
/// one after.
const RELEASE_POINTS: [&str; 2] = ["2026-01-23", "2026-07-12"];

/// 10 U.S.C. 9062, the inventory requirements of the Air Force.
const SECTION_9062: &str = "uscode/title_10/subtitle_D/part_I/chapter_907/section_9062";

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
        record_renumberings(&mut dataset);
        match_by_evidence(&dataset)
            .expect("the matcher should run")
            .matches
    })
}

fn record_renumberings(dataset: &mut Dataset<InMemoryStorage>) {
    let windows = adjacent_expressions(dataset).expect("the windows should list");
    let bill = dataset
        .bill_document(NDAA_ID)
        .expect("the dataset should answer for the bill")
        .expect("the bill is held as a document");
    dataset
        .record_redesignations_over(NDAA_ID, &bill, &windows)
        .expect("the renumberings should record");
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
fn should_link_an_added_provision_whose_own_fields_are_empty_by_the_words_below_it() {
    // Pub. L. 119-60 § 1632(a): "Section 9062 of title 10 … is amended by
    // adding at the end the following new subsection: (n)(1) The Secretary of
    // the Air Force shall maintain a total inventory of intercontinental
    // ballistic missiles …". The Code prints (n) with no heading and no words
    // of its own: every word of it is in its paragraphs. The block shows (n)
    // by the words below it.
    let subsection_n = format!("{SECTION_9062}/subsection_n");

    let of_1632 = linked_paths("931a4d58392f");

    assert!(
        of_1632.contains(&subsection_n),
        "§ 1632(a) added {subsection_n}: {of_1632:?}"
    );
}

#[test]
fn should_not_give_an_added_provision_to_a_block_that_holds_only_its_few_own_words() {
    // 10 U.S.C. 9062(j)(4) prints "In this subsection:" and two definitions
    // of tanker aircraft below it. § 141(b) of the law renumbered the old
    // paragraph (3) as (4), and the window shows (4) as added. The block of
    // § 1632(a) holds "In this subsection, the term 'intercontinental
    // ballistic missile' means …", so it holds the three own words of (4),
    // and not the definitions below them. Three words alone are in many
    // blocks, and they are no evidence that § 1632(a) made (4).
    let paragraph_4 = format!("{SECTION_9062}/subsection_j/paragraph_4");

    let of_1632 = linked_paths("931a4d58392f");

    assert!(
        !of_1632.contains(&paragraph_4),
        "§ 1632(a) did not make {paragraph_4}: {of_1632:?}"
    );
}
