//! A bill is a document: one read of its XML, one stored document.
//!
//! Every case here is read out of the committed public law, `119-hr-1`. A bill
//! used to enter a dataset as `Bill { bill_id, amendments }` alone — a flat map
//! keyed by content hash, with no structure and no path into it — while the
//! parser had held the bill's whole hierarchy since #114
//! (`docs/adr/0009-a-source-is-parsed-once-a-bill-is-a-document.md`).

use std::collections::HashMap;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, WorkId};
use words_to_data::document::DocumentNode;
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::UslmFacts;
use words_to_data::uslm::bill_parser::amendment_paths;
use words_to_data::uslm::bill_redesignation::{redesignations_stated, redesignations_stated_in};

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";

/// How the dataset names the bill: the id `build-dataset` is given, and the id
/// every amendment hash and every link is minted under.
const BILL_ID: &str = "119-hr-1";

/// How the publisher names the same document, which is what its path is built
/// from.
const BILL_WORK: &str = "publiclawdocument_119-21";

/// The date the bill's own markup says it was approved.
const APPROVED: &str = "2025-07-04";

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "bill document".to_string(),
        description: "one public law, stored as a document".to_string(),
        author: "Words to Data LLC".to_string(),
        source_urls: vec![],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        declaration: None,
        method_runs: Vec::new(),
    }
}

/// The bill as the Congress client would hand it over, read from the committed
/// cache, so a test exercises the path a build really takes.
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
        member_jsons: HashMap::new(),
    }
}

fn dataset_holding_the_bill() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(metadata());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    dataset
}

#[test]
fn should_hold_the_bill_as_an_expression_when_a_bill_download_is_loaded() {
    let dataset = dataset_holding_the_bill();

    let work = WorkId::new(BILL_WORK);
    let held = dataset
        .expressions(&work)
        .expect("the dataset should answer for the bill's work");

    assert_eq!(held.len(), 1, "a bill is published once");
    assert_eq!(held[0].id.at, APPROVED);
}

#[test]
fn should_keep_the_structure_the_parser_found_when_the_bill_is_stored() {
    let dataset = dataset_holding_the_bill();

    let held = dataset
        .expressions(&WorkId::new(BILL_WORK))
        .expect("the dataset should answer for the bill's work");
    let bill = dataset
        .get_expression(&held[0].id)
        .expect("the expression should read")
        .expect("the expression should be there");

    assert_eq!(bill.root.data.node_type.as_str(), "bill.public_law");
    // The section and the ten titles below `<main>`. Before #114 a public law
    // parsed to its root and nothing else.
    assert_eq!(bill.root.children.len(), 11);
    assert!(
        bill.root
            .find("publiclawdocument_119-21/title_VII/subtitle_A")
            .is_some(),
        "the bill's own nesting should be there to walk"
    );
}

/// The stored bill document, which every case below reads.
fn stored_bill_root(dataset: &Dataset<InMemoryStorage>) -> DocumentNode {
    let held = dataset
        .expressions(&WorkId::new(BILL_WORK))
        .expect("the dataset should answer for the bill's work");
    dataset
        .get_expression(&held[0].id)
        .expect("the expression should read")
        .expect("the expression should be there")
        .root
}

