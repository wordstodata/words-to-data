//! Tests for candidate gathering (`words_to_data::matching`).
//!
//! The candidate list is what an LLM reads when it decides which change an
//! amendment caused, so its order is part of the model's input. Two runs over
//! the same dataset must present the same candidates in the same order (#73).

use std::collections::HashMap;

use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::diff::TreeDiff;
use words_to_data::matching::build_matches;
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_parser::parse_bill_amendments;

const TITLE_26: &str = "uscode/title_26";
const USC26_18: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const USC26_30: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const PL_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

fn at(date: &str) -> ExpressionId {
    ExpressionId::new(WorkId::new(TITLE_26), date)
}

/// Title 26 across two release points, plus the real public law that amends it.
fn make_fixture() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Matching Fixture".to_string(),
        description: "Title 26, two release points".to_string(),
        author: "Tester".to_string(),
        source_urls: vec!["https://uscode.house.gov".to_string()],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
    });
    dataset
        .add_uslm_xml(USC26_18, "2025-07-18", Some("Before".to_string()))
        .expect("add first version");
    dataset
        .add_uslm_xml(USC26_30, "2025-07-30", Some("After".to_string()))
        .expect("add second version");

    let bill = parse_bill_amendments("119-21", PL_XML).expect("parse bill");
    dataset.add_bill(bill).expect("add bill");
    dataset
}

/// Position of every diff node in a document-order walk of the tree.
fn document_order(diff: &TreeDiff) -> HashMap<String, usize> {
    fn walk(diff: &TreeDiff, index: &mut HashMap<String, usize>) {
        let next = index.len();
        index.entry(diff.root_path.clone()).or_insert(next);
        for child in &diff.child_diffs {
            walk(child, index);
        }
    }
    let mut index = HashMap::new();
    walk(diff, &mut index);
    index
}

fn candidate_paths(matches: &[words_to_data::matching::AmendmentMatch]) -> Vec<Vec<String>> {
    matches
        .iter()
        .map(|m| {
            m.candidates
                .iter()
                .map(|c| c.diff.root_path.clone())
                .collect()
        })
        .collect()
}

#[test]
fn should_order_candidates_identically_when_matches_are_built_twice() {
    let dataset = make_fixture();
    let diff = dataset
        .compute_diff(&at("2025-07-18"), &at("2025-07-30"))
        .expect("compute diff");

    let first = build_matches(&dataset, &diff);
    let second = build_matches(&dataset, &diff);

    // Only an amendment with more than one candidate can expose an ordering bug.
    assert!(
        first.iter().any(|m| m.candidates.len() > 1),
        "This test needs an amendment with more than one candidate to be meaningful"
    );

    let first_ids: Vec<&str> = first.iter().map(|m| m.amendment_id.as_str()).collect();
    let second_ids: Vec<&str> = second.iter().map(|m| m.amendment_id.as_str()).collect();
    assert_eq!(
        first_ids, second_ids,
        "The amendments should be matched in the same order every run"
    );

    assert_eq!(
        candidate_paths(&first),
        candidate_paths(&second),
        "Each amendment's candidates should be in the same order every run"
    );
}

#[test]
fn should_order_candidates_by_document_position_when_building_matches() {
    let dataset = make_fixture();
    let diff = dataset
        .compute_diff(&at("2025-07-18"), &at("2025-07-30"))
        .expect("compute diff");
    let order = document_order(&diff);

    let matches = build_matches(&dataset, &diff);
    assert!(
        matches.iter().any(|m| m.candidates.len() > 1),
        "This test needs an amendment with more than one candidate to be meaningful"
    );

    for m in &matches {
        let positions: Vec<usize> = m
            .candidates
            .iter()
            .map(|c| {
                *order
                    .get(&c.diff.root_path)
                    .unwrap_or_else(|| panic!("{} is not a node of the diff", c.diff.root_path))
            })
            .collect();
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "Candidates for amendment {} are not in document order: {:?}",
            m.amendment_id,
            m.candidates
                .iter()
                .map(|c| c.diff.root_path.as_str())
                .collect::<Vec<_>>()
        );
    }
}
