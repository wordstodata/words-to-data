//! The US Code corpus must read exactly as it did before nodes went class-neutral.
//!
//! #129 demoted USLM from *the* document model to *a* document class. The risk in
//! that is not failing to store a court opinion — it is regressing a working
//! corpus of a million and a half nodes to gain a hypothetical one. So this file
//! asserts the three things that change would break if it were wrong: how many
//! nodes a work holds, what their paths are, and what the diff between two
//! release points says.
//!
//! Every figure here was measured on the build before the change, against these
//! same two committed release points. A path digest rather than a list of paths:
//! title 26 holds 57,391 of them, and a digest fails on any single one moving
//! while staying short enough to read.

use rstest::rstest;
use words_to_data::diff::TreeDiff;
use words_to_data::document::DocumentNode;
use words_to_data::uslm::parser::parse;
use words_to_data::uslm::{BillType, DocumentType, ElementType, USCType};

const EARLY: &str = "2025-07-18";
const LATER: &str = "2025-07-30";

fn release_point(file: &str, date: &str) -> DocumentNode {
    let path = format!("tests/test_data/usc/{date}/{file}");
    parse(&path, date).unwrap_or_else(|e| panic!("{file} at {date} should parse: {e}"))
}

/// Every node of a tree, root included.
fn node_count(node: &DocumentNode) -> usize {
    1 + node.children.iter().map(node_count).sum::<usize>()
}

fn collect_paths(node: &DocumentNode, into: &mut Vec<String>) {
    into.push(node.data.path.to_string());
    for child in &node.children {
        collect_paths(child, into);
    }
}

/// A digest of every path in a tree, in document order.
///
/// Document order is part of what is being held still: the same paths in a
/// different order is a different dataset, because `element_index` is keyed on
/// position and search results come back in reading order.
fn path_digest(node: &DocumentNode) -> String {
    use sha2::{Digest, Sha256};

    let mut paths = Vec::new();
    collect_paths(node, &mut paths);

    let mut hasher = Sha256::new();
    for path in &paths {
        hasher.update(path.as_bytes());
        hasher.update([0u8]);
    }
    hex::encode(hasher.finalize())
}

/// The node counts the dataset holds at this release point, per work.
///
/// The `uscode` container the parser wraps a file in is not one of the work's own
/// nodes, so it is subtracted. Title 26 is the control: it holds no numberless
/// container and no `<article>`, so no recent change may touch it. The four
/// appendices carry most of the awkward cases.
#[rstest]
#[case("usc26.xml", 57_391)]
#[case("usc28a.xml", 3_093)]
#[case("usc11a.xml", 2_364)]
#[case("usc18a.xml", 1_255)]
#[case("usc05A.xml", 118)]
fn should_hold_the_same_node_count_as_before_nodes_became_class_neutral(
    #[case] file: &str,
    #[case] expected: usize,
) {
    let root = release_point(file, EARLY);

    let count = node_count(&root) - 1;
    assert_eq!(
        count, expected,
        "{file} at {EARLY} held {expected} nodes before #129"
    );
}

/// The paths themselves, not just how many there are.
///
/// A node's type is now an open string, and the path segment for a node is the
/// local half of that type. Deriving both from one list is what stops a path and a
/// type from disagreeing — and this is the test that says the list did not move.
///
/// Each digest was taken on the build before #129, over the same file at the same
/// release point, so a single path moving by one character fails here.
#[rstest]
#[case(
    "usc26.xml",
    "978468c3bfb4dfe7714266e20fe3fba56147820cc16b8a04be134413bfab86ea"
)]
#[case(
    "usc28a.xml",
    "68df6802c566ed1cfd7d9024f743e5bd8ad42d49a1026ef10cac2ca946cdbd95"
)]
#[case(
    "usc11a.xml",
    "e601d24af814f16ffc61326bba4ee5c2e1d0b420a6d27c674eb4e9eaf34be983"
)]
#[case(
    "usc18a.xml",
    "0447bea3b109aed654d5bf724db69f9cc83c771d78751cd08afac8a9973b33c9"
)]
#[case(
    "usc05A.xml",
    "52d307e1faf6891b9938ce95a06a856f16c1ab323b46c6ebcb6b5694dc6f0512"
)]
fn should_generate_the_same_paths_as_before_nodes_became_class_neutral(
    #[case] file: &str,
    #[case] expected: &str,
) {
    let root = release_point(file, EARLY);

    assert_eq!(
        path_digest(&root),
        expected,
        "{file} at {EARLY} should hold exactly the paths it held before #129"
    );
}

