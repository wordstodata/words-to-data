//! End-to-end tests that drive the `words_to_data` binary as a subprocess.
//!
//! These pin the behaviour that must survive the core refactor (#51). They assert
//! on `--json` output, because that is the surface an agent reads, and because it
//! survives a change to the human-readable text.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::annotation::{
    AnnotationMetadata, AnnotationStatus, BillReference, ChangeAnnotation,
};
use words_to_data::congress::CongressClient;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::legislature::AmendingAction;
use words_to_data::link::Link;

/// The two US Code release points held in `tests/test_data`.
const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// Every statement a real matching run of H.R. 1 recorded.
const REAL_ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";

/// Section 163 of title 26, the section H.R. 1 changed more than any other.
/// Every statement recorded there sits beneath the section, because an
/// amendment acts on a subsection, paragraph, subparagraph or clause.
const SECTION_163: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_163";
/// No such section, and a raw string prefix of [`SECTION_163`].
const SECTION_16: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_16";

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
        ..Default::default()
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
            ..Default::default()
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

/// A compact-JSON dataset holding both titles at both release points.
///
/// JSON rather than SQLite because the annotation commands read that form, and
/// both titles at both dates because a corpus-wide run needs more than one work
/// to prove it covered them all.
fn both_works_json_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/cli_both_works.json", env!("CARGO_TARGET_TMPDIR"));

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Both Works".to_string(),
            description: "Two titles, both release points".to_string(),
            author: "words_to_data tests".to_string(),
            source_urls: vec![],
            license: "MIT".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        });

        for title in [UNCHANGED_TITLE, AMENDED_TITLE] {
            for date in [EARLY, LATE] {
                let xml = format!("tests/test_data/usc/{date}/{title}.xml");
                dataset
                    .add_uslm_xml(&xml, date, None)
                    .expect("the corpus should parse and load");
            }
        }

        dataset
            .save(&path, Format::Compact)
            .expect("the fixture should save");
        path
    })
}

/// The same two titles, but neither held at both dates.
fn two_works_json_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/cli_two_works.json", env!("CARGO_TARGET_TMPDIR"));

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Two Works".to_string(),
            description: "Two titles, no shared date".to_string(),
            author: "words_to_data tests".to_string(),
            source_urls: vec![],
            license: "MIT".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        });

        for (title, date) in [(UNCHANGED_TITLE, EARLY), (AMENDED_TITLE, LATE)] {
            let xml = format!("tests/test_data/usc/{date}/{title}.xml");
            dataset
                .add_uslm_xml(&xml, date, None)
                .expect("the corpus should parse and load");
        }

        dataset
            .save(&path, Format::Compact)
            .expect("the fixture should save");
        path
    })
}

/// A dataset carrying what this project produces as well as its input: one link
/// over a real provision, the bill behind it, and that bill's legislature facts
/// — its sponsor, the members of the House, and the roll call they voted in.
///
/// `info` on a dataset like this is how a person tells an annotated dataset from
/// a bare corpus.
fn annotated_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!("{}/cli_annotated.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Annotated Fixture".to_string(),
            description: "One title, one public law, one link".to_string(),
            author: "words_to_data tests".to_string(),
            source_urls: vec![],
            license: "MIT".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        });
        for date in [EARLY, LATE] {
            let xml = format!("tests/test_data/usc/{date}/{UNCHANGED_TITLE}.xml");
            dataset
                .add_uslm_xml(&xml, date, None)
                .expect("the corpus should parse and load");
        }

        // The committed download of one real bill. No API key and no expiry:
        // the fixtures must be read without the network and never deleted for
        // being old.
        let client = CongressClient::with_ttl(
            String::new(),
            Some("tests/test_data/congress_client_cache".to_string()),
            None,
        );
        let download = client
            .download_bill("119-hr-1")
            .expect("the cached bill should read");
        dataset
            .load_bill_download(&download)
            .expect("the download should load");

        let amendment_id = dataset
            .get_bill("119-hr-1")
            .expect("the bill should read")
            .expect("the download holds it")
            .amendments
            .keys()
            .next()
            .expect("a real public law carries amendments")
            .clone();
        let annotation = ChangeAnnotation {
            operation: AmendingAction::Strike,
            source_bill: BillReference {
                bill_id: "119-hr-1".to_string(),
                amendment_id,
                causative_text: "by striking 'foo'".to_string(),
            },
            paths: vec![format!("{UNCHANGED_WORK}/chapter_1/section_1")],
            metadata: AnnotationMetadata {
                status: AnnotationStatus::Pending,
                confidence: Some(0.9),
                annotator: "model:test".to_string(),
                timestamp: time::OffsetDateTime::UNIX_EPOCH,
                notes: None,
                reasoning: None,
            },
        };
        let from = ExpressionId::new(WorkId::new(UNCHANGED_WORK), EARLY);
        let to = ExpressionId::new(WorkId::new(UNCHANGED_WORK), LATE);
        for link in Link::from_annotation(&annotation, &from, &to) {
            dataset.add_link(link).expect("the link should be added");
        }

        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// A dataset carrying every statement a real matching run of H.R. 1 recorded.
