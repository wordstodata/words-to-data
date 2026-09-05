//! A dataset written by a different schema must be refused, not half-read.
//!
//! Datasets are rebuilt rather than migrated, so changing the schema is a clean
//! break. That is only safe if the break is loud. Without this check,
//! `CREATE TABLE IF NOT EXISTS` grafts the current build's tables onto an older
//! file, and every query against them returns nothing — an empty answer that
//! actually means "wrong schema", which is the failure this project exists to
//! prevent.
//!
//! Both on-disk forms are guarded. The JSON path fails the quiet way if it is
//! not: most of its fields default to empty, so an older file parses and simply
//! reports that it holds nothing.

use rusqlite::Connection;
use words_to_data::dataset::{
    Dataset, DatasetError, DatasetMetadata, Expression, ExpressionId, Format, WorkId, work_roots,
};
use words_to_data::storage::{DocumentReader, DocumentWriter, SqliteStorage};
use words_to_data::uslm::parser::parse;

const TITLE_9: &str = "tests/test_data/usc/2025-07-18/usc09.xml";
const EARLY: &str = "2025-07-18";

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Schema guard".to_string(),
        description: "Title 9 at one release point".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    }
}

/// Title 9 as one expression, split out of the parsed tree as the writer does.
fn title_9_expression() -> Expression {
    let parsed = parse(TITLE_9, EARLY).expect("the corpus should parse");
    let root = work_roots(parsed).pop().expect("the file holds one title");
    Expression {
        id: ExpressionId::new(WorkId::new(root.data.path.to_string()), EARLY),
        label: None,
        element: root,
    }
}

/// Write a real dataset to disk and hand back its path.
fn written_dataset(name: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);

    let mut storage = SqliteStorage::open(&path).expect("a new dataset should open");
    storage.set_metadata(metadata());
    storage
        .add_expression(title_9_expression())
        .expect("an expression should be added");
    drop(storage);

    path
}

#[test]
fn should_open_a_dataset_written_by_this_build() {
    let path = written_dataset("schema_current");

    let storage = SqliteStorage::open(&path).expect("the dataset should reopen");

    assert_eq!(storage.works().expect("works should list").len(), 1);
}

#[test]
fn should_refuse_a_dataset_written_by_a_different_schema() {
    let path = written_dataset("schema_stale");

    // Mark the file as written by an older build, the way a real one would be.
    let conn = Connection::open(&path).expect("the file should open directly");
    conn.execute("UPDATE schema_version SET version = 1", [])
        .expect("the version should update");
    drop(conn);

    match SqliteStorage::open(&path) {
        Err(DatasetError::SchemaVersionMismatch { found, expected }) => {
            assert_eq!(found, 1);
            assert_ne!(expected, 1, "this build should read a later schema");
        }
        Err(other) => panic!("the failure should name the schema, got {other}"),
        Ok(_) => panic!("a dataset from another schema must not open"),
    }
}

/// The refusal has to say what to do about it, because the answer is not
/// obvious: there is no migration, so the file must be rebuilt.
#[test]
fn should_explain_that_the_dataset_must_be_rebuilt() {
    let error = DatasetError::SchemaVersionMismatch {
        found: 1,
        expected: 2,
    };

    let message = error.to_string();
    assert!(message.contains("schema version 1"), "got: {message}");
    assert!(message.contains("regenerate"), "got: {message}");
}

// --- The JSON form is guarded too ---

/// Write a real compact-JSON dataset to disk and hand back its path.
fn written_json_dataset(name: &str) -> String {
    let path = format!("{}/{name}.json", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);

    let mut dataset = Dataset::new(metadata());
    dataset
        .add_expression(title_9_expression())
        .expect("an expression should be added");
    dataset
        .save(&path, Format::Compact)
        .expect("the dataset should save");

    path
}

#[test]
fn should_open_a_json_dataset_written_by_this_build() {
    let path = written_json_dataset("schema_current_json");

    let dataset = Dataset::load(&path, Format::Compact).expect("the dataset should reopen");

    assert_eq!(dataset.works().expect("works should list").len(), 1);
}

/// A file written before the schema field existed reads as version 0. It must
/// be refused rather than loading as a dataset that holds nothing — which is
/// exactly what it would do, because the rest of the fields default to empty.
#[test]
fn should_refuse_a_json_dataset_written_by_a_different_schema() {
    let path = written_json_dataset("schema_stale_json");

    let json = std::fs::read_to_string(&path).expect("the file should read");
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("it should be JSON");
    value
        .as_object_mut()
        .expect("the dataset should be an object")
        .remove("schema_version");
    std::fs::write(&path, value.to_string()).expect("the file should write");

    match Dataset::load(&path, Format::Compact) {
        Err(DatasetError::SchemaVersionMismatch { found, expected }) => {
            assert_eq!(found, 0, "a file with no schema field reads as 0");
            assert_ne!(expected, 0);
        }
        Err(other) => panic!("the failure should name the schema, got {other}"),
        Ok(_) => panic!("a dataset from another schema must not open"),
    }
}
