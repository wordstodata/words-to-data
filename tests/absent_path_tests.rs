//! A path that no expression in scope holds, said aloud (#305).
//!
//! `diff`, `search` and `annotations` gave "Changed (0)" or "0 match(es)" for a
//! guessed path, the same output as a real path with nothing to report. An agent
//! read the empty answer as proof that a section was not in the dataset, when
//! the section was held at another path.
//!
//! The check is one library helper, [`absent_path`], so every command that takes
//! a path asks the same question the same way.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::inspect::{AbsentPath, absent_path};
use words_to_data::query::{Locator, PathMatch};
use words_to_data::storage::{DocumentReader, InMemoryStorage, SqliteStorage};

/// The two release points the corpus holds.
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// Title 9 (Arbitration), held here at both release points.
const ARBITRATION: &str = "uscode/title_9";
/// Title 4 (Flag and Seal), held here at the earlier one only.
const FLAG: &str = "uscode/title_4";

/// Section 16 of title 9, which carries subsections beneath it.
const SECTION_16: &str = "uscode/title_9/chapter_1/section_16";

/// No such path: a guess of the kind an agent makes, under a real chapter.
const GUESSED: &str = "uscode/title_9/chapter_1/section_999/subsection_a";

/// Two works, one of them at two dates, built from the real release points.
fn two_works() -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (title, date) in [("usc09", EARLY), ("usc09", LATE), ("usc04", EARLY)] {
        dataset
            .add_uslm_xml(
                &format!("tests/test_data/usc/{date}/{title}.xml"),
                date,
                None,
            )
            .expect("the corpus should parse and load");
    }
    dataset
}

/// The same dataset in SQLite. `name` names this test's database file.
fn two_works_in_sqlite(name: &str) -> Dataset<SqliteStorage> {
    let path = format!("{}/absent_path_{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    two_works()
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    Dataset::open_sqlite(&path).expect("the fixture should open")
}

/// Ask one question of both backends, which must answer alike.
fn on_both_backends(name: &str, ask: impl Fn(&str, &dyn DocumentReader)) {
    ask("in-memory", &two_works());
    ask("sqlite", &two_works_in_sqlite(name));
}

#[test]
fn should_name_the_path_and_its_nearest_held_ancestor_when_no_expression_holds_it() {
    on_both_backends("guessed", |backend, reader| {
        let absent = absent_path(reader, &Locator::new().at_path(GUESSED, PathMatch::Subtree))
            .expect("the check should run");

        assert_eq!(
            absent,
            Some(AbsentPath {
                path: GUESSED.to_string(),
                nearest_held: Some("uscode/title_9/chapter_1".to_string()),
            }),
            "{backend}"
        );
    });
}

/// A real path gives no notice, so its empty answers read as they did before.
#[test]
fn should_find_nothing_absent_when_the_path_is_held_or_has_paths_beneath_it() {
    on_both_backends("held", |backend, reader| {
        for matching in [PathMatch::Subtree, PathMatch::Exact] {
            let found = absent_path(reader, &Locator::new().at_path(SECTION_16, matching))
                .expect("the check should run");
            assert_eq!(found, None, "{backend}, {matching:?}");
        }
        // A locator that names no path has nothing to be absent.
        assert_eq!(
            absent_path(reader, &Locator::new()).expect("the check should run"),
            None,
            "{backend}"
        );
    });
}

/// The scope is the command's: a path another work holds is still absent from
/// the work that was asked about. No part of a title 9 path is in title 4, so
/// there is no nearest held path to name.
#[test]
fn should_name_the_path_as_absent_when_only_a_work_outside_the_scope_holds_it() {
    on_both_backends("other_work", |backend, reader| {
        let in_flag = Locator::new()
            .in_work(FLAG)
            .in_window(EARLY, EARLY)
            .at_path(SECTION_16, PathMatch::Subtree);
        assert_eq!(
            absent_path(reader, &in_flag).expect("the check should run"),
            Some(AbsentPath {
                path: SECTION_16.to_string(),
                nearest_held: None,
            }),
            "{backend}"
        );

        let in_arbitration = Locator::new()
            .in_work(ARBITRATION)
            .in_window(LATE, LATE)
            .at_path(SECTION_16, PathMatch::Subtree);
        assert_eq!(
            absent_path(reader, &in_arbitration).expect("the check should run"),
            None,
            "{backend}"
        );
    });
}

/// The same two works as a file on disk, for the command. Built once.
fn cli_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/absent_path_cli.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);
        two_works()
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// Run the CLI as a subprocess, the way an agent or a shell would.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `search` still answers 0 matches and exits zero, because an empty search is
/// an answer. But the answer now says that no expression holds the path.
#[test]
fn should_say_no_expression_holds_the_path_when_search_is_given_a_guessed_path() {
    let output = run(&["search", cli_fixture(), "arbitration", "--path", GUESSED]);
    assert!(output.status.success(), "an empty search should exit zero");
    let text = stdout(&output);
    assert!(
        text.contains(&format!("No expression this search reads holds {GUESSED}")),
        "{text}"
    );
    assert!(
        text.contains("Nearest held path: uscode/title_9/chapter_1"),
        "{text}"
    );
    assert!(text.contains("0 match(es)"), "{text}");

    let output = run(&[
        "search",
        cli_fixture(),
        "arbitration",
        "--path",
        GUESSED,
        "--json",
    ]);
    assert!(output.status.success(), "an empty search should exit zero");
    let answer: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("search --json should emit json");
    assert_eq!(answer["total"], 0);
    assert_eq!(answer["absent_path"]["path"], GUESSED);
    assert_eq!(
        answer["absent_path"]["nearest_held"],
        "uscode/title_9/chapter_1"
    );
}