///
/// Separate from [`annotated_fixture`], which holds one link on purpose so that
/// `info` can count each kind of fact exactly. This one holds 753 statements
/// over hundreds of paths, because a path filter is only tested by a real spread
/// of paths.
///
/// The documents are title 9 while the statements name title 26. A path filter
/// compares paths and never reads a document, so holding the amended title
/// itself would add a minute of parsing and prove nothing more; `inspect_tests`
/// says the same of the same fixture file.
fn matched_statements_fixture() -> &'static str {
    static FIXTURE: OnceLock<String> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let path = format!(
            "{}/cli_matched_statements.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Matched Statements".to_string(),
            description: "One title, plus the statements a real run recorded".to_string(),
            author: "words_to_data tests".to_string(),
            source_urls: vec![],
            license: "MIT".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        });

        for date in [EARLY, LATE] {
            let xml = format!("tests/test_data/usc/{date}/{UNCHANGED_TITLE}.xml");
            dataset
                .add_uslm_xml(&xml, date, None)
                .expect("the corpus should parse and load");
        }

        let file = std::fs::File::open(REAL_ANNOTATIONS).expect("open the recorded annotations");
        let annotations: Vec<ChangeAnnotation> =
            serde_json::from_reader(std::io::BufReader::new(file)).expect("read the annotations");

        let work = WorkId::new(UNCHANGED_WORK);
        let from = ExpressionId::new(work.clone(), EARLY);
        let to = ExpressionId::new(work, LATE);
        for annotation in &annotations {
            for link in Link::from_annotation(annotation, &from, &to) {
                dataset.add_link(link).expect("the link should be added");
            }
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

#[test]
fn should_name_the_link_kind_when_info_reports_links() {
    let output = run(&["info", annotated_fixture()]);

    assert!(
        output.status.success(),
        "info should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8_lossy(&output.stdout);

    // The kind is named in full, namespace included. A reader who meets a
    // namespace it does not know must see it named rather than folded into a
    // total (`docs/adr/0002-links-live-in-the-core.md`).
    assert!(
        text.contains("legislature.amended_by  1"),
        "the link breakdown should name the kind and its count, got:\n{text}"
    );
    assert!(text.contains("Links:       1"), "got:\n{text}");
    assert!(text.contains("Members:     432"), "got:\n{text}");
    assert!(text.contains("Votes:       432"), "got:\n{text}");
    assert!(text.contains("Roll calls:  1"), "got:\n{text}");
    assert!(text.contains("Sponsors:    1"), "got:\n{text}");
    // This dataset holds no evidence, so it says nothing about evidence.
    assert!(!text.contains("Replies:"), "got:\n{text}");
}

#[test]
fn should_carry_the_link_and_legislature_counts_when_info_emits_json() {
    let output = run(&["info", annotated_fixture(), "--json"]);
    let info: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("info --json should emit json");

    // `--json` is what an agent reads, so it carries the same counts.
    assert_eq!(info["link_count"], 1);
    assert_eq!(info["link_counts_by_kind"]["legislature.amended_by"], 1);
    assert_eq!(info["member_count"], 432);
    assert_eq!(info["member_vote_count"], 432);
    assert_eq!(info["roll_call_count"], 1);
    assert_eq!(info["sponsor_count"], 1);
    assert_eq!(info["bill_count"], 1);
    assert!(
        info.get("reply_count").is_none(),
        "a count of zero is left out"
    );
}

#[test]
fn should_omit_a_count_of_zero_when_info_runs_on_a_dataset_without_legislature() {
    let output = run(&["info", amended_fixture()]);
    let text = String::from_utf8_lossy(&output.stdout);

    // A dataset with no legislature extension must not grow a wall of zeroes.
    for label in [
        "Links:",
        "Replies:",
        "Members:",
        "Sponsors:",
        "Roll calls:",
        "Votes:",
    ] {
        assert!(
            !text.contains(label),
            "{label} is zero here and must not be printed, got:\n{text}"
        );
    }
    // The counts reported before this rule are still reported.
    assert!(text.contains("Works:       1"), "got:\n{text}");
    assert!(text.contains("Bills:       0"), "got:\n{text}");
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
/// Those are pure insertions, which `from_nodes` used to discard (#54).
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

/// The annotation list of a `path` or `annotations` run, as JSON.
fn annotation_list(args: &[&str]) -> Vec<serde_json::Value> {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?} should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should emit json");
    // `path` wraps its annotations in a report; `annotations` emits the list.
    let list = match json.get("annotations") {
        Some(annotations) => annotations,
        None => &json,
    };
    list.as_array().expect("an array").clone()
}

#[test]
fn should_report_the_annotations_beneath_a_section_when_path_names_a_section() {
    let dataset = matched_statements_fixture();

    let by_default = annotation_list(&["path", dataset, SECTION_163, "--json"]);
    let exactly_there = annotation_list(&["path", dataset, SECTION_163, "--exact", "--json"]);

    assert_eq!(
        by_default.len(),
        9,
        "naming a section must report the records held beneath it"
    );
    assert!(
        exactly_there.is_empty(),
        "--exact keeps the answer this section used to give"
    );
}

#[test]
fn should_report_the_annotations_beneath_a_section_when_annotations_is_given_one() {
    let dataset = matched_statements_fixture();

    let by_default = annotation_list(&["annotations", dataset, "--path", SECTION_163, "--json"]);
    let exactly_there = annotation_list(&[
        "annotations",
        dataset,
        "--path",
        SECTION_163,
        "--exact",
        "--json",
    ]);

    assert_eq!(
        by_default.len(),
        9,
        "both commands must answer a section the same way"
    );
    assert!(exactly_there.is_empty(), "--exact means exactly");
}

#[test]
fn should_not_report_a_longer_section_number_when_the_path_is_a_string_prefix() {
    let dataset = matched_statements_fixture();

    let shorter_number = annotation_list(&["annotations", dataset, "--path", SECTION_16, "--json"]);

    assert!(
        shorter_number.is_empty(),
        "§16 is not §163: a path matches whole segments, not characters"
    );
}

/// The subtree changes which annotations are listed, and nothing else. An agent
/// reading `--json` must not have to change how it reads the answer.
#[test]
fn should_keep_the_annotation_fields_when_reporting_a_subtree() {
    let dataset = matched_statements_fixture();

    let reported = annotation_list(&["annotations", dataset, "--path", SECTION_163, "--json"]);

    let first = reported.first().expect("a reported annotation");
    for field in [
        "work",
        "from",
        "to",
        "from_date",
        "to_date",
        "operation",
        "bill_id",
        "amendment_id",
        "causative_text",
        "status",
        "confidence",
        "annotator",
        "paths",
    ] {
        assert!(
            first.get(field).is_some(),
            "the annotation shape must not change, {field} is missing from {first}"
        );
    }
}

#[test]
fn should_say_which_paths_the_filter_matches_when_either_command_is_asked_for_help() {
    let annotations = String::from_utf8_lossy(&run(&["annotations", "--help"]).stdout).to_string();
    let path = String::from_utf8_lossy(&run(&["path", "--help"]).stdout).to_string();

    for help in [&annotations, &path] {
        assert!(
            help.contains("beneath"),
            "the help must say the filter takes the subtree, got: {help}"
        );
        assert!(
            help.contains("--exact"),
            "the help must offer the exact rule, got: {help}"
        );
    }
    assert!(
        annotations.contains("path itself"),
        "the exact rule must say what it matches, got: {annotations}"
    );
    assert!(
        path.contains("this path itself"),
        "the exact rule must say what it matches, got: {path}"
    );
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

// --- Covering a corpus, now that a diff is per work ---
//
// A diff used to span the whole `uscode` root, so one `score-amendments` run
// covered every title. It now covers one work, so the command has to loop.
// `score-amendments` is the deterministic half of the pipeline — no LLM — so it
// is where this behaviour is pinned.

/// Run `score-amendments` and return the parsed scores file it wrote.
fn scored(dataset: &str, span: &[&str], out_name: &str) -> (Output, serde_json::Value) {
    let out = format!("{}/{out_name}", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&out);

    let mut args = vec!["score-amendments", dataset];
    args.extend_from_slice(span);
    args.extend_from_slice(&["--output", &out]);
    let output = run(&args);

    let written = std::fs::read_to_string(&out).unwrap_or_else(|_| "null".to_string());
    (
        output,
        serde_json::from_str(&written).expect("the scores file should be json"),
    )
}

#[test]
fn should_cover_every_work_when_score_amendments_is_given_two_dates() {
    let (output, scores) = scored(
        both_works_json_fixture(),
        &["--between", EARLY, LATE],
        "scores_between.json",
    );

    assert!(
        output.status.success(),
        "score-amendments should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let entries = scores.as_array().expect("an array of scored works");
    let works: Vec<&str> = entries
        .iter()
        .map(|e| e["work"].as_str().expect("a work"))
        .collect();

    assert_eq!(
        works,
        vec![AMENDED_WORK, UNCHANGED_WORK],
        "one run should cover both documents"
    );
    // Each entry says which pair it came from, so the file is readable without
    // knowing the command line that produced it.
    assert_eq!(entries[0]["from"], expression(AMENDED_WORK, EARLY));
    assert_eq!(entries[0]["to"], expression(AMENDED_WORK, LATE));
}

#[test]
fn should_cover_one_work_when_score_amendments_is_given_one_pair() {
    let (output, scores) = scored(
        both_works_json_fixture(),
        &[
            "--from",
            &expression(AMENDED_WORK, EARLY),
            "--to",
            &expression(AMENDED_WORK, LATE),
        ],
        "scores_one.json",
    );

    assert!(output.status.success(), "score-amendments should exit zero");

    let entries = scores.as_array().expect("an array of scored works");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["work"], AMENDED_WORK);
}

/// A work held at only one of the two dates cannot be diffed between them.
/// Saying so is the point: a run that covered nothing and reported success
/// would read as having done the job.
#[test]
fn should_name_the_works_it_could_not_cover() {
    let (output, scores) = scored(
        two_works_json_fixture(),
        &["--between", EARLY, LATE],
        "scores_skipped.json",
    );

    assert!(output.status.success(), "score-amendments should exit zero");
    assert_eq!(
        scores.as_array().expect("an array").len(),
        0,
        "neither title spans both dates"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(UNCHANGED_WORK) && stderr.contains(AMENDED_WORK),
        "both skipped works should be named, got: {stderr}"
    );
    assert!(
        stderr.contains("nothing to do"),
        "an empty run should say so, got: {stderr}"
    );
}

#[test]
fn should_reject_a_span_that_names_both_forms() {
    let output = run(&[
        "score-amendments",
        both_works_json_fixture(),
        "--between",
        EARLY,
        LATE,
        "--from",
        &expression(AMENDED_WORK, EARLY),
        "--to",
        &expression(AMENDED_WORK, LATE),
    ]);

    assert!(!output.status.success(), "the two forms are exclusive");
}

#[test]
fn should_reject_a_span_that_names_neither_form() {
    let output = run(&["score-amendments", both_works_json_fixture()]);

    assert!(
        !output.status.success(),
        "one form or the other is required"
    );
}

/// A `--between` date is checked before any work starts, the way `--from` and
/// `--to` are by parsing an expression. Without it a typo is not an error: no
/// work is held on `not-a-date`, so every work is skipped and the run exits
/// zero having done nothing.
#[test]
fn should_reject_a_between_date_that_is_not_a_date() {
    let output = run(&[
        "score-amendments",
        both_works_json_fixture(),
        "--between",
        "not-a-date",
        LATE,
    ]);

    assert!(!output.status.success(), "a malformed date must not run");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("YYYY-MM-DD"),
        "the error should show the expected form, got: {stderr}"
    );
}

/// A real date that no work was published on is a fact about the data, not a
/// user error, so it reports and exits zero. A dataset of court opinions — one
/// expression per work — would legitimately span nothing.
#[test]
fn should_report_and_succeed_when_a_valid_span_covers_no_work() {
    let (output, scores) = scored(
        both_works_json_fixture(),
        &["--between", "1999-01-01", LATE],
        "scores_empty_span.json",
    );

    assert!(output.status.success(), "an empty span is not a failure");
    assert_eq!(scores.as_array().expect("an array").len(), 0);
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("nothing to do"),
        "it must say it did nothing"
    );
}

#[test]
fn should_score_amendments_when_the_dataset_is_sqlite() {
    let output = run(&[
        "score-amendments",
        amended_fixture(),
        "--from",
        &expression(AMENDED_WORK, EARLY),
        "--to",
        &expression(AMENDED_WORK, LATE),
        "--output",
        &format!("{}/sqlite_scores.json", env!("CARGO_TARGET_TMPDIR")),
    ]);

    // Scoring only reads the dataset, so it must work over either backend.
    // Handing it SQLite used to read the database as JSON and report
    // "stream did not contain valid UTF-8", which named neither cause nor cure.
    assert!(
        output.status.success(),
        "score-amendments should accept a SQLite dataset, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn should_explain_the_conversion_when_a_writing_command_is_given_sqlite() {
    // These two write back into the dataset, which SQLite does not yet support.
    // Refusing is fine; refusing without saying what to do next is not.
    let from = expression(AMENDED_WORK, EARLY);
    let to = expression(AMENDED_WORK, LATE);
    let invocations: [Vec<&str>; 2] = [
        vec![
            "match-amendments",
            amended_fixture(),
            "--from",
            &from,
            "--to",
            &to,
        ],
        vec!["extract-changes", amended_fixture()],
    ];

    for args in invocations {
        let command = args[0];
        let output = run(&args);

        assert!(
            !output.status.success(),
            "{command} should refuse a SQLite dataset"
        );

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("SQLite database"),
            "{command} should say the file is a SQLite database, got: {stderr}"
        );
        assert!(
            stderr.contains("convert-dataset"),
            "{command} should name the command that converts it, got: {stderr}"
        );
        assert!(
            !stderr.contains("valid UTF-8"),
            "{command} should not leak the raw decoding error, got: {stderr}"
        );
    }
}

/// Forge the element index an older build wrote, at a chosen path.
fn forge_stale_sqlite(name: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open forged db");
    conn.execute_batch(
        "CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
         INSERT INTO schema_version (version) VALUES (3);
         CREATE TABLE element_index (
             work TEXT NOT NULL, date TEXT NOT NULL, path TEXT NOT NULL,
             element_type TEXT, heading TEXT, content TEXT,
             PRIMARY KEY (work, date, path)
         );",
    )
    .expect("forge the older index");
    drop(conn);
    path
}

#[test]
fn should_replace_the_output_when_converting_onto_an_existing_database() {
    // Converting names an output. Writing into whatever sits at that path
    // leaves a mixture of two datasets, and an index from an older build cannot
    // even accept the rows — it failed with a raw SQL error about a missing
    // column, which is the state a reader reaches by refreshing their database.
    let occupied = forge_stale_sqlite("convert_onto_existing");

    let output = run(&["convert-dataset", both_works_json_fixture(), &occupied]);
    assert!(
        output.status.success(),
        "converting onto an existing database should replace it, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // The result must be a usable dataset, not a hybrid of the two.
    let info = run(&["info", &occupied, "--json"]);
    assert!(
        info.status.success(),
        "the converted database should open, stderr: {}",
        String::from_utf8_lossy(&info.stderr)
    );
    let parsed: serde_json::Value =
        serde_json::from_slice(&info.stdout).expect("info --json should emit json");
    assert!(
        parsed["expression_count"].as_i64().unwrap_or(0) > 0,
        "the converted database should hold the source's expressions"
    );
}

/// A dataset holding one work and one real public law, in both file formats.
///
/// `show-bill` needs a bill id, so something has to be able to hand one over.
fn bill_fixtures() -> &'static (String, String) {
    static FIXTURE: OnceLock<(String, String)> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let sqlite = format!("{}/cli_bills.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let json = format!("{}/cli_bills.json", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&sqlite);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Bills Fixture".to_string(),
            description: "One title and one public law".to_string(),
            author: "words_to_data tests".to_string(),
            source_urls: vec![],
            license: "MIT".to_string(),
            version: "1.0.0".to_string(),
            ..Default::default()
        });
        dataset
            .add_uslm_xml(
                &format!("tests/test_data/usc/{EARLY}/{UNCHANGED_TITLE}.xml"),
                EARLY,
                None,
            )
            .expect("the corpus should parse");
        let bill = words_to_data::uslm::bill_parser::parse_bill_amendments(
            "119-hr-1",
            "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml",
        )
        .expect("the bill should parse");
        dataset.add_bill(bill).expect("add bill");

        dataset.save_to_sqlite(&sqlite).expect("save sqlite");
        dataset.save(&json, Format::Compact).expect("save json");
        (sqlite, json)
    })
}

