//! End-to-end tests over `add-bills`, driving the real binary (#272).
//!
//! A dataset could grow in time (`add-release-points`) and not in law: only
//! `build-dataset --bills` loaded a bill, and it only made a new dataset. A law
//! enacted after the build could not come in without a rebuild, and a rebuild
//! discards every review.
//!
//! The fixture is title 7 at its three committed release points and the public
//! law `119-hr-1`, which renumbers provisions of title 7. Both are read from one
//! cache directory, as a real run reads them: the release points under
//! `<cache>/uslm/<date>` and the Congress responses under `<cache>/bill`,
//! `<cache>/house-vote` and `<cache>/member`. `--offline` keeps every test off
//! the network and away from an API key.

use std::path::Path;
use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, Format, WorkId};
use words_to_data::link::LinkKind;
use words_to_data::storage::LinkReader;

/// The three committed release points of title 7.
const DATES: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];

/// The committed Congress cache. It holds `119-hr-1`, and `119-hr-42` for
/// `outdated_link_tests.rs`.
const CONGRESS_CACHE: &str = "tests/test_data/congress_client_cache";

const BILL: &str = "119-hr-1";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn succeeded(output: &Output, command: &str) {
    assert!(
        output.status.success(),
        "{command} should exit zero, stderr: {}",
        stderr_of(output)
    );
}

/// A path of its own for each test, under cargo's scratch directory.
fn scratch(name: &str) -> String {
    format!("{}/add_bills_{name}", env!("CARGO_TARGET_TMPDIR"))
}

/// Copy a folder and everything under it.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the copy's folder should be creatable");
    for entry in std::fs::read_dir(from).expect("the folder should be readable") {
        let entry = entry.expect("the entry should be readable");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("the file should copy");
        }
    }
}

/// One cache directory holding title 7 at every committed date and the
/// committed Congress responses, laid out as a download leaves them.
fn cache_for(name: &str) -> String {
    let cache = scratch(&format!("{name}_cache"));
    let _ = std::fs::remove_dir_all(&cache);
    copy_tree(Path::new(CONGRESS_CACHE), Path::new(&cache));
    for date in DATES {
        let folder = format!("{cache}/uslm/{date}");
        std::fs::create_dir_all(&folder).expect("the cache folder should be creatable");
        std::fs::copy(
            format!("tests/test_data/usc/{date}/usc07.xml"),
            format!("{folder}/usc07.xml"),
        )
        .expect("the corpus should hold title 7 at this date");
    }
    cache
}

/// A dataset file as one JSON value, in which the order of an object's keys
/// does not count.
fn json_of(path: &str) -> serde_json::Value {
    let text = std::fs::read_to_string(path).expect("the dataset should be readable");
    serde_json::from_str(&text).expect("the dataset should be JSON")
}

/// `build-dataset` over every committed date, offline, with the bills named.
fn build(cache: &str, output: &str, bills: &[&str]) {
    let dates = DATES.join(",");
    let mut args = vec![
        "build-dataset",
        "--uslm-dates",
        &dates,
        "--offline",
        "--cache-dir",
        cache,
        output,
    ];
    let bills = bills.join(",");
    if !bills.is_empty() {
        args.extend(["--bills", &bills]);
    }
    succeeded(&run(&args), "build-dataset");
}

/// The point of the ticket: a law comes into a dataset that is already there,
/// and the dataset is the one a build with that law would have made.
///
/// Compared as whole JSON values, so the bill, its public-law expression, its
/// sponsors, its members, its House votes, its renumbering links and the record
/// of the method that wrote them are all the same. Not byte for byte: a bill's
/// amendments are a hash map, so their order in the file changes from one run
/// to the next, and two builds with the bill differ in the same way. The checks
/// after the comparison make sure the dataset holds each of those things, so
/// the comparison is not of two empty datasets.
#[test]
fn should_hold_what_a_build_with_the_bill_holds_when_the_bill_is_added_afterwards() {
    let cache = cache_for("equals_build");
    let without = scratch("equals_build_without.json");
    let grown = scratch("equals_build_grown.json");
    let with = scratch("equals_build_with.json");

    build(&cache, &without, &[]);
    let output = run(&[
        "add-bills",
        &without,
        "--bills",
        BILL,
        "--offline",
        "--cache-dir",
        &cache,
        "--output",
        &grown,
    ]);
    succeeded(&output, "add-bills");
    build(&cache, &with, &[BILL]);

    assert!(
        json_of(&grown) == json_of(&with),
        "a dataset that took the bill afterwards should hold what a dataset \
         built with it holds"
    );

    let dataset = Dataset::load(&grown, Format::Compact).expect("the dataset should load");
    assert!(dataset.get_bill(BILL).unwrap().is_some(), "the bill");
    let document = dataset
        .bill_document(BILL)
        .unwrap()
        .expect("the public-law document");
    assert_eq!(
        document.id.to_string(),
        "publiclawdocument_119-21@2025-07-04",
        "the public law is stored as an expression dated on its enactment"
    );
    assert!(dataset.get_member("A000375").unwrap().is_some(), "members");
    assert_eq!(
        dataset
            .get_bill_votes(BILL)
            .unwrap()
            .unwrap()
            .roll_calls
            .len(),
        1,
        "the House vote"
    );
    assert!(
        !dataset
            .links_by_kind(LinkKind::REDESIGNATED_AS)
            .unwrap()
            .is_empty(),
        "the renumbering links"
    );
}

