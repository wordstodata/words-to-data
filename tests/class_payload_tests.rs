//! The two class-payload rules, as tests instead of prose (#149).
//!
//! `docs/adr/0004-links-are-stored-and-identified-by-what-they-say.md` and
//! `docs/adr/0006-a-document-node-is-class-neutral.md` state two rules about the
//! payload a node carries:
//!
//! - The core stores it, hands it back unchanged, and **never reads it**.
//! - **Nothing needed to report a node may live in it**, because a reader that
//!   cannot open the payload must still say where the node is, what kind of
//!   thing it is, what it says, and how its text was obtained.
//!
//! Both were prose. The failure mode of the first is invisible and cumulative:
//! nothing in this repo breaks when the core starts reading a payload, because
//! every reader here knows the classes. What breaks is a third party's reader
//! that does not, and no such reader exists yet — so the payloads fill up first
//! and the fault is found last.
//!
//! The second rule is a judgement about one field and cannot be asserted. It is
//! written as a review checklist beside `ClassPayload` and `KindPayload`, where
//! a person adding a field reads it.
//!
//! The fixture is the real corpus, carrying the payload text the USLM parser
//! really wrote and relabelled into a namespace nothing here understands. The
//! bytes are the publisher's, so a reserialization that reorders an object's
//! keys is caught by the same assertion that catches a dropped payload.

use std::sync::Arc;

use tempfile::TempDir;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, work_roots,
};
use words_to_data::document::{ClassPayload, DocumentNode};
use words_to_data::inspect;
use words_to_data::storage::{InMemoryStorage, SqliteStorage};
use words_to_data::uslm::parser::parse;

/// A namespace no reader in this repo understands, and none ever will.
///
/// The name says so out loud on purpose. `westlaw` would read naturally and is
/// a real publisher, so an extension could one day claim it and quietly turn
/// every assertion below into a false pass. Nothing will ever call itself
/// `no_such_class`.
const UNKNOWN_NAMESPACE: &str = "no_such_class";

const EARLY: &str = "2025-07-18";

/// Title 9. Small, and it reads the same at both release points.
const TITLE_9: &str = "usc09.xml";

/// A section of title 9 that carries both a heading and USLM facts.
const SECTION_2: &str = "uscode/title_9/chapter_1/section_2";

/// Text that appears in a `chapeau` in title 9.
const CHAPEAU_TEXT: &str = "In any of the following cases the United States court";

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Class payload fixture".to_string(),
        description: "Real USC text, with its payloads relabelled".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    }
}

fn release_point(file: &str, date: &str) -> DocumentNode {
    let path = format!("tests/test_data/usc/{date}/{file}");
    parse(&path, date).unwrap_or_else(|e| panic!("{file} at {date} should parse: {e}"))
}

/// Move every payload in the tree into [`UNKNOWN_NAMESPACE`], keeping its text.
///
/// The text stays exactly as the parser wrote it. Only the label changes, so the
/// one thing that differs from an ordinary dataset is whether any reader here
/// claims to understand the facts.
fn relabelled(mut node: DocumentNode) -> DocumentNode {
    if let Some(payload) = node.data.payload.take() {
        node.data.payload = Some(ClassPayload {
            namespace: Arc::from(UNKNOWN_NAMESPACE),
            value: payload.value,
        });
    }
    node.children = node.children.into_iter().map(relabelled).collect();
    node
}

fn dataset_of(roots: Vec<(DocumentNode, &str)>) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(metadata());
    for (tree, date) in roots {
        for root in work_roots(tree) {
            dataset
                .add_expression(Expression {
                    id: ExpressionId::new(WorkId::new(root.data.path.to_string()), date),
                    label: None,
                    root,
                })
                .expect("the expression should be stored");
        }
    }
    dataset
}

