//! End-to-end tests over `add-release-points`, driving the real binary (#180).
//!
//! A dataset could not grow: every release point had to be named in one
//! `build-dataset` run, so one more printing of one title meant building
//! everything again. These tests pin what growth does.
//!
//! The fixture is title 1 of the US Code — the smallest title in the corpus — at
//! its two committed printings, 2025-07-18 and 2025-07-30. The command reads a
//! release point from the same cache a download fills, so a copy of the corpus
//! under `<cache>/uslm/<date>` is what a cached release point looks like, and
//! `--offline` keeps the test off the network.

use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::storage::InMemoryStorage;

/// Where the committed release points are.
const CORPUS: &str = "tests/test_data/usc";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Title 1 as it read on each date named, and nothing else.
fn title_1_at(dates: &[&str]) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Title 1 only".to_string(),
        description: "One title, so a run over it is quick".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    for date in dates {
        dataset
            .add_uslm_xml(&format!("{CORPUS}/{date}/usc01.xml"), date, None)
            .expect("title 1 should parse");
    }
    dataset
}

/// A SQLite dataset holding title 1 at one printing, at its own path per test.
fn sqlite_dataset(name: &str, dates: &[&str]) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    title_1_at(dates)
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

/// A compact JSON dataset holding title 1 at each printing named.
fn json_dataset(name: &str, dates: &[&str]) -> String {
    let path = format!("{}/{name}.json", env!("CARGO_TARGET_TMPDIR"));
    title_1_at(dates)
        .save(&path, Format::Compact)
        .expect("the fixture should save");
    path
}

/// A cache directory holding the release points named, as the command reads them.
///
/// A release point is cached at `<cache>/uslm/<date>`, so the corpus copied
/// there is exactly what a download would have left behind. Only title 1 is
/// copied: a release point is a folder of titles, and one title is a release
/// point of one title.
fn cache_holding(name: &str, dates: &[&str]) -> String {
    let cache = format!("{}/{name}_cache", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_dir_all(&cache);
    for date in dates {
        let folder = format!("{cache}/uslm/{date}");
        std::fs::create_dir_all(&folder).expect("the cache folder should be creatable");
        std::fs::copy(
            format!("{CORPUS}/{date}/usc01.xml"),
            format!("{folder}/usc01.xml"),
        )
        .expect("the corpus should hold this release point");
    }
    cache
}

/// The point of the ticket: one more printing, without a rebuild.
#[test]
fn should_hold_the_new_release_point_when_a_sqlite_dataset_grows() {
    let dataset = sqlite_dataset("grow_sqlite", &["2025-07-18"]);
    let cache = cache_holding("grow_sqlite", &["2025-07-30"]);

    let output = run(&[
        "add-release-points",
        &dataset,
        "--uslm-dates",
        "2025-07-30",
        "--offline",
        "--cache-dir",
        &cache,
    ]);

    assert!(
        output.status.success(),
        "add-release-points should exit zero, stderr: {}",
        stderr_of(&output)
    );

    let listed = stdout_of(&run(&["expressions", &dataset]));
    assert!(
        listed.contains("uscode/title_1@2025-07-30"),
        "the release point added should be held, got:\n{listed}"
    );
    assert!(
        listed.contains("uscode/title_1@2025-07-18"),
        "and the printing that was already there is still held, got:\n{listed}"
    );
}

/// A compact JSON dataset is written whole, so a run that wrote back over its
/// input would destroy it if the write stopped part way (#186).
#[test]
fn should_refuse_to_write_over_the_input_when_a_json_dataset_is_given_no_output() {
    let dataset = json_dataset("grow_json_no_output", &["2025-07-18"]);
    let cache = cache_holding("grow_json_no_output", &["2025-07-30"]);
    let before = std::fs::read(&dataset).expect("the fixture should be readable");

    let output = run(&[
        "add-release-points",
        &dataset,
        "--uslm-dates",
        "2025-07-30",
        "--offline",
        "--cache-dir",
        &cache,
    ]);

    assert!(
        !output.status.success(),
        "a write back over the input is refused, not done"
    );
    let complaint = stderr_of(&output);
    assert!(
        complaint.contains("--output"),
        "the refusal should say what to do instead, got: {complaint}"
    );
    assert_eq!(
        std::fs::read(&dataset).expect("the input should still be there"),
        before,
        "the input dataset must not change"
    );
}

/// Told where to write, a compact JSON dataset grows into a new file and the
/// one it grew from is left as it was.
#[test]
fn should_write_the_grown_dataset_to_the_output_when_a_json_dataset_grows() {
    let dataset = json_dataset("grow_json", &["2025-07-18"]);
    let cache = cache_holding("grow_json", &["2025-07-30"]);
    let grown = format!("{}/grow_json_grown.json", env!("CARGO_TARGET_TMPDIR"));
    let before = std::fs::read(&dataset).expect("the fixture should be readable");

    let output = run(&[
        "add-release-points",
        &dataset,
        "--uslm-dates",
        "2025-07-30",
        "--offline",
        "--cache-dir",
        &cache,
        "--output",
        &grown,
    ]);

    assert!(
        output.status.success(),
        "add-release-points should exit zero, stderr: {}",
        stderr_of(&output)
    );

    let listed = stdout_of(&run(&["expressions", &grown]));
    assert!(
        listed.contains("uscode/title_1@2025-07-30")
            && listed.contains("uscode/title_1@2025-07-18"),
        "the grown dataset should hold both printings, got:\n{listed}"
    );
    assert_eq!(
        std::fs::read(&dataset).expect("the input should still be there"),
        before,
        "the dataset it grew from must not change"
    );
}
