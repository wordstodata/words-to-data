//! A work or a date that no expression holds, said aloud by `search` (#337).
//!
//! `search "tax" --work uscode/title_99` and `search "tax" --at 2025-12-01`
//! answered "0 fields, 0 occurrences" and exit 0: the same output as a held
//! work in which the word does not occur. With `--path` as well, the notice
//! blamed the path ("No expression this search reads holds <path>") when the
//! path was held and only the date was not.
//!
//! The check is one library helper, [`absent_scope`], beside [`absent_path`]:
//! it says which filter matched no expression, and what is held near it.
//!
//! [`absent_path`]: words_to_data::inspect::absent_path

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::inspect::{AbsentScope, absent_scope};
use words_to_data::query::{Locator, PathMatch};
use words_to_data::storage::{DocumentReader, InMemoryStorage, SqliteStorage};

/// The two release points the corpus holds.
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";
/// A date between them, which no expression holds.
const BETWEEN: &str = "2025-07-20";
/// A date after both, which no expression holds.
const AFTER: &str = "2025-12-01";

/// Title 9 (Arbitration), held here at both release points.
const ARBITRATION: &str = "uscode/title_9";
/// Title 4 (Flag and Seal), held here at the earlier one only.
const FLAG: &str = "uscode/title_4";
/// No such work.
const UNHELD_WORK: &str = "uscode/title_99";

