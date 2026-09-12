use rstest::rstest;
use words_to_data::{
    diff::{MentionMatch, TreeDiff},
    document::{DocumentNode, TextContentField},
    legislature::BillDiff,
    uslm::{bill_parser::parse_bill_amendments, parser::parse},
};

const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

#[test]
fn test_diff_generation_26() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");

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

    let tree1 = parse(&path1, "2025-07-18")
        .unwrap_or_else(|_| panic!("Failed to parse {} from 2025-07-18", title));

    let tree2 = parse(&path2, "2025-07-30")
        .unwrap_or_else(|_| panic!("Failed to parse {} from 2025-07-30", title));

    // Generate diff
    let diff = TreeDiff::from_nodes(&tree1, &tree2);

    // Verify diff was generated
    assert!(!diff.root_path.is_empty(), "Diff should have a root path");

    // The diff may or may not have changes depending on the title
    // Just verify the diff structure is valid
    assert_eq!(diff.root_path, tree1.data.path.as_ref());
}

#[test]
fn test_similarities() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");
    let diff = TreeDiff::from_nodes(&doc_old, &doc_new);

    let mut amendment_data = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();

    // This part is handled by LLM's, and I don't want to add that logic to
    // this library yet (or at all). It will probably be added as another tool
    // which imports the words-to-data crate. Therefore we stub out this blob of data here
    let bill_diffs = vec![
        BillDiff {
            removed: vec!["specified".to_string()],
            added: vec!["foreign".to_string()],
        },
        BillDiff {
            removed: vec![
                "5-year".to_string(),
                "period".to_string(),
                "(15-year".to_string(),
                "period".to_string(),
                "case".to_string(),
                "any".to_string(),
                "specified".to_string(),
                "research".to_string(),
                "experimental".to_string(),
                "expenditures".to_string(),
                "which".to_string(),
                "attributable".to_string(),
                "foreign".to_string(),
                "research".to_string(),
                "(within".to_string(),
                "meaning".to_string(),
                "section".to_string(),
                "41(d)(4)(F)))".to_string(),
            ],
            added: vec!["15-year".to_string()],
        },
        BillDiff {
            removed: vec!["specified".to_string()],
            added: vec!["foreign".to_string()],
        },
        BillDiff {
            removed: vec![],
            added: vec![
                "which".to_string(),
                "attributable".to_string(),
                "foreign".to_string(),
                "research".to_string(),
                "(within".to_string(),
                "meaning".to_string(),
                "section".to_string(),
                "41(d)(4)(F))".to_string(),
            ],
        },
        BillDiff {
            removed: vec!["Specified".to_string()],
            added: vec!["Foreign".to_string()],
        },
        BillDiff {
            removed: vec!["specified".to_string()],
            added: vec!["foreign".to_string()],
        },
        BillDiff {
            removed: vec![],
            added: vec![
                "reduction".to_string(),
                "amount".to_string(),
                "realized".to_string(),
            ],
        },
    ];

    amendment_data
        .amendments
        .values_mut()
        .find(|amendment| {
            amendment
                .amending_text
                .contains("a taxpayer's foreign research or experimental expenditures")
        })
        .unwrap()
        .changes = bill_diffs;

    let similarity = diff.calculate_amendment_similarities(&amendment_data);

    // Section 174(a) has "specified" -> "foreign" change
    // Both words are in the amendment, so precision should be 1.0
    let s174a_scores = similarity
        .get("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
        .unwrap();
    // Only one amendment carries changes in this fixture, so only one scores.
    assert_eq!(s174a_scores.len(), 1);
    let s174a_sim = &s174a_scores[0];
    assert_eq!(s174a_sim.matched_words, 2);
    assert_eq!(s174a_sim.tree_diff_words, 2);
    assert_eq!(s174a_sim.precision, 1.0);
    assert!(
        s174a_sim.score > 0.0,
        "Score should be positive for a match"
    );

    // Perfect BillDiff match: F1 score should be 1.0
    assert_eq!(s174a_sim.score, 1.0);

    // Section 174(a)(2)(B) has more changes, check it matches well
    let s174a2b_scores = similarity.get("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a/paragraph_2/subparagraph_B").unwrap();
    assert_eq!(s174a2b_scores.len(), 1);
    let s174a2b_sim = &s174a2b_scores[0];
    assert_eq!(s174a2b_sim.matched_words, 17);
    assert!(
        s174a2b_sim.score > 0.0,
        "Score should be positive for a match"
    );
}

const S174A: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";

#[test]
fn should_return_every_amendment_scoring_above_zero_when_two_amendments_touch_one_path() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");
    let diff = TreeDiff::from_nodes(&doc_old, &doc_new);

    let mut amendment_data = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();

    // Word-level changes come from an LLM, which the library does not call, so
    // stub them the way `test_similarities` does. Two amendments are given the
    // same change, which is the real case this covers: one strikes and another
    // inserts at the same subsection, so both explain it.
    let shared_change = vec![BillDiff {
        removed: vec!["specified".to_string()],
        added: vec!["foreign".to_string()],
    }];
    let mut scoring_ids: Vec<String> = amendment_data.amendments.keys().take(2).cloned().collect();
    scoring_ids.sort();
    assert_eq!(scoring_ids.len(), 2, "The bill should hold two amendments");
    for id in &scoring_ids {
        amendment_data
            .amendments
            .get_mut(id)
            .expect("amendment id came from the map")
            .changes = shared_change.clone();
    }

    let similarities = diff.calculate_amendment_similarities(&amendment_data);
    let at_174a = similarities
        .get(S174A)
        .expect("Section 174(a) should score against these amendments");

    let scored_ids: Vec<&str> = at_174a.iter().map(|s| s.amendment_id.as_str()).collect();
    assert_eq!(
        scored_ids.len(),
        2,
        "Both amendments explain this path, so both should survive, got {scored_ids:?}"
    );
    for id in &scoring_ids {
        assert!(
            scored_ids.contains(&id.as_str()),
            "Amendment {id} scores above zero here and should not be discarded"
        );
    }
}

