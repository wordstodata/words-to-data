//! `info` groups what has run over a dataset, so the section says how many
//! methods and windows there are and not how many works (#209).
//!
//! A run is recorded once per work, which is correct: a run is a record of
//! something that happened, and a reader derives the summary it wants from the
//! full record
//! (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
//! The human view is that derivation. On a real corpus the ungrouped list is
//! 116 lines of the 141 `info` prints — two methods, one window, 58 works —
//! and it buries every other count the command reports.
//!
//! `info --json` keeps every individual run, because a machine reader makes its
//! own summary and cannot make one from a summary.

use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, opinion_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId};
use words_to_data::document::text_method;
use words_to_data::method::Method;
use words_to_data::storage::InMemoryStorage;

/// The two US Code release points held in `tests/test_data`.
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// Three of the smallest titles in the corpus. Each one is a work, and each is
/// published at both release points, so each gives one window.
const THREE_TITLES: [&str; 3] = ["usc04", "usc09", "usc27"];

/// Two opinions of the committed CourtListener cache, as `(opinion, cluster)`.
/// Both carry the court's own text layer, so one reading made both.
const TWO_OPINIONS: [(u64, u64); 2] = [(2812209, 2812209), (6248, 6248)];

/// The day the court filed each of those two opinions.
const OBERGEFELL_FILED: &str = "2015-06-26";
const HARRIS_FILED: &str = "1994-03-10";

/// A dataset holding the named titles at both release points, with no record of
/// anything that ran over them.
fn dataset_over(titles: &[&str]) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "What has run over this dataset".to_string(),
        description: "Small titles at two release points".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    for title in titles {
        for date in [EARLY, LATE] {
            let xml = format!("tests/test_data/usc/{date}/{title}.xml");
            dataset
                .add_uslm_xml(&xml, date, None)
                .unwrap_or_else(|error| panic!("{xml} should parse: {error}"));
        }
    }
    dataset
}

/// The same dataset, with both window methods this build really runs recorded
/// over every work it holds.
///
/// The methods are the ones step 4 and step 5 record (`README.md`), rather than
/// names invented here.
fn dataset_with_runs_over(titles: &[&str]) -> Dataset<InMemoryStorage> {
    let mut dataset = dataset_over(titles);
    let works = dataset.works().expect("storage should list its works");
    for work in works {
        let from = ExpressionId::new(work.clone(), EARLY);
        let to = ExpressionId::new(work, LATE);
        for method in [
            words_to_data::legislature::redesignation::reading_method(),
            words_to_data::matching::matching_method(),
        ] {
            dataset
                .record_method_run(method, &from, &to)
                .expect("the run should record");
        }
    }
    dataset
}

/// A dataset holding two court opinions, with the reading that made each
/// opinion's text recorded over the opinion it read, and that reading.
///
/// A court files an opinion once, so the work is one expression and the window
/// the reading ran over starts and ends on the day of the filing.
fn dataset_over_two_opinions() -> (Dataset<InMemoryStorage>, Method) {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "What has run over two opinions".to_string(),
        description: "Two opinions, as CourtListener returned them".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });

    let mut reading: Option<Method> = None;
    for (opinion_id, cluster_id) in TWO_OPINIONS {
        let read = |name: String| {
            std::fs::read_to_string(format!("tests/test_data/courtlistener/{name}"))
                .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
        };
        let opinion = OpinionRecord::from_json(&read(format!("opinion_{opinion_id}.json")))
            .expect("the opinion record should read");
        let cluster = ClusterRecord::from_json(&read(format!("cluster_{cluster_id}.json")))
            .expect("the cluster record should read");
        let (expression, source, _markup) =
            opinion_expression(&opinion, &cluster).expect("the opinion should become a document");

        // The method is the reading that made this text, named by the build
        // rather than by this test.
        let method = Method::new(source.method, text_method::VERSION);
        if let Some(first) = &reading {
            assert_eq!(
                first, &method,
                "the two opinions should be read the same way, \
                 or they are not one method over two windows"
            );
        }
        reading = Some(method.clone());

        let filed = expression.id.clone();
        dataset
            .add_expression(expression)
            .expect("an opinion should be storable");
        dataset
            .record_method_run(method, &filed, &filed)
            .expect("the run should record");
    }

    (dataset, reading.expect("two opinions were read"))
}

/// Save a dataset and run `info` against it the way a person would.
fn info_output(dataset: &Dataset<InMemoryStorage>) -> String {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.sqlite");
    dataset.save_to_sqlite(&file).expect("save to sqlite");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["info", file.to_str().expect("a utf-8 path")])
        .output()
        .expect("the binary should run");
    assert!(output.status.success(), "info should exit zero");
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Save a dataset and read what `info --json` says about it.
fn info_json(dataset: &Dataset<InMemoryStorage>) -> serde_json::Value {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.sqlite");
    dataset.save_to_sqlite(&file).expect("save to sqlite");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["info", file.to_str().expect("a utf-8 path"), "--json"])
        .output()
        .expect("the binary should run");
    assert!(output.status.success(), "info --json should exit zero");
    serde_json::from_slice(&output.stdout).expect("info --json should emit json")
}

