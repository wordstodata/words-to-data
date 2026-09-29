mod common;

use rstest::rstest;
use words_to_data::{
    diff::{Redesignations, TreeDiff},
    document::{DocumentNode, TextContentField},
    legislature::redesignation::resolve,
    uslm::bill_redesignation::redesignations_stated_in_file,
};

const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

/// The website shows this diff of § 174(a) in its "Compute a Diff Between
/// Versions" example. If this fails, update that section in index.html.
#[test]
fn test_diff_generation_26() {
    let doc_old = common::parsed("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18");
    let doc_new = common::parsed("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30");

    let diff = TreeDiff::from_nodes(&doc_old, &doc_new);

    let s174a_diff = diff
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
        .expect("Section 174A has no changes, nor does its children!");
    let change = s174a_diff
        .changes
        .first()
        .expect("Change should be detected on Section 174(A)");
    assert_eq!(change.field_name, TextContentField::Chapeau);
    assert_eq!(
        change.old_value,
        "In the case of a taxpayer's specified research or experimental expenditures for any taxable year\u{2014}"
    );
    assert_eq!(
        change.new_value,
        "In the case of a taxpayer's foreign research or experimental expenditures for any taxable year\u{2014}"
    );
}

// Generate diffs across title pairs
#[rstest]
#[case("01")]
#[case("09")]
#[case("26")]
fn test_diff_generation_across_titles(#[case] title: &str) {
    let path1 = format!("tests/test_data/usc/2025-07-18/usc{}.xml", title);
    let path2 = format!("tests/test_data/usc/2025-07-30/usc{}.xml", title);

    let tree1 = common::parsed(&path1, "2025-07-18");

    let tree2 = common::parsed(&path2, "2025-07-30");

    // Generate diff
    let diff = TreeDiff::from_nodes(&tree1, &tree2);

    // Verify diff was generated
    assert!(!diff.root_path.is_empty(), "Diff should have a root path");

    // The diff may or may not have changes depending on the title
    // Just verify the diff structure is valid
    assert_eq!(diff.root_path, tree1.data.path.as_ref());
}

/// Flatten a diff tree into the exact order its nodes and children are stored,
/// so that two diffs can be compared on ordering and not merely on membership.
fn ordered_paths(diff: &TreeDiff) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(diff: &TreeDiff, out: &mut Vec<String>) {
        out.push(format!("~{}", diff.root_path));
        for element in &diff.added {
            out.push(format!("+{}", element.path));
        }
        for element in &diff.removed {
            out.push(format!("-{}", element.path));
        }
        for child in &diff.child_diffs {
            walk(child, out);
        }
    }
    walk(diff, &mut out);
    out
}

#[test]
fn should_order_diff_children_identically_when_the_same_diff_is_built_twice() {
    let doc_old = common::parsed("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18");
    let doc_new = common::parsed("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30");

    let first = ordered_paths(&TreeDiff::from_nodes(&doc_old, &doc_new));
    let second = ordered_paths(&TreeDiff::from_nodes(&doc_old, &doc_new));

    // Membership has never been the problem: the same paths come back every
    // time. Only their order moves, so compare the sequences, not the sets.
    let mut first_sorted = first.clone();
    let mut second_sorted = second.clone();
    first_sorted.sort();
    second_sorted.sort();
    assert_eq!(
        first_sorted, second_sorted,
        "The two diffs should contain the same paths"
    );

    let first_divergence = first
        .iter()
        .zip(second.iter())
        .position(|(a, b)| a != b)
        .map(|i| format!("index {i}: {:?} vs {:?}", first[i], second[i]))
        .unwrap_or_else(|| "none".to_string());
    assert_eq!(
        first, second,
        "Two diffs of the same input should be ordered identically. \
         First divergence at {first_divergence}"
    );
}

