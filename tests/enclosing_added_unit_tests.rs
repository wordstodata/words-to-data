//! An amendment that adds one provision inside a unit that the window added
//! whole for another reason (#450).
//!
//! The diff lists an added unit as one path, and nothing beneath it (#336).
//! An amendment that enacts the whole unit made that change. An amendment
//! whose words enact only one provision inside the unit did not: its change
//! is the provision at its address.
//!
//! The case reads the committed public law `119-s-1071` (Pub. L. 119-60),
//! title 10 of the Code at the release points around the window it landed
//! in, and the OLRC's committed classification table. Nothing is mocked.

mod common;

use std::sync::OnceLock;

use words_to_data::citation::resolve::SectionPaths;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, ExpressionId, WorkId, adjacent_expressions,
};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::olrc::{ClassificationTable, classify};
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

const NDAA: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";
const NDAA_ID: &str = "119-s-1071";
const TITLE_10: &str = "uscode/title_10";

/// The release point before Pub. L. 119-60 reached title 10, and the first
/// one after.
const RELEASE_POINTS: [&str; 2] = ["2026-01-23", "2026-07-12"];

/// Pub. L. 119-60 § 3117(a): "Subtitle B of title XLVIII of the Atomic Energy
/// Defense Act (50 U.S.C. 2791 et seq.) is amended by adding at the end the
/// following section: SEC. 4816. …". The OLRC classifies it to 10 U.S.C.
/// 6340.
const ADDS_SECTION_6340: &str = "4340e4e026e1";

/// The subpart of title 10 that holds the Atomic Energy Defense Act. The
/// OLRC moved the Act there in the same window, so the window adds the
/// subpart whole.
const SUBPART_B: &str = "uscode/title_10/subtitle_A/part_VI/subpart_B";

/// What the matcher says about every amendment of the law that acts in title
/// 10, with the renumberings the law states recorded first, as
/// `build-dataset` records them, and the OLRC's classification table stated
/// as links, as `add-classifications` states them.
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

        let html = std::fs::read_to_string("tests/test_data/olrc/classification/tbl119pl_1st.htm")
            .expect("the committed table should read");
        let table = ClassificationTable::parse(&html).expect("the committed table should parse");
        let mut paths = SectionPaths::new();
        for date in RELEASE_POINTS {
            let expression = dataset
                .get_expression(&ExpressionId::new(WorkId::new(TITLE_10), date))
                .expect("storage should answer")
                .expect("title 10 is held");
            paths.add_work(&expression.root);
        }
        let scope = dataset.scope().expect("the scope should derive");
        for link in classify(&table.rows, &scope, &paths, "olrc:tbl119pl_1st.htm").links {
            dataset.add_link(link).expect("the link should add");
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
fn should_link_the_added_section_when_the_amendment_enacts_only_that_section_inside_an_added_unit()
{
    // The block of § 3117(a) enacts § 4816 of the Act, which the Code prints
    // as § 6340. It holds no word of the heading of subpart B, and none of the
    // other sections of the Act that the subpart holds.
    let section_6340 = format!("{SUBPART_B}/chapter_608/subchapter_II/section_6340");

    let of_3117 = linked_paths(ADDS_SECTION_6340);

    assert_eq!(of_3117, [section_6340]);
}
