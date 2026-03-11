use words_to_data::uslm::parser::parse;

#[test]
fn should_parse_single_source_credit_with_one_ref() {
    let element = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
        .expect("Failed to parse USC 7");

    // Find section 1 which has a simple source credit
    let section_1 = element
        .find("uscodedocument_7/title_7/chapter_1/section_1")
        .expect("Section 1 not found");

    // Section 1 should have at least one source credit
    assert!(
        !section_1.data.source_credits.is_empty(),
        "Section 1 should have source credits"
    );

    // First source credit should have at least one ref pair
    let first_credit = &section_1.data.source_credits[0];
    assert!(
        !first_credit.ref_pairs.is_empty(),
        "First source credit should have ref pairs"
    );

    // Verify the ref_pair has both ref_id and description
    let first_ref = &first_credit.ref_pairs[0];
    assert!(!first_ref.ref_id.is_empty(), "Ref ID should not be empty");
    assert!(
        !first_ref.description.is_empty(),
        "Description should not be empty"
    );
}

#[test]
fn should_parse_multiple_refs_without_semicolons() {
    let element = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
        .expect("Failed to parse USC 7");

    // Find a section with multiple refs in a single source credit (no semicolons)
    let section_1b = element
        .find("uscodedocument_7/title_7/chapter_1/section_1b")
        .expect("Section 1b not found");

    assert!(
        !section_1b.data.source_credits.is_empty(),
        "Section 1b should have source credits"
    );

    // This section should have at least one source credit with multiple refs
    let credit_with_multiple_refs = section_1b
        .data
        .source_credits
        .iter()
        .find(|sc| sc.ref_pairs.len() > 1);

    assert!(
        credit_with_multiple_refs.is_some(),
        "Should find at least one source credit with multiple ref pairs"
    );
}

#[test]
fn should_split_source_credits_by_semicolons() {
    let element = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
        .expect("Failed to parse USC 7");

    // Section 1a has amendments separated by semicolons
    let section_1a = element
        .find("uscodedocument_7/title_7/chapter_1/section_1a")
        .expect("Section 1a not found");

    // Section 1a should have multiple source credits due to semicolon splitting
    assert!(
        section_1a.data.source_credits.len() > 1,
        "Section 1a should have multiple source credits from semicolon splitting"
    );

    // Each source credit should have at least one ref_pair
    for (idx, credit) in section_1a.data.source_credits.iter().enumerate() {
        assert!(
            !credit.ref_pairs.is_empty(),
            "Source credit {} should have at least one ref_pair",
            idx
        );
    }
}

#[test]
fn should_handle_element_with_no_source_credits() {
    let element = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
        .expect("Failed to parse USC 7");

    // The root element should not have source credits
    assert!(
        element.data.source_credits.is_empty(),
        "Root element should not have source credits"
    );
}

#[test]
fn should_extract_href_as_ref_id() {
    let element = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18")
        .expect("Failed to parse USC 7");

    let section_1 = element
        .find("uscodedocument_7/title_7/chapter_1/section_1")
        .expect("Section 1 not found");

    assert!(!section_1.data.source_credits.is_empty());

    let first_credit = &section_1.data.source_credits[0];
    assert!(!first_credit.ref_pairs.is_empty());

    // The ref_id should start with /us/ (USLM path format)
    let first_ref = &first_credit.ref_pairs[0];
    assert!(
        first_ref.ref_id.starts_with("/us/"),
        "Ref ID should be a USLM path starting with /us/, got: {}",
        first_ref.ref_id
    );
}
