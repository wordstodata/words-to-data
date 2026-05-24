//! Website Example Tests
//!
//! These tests replicate the examples shown on wordstodata.com
//! If any of these tests fail, the website examples at w2d_site/index.html need to be updated.
use words_to_data::{
    dataset::{Dataset, DatasetMetadata},
    diff::TreeDiff,
    uslm::{TextContentField, bill_parser::parse_bill_amendments, parser::parse},
};

const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

// =============================================================================
// WEBSITE EXAMPLE: Parse a US Code Document
// https://wordstodata.com/#examples (Example 1)
// =============================================================================

/// Tests the parsing example shown on the website.
/// If this fails, update the "Parse a US Code Document" section in index.html.
#[test]
fn website_example_parse_usc_document() {
    // Load a USCode Title (as shown on website)
    let title_26 = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Failed to parse USC Title 26");

    // Navigate to §174(a) (path shown on website)
    let s174a_path =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";
    let s174a = title_26
        .find(s174a_path)
        .expect("§174(a) not found - website example path may need updating");

    // Verify the chapeau value matches what's shown on the website
    // Note: The actual XML uses curly apostrophe (') not ASCII apostrophe (')
    let expected_chapeau = "In the case of a taxpayer's specified research or experimental expenditures for any taxable year—";

    assert_eq!(
        s174a.data.chapeau.as_deref(),
        Some(expected_chapeau),
        "Chapeau value doesn't match website example. Update index.html if this changed."
    );
}

/// Tests the JSON structure shown in the website example output.
/// If this fails, update the JSON output section in the "Parse a US Code Document" example.
#[test]
fn website_example_parse_usc_json_structure() {
    let title_26 = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Failed to parse USC Title 26");

    let s174a_path =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";
    let s174a = title_26.find(s174a_path).unwrap();

    // Verify key fields shown in website JSON output
    assert_eq!(s174a.data.path.as_ref(), s174a_path);
    assert_eq!(
        s174a.data.number_value.as_ref(),
        "a",
        "number_value should be 'a' as shown on website"
    );
    assert_eq!(
        s174a.data.number_display.as_ref(),
        "(a)",
        "number_display should be '(a)' as shown on website"
    );
    assert_eq!(
        s174a.data.uslm_id.as_deref(),
        Some("/us/usc/t26/s174/a"),
        "uslm_id should match website example"
    );

    // Verify children exist (paragraph_1 shown in website JSON)
    assert!(
        !s174a.children.is_empty(),
        "§174(a) should have children as shown in website JSON"
    );

    // First child should be paragraph_1
    let first_child = &s174a.children[0];
    assert!(
        first_child.data.path.ends_with("paragraph_1"),
        "First child should be paragraph_1 as shown in website JSON"
    );
}

// =============================================================================
// WEBSITE EXAMPLE: Compute a Diff Between Versions
// https://wordstodata.com/#examples (Example 2)
// =============================================================================

/// Tests the diff computation example shown on the website.
/// If this fails, update the "Compute a Diff Between Versions" section in index.html.
#[test]
fn website_example_compute_diff() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Failed to parse old document");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Failed to parse new document");

    let diff = TreeDiff::from_elements(&doc_old, &doc_new);

    // Find the diff for §174(a) (as shown on website)
    let s174a_path =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";
    let s174a_diff = diff
        .find(s174a_path)
        .expect("§174(a) diff not found - this path has changes and should exist");

    // Verify changes exist
    assert!(
        !s174a_diff.changes.is_empty(),
        "§174(a) should have changes as shown on website"
    );

    // Get the chapeau change (as shown on website)
    let chapeau_change = s174a_diff
        .changes
        .iter()
        .find(|c| c.field_name == TextContentField::Chapeau)
        .expect("Chapeau change should exist as shown on website");

    // Verify old value matches website
    // Note: The actual XML uses curly apostrophe (') not ASCII apostrophe (')
    let expected_old = "In the case of a taxpayer's specified research or experimental expenditures for any taxable year—";
    assert_eq!(
        chapeau_change.old_value, expected_old,
        "Old chapeau value doesn't match website example"
    );

    // Verify new value matches website
    let expected_new = "In the case of a taxpayer's foreign research or experimental expenditures for any taxable year—";
    assert_eq!(
        chapeau_change.new_value, expected_new,
        "New chapeau value doesn't match website example"
    );
}

