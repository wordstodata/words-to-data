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
    Dataset, DatasetMetadata, Expression, ExpressionId, Format, WorkId, work_roots,
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
const LATER: &str = "2025-07-30";

/// Title 9. Small, and it reads the same at both release points.
const TITLE_9: &str = "usc09.xml";

/// Title 7. It really changed between the two release points, so a diff over it
/// has something to say.
const TITLE_7: &str = "usc07.xml";

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
        ("memory", &unknown as &dyn AnyBackend),
        ("sqlite", &sqlite as &dyn AnyBackend),
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

#[test]
fn should_diff_two_expressions_whose_payloads_are_in_an_unknown_namespace() {
    let (known, unknown) = known_and_unknown(TITLE_7, &[EARLY, LATER]);
    let (_dir, sqlite) = to_sqlite(&unknown);

    let work = WorkId::new("uscode/title_7");
    let from = ExpressionId::new(work.clone(), EARLY);
    let to = ExpressionId::new(work, LATER);

    let expected = inspect::diff(&known, &from, &to).expect("diff the known");
    assert!(
        !expected.changed_paths.is_empty(),
        "the two release points should differ"
    );

    for (label, summary) in [
        ("memory", inspect::diff(&unknown, &from, &to)),
        ("sqlite", inspect::diff(&sqlite, &from, &to)),
    ] {
        // The diff pairs nodes on their path and their type and compares the
        // five text fields. A payload it cannot open changes none of that, so
        // the answer must be the one a reader of the class gets.
        let summary = summary.unwrap_or_else(|e| panic!("{label} should diff: {e}"));
        assert_eq!(
            summary.changed_paths, expected.changed_paths,
            "{label} changed paths"
        );
        assert_eq!(
            summary.added_paths, expected.added_paths,
            "{label} added paths"
        );
        assert_eq!(
            summary.removed_paths, expected.removed_paths,
            "{label} removed paths"
        );
    }
}

#[test]
fn should_hand_back_a_payload_byte_for_byte_on_both_backends_and_through_a_w2d_file() {
    let (known, unknown) = known_and_unknown(TITLE_9, &[EARLY]);
    let expected = unknown.payloads();
    assert_eq!(
        expected.len(),
        known.payloads().len(),
        "relabelling should move every payload, not drop any"
    );
    assert!(
        expected.len() > 50,
        "the fixture should carry a payload worth round-tripping, got {}",
        expected.len()
    );

    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.w2d");
    let file = file.to_str().expect("a printable path");
    unknown
        .save(file, Format::Compact)
        .expect("write the W2D file");
    let from_file = Dataset::load(file, Format::Compact).expect("read the W2D file");

    let (_sqlite_dir, sqlite) = to_sqlite(&unknown);

    for (label, carried) in [
        ("memory", unknown.payloads()),
        ("sqlite", sqlite.payloads()),
        ("w2d file", from_file.payloads()),
    ] {
        // `docs/adr/0006` holds a payload as JSON *text* rather than as a value
        // tree so that "hands it back unchanged" means byte for byte. A
        // reserialization would reorder an object's keys, pass any check of
        // what the JSON *means*, and break a third party's signature over the
        // bytes it sent. Two `str` are equal only when their bytes are equal,
        // so comparing the text is the byte comparison, readably.
        assert_eq!(carried.len(), expected.len(), "{label} payload count");
        for ((path, got), (_, want)) in carried.iter().zip(&expected) {
            assert_eq!(
                &*got.namespace, &*want.namespace,
                "{label} renamed the namespace at {path}"
            );
            assert_eq!(
                &*got.value, &*want.value,
                "{label} rewrote the payload at {path}"
            );
        }
    }
}

/// The questions asked of every backend above, behind one name.
///
/// `Dataset<InMemoryStorage>` and `Dataset<SqliteStorage>` are different types,
/// so a loop over them needs one.
trait AnyBackend {
    fn element_counts(&self) -> Vec<usize>;
    fn hits_for(&self, query: &str) -> Vec<(String, String, String)>;
    fn node_at(&self, path: &str) -> Vec<DocumentNode>;
    /// Every payload the dataset holds, by the path of the node carrying it, in
    /// document order.
    fn payloads(&self) -> Vec<(String, ClassPayload)>;
}

fn payloads_of(node: &DocumentNode, into: &mut Vec<(String, ClassPayload)>) {
    if let Some(payload) = &node.data.payload {
        into.push((node.data.path.to_string(), payload.clone()));
    }
    for child in &node.children {
        payloads_of(child, into);
    }
}

impl<S: words_to_data::storage::Storage> AnyBackend for Dataset<S> {
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

    fn payloads(&self) -> Vec<(String, ClassPayload)> {
        let mut found = Vec::new();
        for work in self.works().expect("list works") {
            for info in self.expressions(&work).expect("list expressions") {
                let expression = self
                    .get_expression(&info.id)
                    .expect("read the expression")
                    .expect("the expression should be there");
                payloads_of(&expression.root, &mut found);
            }
        }
        found
    }
}
