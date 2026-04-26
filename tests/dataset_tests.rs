use std::fs::File;
use std::io::BufReader;

use words_to_data::dataset::{Dataset, DatasetMetadata, VersionSnapshot};
use words_to_data::diff::TreeDiff;
use words_to_data::legal_diff::ChangeAnnotation;
use words_to_data::uslm::bill_parser::parse_bill_amendments;
use words_to_data::uslm::parser::parse;

#[test]
fn should_serialize_roundtrip_json() {
    let metadata = DatasetMetadata {
        name: "Test Dataset".to_string(),
        description: "For testing".to_string(),
        author: "Test".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "0.1.0".to_string(),
    };

    let mut dataset = Dataset::new(metadata);

    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc07.xml",
            "2025-07-18",
            Some("test".to_string()),
        )
        .expect("failed to load USLM doc");
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc07.xml",
            "2025-07-30",
            Some("test".to_string()),
        )
        .expect("failed to load USLM doc");

    let annotations = make_annotations();
    for annotation in annotations.into_iter() {
        dataset.add_annotation("2025-07-18", "2025-07-30", annotation);
    }

    // Serialize to JSON and back
    let json = serde_json::to_string(&dataset).unwrap();
    let roundtripped: Dataset = serde_json::from_str(&json).unwrap();

    assert_eq!(roundtripped.metadata.name, "Test Dataset");
    assert_eq!(roundtripped.versions.len(), 2);
    assert_eq!(roundtripped.versions[0].date, "2025-07-18");
    assert_eq!(roundtripped.versions[0].label, Some("test".to_string()));
    assert_eq!(
        roundtripped
            .get_annotations("2025-07-18", "2025-07-30")
            .unwrap()
            .len(),
        753
    );
    assert_eq!(
        roundtripped
            .get_annotations("2025-07-18", "2025-07-20")
            .iter()
            .len(),
        0
    );
}

fn make_test_dataset() -> Dataset {
    let metadata = DatasetMetadata {
        name: "Test".to_string(),
        description: "Test".to_string(),
        author: "Test".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "0.1.0".to_string(),
    };
    Dataset::new(metadata)
}

fn make_snapshot(date: &str, label: Option<&str>) -> VersionSnapshot {
    let element = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    VersionSnapshot {
        date: date.to_string(),
        label: label.map(|s| s.to_string()),
        element,
    }
}

#[test]
fn should_add_version_maintaining_order() {
    let mut dataset = make_test_dataset();

    // Add out of order
    dataset.add_version(make_snapshot("2024-06-01", None));
    dataset.add_version(make_snapshot("2024-01-01", Some("First")));
    dataset.add_version(make_snapshot("2024-12-01", Some("Last")));

    assert_eq!(dataset.versions.len(), 3);
    assert_eq!(dataset.versions[0].date, "2024-01-01");
    assert_eq!(dataset.versions[1].date, "2024-06-01");
    assert_eq!(dataset.versions[2].date, "2024-12-01");
}

#[test]
fn should_get_version_by_date() {
    let mut dataset = make_test_dataset();
    dataset.add_version(make_snapshot("2024-01-01", Some("First")));
    dataset.add_version(make_snapshot("2024-06-01", None));

    let found = dataset.get_version("2024-01-01");
    assert!(found.is_some());
    assert_eq!(found.unwrap().label, Some("First".to_string()));

    let not_found = dataset.get_version("2024-03-01");
    assert!(not_found.is_none());
}

#[test]
fn should_navigate_versions() {
    let mut dataset = make_test_dataset();
    dataset.add_version(make_snapshot("2024-01-01", Some("First")));
    dataset.add_version(make_snapshot("2024-06-01", Some("Middle")));
    dataset.add_version(make_snapshot("2024-12-01", Some("Last")));

    // next_version
    let next = dataset.next_version("2024-01-01");
    assert!(next.is_some());
    assert_eq!(next.unwrap().date, "2024-06-01");

    // prev_version
    let prev = dataset.prev_version("2024-12-01");
    assert!(prev.is_some());
    assert_eq!(prev.unwrap().date, "2024-06-01");

    // Edge cases
    assert!(dataset.next_version("2024-12-01").is_none()); // no next after last
    assert!(dataset.prev_version("2024-01-01").is_none()); // no prev before first
}

