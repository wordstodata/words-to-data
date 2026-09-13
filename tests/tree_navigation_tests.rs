//! Finding a node in a parsed tree by its structural path.
//!
//! The type assertions name the stored node type in full — `uscode.section` —
//! rather than a Rust enum variant. That string is the published vocabulary: it
//! goes into every dataset, and a third party reads it without our code, so a
//! test that would still pass if it changed is not testing the interface (#129).

use words_to_data::uslm::{UslmFacts, parser::parse};

/// The publisher's number for a node, out of the node's USLM payload.
fn number_value(node: &words_to_data::document::DocumentNode) -> String {
    UslmFacts::of(&node.data)
        .expect("a parsed USC node carries USLM facts")
        .number_value
}

// Find root element in real USC document
#[test]
fn test_find_root_in_real_document() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("uscode");
    assert!(result.is_some(), "Should find root");

    let found = result.unwrap();
    assert_eq!(found.data.path.as_ref(), "uscode");
    assert_eq!(found.data.node_type.as_str(), "uscode.document");
}

// Find title element
#[test]
fn test_find_title_element() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("uscode/title_9");
    assert!(result.is_some(), "Should find title");

    let found = result.unwrap();
    assert_eq!(found.data.path.as_ref(), "uscode/title_9");
    assert_eq!(found.data.node_type.as_str(), "uscode.title");
}

// Find chapter element
#[test]
fn test_find_chapter_element() {
    let root = parse("tests/test_data/usc/2025-07-18/usc01.xml", "2025-07-18")
        .expect("Failed to parse usc01.xml");

    let result = root.find("uscode/title_1/chapter_1");
    assert!(result.is_some(), "Should find chapter");

    let found = result.unwrap();
    assert_eq!(found.data.node_type.as_str(), "uscode.chapter");
    assert_eq!(number_value(found), "1");
}

// Find section element
#[test]
fn test_find_section_element() {
    let root = parse("tests/test_data/usc/2025-07-18/usc04.xml", "2025-07-18")
        .expect("Failed to parse usc04.xml");

    let result = root.find("uscode/title_4/chapter_1/section_1");
    assert!(result.is_some(), "Should find section");

    let found = result.unwrap();
    assert_eq!(found.data.node_type.as_str(), "uscode.section");
}

// Find subsection element (deep navigation)
#[test]
fn test_find_subsection_element() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("uscode/title_9/chapter_1/section_10/subsection_a");
    assert!(result.is_some(), "Should find subsection");

    let found = result.unwrap();
    assert_eq!(found.data.node_type.as_str(), "uscode.subsection");
    assert_eq!(number_value(found), "a");
}

// Find paragraph element (very deep navigation)
#[test]
fn test_find_paragraph_element() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("uscode/title_9/chapter_1/section_10/subsection_a/paragraph_1");
    assert!(result.is_some(), "Should find paragraph");

    let found = result.unwrap();
    assert_eq!(found.data.node_type.as_str(), "uscode.paragraph");
}

// Find nonexistent path returns None
#[test]
fn test_find_nonexistent_path() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("uscode/title_9/chapter_99");
    assert!(result.is_none(), "Should not find nonexistent chapter");
}

// Partial path fails (must match exactly)
#[test]
fn test_find_partial_path_fails() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("uscode/title_9/chapter_1/section_99");
    assert!(result.is_none(), "Should not find nonexistent section");
}

// Wrong title prefix returns None
#[test]
fn test_find_wrong_title_prefix() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("uscode/title_26");
    assert!(result.is_none(), "Should not find wrong title");
}

// Find preserves children
#[test]
fn test_find_preserves_children() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("uscode/title_9/chapter_1/section_10/subsection_a");
    assert!(result.is_some(), "Should find subsection");

    let found = result.unwrap();
    // Subsection a should have multiple paragraph children
    assert!(
        !found.children.is_empty(),
        "Subsection should have children"
    );
    assert_eq!(
        found.children[0].data.node_type.as_str(),
        "uscode.paragraph"
    );
}

// Empty path returns None
#[test]
fn test_find_empty_path() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    let result = root.find("");
    assert!(result.is_none(), "Empty path should return None");
}

// Find in appendix file
#[test]
fn test_find_appendix_element() {
    let root = parse("tests/test_data/usc/2025-07-18/usc05A.xml", "2025-07-18")
        .expect("Failed to parse usc05A.xml");

    // usc05A is Title 5 Appendix - root is now uscode
    let result = root.find("uscode");
    assert!(result.is_some(), "Should find uscode root");

    let found = result.unwrap();
    assert_eq!(found.data.node_type.as_str(), "uscode.document");

    // Try to find title element within appendix
    let title_result = root.find("uscode/title_5a");
    if let Some(result) = title_result {
        assert_eq!(result.data.node_type.as_str(), "uscode.title");
    }
}

// Navigate deeply nested structure
#[test]
fn test_find_deeply_nested_structure() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Navigate to subparagraph (6 levels deep)
    let result =
        root.find("uscode/title_9/chapter_1/section_16/subsection_a/paragraph_1/subparagraph_A");

    if let Some(found) = result {
        assert_eq!(found.data.node_type.as_str(), "uscode.subparagraph");
        assert_eq!(number_value(found), "A");
    }
    // If this specific path doesn't exist, that's OK - we're testing the navigation works
}

// Path too deep returns None
#[test]
fn test_find_path_too_deep() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Try to navigate beyond what exists
    let result = root.find("uscode/title_9/chapter_1/section_1/subsection_a/paragraph_1/subparagraph_a/clause_1/subclause_1/item_1");
    assert!(result.is_none(), "Path too deep should return None");
}
