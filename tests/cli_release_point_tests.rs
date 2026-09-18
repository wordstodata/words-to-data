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

/// A release point makes a window, and the steps that resolve one are separate
/// commands. A run that added a printing and said nothing else would read as a
/// finished job, so it names the window it made and what that window holds.
#[test]
fn should_name_the_new_window_and_what_it_holds_when_a_dataset_grows() {
    let dataset = sqlite_dataset("grow_windows", &["2025-07-18"]);
    let cache = cache_holding("grow_windows", &["2025-07-30"]);

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
    let printed = stdout_of(&output);

    assert!(
        printed.contains("uscode/title_1  2025-07-18 -> 2025-07-30"),
        "the window the release point made should be named, got:\n{printed}"
    );
    assert!(
        printed.contains("no link"),
        "and a window nothing has run over holds no link, got:\n{printed}"
    );
    assert!(
        printed.contains("--between 2025-07-18 2025-07-30"),
        "with the span the next steps take, got:\n{printed}"
    );
}

/// The promise of the ticket: growing is not a lesser way of building.
///
/// A dataset that took its two printings one at a time is the same file as a
/// dataset that took both at once, byte for byte. The compact form keeps no
/// order of arrival and no count of release points, which is why no stored type
/// and no schema version changes to let a dataset grow.
#[test]
fn should_hold_the_same_bytes_as_a_rebuild_when_a_dataset_grew_one_step_at_a_time() {
    let dataset = json_dataset("grow_equals_rebuild", &["2025-07-18"]);
    let cache = cache_holding("grow_equals_rebuild", &["2025-07-30"]);
    let grown = format!(
        "{}/grow_equals_rebuild_grown.json",
        env!("CARGO_TARGET_TMPDIR")
    );

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

    let rebuilt = json_dataset("grow_equals_rebuild_rebuilt", &["2025-07-18", "2025-07-30"]);

    assert_eq!(
        std::fs::read(&grown).expect("the grown dataset should be readable"),
        std::fs::read(&rebuilt).expect("the rebuilt dataset should be readable"),
        "a dataset that grew and a dataset that was built hold the same bytes"
    );
}

/// The input is the input, however it is spelled.
///
/// `--output` naming the file the run reads is the same truncating write the
/// refusal exists to prevent, so it is refused in the same way.
#[test]
fn should_refuse_to_write_over_the_input_when_the_output_names_the_input() {
    let dataset = json_dataset("grow_json_same_output", &["2025-07-18"]);
    let cache = cache_holding("grow_json_same_output", &["2025-07-30"]);
    let before = std::fs::read(&dataset).expect("the fixture should be readable");
    // The same file, spelled two ways: as it was given, and with a step that
    // goes nowhere. Both name the dataset the run reads.
    let round_about = dataset.replace("/target/tmp/", "/target/tmp/./");

    for same in [dataset.clone(), round_about] {
        let output = run(&[
            "add-release-points",
            &dataset,
            "--uslm-dates",
            "2025-07-30",
            "--offline",
            "--cache-dir",
            &cache,
            "--output",
            &same,
        ]);

        assert!(
            !output.status.success(),
            "--output {same} names the input, so it is refused"
        );
        let complaint = stderr_of(&output);
        assert!(
            complaint.contains("will not write back over"),
            "the refusal should say why, got: {complaint}"
        );
        assert_eq!(
            std::fs::read(&dataset).expect("the input should still be there"),
            before,
            "the input dataset must not change"
        );
    }
}

/// A release point that is not there is named, not skipped.
///
/// A run that added nothing, said nothing and exited zero would read as a
/// dataset that grew. `build-dataset` builds what it can and reports what it
/// skipped; a run that grows a dataset stops instead.
#[test]
fn should_fail_by_name_when_a_release_point_is_not_cached_and_the_run_is_offline() {
    let dataset = sqlite_dataset("grow_missing", &["2025-07-18"]);
    let cache = cache_holding("grow_missing", &[]);

    let output = run(&[
        "add-release-points",
        &dataset,
        "--uslm-dates",
        "2025-08-13",
        "--offline",
        "--cache-dir",
        &cache,
    ]);

    assert!(
        !output.status.success(),
        "a release point that never arrived is not a success"
    );
    let complaint = stderr_of(&output);
    assert!(
        complaint.contains("2025-08-13") && complaint.contains("cache"),
        "the failure should name the release point and say why, got: {complaint}"
    );

    let listed = stdout_of(&run(&["expressions", &dataset]));
    assert!(
        !listed.contains("2025-08-13"),
        "and the dataset holds nothing from the run that failed, got:\n{listed}"
    );
}
