//! Each amendment of a public law, linked to the change it made from its
//! address, its window and the words it quotes, with no model call (#250).
//!
//! Stages 3 and 4 of
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//!
//! Every case is read out of the committed corpus: the public law `119-hr-1`
//! and title 7 of the Code at its three committed release points. Title 7 is
//! the smaller of the two titles the bill amends most, and 82 of the bill's
//! amendments act in it.

use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::storage::InMemoryStorage;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";
const RELEASE_POINTS: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];
const TITLE_7: &str = "uscode/title_7";

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

/// The bill and title 7 at each committed release point, read once for every
/// test in this file.
fn dataset() -> &'static Dataset<InMemoryStorage> {
    static DATASET: OnceLock<Dataset<InMemoryStorage>> = OnceLock::new();
    DATASET.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        dataset
            .load_bill_download(&committed_bill_download())
            .expect("the committed bill should load");
        for date in RELEASE_POINTS {
            dataset
                .add_uslm_xml(&format!("tests/test_data/usc/{date}/usc07.xml"), date, None)
                .expect("title 7 should parse");
        }
        dataset
    })
}

/// What the matcher says about every amendment, worked out once.
fn matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| match_by_evidence(dataset()).expect("the matcher should run"))
}

/// The matcher's answer for one amendment, by the start of its id.
fn match_of(id_start: &str) -> &'static AmendmentMatch {
    let found: Vec<&AmendmentMatch> = matches()
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect();
    assert_eq!(found.len(), 1, "one amendment starts with {id_start}");
    found[0]
}

fn title_7_at(date: &str) -> ExpressionId {
    ExpressionId::new(WorkId::new(TITLE_7), date)
}

#[test]
fn should_link_the_one_change_under_the_address_when_an_amendment_has_a_single_change() {
    // Section 19(a)(2)(A)(ii) of the Food and Nutrition Act of 2008
    // (7 U.S.C. 2028(a)(2)(A)(ii)) is amended by striking "section 3(u)(4)"
    // and inserting "section 3(u)(3)".
    let found = match_of("bf20b9a41d6c");

    let Outcome::Linked(linked) = &found.outcome else {
        panic!("the amendment should be linked: {:?}", found.outcome);
    };
    assert_eq!(linked.from, title_7_at("2025-07-18"));
    assert_eq!(linked.to, title_7_at("2025-07-30"));
    assert_eq!(
        linked.paths,
        vec![
            "uscode/title_7/chapter_51/section_2028/subsection_a/paragraph_2/subparagraph_A/clause_ii"
                .to_string()
        ]
    );
}
