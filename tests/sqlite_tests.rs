use std::fs::File;
use std::io::BufReader;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, work_roots,
};
use words_to_data::storage::{
    DocumentReader, InMemoryStorage, LegislatureReader, LinkReader, SqliteStorage,
};
use words_to_data::uslm::bill_parser::parse_bill_amendments;
use words_to_data::uslm::parser::parse;

const TEST_PATH: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_163/subsection_j/paragraph_8/subparagraph_A/clause_v";
/// Title 7 is Agriculture. It is the work every expression below belongs to.
const TITLE_7: &str = "uscode/title_7";

fn title_7() -> WorkId {
    WorkId::new(TITLE_7)
}

fn at(date: &str) -> ExpressionId {
    ExpressionId::new(title_7(), date)
}

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

/// Title 7's tree, labelled with `date`.
fn make_expression(date: &str, label: Option<&str>) -> Expression {
    let parsed = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    let root = work_roots(parsed).pop().expect("the file holds one title");
    Expression {
        id: at(date),
        label: label.map(|s| s.to_string()),
        element: root,
    }
}

/// The ids and labels of every expression of title 7, oldest first.
fn listed(dataset: &Dataset<InMemoryStorage>) -> Vec<(ExpressionId, Option<String>)> {
    dataset
        .expressions(&title_7())
        .unwrap()
        .into_iter()
        .map(|info| (info.id, info.label))
        .collect()
}

