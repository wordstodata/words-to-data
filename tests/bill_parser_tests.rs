use words_to_data::uslm::bill_parser::{parse_bill_amendments, parse_bill_amendments_from_str};
const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

#[test]
fn should_parse_bill_and_extract_bill_id() {
    let result = parse_bill_amendments("119-hr-1", PL_XML_PATH);
    assert!(result.is_ok(), "Failed to parse bill: {:?}", result.err());

    let data = result.unwrap();
    assert_eq!(
        data.bill_id, "119-hr-1",
        "Bill ID should be extracted from document"
    );
}

#[test]
fn should_extract_amendments_from_bill() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();

    assert!(
        !data.amendments.is_empty(),
        "Bill should contain at least one amendment"
    );
}

#[test]
fn should_extract_action_types_from_amendments() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();

    // At least some amendments should have action types
    let amendments_with_actions: Vec<_> = data
        .amendments
        .values()
        .filter(|a| !a.action_types.is_empty())
        .collect();

    assert!(
        !amendments_with_actions.is_empty(),
        "Some amendments should have action types"
    );
}

#[test]
fn should_extract_amending_text_from_amendments() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();

    // All amendments should have amending text
    for amendment in data.amendments.values() {
        assert!(
            !amendment.amending_text.is_empty(),
            "Amendment should have amending text"
        );
    }
}

#[test]
fn should_find_amendments_with_multiple_action_types() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();

    // Find amendments with multiple actions
    let multi_action_amendments: Vec<_> = data
        .amendments
        .values()
        .filter(|a| a.action_types.len() > 1)
        .collect();

    assert!(
        !multi_action_amendments.is_empty(),
        "Some amendments should have multiple action types"
    );
}

#[test]
fn should_fail_for_nonexistent_file() {
    let result = parse_bill_amendments("119-hr-1", "tests/test_data/bills/nonexistent.xml");
    assert!(result.is_err(), "Should fail for nonexistent file");
}

#[test]
fn should_parse_bill_id_in_congress_type_number_format() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();

    // The bill id is the one `build-dataset` gives: congress, bill type, number.
    let parts: Vec<&str> = data.bill_id.split('-').collect();
    assert_eq!(
        parts,
        vec!["119", "hr", "1"],
        "Bill ID should have three parts, congress-type-number: {}",
        data.bill_id
    );
}

#[test]
fn should_produce_same_result_from_str_and_file() {
    // Load XML manually
    let xml_str = std::fs::read_to_string(PL_XML_PATH).expect("Failed to read test file");

    // Parse from string
    let from_str_result = parse_bill_amendments_from_str("119-hr-1", &xml_str);
    assert!(
        from_str_result.is_ok(),
        "parse_bill_amendments_from_str failed: {:?}",
        from_str_result.err()
    );

    // Parse from file
    let from_file_result = parse_bill_amendments("119-hr-1", PL_XML_PATH);
    assert!(from_file_result.is_ok());

    // Both should produce identical results
    let from_str = from_str_result.unwrap();
    let from_file = from_file_result.unwrap();

    assert_eq!(from_str.bill_id, from_file.bill_id);
    assert_eq!(from_str.amendments.len(), from_file.amendments.len());
}

#[test]
fn should_capture_amending_text_with_strike_and_insert_language() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();

    // Find amendments that contain typical strike/insert language
    let strike_insert_amendments: Vec<_> = data
        .amendments
        .values()
        .filter(|a| a.amending_text.contains("striking") && a.amending_text.contains("inserting"))
        .collect();

    assert!(
        !strike_insert_amendments.is_empty(),
        "Should find amendments with strike and insert language"
    );
}

#[test]
fn should_find_section_174_amendment_in_bill() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).expect("Failed to parse bill");

    let amendment = data
        .amendments
        .into_values()
        .find(|a| {
            a.amending_text.contains("Section 174 is amended")
                && a.amending_text.contains("foreign research")
        })
        .expect("Should find amendment for section 174");

    // Verify the amendment text mentions section 174
    assert!(
        amendment.amending_text.contains("Section 174 is amended"),
        "Amendment should reference Section 174"
    );

    // Verify it contains the foreign research language
    assert!(
        amendment.amending_text.contains("foreign research"),
        "Amendment should mention foreign research"
    );

    // Verify it has the expected action types
    assert!(
        !amendment.action_types.is_empty(),
        "Amendment should have action types"
    );
}

// =============================================================================
// Amendment ID tests
// =============================================================================

#[test]
fn should_generate_deterministic_amendment_id() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();
    let (id, amendment) = data.amendments.iter().next().unwrap();

    // ID should exist and be 64 chars (full SHA256 hex)
    assert_eq!(amendment.id.len(), 64);

    // Parsing again should produce same IDs
    let data2 = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();
    let amendment2 = data2.amendments.get(id).unwrap();
    assert_eq!(amendment.id, amendment2.id);
}

#[test]
fn should_store_amendments_in_hashmap_by_id() {
    let data = parse_bill_amendments("119-hr-1", PL_XML_PATH).unwrap();

    // Should be able to look up by ID
    for (id, amendment) in &data.amendments {
        assert_eq!(&amendment.id, id);
    }
}