/// Section 16 of title 9, held at both release points.
const SECTION_16: &str = "uscode/title_9/chapter_1/section_16";

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
    let path = format!("{}/absent_scope_{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
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
fn should_name_the_work_and_the_held_works_when_no_expression_is_of_the_work() {
    on_both_backends("work", |backend, reader| {
        let absent = absent_scope(reader, &Locator::new().in_work(UNHELD_WORK))
            .expect("the check should run");

        assert_eq!(
            absent,
            Some(AbsentScope::Work {
                work: UNHELD_WORK.to_string(),
                works_held: 2,
                held: vec![FLAG.to_string(), ARBITRATION.to_string()],
                beneath: vec![],
            }),
            "{backend}"
        );
    });
}

/// "uscode" is not a work. Each title is, so the notice names them.
#[test]
fn should_name_the_works_beneath_it_when_the_work_is_a_part_of_their_paths() {
    on_both_backends("beneath", |backend, reader| {
        let absent =
            absent_scope(reader, &Locator::new().in_work("uscode")).expect("the check should run");

        let Some(AbsentScope::Work { beneath, .. }) = absent else {
            panic!("{backend}: {absent:?}");
        };
        assert_eq!(
            beneath,
            vec![FLAG.to_string(), ARBITRATION.to_string()],
            "{backend}"
        );
    });
}

#[test]
fn should_name_the_date_and_the_held_dates_around_it_when_no_expression_is_of_the_date() {
    on_both_backends("date", |backend, reader| {
        let between = absent_scope(reader, &Locator::new().in_window(BETWEEN, BETWEEN))
            .expect("the check should run");
        assert_eq!(
            between,
            Some(AbsentScope::Date {
                date: BETWEEN.to_string(),
                work: None,
                path: None,
                held_before: Some(EARLY.to_string()),
                held_after: Some(LATE.to_string()),
            }),
            "{backend}"
        );

        let after = absent_scope(
            reader,
            &Locator::new()
                .in_work(ARBITRATION)
                .in_window(AFTER, AFTER)
                .at_path(SECTION_16, PathMatch::Subtree),
        )
        .expect("the check should run");
        assert_eq!(
            after,
            Some(AbsentScope::Date {
                date: AFTER.to_string(),
                work: Some(ARBITRATION.to_string()),
                path: None,
                held_before: Some(LATE.to_string()),
                held_after: None,
            }),
            "{backend}"
        );
    });
}

/// Title 9 is held at the later date and title 4 is not. The dates around it
/// are title 4's own.
#[test]
fn should_name_the_dates_of_the_work_when_another_work_holds_the_date() {
    on_both_backends("other_work", |backend, reader| {
        let absent = absent_scope(reader, &Locator::new().in_work(FLAG).in_window(LATE, LATE))
            .expect("the check should run");
        assert_eq!(
            absent,
            Some(AbsentScope::Date {
                date: LATE.to_string(),
                work: Some(FLAG.to_string()),
                path: None,
                held_before: Some(EARLY.to_string()),
                held_after: None,
            }),
            "{backend}"
        );
    });
}

/// Title 9 is held at the later date, so the date is held. Title 4 is not, so
/// no expression at that date holds a title 4 path: the date is the filter
/// that matched nothing, and the notice names the dates the path is held.
#[test]
fn should_name_the_dates_of_the_path_when_the_path_is_held_only_at_other_dates() {
    on_both_backends("path_date", |backend, reader| {
        let absent = absent_scope(
            reader,
            &Locator::new()
                .in_window(LATE, LATE)
                .at_path(FLAG, PathMatch::Subtree),
        )
        .expect("the check should run");
        assert_eq!(
            absent,
            Some(AbsentScope::Date {
                date: LATE.to_string(),
                work: None,
                path: Some(FLAG.to_string()),
                held_before: Some(EARLY.to_string()),
                held_after: None,
            }),
            "{backend}"
        );
    });
}

/// A held work and date give no notice, so their empty answers read as before.
#[test]
fn should_find_nothing_absent_when_the_work_and_the_date_are_held() {
    on_both_backends("held", |backend, reader| {
        for locator in [
            Locator::new(),
            Locator::new().in_work(ARBITRATION),
            Locator::new().in_window(LATE, LATE),
            Locator::new().in_work(ARBITRATION).in_window(LATE, LATE),
            // The path is the concern of `absent_path`, not of this check.
            Locator::new().at_path("uscode/title_9/no_such", PathMatch::Subtree),
        ] {
            assert_eq!(
                absent_scope(reader, &locator).expect("the check should run"),
                None,
                "{backend}: {locator:?}"
            );
        }
    });
}

/// The same two works as a file on disk, for the command. Built once.
fn cli_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/absent_scope_cli.sqlite", env!("CARGO_TARGET_TMPDIR"));
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

/// A search that finds nothing in a held work and date: the answer, exit 0.
fn search(filters: &[&str]) -> (String, serde_json::Value) {
    let mut args = vec!["search", cli_fixture(), "arbitration"];
    args.extend_from_slice(filters);
    let text = run(&args);
    assert!(text.status.success(), "an empty search should exit zero");

    args.push("--json");
    let json = run(&args);
    assert!(json.status.success(), "an empty search should exit zero");
    (
        String::from_utf8_lossy(&text.stdout).into_owned(),
        serde_json::from_slice(&json.stdout).expect("search --json should emit json"),
    )
}

/// A search over a work or a date not held is refused, as `expressions --work`
/// refuses it: exit 1, the notice on stderr, nothing on stdout, with and
/// without `--json` (#392, #425). Gives the notice.
fn refused(filters: &[&str]) -> String {
    let mut args = vec!["search", cli_fixture(), "arbitration"];
    args.extend_from_slice(filters);
    let mut notices = Vec::new();
    for json in [false, true] {
        if json {
            args.push("--json");
        }
        let output = run(&args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        assert!(
            output.stdout.is_empty(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
        notices.push(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    assert_eq!(notices[0], notices[1], "--json gives the same notice");
    notices.remove(0)
}

#[test]
fn should_refuse_and_name_the_held_works_when_search_is_given_a_work_not_held() {
    let notice = refused(&["--work", UNHELD_WORK]);

    assert!(
        notice.contains(&format!("This dataset holds no work {UNHELD_WORK}.")),
        "{notice}"
    );
    assert!(
        notice.contains(&format!("Held works: {FLAG}, {ARBITRATION}")),
        "{notice}"
    );
}

#[test]
fn should_refuse_and_name_the_dates_around_it_when_search_is_given_a_date_not_held() {
    let notice = refused(&["--at", BETWEEN]);

    assert!(
        notice.contains(&format!(
            "This dataset holds no expression dated {BETWEEN}."
        )),
        "{notice}"
    );
    assert!(
        notice.contains(&format!(
            "Held dates around it: {EARLY} before, {LATE} after"
        )),
        "{notice}"
    );
}

/// The path is held at both dates. Only the date is not, so the notice names
/// the date and does not blame the path.
#[test]
fn should_name_the_date_and_not_the_path_when_search_is_given_a_held_path_at_a_date_not_held() {
    let notice = refused(&["--work", ARBITRATION, "--at", AFTER, "--path", SECTION_16]);

    assert!(
        notice.contains(&format!(
            "This dataset holds no expression of {ARBITRATION} dated {AFTER}."
        )),
        "{notice}"
    );
    assert!(
        notice.contains(&format!("Held dates around it: {LATE} before, none after")),
        "{notice}"
    );
    assert!(
        !notice.contains("No expression this search reads holds"),
        "{notice}"
    );
}

#[test]
fn should_name_the_date_and_not_the_path_when_search_is_given_a_path_held_only_at_other_dates() {
    let notice = refused(&["--at", LATE, "--path", FLAG]);

    assert!(
        notice.contains(&format!(
            "No expression dated {LATE} holds {FLAG} or a path beneath it."
        )),
        "{notice}"
    );
    assert!(
        notice.contains(&format!("Held dates around it: {EARLY} before, none after")),
        "{notice}"
    );
}

/// A held work and date where the word does not occur answer as before.
#[test]
fn should_give_no_notice_when_search_finds_nothing_in_a_held_work_and_date() {
    let (text, json) = search(&["--work", FLAG, "--at", EARLY]);

    assert_eq!(text.trim(), "0 fields, 0 occurrences", "{text}");
    assert!(json.get("absent_scope").is_none(), "{json}");
}
