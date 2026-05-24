//! README Example Tests
//!
//! These tests replicate the exact examples shown in README.md.
//! If any of these tests fail, the README examples need to be updated.

use words_to_data::{
    dataset::{Dataset, DatasetMetadata, Format},
    uslm::bill_parser::parse_bill_amendments,
};

const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

/// Tests the Dataset Workflow example from README.md Quick Start section.
/// This is the primary example showing the full workflow.
#[test]
fn readme_example_dataset_workflow() {
    // -- Exact code from README (with real test paths) --

    let metadata = DatasetMetadata {
        name: "Tax Code Changes".to_string(),
        description: "Tracking Title 26 changes".to_string(),
        author: "Author".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    };
    let mut dataset = Dataset::new(metadata);

    // Add document versions
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc26.xml",
            "2025-07-18",
            Some("Before".into()),
        )
        .expect("Failed to add old version");
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-30/usc26.xml",
            "2025-07-30",
            Some("After".into()),
        )
        .expect("Failed to add new version");

    // Add bill
    let bill = parse_bill_amendments("119-21", PL_XML_PATH).expect("Failed to parse bill");
    dataset.add_bill(bill);

    // Compute diff
    let diff = dataset
        .compute_diff("2025-07-18", "2025-07-30")
        .expect("Failed to compute diff");

    // Navigate to specific section
    if let Some(s174a) = diff
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
    {
        for change in &s174a.changes {
            // README shows: println!("{:?}: {} → {}", change.field_name, change.old_value, change.new_value);
            let _output = format!(
                "{:?}: {} → {}",
                change.field_name, change.old_value, change.new_value
            );
        }
    } else {
        panic!("Section 174(a) not found in diff - README example path may be wrong");
    }

    // Save dataset (to temp file)
    let temp_path = std::env::temp_dir().join("readme_test_dataset.json");
    dataset
        .save(temp_path.to_str().unwrap(), Format::Compact)
        .expect("Failed to save dataset");

    // Cleanup
    let _ = std::fs::remove_file(temp_path);
}

/// Verifies the Dataset example produces expected results.
#[test]
fn readme_example_dataset_workflow_results() {
    let metadata = DatasetMetadata {
        name: "Tax Code Changes".to_string(),
        description: "Tracking Title 26 changes".to_string(),
        author: "Author".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    };
    let mut dataset = Dataset::new(metadata);

    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc26.xml",
            "2025-07-18",
            Some("Before".into()),
        )
        .unwrap();
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-30/usc26.xml",
            "2025-07-30",
            Some("After".into()),
        )
        .unwrap();

    let bill = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();
    dataset.add_bill(bill);

    // Verify versions added
    assert_eq!(dataset.versions.len(), 2, "README shows 2 versions");

    // Verify bill added
    assert_eq!(dataset.bills.len(), 1, "README shows 1 bill");

    // Verify diff works and has changes
    let diff = dataset.compute_diff("2025-07-18", "2025-07-30").unwrap();
    let s174a = diff
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
        .expect("Section should exist");

    assert!(
        !s174a.changes.is_empty(),
        "Section 174(a) should have changes"
    );

    // Verify change has expected fields (as shown in README output)
    let change = &s174a.changes[0];
    assert!(!change.old_value.is_empty());
    assert!(!change.new_value.is_empty());
}
