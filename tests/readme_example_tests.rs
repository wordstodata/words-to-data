//! README Example Tests
//!
//! These tests replicate the exact examples shown in README.md.
//! If any of these tests fail, the README examples need to be updated.

use words_to_data::{
    dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId},
    uslm::bill_parser::parse_bill_amendments,
};

const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const TITLE_26: &str = "uscode/title_26";

/// The two expressions of title 26 the README example diffs.
fn readme_pair() -> (ExpressionId, ExpressionId) {
    let work = WorkId::new(TITLE_26);
    (
        ExpressionId::new(work.clone(), "2025-07-18"),
        ExpressionId::new(work, "2025-07-30"),
    )
}

/// Runs the Dataset Workflow example from the README Quick Start section, step
/// by step, and verifies what each step produces.
#[test]
fn readme_example_dataset_workflow_results() {
    let metadata = DatasetMetadata {
        name: "Tax Code Changes".to_string(),
        description: "Tracking Title 26 changes".to_string(),
        author: "Author".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
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
    dataset.add_bill(bill).unwrap();

    // Verify expressions added: two releases of one work
    let work = WorkId::new(TITLE_26);
    assert_eq!(dataset.works().unwrap(), vec![work.clone()]);
    assert_eq!(
        dataset.expressions(&work).unwrap().len(),
        2,
        "README shows 2 expressions"
    );

    // Verify bill added
    assert_eq!(dataset.storage().bills.len(), 1, "README shows 1 bill");

    // Verify diff works and has changes
    let (before, after) = readme_pair();
    let diff = dataset.compute_diff(&before, &after).unwrap();
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

    // Save dataset (to a directory this test owns, removed when it ends)
    let dir = tempfile::tempdir().expect("a temporary directory");
    let saved = dir.path().join("dataset.json");
    let saved = saved.to_str().expect("a utf-8 path");
    dataset
        .save(saved, Format::Compact)
        .expect("Failed to save dataset");
    let loaded = Dataset::load(saved, Format::Compact).expect("the saved dataset should load");
    assert_eq!(
        loaded.expressions(&work).unwrap().len(),
        2,
        "the saved dataset holds both expressions"
    );
}