#[test]
fn should_list_bill_ids_so_show_bill_can_be_used() {
    let (sqlite, json) = bill_fixtures();

    for dataset in [sqlite, json] {
        let output = run(&["bills", dataset, "--json"]);
        assert!(
            output.status.success(),
            "bills should exit zero for {dataset}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let listed: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("bills --json should emit json");
        let bills = listed.as_array().expect("a list of bills");
        assert_eq!(bills.len(), 1, "the fixture holds one bill");
        assert_eq!(bills[0]["bill_id"], "119-hr-1");
        assert!(
            bills[0]["amendment_count"].as_u64().unwrap_or(0) > 0,
            "a real public law carries amendments"
        );
    }
}

#[test]
fn should_hand_show_bill_an_id_it_accepts() {
    let (sqlite, _) = bill_fixtures();

    // The point of the listing: an id read from it must work as an argument.
    let listed = run(&["bills", sqlite, "--json"]);
    let parsed: serde_json::Value = serde_json::from_slice(&listed.stdout).expect("json");
    let id = parsed[0]["bill_id"]
        .as_str()
        .expect("a bill id")
        .to_string();

    let shown = run(&["show-bill", sqlite, &id]);
    assert!(
        shown.status.success(),
        "show-bill should accept an id that `bills` listed, stderr: {}",
        String::from_utf8_lossy(&shown.stderr)
    );
}
