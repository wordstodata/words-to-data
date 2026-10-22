//! Opening a dataset that already exists must not write to it (#331).
//!
//! SLEUTH and the benchmark run two sessions on one database at the same time.
//! When each open wrote to the file, a read-only command beside another reader
//! failed with "database is locked".

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::Connection;
use words_to_data::dataset::{Dataset, DatasetMetadata};

/// A SQLite dataset that holds title 7 as of 2025-07-18, written into `dir`.
fn title_7_database(dir: &Path) -> PathBuf {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Title 7".to_string(),
        ..Default::default()
    });
    common::add_uslm_xml(
        &mut dataset,
        "tests/test_data/usc/2025-07-18/usc07.xml",
        "2025-07-18",
    );
    let path = dir.join("title-7.sqlite");
    dataset
        .save_to_sqlite(&path)
        .expect("the dataset should save");
    path
}

/// Run the CLI as a subprocess, the way an agent or a shell would.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

#[test]
fn should_answer_a_read_command_when_another_connection_holds_a_read_transaction() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = title_7_database(dir.path());

    // A second reader, as another SLEUTH session would be: a read transaction
    // that is open, and has read, so it holds a shared lock on the file.
    let other = Connection::open(&path).expect("a second connection");
    other.execute_batch("BEGIN").expect("a read transaction");
    let held: i64 = other
        .query_row("SELECT count(*) FROM expressions", [], |row| row.get(0))
        .expect("the second connection reads");
    assert_eq!(held, 1, "the dataset holds one expression of title 7");

    let output = run(&["info", path.to_str().unwrap(), "--json"]);

    assert!(
        output.status.success(),
        "info should exit zero beside another reader, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    other.execute_batch("COMMIT").expect("the read ends");
}

#[test]
fn should_wait_for_a_short_write_when_another_connection_holds_the_file() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = title_7_database(dir.path());

    // A writer, as `settle` would be, that holds the file for one second. An
    // exclusive lock keeps every reader out until it ends.
    let writer = Connection::open(&path).expect("a second connection");
    writer
        .execute_batch("BEGIN EXCLUSIVE")
        .expect("an exclusive lock");
    let release = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(1));
        writer.execute_batch("COMMIT").expect("the write ends");
    });

    let output = run(&["info", path.to_str().unwrap(), "--json"]);
    release.join().expect("the writer thread ends");

    assert!(
        output.status.success(),
        "info should wait for the write and exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn should_leave_the_file_unchanged_when_an_existing_dataset_is_opened() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = title_7_database(dir.path());
    let bytes_before = std::fs::read(&path).expect("the file reads");
    let modified_before = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .expect("the file has a modified time");

    // A later write would get a later time, so a write cannot hide inside
    // the clock's resolution.
    std::thread::sleep(std::time::Duration::from_millis(50));
    let opened = Dataset::open_sqlite(&path).expect("the dataset opens");
    drop(opened);
    let output = run(&["info", path.to_str().unwrap(), "--json"]);
    assert!(output.status.success(), "info should exit zero");

    let modified_after = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .expect("the file has a modified time");
    assert_eq!(
        modified_after, modified_before,
        "the modified time is the same"
    );
    assert!(
        std::fs::read(&path).expect("the file reads") == bytes_before,
        "the bytes are the same"
    );
}
