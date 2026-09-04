//! End-to-end tests that drive the `words_to_data` binary as a subprocess.
//!
//! These pin the behaviour that must survive the core refactor (#51). They assert
//! on `--json` output, because that is the surface an agent reads, and because it
//! survives a change to the human-readable text.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, VersionSnapshot};
use words_to_data::uslm::parser::parse;

/// The two US Code release points held in `tests/test_data`.
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// Title 9 (Arbitration) did not change between the two release points. Its two
/// files differ by ten bytes of release stamp, which the parser ignores.
const UNCHANGED_TITLE: &str = "usc09";

/// Title 51 (National and Commercial Space Programs) did change: it is the
/// smallest title in the corpus that carries real amendments between the two
/// release points, at 2.8 MB and +26 KB.
const AMENDED_TITLE: &str = "usc51";

/// Build a SQLite dataset holding one title at both release points.
fn build_fixture(title: &str) -> String {
    let path = format!("{}/cli_{title}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "CLI Test Fixture".to_string(),
        description: format!("{title} at two release points"),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    });

    for (date, label) in [(EARLY, "Before"), (LATE, "After")] {
        let xml = format!("tests/test_data/usc/{date}/{title}.xml");
        dataset
            .add_version(VersionSnapshot {
                date: date.to_string(),
                label: Some(label.to_string()),
                element: parse(&xml, date).expect("the corpus should parse"),
            })
            .expect("a version should be added");
    }

    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

/// A dataset over a title that really was amended. Built once per test binary.
fn amended_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| build_fixture(AMENDED_TITLE))
}

/// A dataset over a title that did not change. Built once per test binary.
fn unchanged_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| build_fixture(UNCHANGED_TITLE))
}

/// Run the CLI as a subprocess, the way an agent or a shell would.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

#[test]
fn should_report_metadata_and_counts_when_info_runs_on_a_dataset() {
    let output = run(&["info", amended_fixture(), "--json"]);

    assert!(
        output.status.success(),
        "info should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let info: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("info --json should emit json");

    assert_eq!(info["name"], "CLI Test Fixture");
    assert_eq!(info["version_count"], 2);
    assert_eq!(info["bill_count"], 0);
}

/// Run `diff` over a fixture and return its parsed JSON summary.
fn diff_summary(dataset: &str) -> serde_json::Value {
    let output = run(&["diff", dataset, "--from", EARLY, "--to", LATE, "--json"]);

    assert!(
        output.status.success(),
        "diff should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("diff --json should emit json")
}

/// Count the paths under one key of a diff summary.
fn path_count(summary: &serde_json::Value, key: &str) -> usize {
    summary[key].as_array().expect("an array of paths").len()
}

/// Title 51 gained 26 KB of text and 63 elements between the two release points.
/// Those are pure insertions, which `from_elements` used to discard (#54).
#[test]
fn should_list_the_paths_that_changed_between_two_versions_when_diff_runs() {
    let summary = diff_summary(amended_fixture());

    assert_eq!(summary["from_date"], EARLY);
    assert_eq!(summary["to_date"], LATE);

    let total = path_count(&summary, "changed_paths")
        + path_count(&summary, "added_paths")
        + path_count(&summary, "removed_paths");

    assert!(
        total > 0,
        "an amended title should produce at least one path, got {summary}"
    );
}

/// The two files of an unchanged title differ by ten bytes of release stamp.
/// A diff that reported those as changes would flood every real result with
/// noise, so this pins that it reports nothing.
#[test]
fn should_report_no_changes_when_only_the_release_stamp_differs() {
    let summary = diff_summary(unchanged_fixture());

    assert_eq!(
        (
            path_count(&summary, "changed_paths"),
            path_count(&summary, "added_paths"),
            path_count(&summary, "removed_paths"),
        ),
        (0, 0, 0)
    );
}
