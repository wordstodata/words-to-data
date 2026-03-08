use words_to_data::uslm::bill_parser::parse_bill_amendments;

#[test]
fn test_parse_bill_amendments_success() {
    let result = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml");
    assert!(result.is_ok(), "Failed to parse bill: {:?}", result.err());

    let data = result.unwrap();
    assert_eq!(
        data.bill_id, "119-21",
        "Bill ID should be extracted from document"
    );
}

#[test]
fn test_parse_bill_amendments_has_amendments() {
    let result = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml");
    assert!(result.is_ok());

    let data = result.unwrap();
    assert!(
        !data.amendments.is_empty(),
        "Bill should contain at least one amendment"
    );
    println!("Found {} amendments", data.amendments.len());
}

#[test]
fn test_amendments_have_source_paths() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    // All amendments should have source paths
    for amendment in &data.amendments {
        assert!(
            !amendment.source_path.is_empty(),
            "Amendment should have a source path"
        );
        assert!(
            amendment.source_path.starts_with("/us/pl/119/21/"),
            "Source path should start with bill identifier: {}",
            amendment.source_path
        );
    }
}

#[test]
fn test_amendments_have_usc_references() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    // At least some amendments should reference USC sections
    let amendments_with_refs: Vec<_> = data
        .amendments
        .iter()
        .filter(|a| !a.target_paths.is_empty())
        .collect();

    assert!(
        !amendments_with_refs.is_empty(),
        "Some amendments should reference USC sections"
    );

    // Verify USC references have correct format
    for amendment in amendments_with_refs {
        for usc_ref in &amendment.target_paths {
            assert!(
                usc_ref.path.starts_with("/us/usc/"),
                "USC reference should start with /us/usc/: {}",
                usc_ref.path
            );
            assert!(
                !usc_ref.display_text.is_empty(),
                "USC reference should have display text"
            );
        }
    }
}

#[test]
fn test_amendments_have_action_types() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    // At least some amendments should have action types
    let amendments_with_actions: Vec<_> = data
        .amendments
        .iter()
        .filter(|a| !a.action_types.is_empty())
        .collect();

    assert!(
        !amendments_with_actions.is_empty(),
        "Some amendments should have action types"
    );

    // Print out what actions we found for debugging
    for amendment in &data.amendments {
        if !amendment.action_types.is_empty() {
            println!(
                "Amendment {} has {} action(s): {:?}",
                amendment.source_path,
                amendment.action_types.len(),
                amendment.action_types
            );
        }
    }
}

#[test]
fn test_specific_amendment_structure() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    // Find a specific amendment we know exists (from line 1678 of the XML)
    let amendment = data
        .amendments
        .iter()
        .find(|a| a.source_path.contains("/tI/stA/s10101/a"))
        .expect("Should find amendment at /us/pl/119/21/tI/stA/s10101/a");

    // This amendment modifies 7 U.S.C. 2012
    let usc_7_2012 = amendment
        .target_paths
        .iter()
        .find(|r| r.path.contains("/us/usc/t7/s2012"));

    assert!(
        usc_7_2012.is_some(),
        "Amendment should reference 7 U.S.C. 2012"
    );

    // This amendment should have amend, delete, and insert actions
    assert!(
        !amendment.action_types.is_empty(),
        "Amendment should have action types"
    );
}

#[test]
fn test_amendment_with_multiple_actions() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    // Find amendments with multiple actions
    let multi_action_amendments: Vec<_> = data
        .amendments
        .iter()
        .filter(|a| a.action_types.len() > 1)
        .collect();

    assert!(
        !multi_action_amendments.is_empty(),
        "Some amendments should have multiple action types"
    );

    // Print first one for debugging
    if let Some(amendment) = multi_action_amendments.first() {
        println!(
            "Amendment with multiple actions: {} has {:?}",
            amendment.source_path, amendment.action_types
        );
    }
}

#[test]
fn test_parse_bill_nonexistent_file() {
    let result = parse_bill_amendments("tests/test_data/bills/nonexistent.xml");
    assert!(result.is_err(), "Should fail for nonexistent file");
}

#[test]
fn test_amendment_data_structure_completeness() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    // Verify the AmendmentData structure is complete
    assert_eq!(data.bill_id, "119-21");
    assert!(!data.amendments.is_empty());

    // Each amendment should be properly structured
    for (i, amendment) in data.amendments.iter().enumerate() {
        assert!(
            !amendment.source_path.is_empty(),
            "Amendment {} should have source_path",
            i
        );
        // Note: target_paths and action_types can be empty for some amendments,
        // but they should exist as fields
        let _ = &amendment.target_paths;
        let _ = &amendment.action_types;
    }
}

#[test]
fn test_usc_reference_display_text() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    // Find amendments with USC references
    for amendment in &data.amendments {
        for usc_ref in &amendment.target_paths {
            // Display text should be human-readable
            assert!(
                !usc_ref.display_text.is_empty(),
                "USC reference should have display text"
            );

            // It should typically contain some form of "USC" or be a path
            let has_usc_format = usc_ref.display_text.contains("U.S.C.")
                || usc_ref.display_text.contains("USC")
                || usc_ref.display_text.contains("United States Code")
                || usc_ref.display_text.starts_with("/us/usc/");

            assert!(
                has_usc_format,
                "Display text should reference USC: '{}'",
                usc_ref.display_text
            );
        }
    }
}

#[test]
fn test_bill_id_format() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    // Bill ID should be in congress-number format
    assert!(
        data.bill_id.contains('-'),
        "Bill ID should contain a hyphen: {}",
        data.bill_id
    );

    let parts: Vec<&str> = data.bill_id.split('-').collect();
    assert_eq!(
        parts.len(),
        2,
        "Bill ID should have two parts: congress-number"
    );
}

#[test]
fn test_amendment_paths_are_unique() {
    let data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").unwrap();

    let mut seen_paths = std::collections::HashSet::new();
    let mut duplicates = Vec::new();

    for amendment in &data.amendments {
        if !seen_paths.insert(&amendment.source_path) {
            duplicates.push(&amendment.source_path);
        }
    }

    assert!(
        duplicates.is_empty(),
        "Amendment source paths should be unique, found duplicates: {:?}",
        duplicates
    );
}