/// Every element type this parser knows, so the two lists below cover all of them.
fn every_element_type() -> Vec<ElementType> {
    use ElementType::*;
    vec![
        USCodeDocument,
        PublicLawDocument,
        Title,
        Appendix,
        Subtitle,
        Chapter,
        Subchapter,
        Part,
        Subpart,
        Section,
        Subsection,
        Paragraph,
        Subparagraph,
        Clause,
        Subclause,
        Level,
        Item,
        Subitem,
        Subsubitem,
        Division,
        Subdivision,
        Unknown,
    ]
}

/// The word a path segment is made of, checked against the expression the parser
/// used before #129.
///
/// `generate_structural_path` used to write `format!("{:?}", element_type)`
/// lowercased. It now writes [`ElementType::path_segment_name`]. Those two must
/// agree for every variant, or a path moves — which renames every provision
/// beneath it. The digests above say the paths did not move; this says *why*, and
/// it fails the moment a variant is renamed in Rust without the stored name being
/// held still.
#[test]
fn should_name_a_path_segment_exactly_as_the_path_generator_used_to() {
    for element_type in every_element_type() {
        assert_eq!(
            element_type.path_segment_name(),
            format!("{element_type:?}").to_lowercase(),
            "the path segment of {element_type:?} must match the word the old \
             path generator wrote"
        );
    }
}

/// A node type may differ from a path segment, and exactly twice it does.
///
/// The type is read by a person and by another party's reader, and the two
/// document roots are the only elements a path never shows — the US Code's root is
/// hardcoded as `uscode`, and a bill's root is the only node that could say
/// whether the bill is enacted. So they are named for a reader:
/// `uscode.document` and `bill.public_law`.
///
/// Everywhere else the two words must be the same, or a reader would meet
/// `uscode.section` at a path segment called something else.
#[test]
fn should_name_a_node_type_as_its_path_segment_except_for_the_two_document_roots() {
    let renamed = [ElementType::USCodeDocument, ElementType::PublicLawDocument];

    for element_type in every_element_type() {
        if renamed.contains(&element_type) {
            assert_ne!(
                element_type.type_name(),
                element_type.path_segment_name(),
                "{element_type:?} is a document root and is named for a reader"
            );
            continue;
        }
        assert_eq!(
            element_type.type_name(),
            element_type.path_segment_name(),
            "{element_type:?} is not a document root, so its type and its path \
             segment must be one word"
        );
    }

    assert_eq!(ElementType::USCodeDocument.type_name(), "document");
    assert_eq!(ElementType::PublicLawDocument.type_name(), "public_law");
}

/// No two element types may share a node type, or two different things would be
/// stored as one and the diff would pair them.
#[test]
fn should_give_every_element_type_a_node_type_of_its_own() {
    let mut seen = std::collections::BTreeSet::new();
    for element_type in every_element_type() {
        assert!(
            seen.insert(element_type.type_name()),
            "{element_type:?} shares the node type `{}` with another element",
            element_type.type_name()
        );
    }
}

/// The published vocabulary, written out in full.
///
/// These strings go into every W2D file and are read by parties who do not have
/// our code, so they are an interface. This is the list, in one place, and it
/// fails on any change to it rather than leaving the change to be noticed in a
/// dataset.
///
/// Two of the forty-four cannot occur, and are listed because the two namespaces
/// share one vocabulary and this is the cross product of it: `uscode.public_law`
/// (a US Code file has no public law root) and `bill.document` (a bill's root is
/// its enacted state, never a bare document).
#[test]
fn should_store_the_node_types_this_vocabulary_publishes() {
    let usc = DocumentType::USCode {
        usc_type: USCType::Title,
    };
    let bill = DocumentType::Bill {
        bill_type: BillType::PublicLaw,
        bill_id: "119-21".to_string(),
    };

    let published: Vec<String> = every_element_type()
        .into_iter()
        .map(|element_type| element_type.node_type(&usc).as_str().to_string())
        .chain(
            every_element_type()
                .into_iter()
                .map(|element_type| element_type.node_type(&bill).as_str().to_string()),
        )
        .collect();

    assert_eq!(
        published,
        vec![
            "uscode.document",
            "uscode.public_law",
            "uscode.title",
            "uscode.appendix",
            "uscode.subtitle",
            "uscode.chapter",
            "uscode.subchapter",
            "uscode.part",
            "uscode.subpart",
            "uscode.section",
            "uscode.subsection",
            "uscode.paragraph",
            "uscode.subparagraph",
            "uscode.clause",
            "uscode.subclause",
            "uscode.level",
            "uscode.item",
            "uscode.subitem",
            "uscode.subsubitem",
            "uscode.division",
            "uscode.subdivision",
            "uscode.unknown",
            "bill.document",
            "bill.public_law",
            "bill.title",
            "bill.appendix",
            "bill.subtitle",
            "bill.chapter",
            "bill.subchapter",
            "bill.part",
            "bill.subpart",
            "bill.section",
            "bill.subsection",
            "bill.paragraph",
            "bill.subparagraph",
            "bill.clause",
            "bill.subclause",
            "bill.level",
            "bill.item",
            "bill.subitem",
            "bill.subsubitem",
            "bill.division",
            "bill.subdivision",
            "bill.unknown",
        ]
    );
}

