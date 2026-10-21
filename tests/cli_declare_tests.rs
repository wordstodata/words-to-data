//! End-to-end tests over `declare`, driving the real binary (#293).
//!
//! Only `build-dataset --declaration` could set a declaration, so a dataset
//! built without one could never say "out of scope" apart from "not found".
//! `declare` sets the declaration of a dataset that is already there.
//!
//! The fixture is title 1 of the US Code, the smallest title in the corpus, at
//! one committed printing. The declaration names title 1, which the dataset
//! holds, and title 26, which it does not, so the gap is known before the run.
//! Every test writes only to its own copy under `CARGO_TARGET_TMPDIR`.

use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::storage::InMemoryStorage;

const TITLE_1: &str = "tests/test_data/usc/2025-07-18/usc01.xml";
const DATE: &str = "2025-07-18";

/// Title 1 is held. Title 26 is declared and not held, so it is a gap.
const DECLARATION: &str = r#"{
    "intends": ["uscode/title_1", "uscode/title_26"],
    "excludes": [],
    "dates": null,
    "namespaces": []
}"#;

/// The gap line `info` prints for [`DECLARATION`] over title 1.
const GAP_LINE: &str = "INCOMPLETE:  declared but not held: uscode/title_26";

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

/// Title 1 at one printing, with no declaration.
fn title_1() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Title 1 only".to_string(),
        description: "One title, so a run over it is quick".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_1, DATE, None)
        .expect("title 1 should parse");
    dataset
}

/// A SQLite dataset built without a declaration, at its own path per test.
fn sqlite_dataset(name: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    title_1()
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

/// A compact JSON dataset built without a declaration.
fn json_dataset(name: &str) -> String {
    let path = format!("{}/{name}.json", env!("CARGO_TARGET_TMPDIR"));
    title_1()
        .save(&path, Format::Compact)
        .expect("the fixture should save");
    path
}

/// A file holding `text`, at its own path per test.
fn file_holding(name: &str, text: &str) -> String {
    let path = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    std::fs::write(&path, text).expect("the file should write");
    path
}

#[test]
fn should_make_info_print_the_declaration_and_its_gap_when_a_sqlite_dataset_is_declared() {
    let dataset = sqlite_dataset("declare_sqlite");
    let declaration = file_holding("declare_sqlite.json", DECLARATION);

    let declared = run(&["declare", &dataset, &declaration]);
    assert!(
        declared.status.success(),
        "declare should succeed: {}",
        stderr_of(&declared)
    );

    let info = stdout_of(&run(&["info", &dataset]));
    assert!(
        info.contains("Declared:    uscode/title_1, uscode/title_26"),
        "info should print the declaration that declare set:\n{info}"
    );
    assert!(
        info.contains(GAP_LINE),
        "info should name the declared work the dataset does not hold:\n{info}"
    );
}

/// A compact JSON dataset is written whole, so it follows the rule of every
/// command that grows one: the result goes to `--output`, never over the input.
#[test]
fn should_make_info_print_the_declaration_and_its_gap_when_a_json_dataset_is_declared() {
    let dataset = json_dataset("declare_json");
    let declaration = file_holding("declare_json_declaration.json", DECLARATION);
    let output = format!("{}/declare_json_out.json", env!("CARGO_TARGET_TMPDIR"));

    let declared = run(&["declare", &dataset, &declaration, "--output", &output]);
    assert!(
        declared.status.success(),
        "declare should succeed: {}",
        stderr_of(&declared)
    );

    let info = stdout_of(&run(&["info", &output]));
    assert!(
        info.contains("Declared:    uscode/title_1, uscode/title_26"),
        "info should print the declaration that declare set:\n{info}"
    );
    assert!(
        info.contains(GAP_LINE),
        "info should name the declared work the dataset does not hold:\n{info}"
    );
}

/// A declaration that does not parse stops the run before anything is written,
/// as `build-dataset --declaration` does.
#[test]
fn should_fail_and_leave_the_dataset_unchanged_when_the_declaration_does_not_parse() {
    let dataset = sqlite_dataset("declare_bad_file");
    let before = std::fs::read(&dataset).expect("the fixture should read");
    let declaration = file_holding(
        "declare_bad_file.json",
        r#"{ "intends": ["uscode/title_1"], "excludes": "#,
    );

    let declared = run(&["declare", &dataset, &declaration]);

    assert!(
        !declared.status.success(),
        "a declaration that does not parse should fail the run"
    );
    assert!(
        stderr_of(&declared).contains("Error parsing declaration"),
        "the run should say why it stopped: {}",
        stderr_of(&declared)
    );
    assert_eq!(
        std::fs::read(&dataset).expect("the dataset should still read"),
        before,
        "the dataset bytes should not change"
    );
}

/// A second declaration takes the place of the first, and the run says so: a
/// statement someone made should not go without a word.
#[test]
fn should_replace_the_declaration_and_say_so_when_the_dataset_has_one() {
    let dataset = sqlite_dataset("declare_replace");
    let first = file_holding("declare_replace_first.json", DECLARATION);
    let second = file_holding(
        "declare_replace_second.json",
        r#"{
            "intends": ["uscode/title_1"],
            "excludes": [],
            "dates": null,
            "namespaces": []
        }"#,
    );

    let first_run = run(&["declare", &dataset, &first]);
    assert!(
        !stderr_of(&first_run).contains("Replaced"),
        "a dataset with no declaration has none to replace: {}",
        stderr_of(&first_run)
    );

    let second_run = run(&["declare", &dataset, &second]);
    assert!(
        second_run.status.success(),
        "declare should succeed: {}",
        stderr_of(&second_run)
    );
    assert!(
        stderr_of(&second_run).contains("Replaced the declaration"),
        "the run should say it replaced a declaration: {}",
        stderr_of(&second_run)
    );

    let info = stdout_of(&run(&["info", &dataset]));
    assert!(
        info.contains("Declared:    uscode/title_1\n"),
        "info should print the new declaration alone:\n{info}"
    );
    assert!(
        !info.contains("INCOMPLETE:"),
        "the new declaration names nothing the dataset does not hold:\n{info}"
    );
}

/// The person who declares sees any gap at once, in the lines `info` prints,
/// and does not have to run `info` to find it.
#[test]
fn should_print_the_lines_info_prints_for_the_declaration_when_it_writes() {
    let dataset = sqlite_dataset("declare_prints_gap");
    let declaration = file_holding("declare_prints_gap.json", DECLARATION);

    let declared = stdout_of(&run(&["declare", &dataset, &declaration]));
    let info = stdout_of(&run(&["info", &dataset]));

    let from_info = &info[info
        .find("Declared:")
        .expect("info should print the declaration")..];
    assert!(
        declared.contains(GAP_LINE),
        "declare should name the declared work the dataset does not hold:\n{declared}"
    );
    assert!(
        declared.ends_with(from_info),
        "declare should print the declared lines exactly as info does.\n\
         declare:\n{declared}\ninfo:\n{from_info}"
    );
}