/// Tests the JSON diff structure shown in the website example output.
/// If this fails, update the JSON output section in the "Compute a Diff" example.
#[test]
fn website_example_diff_json_structure() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18").unwrap();
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30").unwrap();

    let diff = TreeDiff::from_elements(&doc_old, &doc_new);
    let s174a_path =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";
    let s174a_diff = diff.find(s174a_path).unwrap();

    let chapeau_change = s174a_diff
        .changes
        .iter()
        .find(|c| c.field_name == TextContentField::Chapeau)
        .unwrap();

    // Verify the specific word-level changes shown in the website JSON
    // Website shows: delete "specified" at old_index 12, insert "foreign" at new_index 12
    let delete_change = chapeau_change
        .changes
        .iter()
        .find(|c| c.value == "specified")
        .expect("Should have 'specified' deletion as shown on website");
    assert_eq!(
        delete_change.old_index,
        Some(12),
        "Website shows old_index: 12 for 'specified'"
    );
    assert!(
        delete_change.new_index.is_none(),
        "Website shows new_index: null for delete"
    );

    let insert_change = chapeau_change
        .changes
        .iter()
        .find(|c| c.value == "foreign")
        .expect("Should have 'foreign' insertion as shown on website");
    assert!(
        insert_change.old_index.is_none(),
        "Website shows old_index: null for insert"
    );
    assert_eq!(
        insert_change.new_index,
        Some(12),
        "Website shows new_index: 12 for 'foreign'"
    );
}

// =============================================================================
// WEBSITE EXAMPLE: Extract Amendments from a Bill
// https://wordstodata.com/#examples (Example 3)
// =============================================================================

/// Tests the bill amendment extraction example shown on the website.
/// If this fails, update the "Extract Amendments from a Bill" section in index.html.
#[test]
fn website_example_extract_amendments() {
    let data =
        parse_bill_amendments("119-21", PL_XML_PATH).expect("Failed to parse bill amendments");

    // Verify bill_id matches website (shows "119-21")
    assert_eq!(
        data.bill_id, "119-21",
        "Bill ID doesn't match website example. Update index.html if format changed."
    );

    // Website shows "603 amendments found"
    // NOTE: If this number changes, update the website output section
    let amendment_count = data.amendments.len();
    assert_eq!(
        amendment_count, 603,
        "Website shows '603 amendments found'. Current count: {}. Update website if this changed.",
        amendment_count
    );
}

/// Tests that amendments have the structure shown in the website example.
/// If this fails, update the amendment output section in index.html.
#[test]
fn website_example_amendment_structure() {
    let data = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();

    // Verify amendments have the fields shown on website
    for amendment in data.amendments.values() {
        // Website shows action_types field
        // (may be empty for some amendments, but field should exist)
        let _ = &amendment.action_types;

        // Website shows amending_text field (used indirectly via source_path)
        assert!(
            !amendment.amending_text.is_empty(),
            "Amendments should have amending_text"
        );
    }

    // Verify some amendments have multiple action types (as shown in website output)
    let has_multiple_actions = data.amendments.values().any(|a| a.action_types.len() > 1);
    assert!(
        has_multiple_actions,
        "Some amendments should have multiple action types as shown on website"
    );
}

// =============================================================================
// DOWNLOAD LINKS VERIFICATION
// https://wordstodata.com/#examples (download links section)
// =============================================================================

/// Verifies that all test data files referenced on the website exist.
/// If this fails, either the files are missing or the download links need updating.
#[test]
fn website_download_links_files_exist() {
    use std::path::Path;

    // Files referenced in the website download links section
    let files = [
        "tests/test_data/usc/2025-07-18/usc26.xml",
        "tests/test_data/usc/2025-07-30/usc26.xml",
        PL_XML_PATH,
    ];

    for file in files {
        assert!(
            Path::new(file).exists(),
            "Test data file '{}' referenced on website doesn't exist",
            file
        );
    }
}

// =============================================================================
// WEBSITE EXAMPLE: Dataset Workflow
// https://wordstodata.com/#examples (Example 4)
// =============================================================================

/// Tests the Dataset workflow example shown on the website.
/// If this fails, update the "Dataset Quick Start" section in index.html and README.md.
#[test]
fn website_example_dataset_workflow() {
    let metadata = DatasetMetadata {
        name: "Tax Code Changes".to_string(),
        description: "Tracking Title 26 changes".to_string(),
        author: "Author".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    };
    let mut dataset = Dataset::new(metadata);

    // Add versions
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc26.xml",
            "2025-07-18",
            Some("Pre-Amendment".to_string()),
        )
        .expect("add version");

    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-30/usc26.xml",
            "2025-07-30",
            Some("Post-Amendment".to_string()),
        )
        .expect("add version");

    assert_eq!(dataset.versions.len(), 2);

    // Add bill
    let bill = parse_bill_amendments("119-21", PL_XML_PATH).expect("parse bill");
    dataset.add_bill(bill);

    // Compute diff via dataset
    let diff = dataset
        .compute_diff("2025-07-18", "2025-07-30")
        .expect("diff");
    assert!(
        diff.find(
            "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
        )
        .is_some()
    );
}