/// One title at each date, twice: untouched, and with every payload relabelled.
///
/// Returned as a pair because the untouched dataset is the answer key — what a
/// reader that understands the class says — and the relabelled one must match
/// it. Each release point is parsed once and the tree cloned, because a parse of
/// a large title costs far more than a clone of it.
fn known_and_unknown(
    file: &str,
    dates: &[&str],
) -> (Dataset<InMemoryStorage>, Dataset<InMemoryStorage>) {
    let trees: Vec<(DocumentNode, &str)> = dates
        .iter()
        .map(|date| (release_point(file, date), *date))
        .collect();
    let known = dataset_of(trees.clone());
    let unknown = dataset_of(
        trees
            .into_iter()
            .map(|(tree, date)| (relabelled(tree), date))
            .collect(),
    );
    (known, unknown)
}

/// Round-trip a dataset through SQLite, the second backend.
///
/// The caller must keep the returned directory in scope: dropping it removes the
/// database.
fn to_sqlite(dataset: &Dataset<InMemoryStorage>) -> (TempDir, Dataset<SqliteStorage>) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("dataset.sqlite");
    dataset.save_to_sqlite(&path).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&path).expect("open sqlite");
    (dir, sqlite)
}

/// Which field of which path a search matched, in order.
fn hits(results: &[words_to_data::dataset::SearchResult]) -> Vec<(String, String, String)> {
    results
        .iter()
        .map(|r| (r.expression.to_string(), r.path.clone(), r.field.clone()))
        .collect()
}

#[test]
fn should_report_and_search_nodes_whose_payload_is_in_an_unknown_namespace() {
    let (known, unknown) = known_and_unknown(TITLE_9, &[EARLY]);
    let (_dir, sqlite) = to_sqlite(&unknown);

    let expected_counts: Vec<usize> = inspect::expressions(&known, None)
        .expect("list the known expressions")
        .iter()
        .map(|e| e.element_count)
        .collect();
    let expected_hits = hits(&inspect::search(&known, CHAPEAU_TEXT).expect("search the known"));
    assert!(
        !expected_hits.is_empty(),
        "the fixture should hold the chapeau text"
    );

    for (label, dataset) in [
        ("memory", &unknown as &dyn ReportsNodes),
        ("sqlite", &sqlite as &dyn ReportsNodes),
    ] {
        // A payload it cannot open must cost a reader nothing. It still counts
        // the nodes, finds one by path, and searches the words inside it.
        assert_eq!(
            dataset.element_counts(),
            expected_counts,
            "{label} lost nodes it could not read the payload of"
        );
        assert_eq!(
            dataset.hits_for(CHAPEAU_TEXT),
            expected_hits,
            "{label} searched the text differently"
        );

        let found = dataset.node_at(SECTION_2);
        assert_eq!(found.len(), 1, "{label} should find {SECTION_2}");
        let payload = found[0]
            .data
            .payload
            .as_ref()
            .unwrap_or_else(|| panic!("{label} dropped the payload of {SECTION_2}"));
        assert_eq!(&*payload.namespace, UNKNOWN_NAMESPACE, "{label} namespace");
    }
}

/// The three questions asked of both backends above, behind one name.
///
/// `Dataset<InMemoryStorage>` and `Dataset<SqliteStorage>` are different types,
/// so a loop over the pair needs one.
trait ReportsNodes {
    fn element_counts(&self) -> Vec<usize>;
    fn hits_for(&self, query: &str) -> Vec<(String, String, String)>;
    fn node_at(&self, path: &str) -> Vec<DocumentNode>;
}

impl<S: words_to_data::storage::Storage> ReportsNodes for Dataset<S> {
    fn element_counts(&self) -> Vec<usize> {
        inspect::expressions(self, None)
            .expect("list expressions")
            .iter()
            .map(|e| e.element_count)
            .collect()
    }

    fn hits_for(&self, query: &str) -> Vec<(String, String, String)> {
        hits(&inspect::search(self, query).expect("search"))
    }

    fn node_at(&self, path: &str) -> Vec<DocumentNode> {
        self.find_nodes(path)
            .expect("find nodes")
            .into_iter()
            .map(|(_, node)| node)
            .collect()
    }
}
