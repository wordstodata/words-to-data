use words_to_data::uslm::{ElementType, parser::parse};

// ========== Error Handling Tests ==========

// Parse nonexistent file returns error
#[test]
fn test_parse_nonexistent_file() {
    let result = parse(
        "tests/test_data/usc/2025-07-18/nonexistent.xml",
        "2025-07-18",
    );
    assert!(result.is_err(), "Should return error for nonexistent file");
}

// Parse with invalid date format
#[test]
fn test_parse_invalid_date_format() {
    let result = parse("tests/test_data/usc/2025-07-18/usc01.xml", "invalid-date");
    assert!(
        result.is_err(),
        "Should return error for invalid date format"
    );
}

// Parse with malformed date (wrong format)
#[test]
fn test_parse_malformed_date() {
    let result = parse("tests/test_data/usc/2025-07-18/usc01.xml", "07-18-2025");
    assert!(
        result.is_err(),
        "Should return error for malformed date format"
    );
}

// Parse with incomplete date
#[test]
fn test_parse_incomplete_date() {
    let result = parse("tests/test_data/usc/2025-07-18/usc01.xml", "2025-07");
    assert!(result.is_err(), "Should return error for incomplete date");
}

// Parse with invalid month in date
#[test]
fn test_parse_invalid_month() {
    let result = parse("tests/test_data/usc/2025-07-18/usc01.xml", "2025-13-18");
    assert!(result.is_err(), "Should return error for invalid month");
}

// ========== Boundary Condition Tests ==========

// Parse smallest USC file (usc09.xml ~107K)
#[test]
fn test_parse_smallest_file() {
    let result = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18");
    assert!(result.is_ok(), "Should successfully parse smallest file");

    let root = result.unwrap();
    assert_eq!(root.data.uslm_id.as_ref().unwrap(), "/us/usc/t9");

    // Should have at least some content
    assert!(
        !root.children.is_empty(),
        "Smallest file should still have children"
    );
}

// Parse large file (usc07.xml ~28M)
#[test]
fn test_parse_large_file() {
    let result = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18");
    assert!(result.is_ok(), "Should successfully parse large file");

    let root = result.unwrap();
    assert_eq!(root.data.uslm_id.as_ref().unwrap(), "/us/usc/t7");

    // Count elements to verify complete parsing
    fn count_elements(elem: &words_to_data::uslm::USLMElement) -> usize {
        1 + elem.children.iter().map(count_elements).sum::<usize>()
    }

    let count = count_elements(&root);
    assert!(
        count > 1000,
        "Large file should have many elements, got {}",
        count
    );
}

// Element with no children
#[test]
fn test_empty_element_no_children() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Find a leaf paragraph element with no children
    let paragraph = root
        .find("uscodedocument_9/title_9/chapter_1/section_10/subsection_a/paragraph_1")
        .expect("Failed to find paragraph");

    assert_eq!(paragraph.data.element_type, ElementType::Paragraph);
    assert!(paragraph.children.is_empty());
}

// Element with no text fields
#[test]
fn test_element_with_no_text_fields() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Some elements might have no text content fields
    // At minimum, verify root document can be parsed even if it has no direct text
    assert_eq!(root.data.element_type, ElementType::USCodeDocument);
    assert!(
        root.data.heading.is_none()
            && root.data.chapeau.is_none()
            && root.data.content.is_none()
            && root.data.continuation.is_none()
    );
}

// ========== Special Structure Tests ==========

// Parse appendix title
#[test]
fn test_parse_appendix_title() {
    // Parse both a regular appendix file
    let result_5a = parse("tests/test_data/usc/2025-07-18/usc05A.xml", "2025-07-18");
    assert!(
        result_5a.is_ok(),
        "Should successfully parse usc05A.xml (Title 5 Appendix)"
    );

    let result_11a = parse("tests/test_data/usc/2025-07-18/usc11a.xml", "2025-07-18");
    assert!(
        result_11a.is_ok(),
        "Should successfully parse usc11a.xml (Title 11 Appendix)"
    );

    // Verify they parse correctly
    let root_5a = result_5a.unwrap();
    let root_11a = result_11a.unwrap();

    assert_eq!(root_5a.data.element_type, ElementType::USCodeDocument);
    assert_eq!(root_11a.data.element_type, ElementType::USCodeDocument);
}

// Navigate deeply nested structure (6+ levels)
#[test]
fn test_deeply_nested_structure() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Try to find a deeply nested element (paragraph or deeper)
    // uscodedocument_9/title_9/chapter_1/section_10/subsection_a/paragraph_1 = 6 levels
    let deep_elem =
        root.find("uscodedocument_9/title_9/chapter_1/section_10/subsection_a/paragraph_1");

    assert!(deep_elem.is_some(), "Should find deeply nested paragraph");

    let found = deep_elem.unwrap();
    assert_eq!(found.data.element_type, ElementType::Paragraph);

    // Verify the path contains 6 segments
    let segments: Vec<&str> = found.data.path.split('/').collect();
    assert_eq!(segments.len(), 6, "Should have 6 path segments");
}