#[test]
fn should_save_and_load_file() {
    let mut dataset = make_test_dataset();
    dataset.add_version(make_snapshot("2024-01-01", Some("First")));

    let path = "/tmp/dataset_test_save_load.json";

    // Save
    dataset.save(path).expect("save should succeed");

    // Load
    let loaded = Dataset::load(path).expect("load should succeed");

    assert_eq!(loaded.metadata.name, "Test");
    assert_eq!(loaded.versions.len(), 1);
    assert_eq!(loaded.versions[0].date, "2024-01-01");

    // Cleanup
    std::fs::remove_file(path).ok();
}

#[test]
fn should_compute_diff_between_versions() {
    let mut dataset = make_test_dataset();

    // Use two real versions
    let elem1 = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    let elem2 = parse("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30").unwrap();

    dataset.add_version(VersionSnapshot {
        date: "2025-07-18".to_string(),
        label: Some("First".to_string()),
        element: elem1,
    });
    dataset.add_version(VersionSnapshot {
        date: "2025-07-30".to_string(),
        label: Some("Second".to_string()),
        element: elem2,
    });

    let diff: TreeDiff = dataset.compute_diff("2025-07-18", "2025-07-30").unwrap();

    // Should have same root path
    assert_eq!(diff.root_path, "uscode");
}

#[test]
fn should_add_and_query_bills() {
    let mut dataset = make_test_dataset();

    let bill = parse_bill_amendments("tests/test_data/bills/pl-119-21.xml").unwrap();
    let bill_id = bill.bill_id.clone();

    dataset.add_bill(bill);

    assert_eq!(dataset.bills.len(), 1);

    let found = dataset.get_bill(&bill_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().bill_id, "119-21");

    let not_found = dataset.get_bill("nonexistent");
    assert!(not_found.is_none());
}

fn make_annotations() -> Vec<ChangeAnnotation> {
    let file = File::open("tests/test_data/processed/annotations.json")
        .expect("should be able to open annotations file");
    let annotations: Vec<ChangeAnnotation> = serde_json::from_reader(BufReader::new(file)).unwrap();
    annotations
}

#[test]
fn should_query_annotations_by_path() {
    let mut dataset = make_test_dataset();

    for annotation in make_annotations().into_iter() {
        dataset.add_annotation("2025-07-18", "2025-07-30", annotation);
    }

    // Query by path
    let found = dataset.annotations_for_path("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_163/subsection_j/paragraph_8/subparagraph_A/clause_v");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].source_bill.bill_id, "119-21");

    // Query by bill
    let found = dataset.annotations_for_bill("119-21");
    assert_eq!(found.len(), 753);

    // No matches
    let found = dataset.annotations_for_path("uscode/title_99/section_1");
    assert_eq!(found.len(), 0);
}

#[test]
fn should_find_element_across_versions() {
    let mut dataset = make_test_dataset();

    let elem1 = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    let elem2 = parse("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30").unwrap();

    dataset.add_version(VersionSnapshot {
        date: "2025-07-18".to_string(),
        label: None,
        element: elem1,
    });
    dataset.add_version(VersionSnapshot {
        date: "2025-07-30".to_string(),
        label: None,
        element: elem2,
    });

    // Find element across versions
    let results = dataset.find_element("uscode/title_7");

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].0, "2025-07-18");
    assert_eq!(results[1].0, "2025-07-30");
}

#[test]
fn should_get_version_by_label() {
    let mut dataset = make_test_dataset();
    dataset.add_version(make_snapshot("2024-01-01", Some("Pre-Tax Cuts Act")));
    dataset.add_version(make_snapshot("2024-06-01", None));
    dataset.add_version(make_snapshot("2024-12-01", Some("Post-Tax Cuts Act")));

    let found = dataset.get_version_by_label("Pre-Tax Cuts Act");
    assert!(found.is_some());
    assert_eq!(found.unwrap().date, "2024-01-01");

    let found = dataset.get_version_by_label("Post-Tax Cuts Act");
    assert!(found.is_some());
    assert_eq!(found.unwrap().date, "2024-12-01");

    let not_found = dataset.get_version_by_label("Nonexistent");
    assert!(not_found.is_none());
}

#[test]
fn should_search_text_across_versions() {
    let mut dataset = make_test_dataset();

    let elem = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    dataset.add_version(VersionSnapshot {
        date: "2025-07-18".to_string(),
        label: None,
        element: elem,
    });

    // Search for text that exists in Title 7 (Agriculture)
    let results = dataset.search_text("Agriculture");

    // Should find at least one match
    assert!(!results.is_empty());
    // Results should include version date and path
    assert_eq!(results[0].date, "2025-07-18");
}