/// `--at` takes a date. A `work@date` used to be taken without a word, and it
/// matched no expression, so the search answered 0 matches.
#[test]
fn should_refuse_a_value_that_is_not_a_date_when_search_is_given_at() {
    let at = format!("{ARBITRATION}@{EARLY}");
    let output = run(&["search", cli_fixture(), "arbitration", "--at", &at]);
    assert!(
        !output.status.success(),
        "a value that is not a date should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("`{at}` is not a date")) && stderr.contains("YYYY-MM-DD"),
        "{stderr}"
    );
    assert!(stdout(&output).is_empty(), "no answer should be printed");
}

/// The two ends of a diff over title 9, which did not change between them.
fn diff_args<'a>(from: &'a str, to: &'a str, path: &'a str) -> Vec<&'a str> {
    vec![
        "diff",
        cli_fixture(),
        "--from",
        from,
        "--to",
        to,
        "--path",
        path,
    ]
}

/// "Changed (0)" over a guessed path read as proof that nothing changed there.
/// A diff that compared no provision has no answer, so it fails and says why.
#[test]
fn should_fail_and_name_the_path_and_both_expressions_when_diff_is_given_a_guessed_path() {
    let (from, to) = (
        format!("{ARBITRATION}@{EARLY}"),
        format!("{ARBITRATION}@{LATE}"),
    );
    let output = run(&diff_args(&from, &to, GUESSED));
    assert!(!output.status.success(), "{}", stdout(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&format!("Neither {from} nor {to} holds {GUESSED}")),
        "{stderr}"
    );
    assert!(
        stderr.contains("Nearest held path: uscode/title_9/chapter_1"),
        "{stderr}"
    );
    assert!(
        !stdout(&output).contains("Changed (0)"),
        "{}",
        stdout(&output)
    );
}

/// A real path where nothing changed still answers as it did: exit zero and an
/// empty list, because that empty list is the answer.
#[test]
fn should_list_no_change_and_exit_zero_when_diff_is_given_a_held_path_that_did_not_change() {
    let (from, to) = (
        format!("{ARBITRATION}@{EARLY}"),
        format!("{ARBITRATION}@{LATE}"),
    );
    let output = run(&diff_args(&from, &to, SECTION_16));
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Changed (0):"), "{text}");
    assert!(!text.contains("holds"), "{text}");
}

/// `annotations` over a guessed path says that no expression holds it, in the
/// text and in `--json`, beside the empty list.
#[test]
fn should_say_no_expression_holds_the_path_when_annotations_is_given_a_guessed_path() {
    let output = run(&["annotations", cli_fixture(), "--path", GUESSED]);
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(
        text.contains(&format!(
            "No expression in the dataset holds {GUESSED} or a path beneath it."
        )),
        "{text}"
    );
    assert!(
        text.contains("Nearest held path: uscode/title_9/chapter_1"),
        "{text}"
    );
    assert!(text.contains("0 annotation(s)"), "{text}");

    let output = run(&["annotations", cli_fixture(), "--path", GUESSED, "--json"]);
    assert!(output.status.success());
    let answer: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("annotations --json should emit json");
    assert_eq!(answer["total"], 0);
    assert_eq!(answer["absent_path"]["path"], GUESSED);
    assert_eq!(
        answer["absent_path"]["nearest_held"],
        "uscode/title_9/chapter_1"
    );
}

/// A real path with no match answers as it did: no notice, and no new field.
#[test]
fn should_answer_as_before_when_search_and_annotations_are_given_a_held_path() {
    let output = run(&[
        "search",
        cli_fixture(),
        "zzqqxnotarealterm",
        "--path",
        SECTION_16,
    ]);
    assert!(output.status.success());
    assert_eq!(stdout(&output), "0 match(es)\n");

    for command in ["search", "annotations"] {
        let mut args = vec![command, cli_fixture()];
        if command == "search" {
            args.push("zzqqxnotarealterm");
        }
        args.extend(["--path", SECTION_16, "--json"]);
        let output = run(&args);
        assert!(output.status.success(), "{command}");
        let answer: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("--json should emit json");
        assert_eq!(answer["total"], 0, "{command}");
        assert!(answer.get("absent_path").is_none(), "{command}: {answer}");
    }

    let output = run(&["annotations", cli_fixture(), "--path", SECTION_16]);
    assert!(output.status.success());
    assert_eq!(stdout(&output), "0 annotation(s)\n");
}