#[test]
fn should_locate_every_amendment_by_a_path_when_the_bill_is_stored() {
    let dataset = dataset_holding_the_bill();
    let bill = dataset
        .get_bill(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the bill should be there");
    let root = stored_bill_root(&dataset);

    let located = amendment_paths(&root);

    assert_eq!(
        located.len(),
        bill.amendments.len(),
        "every amendment the bill states should sit at a path in the bill"
    );
    for id in bill.amendments.keys() {
        assert!(
            located.contains_key(id),
            "amendment {id} is stored with no path into the bill"
        );
    }
}

#[test]
fn should_keep_the_content_hash_as_the_identity_when_an_amendment_gains_a_path() {
    let dataset = dataset_holding_the_bill();
    let bill = dataset
        .get_bill(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the bill should be there");
    let root = stored_bill_root(&dataset);
    let located = amendment_paths(&root);

    // The motivating amendment: "Section 898(c) is amended by striking
    // paragraph (2) and redesignating paragraph (3) as paragraph (2)."
    let (id, _) = bill
        .amendments
        .iter()
        .find(|(_, amendment)| amendment.amending_text.contains("Section 898(c)"))
        .expect("the bill amends section 898(c)");

    let path = located.get(id).expect("that amendment should be located");
    assert!(
        path.starts_with(BILL_WORK),
        "an amendment sits inside the bill, at {path}"
    );
    let node = root
        .find(path)
        .expect("the path should find the node whose words the amendment is");
    // The path locates and the hash identifies, so the node found this way
    // still states the amendment the hash was taken over (ADR 0001).
    assert_eq!(
        UslmFacts::of(&node.data)
            .and_then(|facts| facts.amendment)
            .map(|amendment| amendment.id),
        Some(id.clone())
    );
}

#[test]
fn should_state_the_same_redesignations_from_the_stored_bill_as_from_its_markup() {
    let dataset = dataset_holding_the_bill();
    let root = stored_bill_root(&dataset);

    let xml = std::fs::read_to_string(format!("{BILL_DIR}/public_law.xml"))
        .expect("the bill should be committed");
    let from_markup = redesignations_stated(BILL_ID, &xml).expect("the bill's markup should read");

    let from_the_dataset = redesignations_stated_in(BILL_ID, &root);

    // The second read of the bill recovered nothing the first one could not
    // keep. `119-hr-1` states 57 renumberings, and the stored bill states the
    // same 57, word for word (ADR 0009).
    assert_eq!(from_markup.len(), 57);

    // The stored bill gives one thing the markup cannot: where in the bill the
    // words sit. A path is generated when the bill becomes a document, so the
    // markup reader has none to give, and the comparison sets it aside.
    let without_the_path: Vec<_> = from_the_dataset
        .iter()
        .cloned()
        .map(|mut statement| {
            assert!(
                statement.path.is_some(),
                "a statement read from the stored bill knows where it sat"
            );
            statement.path = None;
            statement
        })
        .collect();
    assert_eq!(without_the_path, from_markup);
}

/// Whether any node of a tree carries this heading.
fn any_heading(node: &DocumentNode, heading: &str) -> bool {
    node.data.heading.as_deref() == Some(heading)
        || node
            .children
            .iter()
            .any(|child| any_heading(child, heading))
}

#[test]
fn should_keep_the_words_a_bill_enacts_in_the_payload_when_the_tree_leaves_them_out() {
    let dataset = dataset_holding_the_bill();
    let bill = dataset
        .get_bill(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the bill should be there");
    let root = stored_bill_root(&dataset);
    let located = amendment_paths(&root);

    // 119-hr-1 adds a whole new section 20306 to chapter 203 of title 51.
    let (id, _) = bill
        .amendments
        .iter()
        .find(|(_, amendment)| {
            amendment
                .amending_text
                .contains("Chapter 203 of title 51, United States Code, is amended")
        })
        .expect("the bill adds a section to chapter 203 of title 51");

    let node = root
        .find(&located[id])
        .expect("the amendment should sit at a path in the bill");
    let stated = UslmFacts::of(&node.data)
        .and_then(|facts| facts.amendment)
        .expect("an instruction node states its amendment");

    assert_eq!(
        stated.enacted_text.len(),
        1,
        "the instruction quotes one block, so one block is kept"
    );
    // No space between the two. The number and the heading are separate
    // elements, and only one of the two printings of this act puts a space
    // between them: the cached one writes `<heading> <sidenote>` and the
    // Government Publishing Office writes `<heading><sidenote>`. A text node
    // that is entirely whitespace is therefore a fact about the file and not
    // about the law, so it does not reach the text (#219).
    assert!(
        stated.enacted_text[0].contains("20306.Deadlines."),
        "the words the bill enacts should travel with the node that enacts them"
    );

    // And they are not law in force, so they are not a provision of the bill
    // (#86). Were the quoted section in the tree, it would be a node with its
    // own heading, which an annotation could then name.
    assert!(
        !any_heading(
            &root,
            "Special appropriations for Mars missions, Artemis missions, and Moon to Mars program"
        ),
        "quoted amendment text must not enter the hierarchy as a provision"
    );
}