/// A run is idempotent: a bill the dataset holds already is not added again,
/// and the run says so rather than print a success that did nothing.
#[test]
fn should_add_nothing_and_say_so_when_the_dataset_already_holds_the_bill() {
    let cache = cache_for("already_held");
    let with = scratch("already_held_with.json");
    let again = scratch("already_held_again.json");
    build(&cache, &with, &[BILL]);

    let output = run(&[
        "add-bills",
        &with,
        "--bills",
        BILL,
        "--offline",
        "--cache-dir",
        &cache,
        "--output",
        &again,
    ]);
    succeeded(&output, "add-bills");

    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        said.contains(&format!("already holds {BILL}")),
        "the run should say the bill was held already: {said}"
    );
    assert!(
        json_of(&again) == json_of(&with),
        "a second add of the same bill should change nothing"
    );
}

/// A SQLite dataset changes in place, so a run that fails must fail before it
/// writes. `119-hr-1` is in the cache and `119-hr-2` is not: the run must not
/// store the first and then stop at the second.
#[test]
fn should_fail_by_name_and_leave_the_dataset_as_it_was_when_an_offline_bill_is_not_cached() {
    let cache = cache_for("not_cached");
    let without = scratch("not_cached_without.json");
    build(&cache, &without, &[]);
    let sqlite = scratch("not_cached.sqlite");
    let _ = std::fs::remove_file(&sqlite);
    Dataset::load(&without, Format::Compact)
        .expect("the dataset should load")
        .save_to_sqlite(&sqlite)
        .expect("the dataset should convert");
    let before = std::fs::read(&sqlite).expect("the database should be readable");

    let output = run(&[
        "add-bills",
        &sqlite,
        "--bills",
        "119-hr-1,119-hr-2",
        "--offline",
        "--cache-dir",
        &cache,
    ]);

    assert!(!output.status.success(), "the run should exit non-zero");
    let complaint = stderr_of(&output);
    assert!(
        complaint.contains("119-hr-2") && complaint.contains("cache"),
        "the run should name the bill the cache has not got: {complaint}"
    );
    assert!(
        std::fs::read(&sqlite).expect("the database should be readable") == before,
        "a run that failed should leave the dataset as it was"
    );
}

/// A SQLite dataset takes the bill in place, and the run names the steps that
/// read a new law. `add-bills` runs none of them, so a run that said nothing
/// more would read as a finished job.
#[test]
fn should_hold_the_bill_in_place_and_name_the_next_steps_when_a_sqlite_dataset_takes_a_bill() {
    let cache = cache_for("sqlite_in_place");
    let without = scratch("sqlite_in_place_without.json");
    build(&cache, &without, &[]);
    let sqlite = scratch("sqlite_in_place.sqlite");
    let _ = std::fs::remove_file(&sqlite);
    Dataset::load(&without, Format::Compact)
        .expect("the dataset should load")
        .save_to_sqlite(&sqlite)
        .expect("the dataset should convert");

    let output = run(&[
        "add-bills",
        &sqlite,
        "--bills",
        BILL,
        "--offline",
        "--cache-dir",
        &cache,
    ]);
    succeeded(&output, "add-bills");

    let dataset = Dataset::open_sqlite(&sqlite).expect("the database should open");
    assert!(
        dataset.get_bill(BILL).unwrap().is_some(),
        "the database should hold the bill"
    );
    let said = String::from_utf8_lossy(&output.stdout);
    for step in [
        format!("words_to_data add-classifications {sqlite}"),
        format!("words_to_data link-by-evidence {sqlite}"),
        format!("words_to_data residue {sqlite} --bill {BILL}"),
    ] {
        assert!(said.contains(&step), "the run should name `{step}`: {said}");
    }
}

/// A dataset named `.sqlite` is a SQLite file from the start (#288).
///
/// Every other command reads the extension to choose the backend, so a build
/// that wrote JSON under a SQLite name gave a file no command could open, and
/// a first run needed `convert-dataset` before anything else.
#[test]
fn should_write_a_sqlite_dataset_when_the_output_is_named_sqlite() {
    let cache = cache_for("sqlite_output");
    let output = scratch("sqlite_output.sqlite");
    let _ = std::fs::remove_file(&output);

    build(&cache, &output, &[]);

    let dataset = Dataset::open_sqlite(&output).expect("the output should be a SQLite dataset");
    let expressions = dataset
        .expressions(&WorkId::new("uscode/title_7"))
        .expect("the expressions should be readable");
    assert_eq!(
        expressions.len(),
        DATES.len(),
        "the dataset should hold title 7 at every date it was built from"
    );
}

/// A bill load says how many renumberings it could not place, and where to read
/// them, in place of the statements themselves (#290).
///
/// Each statement and its clause, printed after a successful build, looks like
/// a failure to a first-time user. `redesignation-report` already lists them.
/// The fixture holds title 7 alone, so most of the bill's statements, which
/// renumber title 26 and others, cannot be placed.
#[test]
fn should_summarise_the_unplaced_renumberings_when_a_build_loads_a_bill() {
    let cache = cache_for("quiet_unplaced");
    let output = scratch("quiet_unplaced.json");
    let dates = DATES.join(",");

    let build = run(&[
        "build-dataset",
        "--uslm-dates",
        &dates,
        "--offline",
        "--cache-dir",
        &cache,
        "--bills",
        BILL,
        &output,
    ]);

    succeeded(&build, "build-dataset");
    let said = stderr_of(&build);
    assert!(
        said.contains(&format!("redesignation-report <dataset> --bill-id {BILL}")),
        "the build should name the command that lists them: {said}"
    );
    let detail: Vec<&str> = said
        .lines()
        .filter(|line| line.contains(" reader: "))
        .collect();
    assert!(
        detail.is_empty(),
        "the build should not print each unplaced statement: {detail:?}"
    );
}
