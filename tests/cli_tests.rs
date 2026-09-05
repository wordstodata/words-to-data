//! End-to-end tests that drive the `words_to_data` binary as a subprocess.
//!
//! These pin the behaviour that must survive the core refactor (#51). They assert
//! on `--json` output, because that is the surface an agent reads, and because it
//! survives a change to the human-readable text.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};

/// The two US Code release points held in `tests/test_data`.
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// Title 9 (Arbitration) did not change between the two release points. Its two
/// files differ by ten bytes of release stamp, which the parser ignores.
const UNCHANGED_TITLE: &str = "usc09";
const UNCHANGED_WORK: &str = "uscode/title_9";

/// Title 51 (National and Commercial Space Programs) did change: it is the
/// smallest title in the corpus that carries real amendments between the two
/// release points, at 2.8 MB and +26 KB.
const AMENDED_TITLE: &str = "usc51";
const AMENDED_WORK: &str = "uscode/title_51";

/// The `work@date` form the CLI takes and prints.
fn expression(work: &str, date: &str) -> String {
    format!("{work}@{date}")
}

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
            .add_uslm_xml(&xml, date, Some(label.to_string()))
            .expect("the corpus should parse and load");
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

/// A dataset holding two documents that share no publication date: title 9 at
/// the earlier release point, title 51 at the later one.
///
/// This is the shape #69 exists for. Ten court opinions would look the same,
/// and the old global version list could not hold it at all.
fn two_works_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/cli_two_works.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Two Works".to_string(),
            description: "Two titles, no shared date".to_string(),
            author: "words_to_data tests".to_string(),
            source_urls: vec![],
            license: "MIT".to_string(),
            version: "1.0.0".to_string(),
        });

        for (title, date) in [(UNCHANGED_TITLE, EARLY), (AMENDED_TITLE, LATE)] {
            let xml = format!("tests/test_data/usc/{date}/{title}.xml");
            dataset
                .add_uslm_xml(&xml, date, None)
                .expect("the corpus should parse and load");
        }

        dataset
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
    assert_eq!(info["work_count"], 1, "two releases of one title, one work");
    assert_eq!(info["expression_count"], 2);
    assert_eq!(info["bill_count"], 0);

    // Scope answers "why did my query find nothing". Without it, an agent
    // reading this output cannot tell an absent provision from an absent title.
    // Each work carries its own dates, so a dataset that holds one work in
    // July and another in August cannot read as holding both in both.
    assert_eq!(info["scope"]["held"][0]["work"], AMENDED_WORK);
    assert_eq!(info["scope"]["held"][0]["dates"][0], EARLY);
    assert_eq!(info["scope"]["held"][0]["dates"][1], LATE);
}

