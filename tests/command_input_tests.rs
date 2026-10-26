//! How a command reads its inputs: a bill, a work, a window and a path.
//!
//! Three faults of one kind, from a bug hunt on `eval.sqlite`:
//!
//! - **#392.** A bill, a work or an expression that the dataset does not hold
//!   gave "0 annotation(s)" and exit 0 in `annotations`, `section-agreement`,
//!   `path` and `expressions`. A window of two works was read as a window of
//!   the first, and a window with `--from` newer than `--to` was accepted.
//! - **#388.** A link is recorded for one window, and `annotations` and
//!   `coverage` matched only that exact pair of dates. A span of two windows
//!   gave 0, which reads as "no amendment changed this".
//! - **#395.** A path argument was used as typed. A trailing `/`, or an ASCII
//!   hyphen where the held section number has an en dash, matched nothing, and
//!   `cases-citing` then said that no opinion cites a section that 7 cite.
//!
//! The fixture is real: title 7 at the three committed release points, title 9
//! at two, the committed public law `119-hr-1`, and the title 7 links of the
//! committed matching run (`tests/test_data/processed/annotations.json`), each
//! recorded over the window 2025-07-18 -> 2025-07-30. Title 7 is the same on
//! 2025-07-30 and 2025-08-14, so the span 2025-07-18 -> 2025-08-14 holds the
//! same change as its first window.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::link::Link;

const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";
const LATEST: &str = "2025-08-14";
/// A date between the first two release points, which no expression holds.
const BETWEEN: &str = "2025-07-20";

const TITLE_7: &str = "uscode/title_7";
const TITLE_9: &str = "uscode/title_9";

const BILL: &str = "119-hr-1";
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const REAL_ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";

/// 7 U.S.C. 1308-1, as the publisher numbers it: with an en dash. The
/// committed run links three of its subparagraphs.
const SECTION_1308_1: &str = "uscode/title_7/chapter_35/subchapter_II/part_A/section_1308–1";
/// The same section, typed with an ASCII hyphen.
const SECTION_1308_1_HYPHEN: &str = "uscode/title_7/chapter_35/subchapter_II/part_A/section_1308-1";

/// § 174, which the committed opinions cite.
const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn at(work: &str, date: &str) -> String {
    format!("{work}@{date}")
}

/// Title 7 at three dates, title 9 at two, the bill, and the title 7 links.
fn dataset() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!("{}/command_input.sqlite", env!("CARGO_TARGET_TMPDIR"));
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata::default());
        for (title, date) in [
            ("usc07", EARLY),
            ("usc07", LATE),
            ("usc07", LATEST),
            ("usc09", EARLY),
            ("usc09", LATE),
        ] {
            dataset
                .add_uslm_xml(
                    &format!("tests/test_data/usc/{date}/{title}.xml"),
                    date,
                    None,
                )
                .expect("the corpus should parse and load");
        }
        dataset
            .load_bill_download(&committed_bill())
            .expect("the committed bill should load");

        let json =
            std::fs::read_to_string(REAL_ANNOTATIONS).expect("the fixture should be readable");
        let annotations: Vec<ChangeAnnotation> =
            serde_json::from_str(&json).expect("the fixture should parse as annotations");
        let from = ExpressionId::new(WorkId::new(TITLE_7), EARLY);
        let to = ExpressionId::new(WorkId::new(TITLE_7), LATE);
        for annotation in annotations {
            let mut annotation = annotation.clone();
            annotation
                .paths
                .retain(|path| path.starts_with(&format!("{TITLE_7}/")));
            if annotation.paths.is_empty() {
                continue;
            }
            annotation.source_bill.bill_id = BILL.to_string();
            for link in Link::from_annotation(&annotation, &from, &to) {
                dataset.add_link(link).expect("the link should be added");
            }
        }
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// Title 26 at one date and the committed opinions that cite it.
fn opinions_dataset() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/command_input_opinions.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);
        let mut dataset = Dataset::new(DatasetMetadata::default());
        dataset
            .add_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", EARLY, None)
            .expect("title 26 should parse and load");
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        let added = run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            "tests/test_data",
            "--opinions",
            "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879",
        ]);
        assert!(added.status.success(), "{}", stderr(&added));
        path
    })
}