#[test]
fn should_order_similarities_by_score_then_amendment_id_when_several_score_at_one_path() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");
    let diff = TreeDiff::from_nodes(&doc_old, &doc_new);

    let mut amendment_data = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();

    // A weaker and a stronger explanation of the same change, plus a tie, so
    // both halves of the ordering rule are exercised.
    let strong = vec![BillDiff {
        removed: vec!["specified".to_string()],
        added: vec!["foreign".to_string()],
    }];
    let weak = vec![BillDiff {
        removed: vec!["specified".to_string()],
        added: vec!["unrelated".to_string(), "wording".to_string()],
    }];
    let mut ids: Vec<String> = amendment_data.amendments.keys().take(3).cloned().collect();
    ids.sort();
    assert_eq!(ids.len(), 3, "The bill should hold three amendments");
    amendment_data.amendments.get_mut(&ids[0]).unwrap().changes = strong.clone();
    amendment_data.amendments.get_mut(&ids[1]).unwrap().changes = strong;
    amendment_data.amendments.get_mut(&ids[2]).unwrap().changes = weak;

    let similarities = diff.calculate_amendment_similarities(&amendment_data);
    let at_174a = similarities
        .get(S174A)
        .expect("Section 174(a) should score against these amendments");

    let ordering: Vec<(f32, &str)> = at_174a
        .iter()
        .map(|s| (s.score, s.amendment_id.as_str()))
        .collect();
    let mut expected = ordering.clone();
    expected.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    assert_eq!(
        ordering, expected,
        "Similarities at one path should run from best score down, ties broken by amendment id"
    );
}

#[test]
fn test_correct_matching_regex() {
    let result_a = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("unable to parse doc");
    let result_b = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("unable to parse doc");

    let diff: TreeDiff = TreeDiff::from_nodes(&result_a, &result_b);

    let s174a = diff.find("uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_45F/subsection_c/paragraph_1/subparagraph_A/clause_iii").unwrap();
    let mention_regex = s174a.mention_regex().unwrap();
    let section_regex = s174a.section_regex().unwrap();
    dbg!(&mention_regex);
    dbg!(&section_regex);
    let target = "some preceding text Section 45F(c)(1)(A)(iii) is amended by inserting";
    let mat = mention_regex.find(target).expect("Unable to find mention");
    assert_eq!(mat.as_str(), "Section 45F(c)(1)(A)(iii) ");
    let mat = section_regex.find(target).expect("Unable to find section");
    assert_eq!(mat.as_str(), "Section 45F(");
}

#[test]
fn test_get_all_regexes() {
    let result_a = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("unable to parse doc");
    let result_b = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("unable to parse doc");
    let diff: TreeDiff = TreeDiff::from_nodes(&result_a, &result_b);
    let s174a = diff.find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a/paragraph_2").unwrap();

    let regs = s174a.all_regexes();
    assert_eq!(regs.len(), 2);
}

#[test]
fn test_shallow_should_return_tree_diff_without_children() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");
    let diff = TreeDiff::from_nodes(&doc_old, &doc_new);

    // Find a node that has children
    let s174 = diff
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174")
        .expect("Section 174 should exist");
    assert!(
        !s174.child_diffs.is_empty(),
        "Section 174 should have child diffs"
    );

    // Get a shallow copy
    let shallow = s174.shallow();

    // Shallow copy should have same data but no children
    assert_eq!(shallow.root_path, s174.root_path);
    assert_eq!(shallow.changes.len(), s174.changes.len());
    assert_eq!(shallow.added.len(), s174.added.len());
    assert_eq!(shallow.removed.len(), s174.removed.len());
    assert!(
        shallow.child_diffs.is_empty(),
        "Shallow copy should have no children"
    );
}

#[test]
fn test_scan_for_mentions_should_find_section_45f_mentions_in_bill() {
    // Parse USC documents and create TreeDiff
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");
    let diff = TreeDiff::from_nodes(&doc_old, &doc_new);

    // Parse the bill that amends Section 45F
    let amendment_data = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();

    // Scan for mentions - this should find "Section 45F" mentions in the amendment texts
    let mentions = diff.scan_for_mentions(&amendment_data);

    // We expect at least one amendment to mention Section 45F
    assert!(
        !mentions.is_empty(),
        "Should find at least one amendment mentioning Section 45F"
    );

    // Check that the matched text includes "Section 45F"
    let all_matches: Vec<&MentionMatch> = mentions.values().flatten().collect();
    let has_section_45f = all_matches.iter().any(|m| m.matched_text.contains("45F"));
    assert!(
        has_section_45f,
        "Should find a match containing '45F', got matches: {:?}",
        all_matches
    );
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
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");

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
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");

    let diff = TreeDiff::from_nodes(&doc_old, &doc_new);

    assert_document_order(&diff, &doc_old, &doc_new);
}
