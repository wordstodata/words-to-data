use rstest::rstest;
use words_to_data::uslm::parser::{parse, parse_from_str};

#[test]
fn test_parse_usc_title_7() {
    let result = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18");
    assert!(
        result.is_ok(),
        "Failed to parse USC Title 7: {:?}",
        result.err()
    );

    let root = result.unwrap();
    // Root is now uscode container
    assert_eq!(root.data.path, "uscode");
    assert!(
        root.data.uslm_id.is_none(),
        "USCode container has no uslm_id"
    );

    // Check that children also have USLM format paths
    // The first child is a Title element
    assert!(!root.children.is_empty());
    let title = &root.children[0];
    assert_eq!(
        title.data.uslm_id.as_ref().unwrap(),
        "/us/usc/t7",
        "First child (Title) should have uslm_id /us/usc/t7"
    );
    assert_eq!(title.data.path, "uscode/title_7");
}

#[test]
fn test_parse_public_law() {
    let result = parse("tests/test_data/bills/pl-119-21.xml", "2025-07-04");
    assert!(
        result.is_ok(),
        "Failed to parse Public Law: {:?}",
        result.err()
    );

    let root = result.unwrap();
    // Check that the root path is in USLM format
    // Note: XML uses "119-21" format (with hyphen)
    let uslm_id = root.data.uslm_id.unwrap();
    assert_eq!(uslm_id, "/us/pl/119-21");

    // Check that children have structural format paths
    for child in &root.children {
        if let Some(uslm_id) = &child.data.uslm_id {
            assert!(uslm_id.starts_with("/us/pl/119-21/"));
        }
    }
}

// Full parse → serialize → deserialize → verify roundtrip
#[rstest]
#[case("01")]
#[case("04")]
#[case("09")]
#[case("26")]
fn test_cross_title_serialization(#[case] title: &str) {
    let path = format!("tests/test_data/usc/2025-07-18/usc{}.xml", title);
    let root =
        parse(&path, "2025-07-18").unwrap_or_else(|_| panic!("Failed to parse usc{}.xml", title));

    // Serialize to JSON
    let json = serde_json::to_string(&root).expect("Failed to serialize to JSON");

    // Deserialize back
    let deserialized: words_to_data::uslm::USLMElement =
        serde_json::from_str(&json).expect("Failed to deserialize from JSON");

    // Verify paths match
    assert_eq!(root.data.path, deserialized.data.path);
    assert_eq!(root.data.uslm_id, deserialized.data.uslm_id);
}

// Compare appendix vs regular titles
#[rstest]
#[case("05", "05A")] // Title 5 vs Title 5 Appendix
fn test_appendix_vs_regular_titles(#[case] regular: &str, #[case] appendix: &str) {
    use words_to_data::uslm::DocumentType;

    let regular_path = format!("tests/test_data/usc/2025-07-18/usc{}.xml", regular);
    let appendix_path = format!("tests/test_data/usc/2025-07-18/usc{}.xml", appendix);

    let regular_root = parse(&regular_path, "2025-07-18")
        .unwrap_or_else(|_| panic!("Failed to parse usc{}.xml", regular));

    let appendix_root = parse(&appendix_path, "2025-07-18")
        .unwrap_or_else(|_| panic!("Failed to parse usc{}.xml", appendix));

    // Both roots are uscode containers
    assert_eq!(regular_root.data.path, "uscode");
    assert_eq!(appendix_root.data.path, "uscode");

    // Both should be USC documents
    assert!(matches!(
        regular_root.data.document_type,
        DocumentType::USCode { .. }
    ));
    assert!(matches!(
        appendix_root.data.document_type,
        DocumentType::USCode { .. }
    ));

    // The first child (title) paths should be different
    let regular_title = &regular_root.children[0];
    let appendix_title = &appendix_root.children[0];
    assert_ne!(regular_title.data.path, appendix_title.data.path);
}

#[test]
fn test_parse_from_str_should_produce_same_result_as_parse() {
    // Load XML manually
    let xml_str = std::fs::read_to_string("tests/test_data/usc/2025-07-18/usc07.xml")
        .expect("Failed to read test file");

    // Parse from string
    let from_str_result = parse_from_str(&xml_str, "2025-07-18");
    assert!(
        from_str_result.is_ok(),
        "parse_from_str failed: {:?}",
        from_str_result.err()
    );

    // Parse from file
    let from_file_result = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18");
    assert!(from_file_result.is_ok());

    // Both should produce identical results
    let from_str = from_str_result.unwrap();
    let from_file = from_file_result.unwrap();

    assert_eq!(from_str.data.uslm_id, from_file.data.uslm_id);
    assert_eq!(from_str.data.path, from_file.data.path);
    assert_eq!(from_str.children.len(), from_file.children.len());
}