fn committed_bill() -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: BILL.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: std::collections::HashMap::new(),
    }
}

/// Assert that a run stopped with exit 1, said `notice` on stderr, and gave no
/// answer on stdout.
fn assert_refused(args: &[&str], notice: &str) -> String {
    let output = run(args);
    let said = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "{args:?}: {said}");
    assert!(
        said.contains(notice),
        "{args:?} should say {notice:?}: {said}"
    );
    assert!(
        output.stdout.is_empty(),
        "{args:?} should print no answer: {}",
        stdout(&output)
    );
    said
}

// ---- #392: a bill, a work or a window that the dataset does not hold ----

#[test]
fn should_refuse_a_bill_not_held_when_annotations_section_agreement_or_amendment_addresses_is_given_one()
 {
    let ds = dataset();
    for command in ["annotations", "section-agreement", "amendment-addresses"] {
        assert_refused(
            &[command, ds, "--bill", "119-hr-9999"],
            "This dataset holds no bill 119-hr-9999.",
        );
        let said = assert_refused(
            &[command, ds, "--bill", "119-HR-1"],
            "This dataset holds no bill 119-HR-1.",
        );
        assert!(
            said.contains("Held bills with the number 1: 119-hr-1"),
            "{command}: {said}"
        );
    }
}

#[test]
fn should_answer_for_a_held_bill_when_annotations_is_given_one() {
    let output = run(&["annotations", dataset(), "--bill", BILL]);

    assert!(output.status.success(), "{}", stderr(&output));
    assert!(!stdout(&output).contains("\n0 annotation(s)"));
}

#[test]
fn should_refuse_a_work_not_held_when_expressions_is_given_one() {
    let said = assert_refused(
        &["expressions", dataset(), "--work", "uscode/title_99"],
        "This dataset holds no work uscode/title_99.",
    );
    assert!(said.contains(TITLE_7), "names the held works: {said}");
}

#[test]
fn should_refuse_and_name_the_held_dates_when_a_command_is_given_an_expression_not_held() {
    let ds = dataset();
    let from = at(TITLE_7, BETWEEN);
    let to = at(TITLE_7, LATE);
    for args in [
        vec!["annotations", ds, "--from", &from, "--to", &to],
        vec!["section-agreement", ds, "--from", &from, "--to", &to],
        vec!["path", ds, TITLE_7, "--from", &from, "--to", &to],
        vec!["diff", ds, "--from", &from, "--to", &to],
        vec!["coverage", ds, "--from", &from, "--to", &to],
    ] {
        let said = assert_refused(&args, &format!("This dataset holds no expression {from}."));
        assert!(
            said.contains(&format!(
                "Held dates of {TITLE_7}: {EARLY}, {LATE}, {LATEST}"
            )),
            "{args:?}: {said}"
        );
    }
}

#[test]
fn should_refuse_a_window_of_two_works() {
    let ds = dataset();
    let from = at(TITLE_7, EARLY);
    let to = at(TITLE_9, LATE);
    for args in [
        vec!["annotations", ds, "--from", &from, "--to", &to],
        vec!["section-agreement", ds, "--from", &from, "--to", &to],
        vec!["path", ds, TITLE_7, "--from", &from, "--to", &to],
        vec!["diff", ds, "--from", &from, "--to", &to],
        vec!["coverage", ds, "--from", &from, "--to", &to],
    ] {
        assert_refused(
            &args,
            &format!("--from {from} and --to {to} are expressions of two works."),
        );
    }
}

#[test]
fn should_refuse_a_window_whose_from_is_not_older_than_its_to() {
    let ds = dataset();
    let from = at(TITLE_7, LATE);
    let to = at(TITLE_7, EARLY);
    for args in [
        vec!["annotations", ds, "--from", &from, "--to", &to],
        vec!["section-agreement", ds, "--from", &from, "--to", &to],
        vec!["path", ds, TITLE_7, "--from", &from, "--to", &to],
        vec!["diff", ds, "--from", &from, "--to", &to],
        vec!["coverage", ds, "--from", &from, "--to", &to],
    ] {
        assert_refused(
            &args,
            &format!("--from {from} is not older than --to {to}."),
        );
    }
}

