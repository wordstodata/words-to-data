use std::fs::File;
use std::io::BufReader;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::dataset::{Dataset, DatasetMetadata, VersionSnapshot};
use words_to_data::storage::{DatasetReader, InMemoryStorage, SqliteStorage};
use words_to_data::uslm::bill_parser::parse_bill_amendments;
use words_to_data::uslm::parser::parse;

const TEST_PATH: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_163/subsection_j/paragraph_8/subparagraph_A/clause_v";

fn make_test_dataset() -> Dataset<InMemoryStorage> {
    let metadata = DatasetMetadata {
        name: "SQLite Test".to_string(),
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
fn should_save_and_load_sqlite_format() {
    println!("RUNNING");
    let mut dataset = make_test_dataset();
    println!("ADDING 1");
    dataset
        .add_version(make_snapshot("2024-01-01", Some("First")))
        .unwrap();
    println!("ADDING 2");
    dataset
        .add_version(make_snapshot("2024-06-01", None))
        .unwrap();

    let path = "/tmp/test_dataset.db";

    println!("SAVING");
    // Save as SQLite
    dataset.save_to_sqlite(path).expect("save should succeed");

    // Load from SQLite
    println!("LOADING");
    let loaded = Dataset::open_sqlite(path)
        .expect("open should succeed")
        .to_memory()
        .expect("to_memory should succeed");

    assert_eq!(loaded.metadata().name, "SQLite Test");
    assert_eq!(loaded.storage().versions.len(), 2);
    assert_eq!(loaded.storage().versions[0].date, "2024-01-01");
    assert_eq!(
        loaded.storage().versions[0].label,
        Some("First".to_string())
    );
    assert_eq!(loaded.storage().versions[1].date, "2024-06-01");
    assert_eq!(loaded.storage().versions[1].label, None);

    // Verify element data preserved
    let elem = &loaded.storage().versions[0].element;
    assert_eq!(elem.data.path.as_ref(), "uscode");

    // Cleanup
    std::fs::remove_file(path).ok();
}

const PL_XML_PATH: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

fn load_annotations() -> Vec<ChangeAnnotation> {
    let file = File::open("tests/test_data/processed/annotations.json")
        .expect("should be able to open annotations file");
    serde_json::from_reader(BufReader::new(file)).unwrap()
}

#[test]
fn should_roundtrip_bills_and_annotations_sqlite() {
    let mut dataset = make_test_dataset();
    dataset
        .add_version(make_snapshot("2025-07-18", None))
        .unwrap();
    dataset
        .add_version(make_snapshot("2025-07-30", None))
        .unwrap();

    // Add bill
    let bill = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();
    dataset.add_bill(bill).unwrap();

    // Add annotations
    for annotation in load_annotations() {
        dataset
            .add_annotation("2025-07-18", "2025-07-30", annotation)
            .unwrap();
    }

    let path = "/tmp/test_dataset_full.db";

    dataset.save_to_sqlite(path).unwrap();
    let loaded = Dataset::open_sqlite(path).unwrap().to_memory().unwrap();

    // Verify bill
    assert_eq!(loaded.storage().bills.len(), 1);
    let loaded_bill = loaded.get_bill("119-21").unwrap().unwrap();
    assert_eq!(loaded_bill.bill_id, "119-21");

    // Verify annotations
    let anns = loaded
        .get_annotations("2025-07-18", "2025-07-30")
        .unwrap()
        .unwrap();
    assert_eq!(anns.len(), 753);

    std::fs::remove_file(path).ok();
}

#[test]
fn should_support_incremental_save_sqlite() {
    let path = "/tmp/test_incremental.db";

    // First save with one version
    {
        let mut dataset = make_test_dataset();
        dataset
            .add_version(make_snapshot("2024-01-01", Some("V1")))
            .unwrap();
        dataset.save_to_sqlite(path).unwrap();
    }

    // Load, add another version, save again
    {
        let mut dataset = Dataset::open_sqlite(path).unwrap().to_memory().unwrap();
        assert_eq!(dataset.storage().versions.len(), 1);

        dataset
            .add_version(make_snapshot("2024-06-01", Some("V2")))
            .unwrap();
        dataset.save_to_sqlite(path).unwrap();
    }

    // Verify both versions present
    let loaded = Dataset::open_sqlite(path).unwrap().to_memory().unwrap();
    assert_eq!(loaded.storage().versions.len(), 2);
    assert_eq!(loaded.storage().versions[0].label, Some("V1".to_string()));
    assert_eq!(loaded.storage().versions[1].label, Some("V2".to_string()));

    std::fs::remove_file(path).ok();
}

#[test]
fn should_query_via_trait_interface() {
    // Setup: save dataset to SQLite
    let mut dataset = make_test_dataset();
    dataset
        .add_version(make_snapshot("2025-07-18", Some("V1")))
        .unwrap();
    dataset
        .add_version(make_snapshot("2025-07-30", Some("V2")))
        .unwrap();

    let bill = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();
    dataset.add_bill(bill).unwrap();

    for annotation in load_annotations() {
        dataset
            .add_annotation("2025-07-18", "2025-07-30", annotation)
            .unwrap();
    }

    let path = "/tmp/test_trait_query.db";
    dataset.save_to_sqlite(path).unwrap();

    // Query via trait - works for both Dataset and SqliteStorage
    fn check_reader(reader: &impl DatasetReader) {
        // list_versions
        let versions = reader.list_versions().unwrap();
        assert_eq!(versions.len(), 2);

        // get_version
        let v1 = reader.get_version("2025-07-18").unwrap().unwrap();
        assert_eq!(v1.label, Some("V1".to_string()));

        // get_bill
        let bill = reader.get_bill("119-21").unwrap().unwrap();
        assert_eq!(bill.bill_id, "119-21");

        // get_annotations
        let anns = reader
            .get_annotations("2025-07-18", "2025-07-30")
            .unwrap()
            .unwrap();
        assert_eq!(anns.len(), 753);

        // compute_diff
        let diff = reader.compute_diff("2025-07-18", "2025-07-30").unwrap();
        assert_eq!(diff.root_path, "uscode");
    }

    // Test with Dataset
    check_reader(&dataset);

    // Test with SqliteStorage
    let storage = SqliteStorage::open(path).unwrap();
    check_reader(&storage);

    std::fs::remove_file(path).ok();
}

#[test]
fn should_load_window_with_two_versions() {
    // Setup: save dataset with 3 versions
    let mut dataset = make_test_dataset();
    dataset
        .add_version(make_snapshot("2024-01-01", Some("V1")))
        .unwrap();
    dataset
        .add_version(make_snapshot("2024-06-01", Some("V2")))
        .unwrap();
    dataset
        .add_version(make_snapshot("2024-12-01", Some("V3")))
        .unwrap();

    for annotation in load_annotations() {
        dataset
            .add_annotation("2024-01-01", "2024-06-01", annotation.clone())
            .unwrap();
        dataset
            .add_annotation("2024-06-01", "2024-12-01", annotation)
            .unwrap();
    }

    let path = "/tmp/test_load_window.db";
    dataset.save_to_sqlite(path).unwrap();

    // Load window with just 2 versions (returns InMemoryStorage)
    let storage = SqliteStorage::open(path).unwrap();
    let windowed = storage.load_window("2024-01-01", "2024-06-01").unwrap();

    // Should have exactly 2 versions
    assert_eq!(windowed.versions.len(), 2);
    assert_eq!(windowed.versions[0].date, "2024-01-01");
    assert_eq!(windowed.versions[1].date, "2024-06-01");

    // Should have annotations for that pair only
    assert!(
        windowed
            .diff_annotations
            .contains_key(&("2024-01-01".to_string(), "2024-06-01".to_string()))
    );
    assert!(
        !windowed
            .diff_annotations
            .contains_key(&("2024-06-01".to_string(), "2024-12-01".to_string()))
    );

    // Bills/members/sponsors should be empty (query from storage when needed)
    assert!(windowed.bills.is_empty());

    std::fs::remove_file(path).ok();
}

#[test]
fn should_query_annotations_for_path_via_trait() {
    let mut dataset = make_test_dataset();
    dataset
        .add_version(make_snapshot("2025-07-18", None))
        .unwrap();
    dataset
        .add_version(make_snapshot("2025-07-30", None))
        .unwrap();

    for annotation in load_annotations() {
        dataset
            .add_annotation("2025-07-18", "2025-07-30", annotation)
            .unwrap();
    }

    let path = "/tmp/test_ann_path.db";
    dataset.save_to_sqlite(path).unwrap();

    // Test via trait - should work for both
    fn check_annotations_for_path(reader: &impl DatasetReader) {
        let anns = reader.annotations_for_path(TEST_PATH).unwrap();
        assert!(!anns.is_empty());
        // All returned annotations should contain the path
        for ann in &anns {
            assert!(ann.paths.iter().any(|p| p == TEST_PATH));
        }
    }

    check_annotations_for_path(&dataset);

    let storage = SqliteStorage::open(path).unwrap();
    check_annotations_for_path(&storage);

    std::fs::remove_file(path).ok();
}
