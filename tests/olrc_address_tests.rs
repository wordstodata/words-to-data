//! The OLRC's classification table as the address of an amendment whose
//! markup names none (#259).
//!
//! The Office of Law Revision Counsel classifies each section of a public law
//! to the section of the Code it changed, and the dataset stores each row as an
//! `olrc.classified_from` link (#247). When the bill's markup gives no address,
//! the row that names the amendment's place in the law gives the section.
//!
//! Every case reads the committed corpus: the public law `119-hr-1`, title 26
//! of the Code at its three committed release points, and the committed OLRC
//! table for the 119th Congress, 1st session. Nothing is mocked.

use std::sync::OnceLock;

use words_to_data::citation::resolve::SectionPaths;
use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, ExpressionId, WorkId, adjacent_expressions,
};
use words_to_data::legislature::evidence_matching::{
    AmendmentMatch, EvidenceMatching, Outcome, Stage, match_by_evidence,
};
use words_to_data::olrc::{ClassificationTable, classify};
use words_to_data::storage::InMemoryStorage;

const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";
const RELEASE_POINTS: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];
const TITLE_26: &str = "uscode/title_26";

/// Section 70431(a)(5)(C) of Pub. L. 119-21:
///
/// > Sections 1202(b)(2), 1202(g)(2)(A), and 1202(j)(1)(A) are each amended by
/// > striking "more than 5 years" and inserting "at least 3 years (more than
/// > 5 years …
///
/// The markup reads no section out of a plural *Sections*. The OLRC table
/// classifies 70431(a)(5) to 26 U.S.C. 1202, and nothing else.
const THREE_PLACES_IN_1202: &str = "0907f7336a23";

fn committed_bill_download() -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: BILL_ID.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: std::collections::HashMap::new(),
    }
}

/// The bill and title 26 at each committed release point, with the
/// renumberings recorded as `build-dataset` records them, and the committed
/// OLRC table stated as links, as `add-classifications` states them.
fn dataset() -> &'static Dataset<InMemoryStorage> {
    static DATASET: OnceLock<Dataset<InMemoryStorage>> = OnceLock::new();
    DATASET.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        dataset
            .load_bill_download(&committed_bill_download())
            .expect("the committed bill should load");
        for date in RELEASE_POINTS {
            dataset
                .add_uslm_xml(&format!("tests/test_data/usc/{date}/usc26.xml"), date, None)
                .expect("title 26 should parse");
        }
        let windows = adjacent_expressions(&dataset).expect("the windows should list");
        let bill = dataset
            .bill_document(BILL_ID)
            .expect("the dataset should answer for the bill")
            .expect("the bill is held as a document");
        dataset
            .record_redesignations_over(BILL_ID, &bill.root, &windows)
            .expect("the renumberings should record");

        let html = std::fs::read_to_string("tests/test_data/olrc/classification/tbl119pl_1st.htm")
            .expect("the committed table should read");
        let table = ClassificationTable::parse(&html).expect("the committed table should parse");
        let mut paths = SectionPaths::new();
        for date in RELEASE_POINTS {
            let expression = dataset
                .get_expression(&ExpressionId::new(WorkId::new(TITLE_26), date))
                .expect("storage should answer")
                .expect("title 26 is held");
            paths.add_work(&expression.root);
        }
        let scope = dataset.scope().expect("the scope should derive");
        for link in classify(&table.rows, &scope, &paths, "olrc:tbl119pl_1st.htm").links {
            dataset.add_link(link).expect("the link should add");
        }
        dataset
    })
}

/// What the matcher finds over the dataset, once for every case.
fn matched() -> &'static EvidenceMatching {
    static MATCHED: OnceLock<EvidenceMatching> = OnceLock::new();
    MATCHED.get_or_init(|| match_by_evidence(dataset()).expect("the matcher should run"))
}

/// The matcher's answer for one amendment, by the start of its id.
fn answer_for(id_start: &str) -> &'static AmendmentMatch {
    let found: Vec<&AmendmentMatch> = matched()
        .matches
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect();
    let [one] = found[..] else {
        panic!("one answer for {id_start}, found {}", found.len());
    };
    one
}

#[test]
fn should_address_the_section_the_olrc_classifies_when_the_markup_names_none() {
    let answer = answer_for(THREE_PLACES_IN_1202);

    assert_eq!(answer.address.section.as_deref(), Some("/us/usc/t26/s1202"));
    let Outcome::Linked(linked) = &answer.outcome else {
        panic!(
            "the change to § 1202 should be linked: {:?}",
            answer.outcome
        );
    };
    for path in linked.paths() {
        assert!(
            path.contains("/section_1202/"),
            "every change is inside § 1202: {path}"
        );
    }
}

/// A reviewer must see which evidence gave the address: the markup, or the
/// OLRC's row, and which row.
#[test]
fn should_say_in_the_evidence_that_the_olrc_table_gave_the_address_when_it_did() {
    let answer = answer_for(THREE_PLACES_IN_1202);

    let links = answer.links();
    assert!(!links.is_empty(), "the amendment is linked");
    for link in links {
        let reasoning = link
            .provenance
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reasoning.clone())
            .expect("the link carries its reasoning");
        assert!(
            reasoning.contains("read from the OLRC classification table"),
            "the reasoning names the source: {reasoning}"
        );
        assert!(
            reasoning.contains("Pub. L. 119-21 § 70431(a)(5)"),
            "the reasoning names the row: {reasoning}"
        );
        assert!(
            !reasoning.contains("read from the bill's markup"),
            "the markup gave no address: {reasoning}"
        );
    }
}

/// Section 70118(a) of Pub. L. 119-21:
///
/// > Section 11026(a) of Public Law 115–97 is amended by striking ", with
/// > respect to the applicable period".
///
/// The OLRC classifies 70118(a)-(c) to 26 U.S.C. 112 as `nt`: a note under
/// § 112, which the dataset does not hold. A note row never addresses a
/// section's own text, so the amendment keeps no address.
#[test]
fn should_not_address_an_amendment_by_a_row_that_classifies_only_a_note() {
    let answer = answer_for("5219c7e9a020");

    assert_eq!(answer.address.section, None);
    let Outcome::Residue(residue) = &answer.outcome else {
        panic!("a note gives no address: {:?}", answer.outcome);
    };
    assert_eq!(residue.stage, Stage::Address);
}

/// Section 70116(a)(2) of Pub. L. 119-21:
///
/// > Paragraph (1) of section 103(e) of the SECURE 2.0 Act of 2022 is
/// > repealed, and the Internal Revenue Code of 1986 shall be applied and
/// > administered as though such paragraph were never enacted.
///
/// The OLRC classifies 70116(a)(2) to 26 U.S.C. 25B, and a row gives a section
/// and nothing below it. The instruction quotes no words, so nothing in it can
/// tell its change from the other changes to § 25B in the window. A section
/// with nothing to choose among its changes is not an address to link from.
#[test]
fn should_not_address_by_the_olrc_an_amendment_that_quotes_no_words() {
    let answer = answer_for("ac1415f08ef8");

    assert_eq!(answer.address.section, None);
    let Outcome::Residue(residue) = &answer.outcome else {
        panic!("no address, so no link: {:?}", answer.outcome);
    };
    assert_eq!(residue.stage, Stage::Address);
}