/// Assert that every list on this node follows the order of the source
/// document, then recurse. `removed` and `child_diffs` follow the older
/// expression; `added` follows the newer one.
fn assert_document_order(diff: &TreeDiff, from: &DocumentNode, to: &DocumentNode) {
    // A path can name more than one child, so a path maps to every position it
    // occupies. A list is in document order when its entries can be laid onto
    // those positions strictly increasing, taking the earliest that still fits.
    fn positions(element: &DocumentNode) -> std::collections::HashMap<&str, Vec<usize>> {
        let mut by_path: std::collections::HashMap<&str, Vec<usize>> =
            std::collections::HashMap::new();
        for (at, child) in element.children.iter().enumerate() {
            by_path.entry(&child.data.path).or_default().push(at);
        }
        by_path
    }

    fn walk_in_order(
        paths: impl Iterator<Item = String>,
        index: &std::collections::HashMap<&str, Vec<usize>>,
    ) -> Option<Vec<usize>> {
        let mut cursor: Option<usize> = None;
        let mut chosen = Vec::new();
        for path in paths {
            let next = index
                .get(path.as_str())?
                .iter()
                .copied()
                .find(|at| cursor.is_none_or(|last| *at > last))?;
            cursor = Some(next);
            chosen.push(next);
        }
        Some(chosen)
    }

    let from_positions = positions(from);
    let to_positions = positions(to);

    let matched = walk_in_order(
        diff.child_diffs.iter().map(|c| c.root_path.clone()),
        &from_positions,
    );
    let matched = matched.unwrap_or_else(|| {
        panic!(
            "child_diffs of {} are not in document order: {:?}",
            diff.root_path,
            diff.child_diffs
                .iter()
                .map(|c| c.root_path.as_str())
                .collect::<Vec<_>>()
        )
    });

    assert!(
        walk_in_order(
            diff.removed.iter().map(|e| e.path.to_string()),
            &from_positions
        )
        .is_some(),
        "removed of {} is not in document order",
        diff.root_path
    );
    assert!(
        walk_in_order(diff.added.iter().map(|e| e.path.to_string()), &to_positions).is_some(),
        "added of {} is not in document order",
        diff.root_path
    );

    // Recurse against the children each matched diff actually came from.
    let mut to_cursor: Option<usize> = None;
    for (child, at) in diff.child_diffs.iter().zip(matched) {
        let from_child = &from.children[at];
        let to_at = to_positions
            .get(child.root_path.as_str())
            .and_then(|places| {
                places
                    .iter()
                    .copied()
                    .find(|place| to_cursor.is_none_or(|last| *place > last))
            })
            .expect("a matched child should exist in the newer expression");
        to_cursor = Some(to_at);
        assert_document_order(child, from_child, &to.children[to_at]);
    }
}

#[test]
fn should_order_diff_children_by_document_position_when_diffing_a_title() {
    let doc_old = common::parsed("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18");
    let doc_new = common::parsed("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30");

    let diff = TreeDiff::from_nodes(&doc_old, &doc_new);

    assert_document_order(&diff, &doc_old, &doc_new);
}

/// § 9032 of title 7, "Seed cotton", which `119-hr-1` renumbered and rewrote at
/// once: "by redesignating subsections (c) and (d) as subsections (d) and (e),
/// respectively", and the loan rate inside the renumbered subsection went from
/// $0.25 to $0.30 per pound. The subsection's own heading did not change, so
/// nothing but a walk into the pair can find the new rate.
const SECTION_9032: &str = "uscode/title_7/chapter_115/subchapter_II/section_9032";

/// The redesignations `119-hr-1` states, resolved against a title's two
/// expressions.
///
/// Both are needed: a redesignation moves a provision away from one path and
/// onto another, and `resolve` checks each end against the document that should
/// hold it (#151).
fn redesignations_stated_by_the_public_law(
    before: &DocumentNode,
    after: &DocumentNode,
) -> Redesignations {
    let stated = redesignations_stated_in_file("119-hr-1", PL_XML_PATH).expect("the bill parses");
    Redesignations::from_pairs(
        resolve(&stated, before, after)
            .resolved
            .iter()
            .map(|r| (r.from_path.clone(), r.to_path.clone())),
    )
}

#[test]
fn should_report_a_rewrite_inside_a_renumbered_subsection_at_its_new_path_when_a_bill_renumbered_it()
 {
    let before = common::parsed("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18");
    let after = common::parsed("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30");
    let known = redesignations_stated_by_the_public_law(&before, &after);

    let diff = TreeDiff::from_nodes_with(&before, &after, &known);
    let at = diff.find(SECTION_9032).expect("§ 9032 changed");

    // The move is reported once, for the pair the bill named.
    let moved: Vec<(String, String)> = at
        .moved
        .iter()
        .map(|m| (m.from.path.to_string(), m.to.path.to_string()))
        .collect();
    assert_eq!(
        moved
            .iter()
            .filter(|(from, _)| *from == format!("{SECTION_9032}/subsection_d"))
            .count(),
        1,
        "subsection (d) moved once, got {moved:?}"
    );
    assert!(
        moved.contains(&(
            format!("{SECTION_9032}/subsection_d"),
            format!("{SECTION_9032}/subsection_e"),
        )),
        "subsection (d) became subsection (e), got {moved:?}"
    );

    // And the rewrite below it is reported, at the path the paragraph now holds.
    let rewritten = diff
        .find(&format!("{SECTION_9032}/subsection_e/paragraph_1"))
        .expect("the rewrite inside the renumbered subsection should be reported");
    let change = rewritten
        .changes
        .first()
        .expect("the paragraph's words should read as changed");
    assert!(
        change.old_value.contains("$0.25 per pound"),
        "the old rate should be the old value, got {:?}",
        change.old_value
    );
    assert!(
        change.new_value.contains("$0.30 per pound"),
        "the new rate should be the new value, got {:?}",
        change.new_value
    );
}