/// The one path a node-type rename could have moved.
///
/// A US Code file's root path is the constant `uscode`, so `uscode.document`
/// renaming the root type costs nothing. A bill's root path is *generated*, from
/// the same element type, so it is the one place where naming the type
/// `bill.public_law` would have moved a stored path — and a root path is a
/// `WorkId`, which links point at.
///
/// It did not move, because the type name and the path segment are two lists
/// (`ElementType::type_name` and `ElementType::path_segment_name`). The path
/// segment stayed frozen at the ugly word on purpose.
#[test]
fn should_leave_the_root_path_of_a_bill_where_it_was() {
    let bill = parse(
        "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml",
        "2025-07-04",
    )
    .expect("the public law should parse");

    assert_eq!(
        bill.data.path.as_ref(),
        "publiclawdocument_119-21",
        "a bill's root path is its WorkId and must not move"
    );
    assert_eq!(
        bill.data.node_type.as_str(),
        "bill.public_law",
        "its type is named for a reader, and does not follow the path"
    );
}

/// The four appendices and the three ordinary titles that carry numberless
/// containers. A uuid in a path would make a provision untypable, and it was one
/// of these files that carried them (#115).
#[rstest]
#[case("usc28a.xml")]
#[case("usc11a.xml")]
#[case("usc18a.xml")]
#[case("usc05A.xml")]
#[case("usc12.xml")]
#[case("usc29.xml")]
#[case("usc38.xml")]
fn should_hold_no_uuid_path_when_a_node_type_is_an_open_string(#[case] file: &str) {
    let root = release_point(file, EARLY);
    let mut paths = Vec::new();
    collect_paths(&root, &mut paths);

    let uuid_paths: Vec<&String> = paths
        .iter()
        .filter(|path| path.contains("level_id"))
        .take(3)
        .collect();

    assert!(
        uuid_paths.is_empty(),
        "{file} should hold no uuid path, got {uuid_paths:?}"
    );
}

/// The diff between the two committed release points.
///
/// The diff pairs two nodes on their path *and their type*, and the type is now an
/// open string rather than an enum. If the two release points spelled a type
/// differently, every changed provision under it would read as one node removed
/// and another added rather than as a change — so the shape of the diff is the
/// assertion that matters, not only its size.
#[test]
fn should_compute_the_same_diff_as_before_nodes_became_class_neutral() {
    let early = release_point("usc26.xml", EARLY);
    let later = release_point("usc26.xml", LATER);

    let diff = TreeDiff::from_nodes(&early, &later);

    let mut changed = Vec::new();
    collect_changed_paths(&diff, &mut changed);

    // Measured on the build before #129, over these same two release points.
    assert_eq!(
        changed.len(),
        537,
        "title 26 changed at 537 paths between {EARLY} and {LATER}"
    );
    assert_eq!(count_added(&diff), 360, "360 nodes were added");
    assert_eq!(count_removed(&diff), 161, "161 nodes were removed");

    // A specific one, so the count cannot be right by accident.
    assert!(
        changed.iter().any(|path| path
            == "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"),
        "section 174(a) changed between these dates, got {changed:#?}"
    );
}

fn collect_changed_paths(diff: &TreeDiff, into: &mut Vec<String>) {
    if !diff.changes.is_empty() {
        into.push(diff.root_path.clone());
    }
    for child in &diff.child_diffs {
        collect_changed_paths(child, into);
    }
}

fn count_added(diff: &TreeDiff) -> usize {
    diff.added.len() + diff.child_diffs.iter().map(count_added).sum::<usize>()
}

fn count_removed(diff: &TreeDiff) -> usize {
    diff.removed.len() + diff.child_diffs.iter().map(count_removed).sum::<usize>()
}
