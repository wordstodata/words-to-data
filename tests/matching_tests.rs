//! Tests for candidate gathering (`words_to_data::matching`).
//!
//! The candidate list is what an LLM reads when it decides which change an
//! amendment caused, so its order is part of the model's input. Two runs over
//! the same dataset must present the same candidates in the same order (#73).

use std::collections::HashMap;

use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::diff::TreeDiff;
use words_to_data::legislature::BillDiff;
use words_to_data::matching::{DEFAULT_SIMILARITY_CUTOFF, build_matches};
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_parser::parse_bill_amendments;

const TITLE_26: &str = "uscode/title_26";
const USC26_18: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const USC26_30: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const PL_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
/// The public law number the act carries, and the id the Congress.gov download
/// goes in under.
const PL_ID: &str = "119-21";

/// The same act, as govinfo publishes it: a second file, of the one law the
/// corpus holds.
///
/// It goes in under the act's bill number, so the dataset holds two bills to
/// choose between. Both documents are real and both are in `tests/test_data`;
/// only the name each goes in under is chosen here.
const GOVINFO_PL_XML: &str = "tests/test_data/bills/hr-119-21.xml";
const BILL_ID: &str = "119-hr-1";

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
        ..Default::default()
    });
    dataset
        .add_uslm_xml(USC26_18, "2025-07-18", Some("Before".to_string()))
        .expect("add first version");
    dataset
        .add_uslm_xml(USC26_30, "2025-07-30", Some("After".to_string()))
        .expect("add second version");

    let bill = parse_bill_amendments(PL_ID, PL_XML).expect("parse bill");
    dataset.add_bill(bill).expect("add bill");
    dataset
}

/// The same fixture, but with word-level changes stubbed onto every amendment
/// so that the similarity channel actually produces scores.
///
/// Word-level changes come from an LLM, which the library does not call, so a
/// dataset built here carries none and only the mention channel fires.
fn fixture_with_scored_amendments() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Matching Fixture".to_string(),
        description: "Title 26, two release points".to_string(),
        author: "Tester".to_string(),
        source_urls: vec!["https://uscode.house.gov".to_string()],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(USC26_18, "2025-07-18", Some("Before".to_string()))
        .expect("add first version");
    dataset
        .add_uslm_xml(USC26_30, "2025-07-30", Some("After".to_string()))
        .expect("add second version");

    let mut bill = parse_bill_amendments(PL_ID, PL_XML).expect("parse bill");
    for amendment in bill.amendments.values_mut() {
        amendment.changes = vec![BillDiff {
            removed: vec!["specified".to_string()],
            added: vec!["foreign".to_string()],
        }];
    }
    dataset.add_bill(bill).expect("add bill");
    dataset
}

#[test]
fn should_drop_candidates_at_or_below_the_similarity_cutoff_when_building_matches() {
    let dataset = fixture_with_scored_amendments();
    let diff = dataset
        .compute_diff(&at("2025-07-18"), &at("2025-07-30"))
        .expect("compute diff");

    let bills = every_bill(&dataset);
    let permissive = build_matches(&dataset, &diff, 0.0, &bills);
    let strict = build_matches(&dataset, &diff, 0.99, &bills);

    let scored = |matches: &[words_to_data::matching::AmendmentMatch]| -> usize {
        matches
            .iter()
            .flat_map(|m| m.candidates.iter())
            .filter(|c| c.similarity.is_some())
            .count()
    };
    assert!(
        scored(&permissive) > scored(&strict),
        "A higher cutoff should leave fewer scored candidates, got {} then {}",
        scored(&permissive),
        scored(&strict)
    );

    // Nothing at or below the cutoff should survive it.
    for m in &strict {
        for candidate in &m.candidates {
            if let Some(similarity) = &candidate.similarity {
                assert!(
                    similarity.score > 0.99,
                    "Candidate {} scored {} and should have been dropped",
                    candidate.diff.root_path,
                    similarity.score
                );
            }
        }
    }
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

    let bills = every_bill(&dataset);
    let first = build_matches(&dataset, &diff, DEFAULT_SIMILARITY_CUTOFF, &bills);
    let second = build_matches(&dataset, &diff, DEFAULT_SIMILARITY_CUTOFF, &bills);

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

    let matches = build_matches(
        &dataset,
        &diff,
        DEFAULT_SIMILARITY_CUTOFF,
        &every_bill(&dataset),
    );
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

/// The same fixture, holding two bills instead of one.
///
/// The corpus holds one act, published twice: the Congress.gov download and the
/// govinfo USLM file. Each goes in under one of the two names the act is
/// published by, so "every bill" and "one bill" are different sets. An
/// amendment id is a hash of its bill id and its text, so no amendment of one
/// bill can be mistaken for an amendment of the other.
fn fixture_with_two_bills() -> Dataset<InMemoryStorage> {
    let mut dataset = make_fixture();
    let bill = parse_bill_amendments(BILL_ID, GOVINFO_PL_XML).expect("parse the second bill");
    dataset.add_bill(bill).expect("add the second bill");
    dataset
}

/// Every bill the dataset holds, which is what a run that names none covers.
fn every_bill(dataset: &Dataset<InMemoryStorage>) -> Vec<String> {
    dataset.list_bill_ids().expect("list bills")
}

/// The amendments of one bill, in the order the matches give them.
fn amendments_of<'a>(
    matches: &'a [words_to_data::matching::AmendmentMatch],
    bill_id: &str,
) -> Vec<&'a str> {
    matches
        .iter()
        .filter(|m| m.bill_id == bill_id)
        .map(|m| m.amendment_id.as_str())
        .collect()
}

/// A narrowed run and a full run must agree about the bill they share. The
/// match list is what the model reads, so a filter that shuffled it would
/// change the answers as well as the count (#73).
#[test]
fn should_match_only_the_named_bill_when_one_bill_of_two_is_named() {
    let dataset = fixture_with_two_bills();
    let diff = dataset
        .compute_diff(&at("2025-07-18"), &at("2025-07-30"))
        .expect("compute diff");

    let every = build_matches(
        &dataset,
        &diff,
        DEFAULT_SIMILARITY_CUTOFF,
        &every_bill(&dataset),
    );
    let narrowed = build_matches(
        &dataset,
        &diff,
        DEFAULT_SIMILARITY_CUTOFF,
        &[BILL_ID.to_string()],
    );

    assert!(
        every.iter().any(|m| m.bill_id == PL_ID) && every.iter().any(|m| m.bill_id == BILL_ID),
        "a run over every bill should cover both bills"
    );
    assert!(
        !narrowed.is_empty(),
        "the named bill should still have matched amendments"
    );
    assert!(
        narrowed.iter().all(|m| m.bill_id == BILL_ID),
        "a run told one bill should match no other bill"
    );

    assert_eq!(
        amendments_of(&narrowed, BILL_ID),
        amendments_of(&every, BILL_ID),
        "the two runs should give the shared bill's amendments in the same order"
    );
    let shared: Vec<_> = every.into_iter().filter(|m| m.bill_id == BILL_ID).collect();
    assert_eq!(
        candidate_paths(&narrowed),
        candidate_paths(&shared),
        "the two runs should give each amendment the same candidates, in the same order"
    );
}