/// The indented lines of the `Methods run` section, and nothing else.
fn methods_run_lines(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .skip_while(|line| *line != "Methods run:")
        .skip(1)
        .take_while(|line| line.starts_with("  "))
        .map(str::to_string)
        .collect()
}

/// Two methods over one window is two lines, whether three works or sixty ran
/// through them.
#[test]
fn should_print_one_line_for_each_method_when_the_same_window_ran_over_many_works() {
    let dataset = dataset_with_runs_over(&THREE_TITLES);

    let stdout = info_output(&dataset);
    let lines = methods_run_lines(&stdout);

    assert_eq!(
        lines.len(),
        2,
        "two methods over one window is two lines, got:\n{stdout}"
    );

    // Each line states the three facts a reader came for — which method, at
    // which version, over which window — and how much of the dataset it
    // covers. The method is spelled as one column, as a single run's line
    // spells it (`Method`'s `Display`).
    for method in [
        words_to_data::legislature::redesignation::reading_method(),
        words_to_data::matching::matching_method(),
    ] {
        let named = method.to_string();
        let line = lines
            .iter()
            .find(|line| line.contains(&named))
            .unwrap_or_else(|| panic!("no line names {named}, got:\n{stdout}"));
        assert!(
            line.contains(&format!("{EARLY} -> {LATE}")),
            "{named} should name its window, got:\n{line}"
        );
        assert!(
            line.contains("(3 works)"),
            "{named} should say how many works it covers, got:\n{line}"
        );
    }
}

/// Two windows of one method are two lines, and not one.
///
/// The two opinions were filed twenty-one years apart. A reader given one line
/// could not tell which of the two windows it named, and a line that names one
/// window and counts two works says something that is not true.
#[test]
fn should_print_a_line_for_each_window_when_one_method_ran_over_two_of_them() {
    let (dataset, reading) = dataset_over_two_opinions();

    let stdout = info_output(&dataset);
    let lines = methods_run_lines(&stdout);

    assert_eq!(
        lines.len(),
        2,
        "one method over two windows is two lines, got:\n{stdout}"
    );
    for filed in [OBERGEFELL_FILED, HARRIS_FILED] {
        let window = format!("{filed} -> {filed}");
        assert!(
            lines.iter().any(|line| line.contains(&window)),
            "no line names the window {window}, got:\n{stdout}"
        );
    }
    let named = reading.to_string();
    assert!(
        lines.iter().all(|line| line.contains(&named)),
        "both lines should name {named}, got:\n{stdout}"
    );
}

/// The section's length tracks the methods and the windows, and nothing else.
///
/// This is the whole point: a corpus grows a title at a time, and the list of
/// what has run over it must not grow with it.
#[test]
fn should_print_the_same_number_of_lines_when_a_dataset_holds_more_works() {
    let one = info_output(&dataset_with_runs_over(&THREE_TITLES[..1]));
    let three = info_output(&dataset_with_runs_over(&THREE_TITLES));

    assert_eq!(
        methods_run_lines(&one).len(),
        methods_run_lines(&three).len(),
        "one work and three works are the same two methods over the same \
         window, got:\n{one}\nand:\n{three}"
    );

    // The count is the one thing that does change, and a line that covers one
    // work says so in the singular.
    assert!(
        methods_run_lines(&one)
            .iter()
            .all(|line| line.contains("(1 work)")),
        "got:\n{one}"
    );
    assert!(
        methods_run_lines(&three)
            .iter()
            .all(|line| line.contains("(3 works)")),
        "got:\n{three}"
    );
}

/// `--json` carries every run, one for each work.
///
/// A machine reader makes the summary it wants out of the full record and
/// cannot make one out of a summary
/// (`docs/adr/0007-a-record-is-what-was-said-everything-else-is-derived.md`).
/// The human view is a rendering, and nothing stored changes with it.
#[test]
fn should_carry_every_individual_run_when_info_emits_json() {
    let dataset = dataset_with_runs_over(&THREE_TITLES);

    let info = info_json(&dataset);
    let runs = info["method_runs"]
        .as_array()
        .expect("info --json should carry the runs");

    assert_eq!(
        runs.len(),
        6,
        "two methods over three works is six runs, got:\n{runs:#?}"
    );
    for run in runs {
        assert!(run["method"]["name"].is_string(), "got:\n{run:#?}");
        assert_eq!(run["method"]["version"], 1, "got:\n{run:#?}");
        assert!(
            run["work"]
                .as_str()
                .is_some_and(|work| work.starts_with("uscode/title_")),
            "each run names the work it ran over, got:\n{run:#?}"
        );
        assert_eq!(run["from_date"], EARLY, "got:\n{run:#?}");
        assert_eq!(run["to_date"], LATE, "got:\n{run:#?}");
    }
}

/// A dataset that recorded nothing says nothing.
///
/// An empty list means nothing was recorded, which is every dataset built
/// before the record existed. It is not a statement that no method ran, and a
/// heading over no lines would read as one.
#[test]
fn should_print_no_methods_run_section_when_nothing_was_recorded() {
    let stdout = info_output(&dataset_over(&THREE_TITLES));

    assert!(
        !stdout.contains("Methods run"),
        "a dataset that recorded nothing prints no heading, got:\n{stdout}"
    );
}