// ---- #388: a span of more than one window ----

#[test]
fn should_list_the_links_of_every_window_inside_a_span_when_annotations_is_given_a_span() {
    let ds = dataset();
    let listed = |to: &str| {
        let output = run(&[
            "annotations",
            ds,
            "--from",
            &at(TITLE_7, EARLY),
            "--to",
            &at(TITLE_7, to),
            "--path",
            SECTION_1308_1,
            "--all",
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        stdout(&output)
    };

    let one_window = listed(LATE);
    assert!(
        !one_window.contains("\n0 annotation(s)"),
        "the window holds links: {one_window}"
    );
    assert_eq!(listed(LATEST), one_window);
}

#[test]
fn should_count_the_links_of_every_window_inside_a_span_when_coverage_is_given_a_span() {
    let ds = dataset();
    let annotated = |to: &str| {
        let output = run(&[
            "coverage",
            ds,
            "--from",
            &at(TITLE_7, EARLY),
            "--to",
            &at(TITLE_7, to),
            "--json",
        ]);
        assert!(output.status.success(), "{}", stderr(&output));
        let report: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("the report should be JSON");
        report["annotated_count"]
            .as_u64()
            .expect("the report should count annotated paths")
    };

    let one_window = annotated(LATE);
    assert!(one_window > 0, "the window holds links");
    assert_eq!(annotated(LATEST), one_window);
}

// ---- #395: one spelling of a path argument ----

/// Each command that takes a path, asked about `path`.
fn each_path_command(ds: &'static str, path: &str) -> Vec<Vec<String>> {
    let from = at(TITLE_7, EARLY);
    let to = at(TITLE_7, LATE);
    [
        vec!["path", ds, path],
        vec!["path", ds, path, "--from", &from, "--to", &to],
        vec!["annotations", ds, "--path", path],
        vec!["search", ds, "payment", "--path", path],
        vec!["diff", ds, "--from", &from, "--to", &to, "--path", path],
        vec!["section-agreement", ds, "--path", path],
    ]
    .into_iter()
    .map(|args| args.into_iter().map(str::to_string).collect())
    .collect()
}

fn assert_same_answer(asked: &[String], held: &[String]) {
    let asked_output = run(&asked.iter().map(String::as_str).collect::<Vec<_>>());
    let held_output = run(&held.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(
        held_output.status.success(),
        "{held:?}: {}",
        stderr(&held_output)
    );
    assert_eq!(
        asked_output.status.code(),
        Some(0),
        "{asked:?}: {}",
        stderr(&asked_output)
    );
    assert_eq!(stdout(&asked_output), stdout(&held_output), "{asked:?}");
}

#[test]
fn should_give_the_answer_for_the_held_path_when_a_path_argument_ends_in_a_slash() {
    let ds = dataset();
    let slashed = format!("{SECTION_1308_1}/");
    for (asked, held) in each_path_command(ds, &slashed)
        .into_iter()
        .zip(each_path_command(ds, SECTION_1308_1))
    {
        assert_same_answer(&asked, &held);
    }
}

#[test]
fn should_give_the_answer_for_the_held_path_when_a_section_number_has_an_ascii_hyphen() {
    let ds = dataset();
    for (asked, held) in each_path_command(ds, SECTION_1308_1_HYPHEN)
        .into_iter()
        .zip(each_path_command(ds, SECTION_1308_1))
    {
        assert_same_answer(&asked, &held);
    }
}

#[test]
fn should_give_the_answer_for_the_held_path_when_cases_citing_is_given_a_path_with_a_slash() {
    let ds = opinions_dataset();
    let args = |path: &str| -> Vec<String> {
        ["cases-citing", ds, "--path", path]
            .into_iter()
            .map(str::to_string)
            .collect()
    };

    let held = run(&["cases-citing", ds, "--path", SECTION_174]);
    assert!(
        !stdout(&held).contains("No opinion in this dataset cites this path itself"),
        "the opinions cite § 174 itself: {}",
        stdout(&held)
    );
    assert_same_answer(&args(&format!("{SECTION_174}/")), &args(SECTION_174));
}
