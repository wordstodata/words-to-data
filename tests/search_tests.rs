//! Search must give the same answer whichever backend holds the dataset.
//!
//! A reader asks a search "is this text in the law". A backend that answers
//! "no" because it never indexed the field the text sits in is worse than one
//! that errors: the reader is told the law is absent (#82).

use tempfile::TempDir;
use words_to_data::dataset::{Dataset, DatasetMetadata, SearchResult};
use words_to_data::inspect;
use words_to_data::storage::{InMemoryStorage, SqliteStorage};

const USC09_18: &str = "tests/test_data/usc/2025-07-18/usc09.xml";
const USC09_30: &str = "tests/test_data/usc/2025-07-30/usc09.xml";
const USC26_18: &str = "tests/test_data/usc/2025-07-18/usc26.xml";

/// Text that appears in a `chapeau` in title 9.
const CHAPEAU_TEXT: &str = "In any of the following cases the United States court";

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Search Fixture".to_string(),
        description: "Real USC text, for search parity".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    }
}

/// Title 9 at both release points. Small, and it carries heading, chapeau and
/// content text.
fn title_9() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(metadata());
    for (xml, date) in [(USC09_18, "2025-07-18"), (USC09_30, "2025-07-30")] {
        dataset
            .add_uslm_xml(xml, date, None)
            .expect("the fixture should parse");
    }
    dataset
}

/// One expression of title 26. It is the only fixture carrying `proviso` and
/// `continuation` text, and one expression is enough to search.
fn title_26() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(metadata());
    dataset
        .add_uslm_xml(USC26_18, "2025-07-18", None)
        .expect("the fixture should parse");
    dataset
}

/// Round-trip the fixture through SQLite. The caller must keep the returned
/// directory in scope: dropping it removes the database.
fn to_sqlite(fixture: &Dataset<InMemoryStorage>) -> (TempDir, Dataset<SqliteStorage>) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("dataset.sqlite");
    fixture.save_to_sqlite(&path).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&path).expect("open sqlite");
    (dir, sqlite)
}

/// The comparable shape of a hit: which field of which path matched.
fn hits(results: &[SearchResult]) -> Vec<(String, String, String)> {
    results
        .iter()
        .map(|r| (r.expression.to_string(), r.path.clone(), r.field.clone()))
        .collect()
}

#[test]
fn should_find_chapeau_text_on_both_backends_when_searching() {
    let memory = title_9();
    let (_dir, sqlite) = to_sqlite(&memory);

    let from_memory = inspect::search(&memory, CHAPEAU_TEXT).expect("search memory");
    let from_sqlite = inspect::search(&sqlite, CHAPEAU_TEXT).expect("search sqlite");

    assert!(
        from_memory.iter().any(|r| r.field == "chapeau"),
        "the fixture should hold this text in a chapeau, got {:?}",
        hits(&from_memory)
    );
    assert_eq!(
        hits(&from_sqlite),
        hits(&from_memory),
        "both backends should return the same hits for the same query"
    );
}

#[test]
fn should_find_proviso_and_continuation_text_on_both_backends_when_searching() {
    let memory = title_26();
    let (_dir, sqlite) = to_sqlite(&memory);

    // Real text from title 26. The proviso is the only one in the title.
    for (field, query) in [
        (
            "proviso",
            "Provided however, That an individual not a citizen",
        ),
        (
            "continuation",
            "a tax determined in accordance with the following table",
        ),
    ] {
        let from_memory = inspect::search(&memory, query).expect("search memory");
        let from_sqlite = inspect::search(&sqlite, query).expect("search sqlite");

        assert!(
            from_memory.iter().any(|r| r.field == field),
            "the fixture should hold this text in a {field}, got {:?}",
            hits(&from_memory)
        );
        assert_eq!(
            hits(&from_sqlite),
            hits(&from_memory),
            "both backends should agree on a {field} match"
        );
    }
}

#[test]
fn should_return_results_in_the_same_order_on_both_backends_when_a_query_matches_widely() {
    let memory = title_9();
    let (_dir, sqlite) = to_sqlite(&memory);

    // Title 9 is the Federal Arbitration Act, so this matches throughout it.
    let from_memory = inspect::search(&memory, "arbitration").expect("search memory");
    let from_sqlite = inspect::search(&sqlite, "arbitration").expect("search sqlite");

    assert!(
        from_memory.len() > 5,
        "this test needs a broad match to be meaningful, got {}",
        from_memory.len()
    );
    assert_eq!(
        hits(&from_sqlite),
        hits(&from_memory),
        "both backends should return the same hits in the same order"
    );
}

#[test]
fn should_return_the_same_results_when_the_same_query_runs_twice() {
    let memory = title_9();
    let (_dir, sqlite) = to_sqlite(&memory);

    for (label, first, second) in [
        (
            "memory",
            inspect::search(&memory, "arbitration").expect("search"),
            inspect::search(&memory, "arbitration").expect("search"),
        ),
        (
            "sqlite",
            inspect::search(&sqlite, "arbitration").expect("search"),
            inspect::search(&sqlite, "arbitration").expect("search"),
        ),
    ] {
        assert_eq!(
            hits(&first),
            hits(&second),
            "{label} should answer the same query the same way every time"
        );
    }
}

/// Forge the element index an older build wrote: heading and content only, and
/// no document position. The public API cannot produce one, because this build
/// always writes the full shape.
///
/// The caller must keep the returned directory in scope: dropping it removes the
/// database.
fn stale_index_dataset() -> (TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("stale.sqlite");

    let conn = rusqlite::Connection::open(&path).expect("open forged db");
    // The schema version must be current, or the version guard answers first
    // and this test stops asking about the search index at all.
    conn.execute_batch(&format!(
        "CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
         INSERT INTO schema_version (version) VALUES ({});
         CREATE TABLE element_index (
             work TEXT NOT NULL, date TEXT NOT NULL, path TEXT NOT NULL,
             element_type TEXT, heading TEXT, content TEXT,
             PRIMARY KEY (work, date, path)
         );",
        words_to_data::storage::SCHEMA_VERSION
    ))
    .expect("forge the older index");
    drop(conn);
    (dir, path)
}

#[test]
fn should_refuse_a_dataset_whose_search_index_predates_this_build() {
    let (_dir, path) = stale_index_dataset();

    let opened = Dataset::open_sqlite(&path);

    let error = opened
        .err()
        .expect("a dataset with the older index should not open");
    let message = error.to_string();

    // Answering from two fields of five would report law as absent, which is
    // the failure this refusal exists to prevent. It must also say what to do.
    assert!(
        message.contains("search index"),
        "the error should name the search index, got: {message}"
    );
    assert!(
        message.contains("convert-dataset"),
        "the error should name the command that regenerates it, got: {message}"
    );
    assert!(
        !message.contains("no such column"),
        "the raw SQL error should not reach the reader, got: {message}"
    );
}
