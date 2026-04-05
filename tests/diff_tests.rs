use rstest::rstest;
use words_to_data::{
    diff::TreeDiff,
    uslm::{BillDiff, TextContentField, bill_parser::parse_bill_amendments, parser::parse},
};

#[test]
fn test_diff_generation_26() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");

    let diff = TreeDiff::from_elements(&doc_old, &doc_new);

    let s174a_diff = diff.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a").expect("Section 174A has no changes, nor does its children!");
    let change = s174a_diff
        .changes
        .first()
        .expect("Change should be detected on Section 174(A)");
    assert_eq!(change.field_name, TextContentField::Chapeau);
    assert_eq!(change.changes.len(), 2);
    assert_eq!(
        change.old_value,
        "In the case of a taxpayer’s specified research or experimental expenditures for any taxable year—"
    );
    assert_eq!(
        change.new_value,
        "In the case of a taxpayer’s foreign research or experimental expenditures for any taxable year—"
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
    let diff = TreeDiff::from_elements(&tree1, &tree2);

    // Verify diff was generated
    assert!(!diff.root_path.is_empty(), "Diff should have a root path");

    // The diff may or may not have changes depending on the title
    // Just verify the diff structure is valid
    assert_eq!(diff.root_path, tree1.data.path);
}

#[test]
fn test_similarities() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");
    let diff = TreeDiff::from_elements(&doc_old, &doc_new);

    let mut amendment_data = parse_bill_amendments(
        "/home/jesse/code/rust/words_to_data/tests/test_data/bills/hr-119-21.xml",
    )
    .unwrap();

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
        .get_mut("3a2c877b0d3cd21b5f3942bdd611408504f60625b41a623aa79ad02274a2cfaf")
        .unwrap()
        .changes = bill_diffs;

    let similarity = diff.calculate_amendment_similarities(&amendment_data);

    // Section 174(a) has "specified" -> "foreign" change
    // Both words are in the amendment, so precision should be 1.0
    let s174a_sim = similarity.get("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a").unwrap();
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
    let s174a2b_sim = similarity.get("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a/paragraph_2/subparagraph_B").unwrap();
    assert_eq!(s174a2b_sim.matched_words, 17);
    assert!(
        s174a2b_sim.score > 0.0,
        "Score should be positive for a match"
    );
}