#[test]
fn should_save_and_load_sqlite_format() {
    let mut dataset = make_test_dataset();
    dataset
        .add_expression(make_expression("2024-01-01", Some("First")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-06-01", None))
        .unwrap();

    let path = "/tmp/test_dataset.db";

    // Save as SQLite
    dataset.save_to_sqlite(path).expect("save should succeed");

    // Load from SQLite
    let loaded = Dataset::open_sqlite(path)
        .expect("open should succeed")
        .to_memory()
        .expect("to_memory should succeed");

    assert_eq!(loaded.metadata().name, "SQLite Test");
    assert_eq!(
        listed(&loaded),
        vec![
            (at("2024-01-01"), Some("First".to_string())),
            (at("2024-06-01"), None),
        ]
    );

    // Verify element data preserved, rooted at the work
    let expression = loaded.get_expression(&at("2024-01-01")).unwrap().unwrap();
    assert_eq!(expression.element.data.path.as_ref(), TITLE_7);

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
        .add_expression(make_expression("2025-07-18", None))
        .unwrap();
    dataset
        .add_expression(make_expression("2025-07-30", None))
        .unwrap();

    // Add bill
    let bill = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();
    dataset.add_bill(bill).unwrap();

    // Add annotations
    for annotation in load_annotations() {
        dataset
            .add_annotation(&at("2025-07-18"), &at("2025-07-30"), annotation)
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
        .get_annotations(&at("2025-07-18"), &at("2025-07-30"))
        .unwrap()
        .unwrap();
    assert_eq!(anns.len(), 753);

    std::fs::remove_file(path).ok();
}

#[test]
fn should_support_incremental_save_sqlite() {
    let path = "/tmp/test_incremental.db";

    // First save with one expression
    {
        let mut dataset = make_test_dataset();
        dataset
            .add_expression(make_expression("2024-01-01", Some("V1")))
            .unwrap();
        dataset.save_to_sqlite(path).unwrap();
    }

    // Load, add another expression, save again
    {
        let mut dataset = Dataset::open_sqlite(path).unwrap().to_memory().unwrap();
        assert_eq!(listed(&dataset).len(), 1);

        dataset
            .add_expression(make_expression("2024-06-01", Some("V2")))
            .unwrap();
        dataset.save_to_sqlite(path).unwrap();
    }

    // Verify both expressions present
    let loaded = Dataset::open_sqlite(path).unwrap().to_memory().unwrap();
    assert_eq!(
        listed(&loaded),
        vec![
            (at("2024-01-01"), Some("V1".to_string())),
            (at("2024-06-01"), Some("V2".to_string())),
        ]
    );

    std::fs::remove_file(path).ok();
}

#[test]
fn should_query_via_trait_interface() {
    // Setup: save dataset to SQLite
    let mut dataset = make_test_dataset();
    dataset
        .add_expression(make_expression("2025-07-18", Some("V1")))
        .unwrap();
    dataset
        .add_expression(make_expression("2025-07-30", Some("V2")))
        .unwrap();

    let bill = parse_bill_amendments("119-21", PL_XML_PATH).unwrap();
    dataset.add_bill(bill).unwrap();

    for annotation in load_annotations() {
        dataset
            .add_annotation(&at("2025-07-18"), &at("2025-07-30"), annotation)
            .unwrap();
    }

    let path = "/tmp/test_trait_query.db";
    dataset.save_to_sqlite(path).unwrap();

    // Query via trait - works for both Dataset and SqliteStorage
    fn check_reader(reader: &(impl DocumentReader + LinkReader + LegislatureReader)) {
        // works
        assert_eq!(reader.works().unwrap(), vec![title_7()]);

        // expressions
        let expressions = reader.expressions(&title_7()).unwrap();
        assert_eq!(expressions.len(), 2);
        assert_eq!(expressions[0].label, Some("V1".to_string()));

        // get_expression
        let v1 = reader.get_expression(&at("2025-07-18")).unwrap().unwrap();
        assert_eq!(v1.label, Some("V1".to_string()));

        // get_bill
        let bill = reader.get_bill("119-21").unwrap().unwrap();
        assert_eq!(bill.bill_id, "119-21");

        // get_annotations
        let anns = reader
            .get_annotations(&at("2025-07-18"), &at("2025-07-30"))
            .unwrap()
            .unwrap();
        assert_eq!(anns.len(), 753);

        // compute_diff
        let diff = reader
            .compute_diff(&at("2025-07-18"), &at("2025-07-30"))
            .unwrap();
        assert_eq!(diff.root_path, TITLE_7);

        // find_element, in the same order from either backend
        let found = reader.find_element(TITLE_7).unwrap();
        assert_eq!(
            found.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
            vec![at("2025-07-18"), at("2025-07-30")]
        );
    }

    // Test with Dataset
    check_reader(&dataset);

    // Test with SqliteStorage
    let storage = SqliteStorage::open(path).unwrap();
    check_reader(&storage);

    std::fs::remove_file(path).ok();
}

#[test]
fn should_load_window_with_two_expressions() {
    // Setup: save dataset with 3 expressions
    let mut dataset = make_test_dataset();
    dataset
        .add_expression(make_expression("2024-01-01", Some("V1")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-06-01", Some("V2")))
        .unwrap();
    dataset
        .add_expression(make_expression("2024-12-01", Some("V3")))
        .unwrap();

    for annotation in load_annotations() {
        dataset
            .add_annotation(&at("2024-01-01"), &at("2024-06-01"), annotation.clone())
            .unwrap();
        dataset
            .add_annotation(&at("2024-06-01"), &at("2024-12-01"), annotation)
            .unwrap();
    }

    let path = "/tmp/test_load_window.db";
    dataset.save_to_sqlite(path).unwrap();

    // Load window with just 2 expressions (returns InMemoryStorage)
    let storage = SqliteStorage::open(path).unwrap();
    let windowed = storage
        .load_window(&at("2024-01-01"), &at("2024-06-01"))
        .unwrap();

    // Should have exactly 2 expressions, of the one work
    assert_eq!(
        windowed
            .all_expressions()
            .map(|e| e.id.clone())
            .collect::<Vec<_>>(),
        vec![at("2024-01-01"), at("2024-06-01")]
    );

    // Should have annotations for that pair only
    assert!(
        windowed
            .diff_annotations
            .contains_key(&(at("2024-01-01"), at("2024-06-01")))
    );
    assert!(
        !windowed
            .diff_annotations
            .contains_key(&(at("2024-06-01"), at("2024-12-01")))
    );

    // Bills/members/sponsors should be empty (query from storage when needed)
    assert!(windowed.bills.is_empty());

    std::fs::remove_file(path).ok();
}

#[test]
fn should_query_annotations_for_path_via_trait() {
    let mut dataset = make_test_dataset();
    dataset
        .add_expression(make_expression("2025-07-18", None))
        .unwrap();
    dataset
        .add_expression(make_expression("2025-07-30", None))
        .unwrap();

    for annotation in load_annotations() {
        dataset
            .add_annotation(&at("2025-07-18"), &at("2025-07-30"), annotation)
            .unwrap();
    }

    let path = "/tmp/test_ann_path.db";
    dataset.save_to_sqlite(path).unwrap();

    // Test via trait - should work for both
    fn check_annotations_for_path(reader: &impl LinkReader) {
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