/// Run `diff` over a fixture and return its parsed JSON summary.
fn diff_summary(dataset: &str, work: &str) -> serde_json::Value {
    let output = run(&[
        "diff",
        dataset,
        "--from",
        &expression(work, EARLY),
        "--to",
        &expression(work, LATE),
        "--json",
    ]);

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
fn should_list_the_paths_that_changed_between_two_expressions_when_diff_runs() {
    let summary = diff_summary(amended_fixture(), AMENDED_WORK);

    assert_eq!(summary["work"], AMENDED_WORK);
    assert_eq!(summary["from"], expression(AMENDED_WORK, EARLY));
    assert_eq!(summary["to"], expression(AMENDED_WORK, LATE));

    let total = path_count(&summary, "changed_paths")
        + path_count(&summary, "added_paths")
        + path_count(&summary, "removed_paths");

    assert!(
        total > 0,
        "an amended title should produce at least one path, got {summary}"
    );
}

#[test]
fn should_list_every_expression_with_its_element_count_when_expressions_runs() {
    let output = run(&["expressions", amended_fixture(), "--json"]);

    assert!(output.status.success(), "expressions should exit zero");

    let expressions: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("expressions --json should emit json");
    let expressions = expressions.as_array().expect("an array of expressions");

    assert_eq!(expressions.len(), 2);
    // The id is the form that goes straight back in on `--from` / `--to`.
    assert_eq!(expressions[0]["id"], expression(AMENDED_WORK, EARLY));
    assert_eq!(expressions[0]["work"], AMENDED_WORK);
    assert_eq!(expressions[0]["date"], EARLY);
    assert_eq!(expressions[0]["label"], "Before");
    assert_eq!(expressions[1]["id"], expression(AMENDED_WORK, LATE));
    assert_eq!(expressions[1]["label"], "After");

    // The later release carries the two sections title 51 gained, so it must
    // hold more elements than the earlier one.
    let count = |v: &serde_json::Value| v["element_count"].as_u64().expect("a count");
    assert!(
        count(&expressions[1]) > count(&expressions[0]),
        "the amended expression should hold more elements: {} then {}",
        count(&expressions[0]),
        count(&expressions[1])
    );
}

/// `--work` narrows the list to one document. On a one-work dataset it changes
/// nothing, which is the point: it is a filter, not a required argument.
#[test]
fn should_list_only_the_named_work_when_expressions_is_given_one() {
    let output = run(&[
        "expressions",
        amended_fixture(),
        "--work",
        AMENDED_WORK,
        "--json",
    ]);

    assert!(output.status.success(), "expressions should exit zero");

    let expressions: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("expressions --json should emit json");
    let expressions = expressions.as_array().expect("an array of expressions");

    assert_eq!(expressions.len(), 2);
    assert!(expressions.iter().all(|e| e["work"] == AMENDED_WORK));
}

#[test]
fn should_find_matching_text_across_expressions_when_search_runs() {
    let output = run(&["search", amended_fixture(), "space", "--json"]);

    assert!(output.status.success(), "search should exit zero");

    let hits: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("search --json should emit json");
    let hits = hits.as_array().expect("an array of hits");

    assert!(!hits.is_empty(), "title 51 should contain the word 'space'");

    for hit in hits {
        // A hit names the expression it was found in. A bare date could not
        // say which document the text belongs to.
        let work = hit["expression"]["work"].as_str().expect("a work");
        let date = hit["expression"]["at"].as_str().expect("a date");
        assert_eq!(work, AMENDED_WORK);
        assert!(
            date == EARLY || date == LATE,
            "a hit should name one of the two expressions, got {date}"
        );
        let path = hit["path"].as_str().expect("a path");
        assert!(
            path.starts_with("uscode/title_51"),
            "a hit should sit under the title in the dataset, got {path}"
        );
        assert!(
            !hit["snippet"].as_str().expect("a snippet").is_empty(),
            "a hit should carry a snippet"
        );
    }
}

/// A search that finds nothing must say so, rather than fail or return noise.
#[test]
fn should_find_nothing_when_search_has_no_match() {
    let output = run(&["search", amended_fixture(), "zzqqxnotarealterm", "--json"]);

    assert!(output.status.success(), "an empty search should exit zero");

    let hits: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("search --json should emit json");

    assert_eq!(hits.as_array().expect("an array of hits").len(), 0);
}

/// Nothing in this fixture is annotated, so every changed path is unannotated.
/// This pins the empty end of the scale: coverage must report the work left,
/// not zero work.
#[test]
fn should_report_every_changed_path_as_unannotated_when_nothing_is_annotated() {
    let output = run(&[
        "coverage",
        amended_fixture(),
        "--from",
        &expression(AMENDED_WORK, EARLY),
        "--to",
        &expression(AMENDED_WORK, LATE),
        "--json",
    ]);

    assert!(output.status.success(), "coverage should exit zero");

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("coverage --json should emit json");

    let changed = report["changed_path_count"].as_u64().expect("a count");
    assert!(
        changed > 0,
        "the amended title should have a change universe"
    );
    assert_eq!(report["annotated_count"], 0);
    assert_eq!(report["unannotated_count"], changed);
    assert_eq!(report["coverage"], 0.0);
}

#[test]
fn should_return_no_annotations_when_the_dataset_has_none() {
    let output = run(&[
        "annotations",
        amended_fixture(),
        "--from",
        &expression(AMENDED_WORK, EARLY),
        "--to",
        &expression(AMENDED_WORK, LATE),
        "--json",
    ]);

    assert!(output.status.success(), "annotations should exit zero");

    let annotations: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("annotations --json should emit json");

    assert_eq!(annotations.as_array().expect("an array").len(), 0);
}

/// `annotations` needs one of three filters. Asking for everything is a usage
/// error, and it exits 2 rather than panicking.
#[test]
fn should_exit_two_when_annotations_is_given_no_filter() {
    let output = run(&["annotations", amended_fixture(), "--json"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("filter"),
        "the error should say what is missing"
    );
}

/// These two pin that a bad request fails rather than reporting an empty
/// result. They assert failure, not a specific code, because the codes are not
/// yet distinct: most commands panic with 101 today. When #51 gives them
/// meaningful codes, these tests still hold.
#[test]
fn should_fail_when_the_dataset_file_does_not_exist() {
    let output = run(&["info", "no/such/dataset.sqlite", "--json"]);

    assert!(
        !output.status.success(),
        "a missing dataset must not look like an empty one"
    );
    assert!(!output.stderr.is_empty(), "the failure should be explained");
}

#[test]
fn should_fail_when_the_expression_is_unknown() {
    let output = run(&[
        "diff",
        amended_fixture(),
        "--from",
        &expression(AMENDED_WORK, "1999-01-01"),
        "--to",
        &expression(AMENDED_WORK, LATE),
        "--json",
    ]);

    assert!(
        !output.status.success(),
        "an unknown expression must not look like an empty diff"
    );
    assert!(!output.stderr.is_empty(), "the failure should be explained");
}

/// A diff across two works would compare unrelated documents and report the
/// whole of each as changed. It has to be refused, not answered.
#[test]
fn should_fail_when_a_diff_names_two_works() {
    let output = run(&[
        "diff",
        amended_fixture(),
        "--from",
        &expression(AMENDED_WORK, EARLY),
        "--to",
        &expression(UNCHANGED_WORK, LATE),
        "--json",
    ]);

    assert!(
        !output.status.success(),
        "a diff across two works must not be answered"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("one work"),
        "the failure should say why, got: {stderr}"
    );
}

/// A malformed expression is caught when the argument is parsed, so a typo
/// cannot become a lookup that quietly finds nothing.
#[test]
fn should_fail_when_an_expression_argument_is_malformed() {
    let output = run(&[
        "diff",
        amended_fixture(),
        "--from",
        "2025-07-18",
        "--to",
        &expression(AMENDED_WORK, LATE),
        "--json",
    ]);

    assert!(
        !output.status.success(),
        "a bare date no longer names anything"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("work") && stderr.contains("date"),
        "the failure should show the expected form, got: {stderr}"
    );
}

// --- Two documents that share no publication date ---
//
// The clause #69 is done when: a dataset can hold two documents with unrelated
// publication dates, and every command still answers correctly for both.

#[test]
fn should_report_each_work_with_its_own_dates_when_info_runs_on_two_works() {
    let output = run(&["info", two_works_fixture(), "--json"]);

    assert!(output.status.success(), "info should exit zero");
    let info: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("info --json should emit json");

    assert_eq!(info["work_count"], 2);
    assert_eq!(info["expression_count"], 2);

    // Each work carries only the date it was actually published on. Two flat
    // lists would report both works on both dates, which is false.
    let held = info["scope"]["held"].as_array().expect("held works");
    let dates_of = |work: &str| {
        held.iter()
            .find(|h| h["work"] == work)
            .map(|h| h["dates"].clone())
            .unwrap_or_else(|| panic!("{work} should be held"))
    };
    assert_eq!(dates_of(UNCHANGED_WORK), serde_json::json!([EARLY]));
    assert_eq!(dates_of(AMENDED_WORK), serde_json::json!([LATE]));
}

#[test]
fn should_list_both_works_when_expressions_runs_on_two_works() {
    let output = run(&["expressions", two_works_fixture(), "--json"]);

    assert!(output.status.success(), "expressions should exit zero");
    let expressions: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("expressions --json should emit json");
    let expressions = expressions.as_array().expect("an array of expressions");

    assert_eq!(expressions.len(), 2);
    let ids: Vec<&str> = expressions
        .iter()
        .map(|e| e["id"].as_str().expect("an id"))
        .collect();
    assert!(ids.contains(&expression(UNCHANGED_WORK, EARLY).as_str()));
    assert!(ids.contains(&expression(AMENDED_WORK, LATE).as_str()));
}

/// Each document is searchable, and every hit says which one it came from.
#[test]
fn should_attribute_search_hits_to_the_right_work_when_two_works_are_held() {
    let dataset = two_works_fixture();

    for (query, expected_work) in [("arbitration", UNCHANGED_WORK), ("space", AMENDED_WORK)] {
        let output = run(&["search", dataset, query, "--json"]);
        assert!(output.status.success(), "search should exit zero");

        let hits: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("search --json should emit json");
        let hits = hits.as_array().expect("an array of hits");

        assert!(!hits.is_empty(), "{expected_work} should contain {query:?}");
        assert!(
            hits.iter()
                .all(|h| h["expression"]["work"] == expected_work),
            "every hit for {query:?} should name {expected_work}"
        );
    }
}

/// The old global list would answer this by handing back the other document.
#[test]
fn should_report_no_second_expression_for_a_work_published_once() {
    let output = run(&[
        "expressions",
        two_works_fixture(),
        "--work",
        UNCHANGED_WORK,
        "--json",
    ]);

    assert!(output.status.success(), "expressions should exit zero");
    let expressions: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("expressions --json should emit json");

    assert_eq!(
        expressions.as_array().expect("an array").len(),
        1,
        "title 9 was published once here; title 51 is not a later reading of it"
    );
}

/// The two files of an unchanged title differ by ten bytes of release stamp.
/// A diff that reported those as changes would flood every real result with
/// noise, so this pins that it reports nothing.
#[test]
fn should_report_no_changes_when_only_the_release_stamp_differs() {
    let summary = diff_summary(unchanged_fixture(), UNCHANGED_WORK);

    assert_eq!(
        (
            path_count(&summary, "changed_paths"),
            path_count(&summary, "added_paths"),
            path_count(&summary, "removed_paths"),
        ),
        (0, 0, 0)
    );
}
