//! How `cases-citing` reports a change window, so that a reader cannot take a
//! true line for a wrong answer (#340).
//!
//! The fixture is real: three printings of title 26 from the committed corpus
//! (2025-07-18, 2025-07-30 and 2025-08-14), the ten committed CourtListener
//! records, and two later cases added by the `add-opinions` command offline:
//! *Mayo Clinic v. United States*, filed 2025-07-25, between the first two
//! printings, and *Bmw (Us) Holding Corporation v. United States*, filed
//! 2026-08-05, after every printing held. Between the first two printings Pub. L. 119-21 changed
//! the chapeau of § 174(a) and left the words of § 174(a)(1) as they were.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::document::TextContentField;
use words_to_data::judicial::reliance::{self, CitedProvision, Citing, CitingCase};
use words_to_data::storage::SqliteStorage;

const TITLE_26: [(&str, &str); 3] = [
    ("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18"),
    ("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30"),
    ("tests/test_data/usc/2025-08-14/usc26.xml", "2025-08-14"),
];

/// The ten opinions, and the two later cases.
const OPINIONS: &str =
    "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879,11409814,11107760";

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Three printings of title 26 and the opinions, built once for the file.
fn dataset_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/cases_citing_windows.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Title 26 and the opinions that cite it".to_string(),
            description: "Three printings of title 26, for how change windows are reported"
                .to_string(),
            author: "words_to_data tests".to_string(),
            license: "Public Domain".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        });
        for (file, date) in TITLE_26 {
            dataset
                .add_uslm_xml(file, date, None)
                .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
        }
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");

        let added = run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            CACHE,
            "--opinions",
            OPINIONS,
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store the opinions, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

fn dataset() -> Dataset<SqliteStorage> {
    Dataset::open_sqlite(dataset_path()).expect("the fixture should open")
}

/// The writing that cites, by its path.
fn work_of(case: &CitingCase) -> String {
    match &case.citing {
        Citing::Held { path, .. } => path.clone(),
        Citing::NotHeld { reference, .. } => reference.clone(),
    }
}

/// The one citation `work` makes at or under `asked`.
fn citation_by(asked: &str, work: &str) -> CitedProvision {
    let cases = reliance::cases_citing(&dataset(), asked).expect("the query should run");
    let case = cases
        .into_iter()
        .find(|case| work_of(case) == work)
        .unwrap_or_else(|| panic!("{work} cites {asked}"));
    assert_eq!(case.cites.len(), 1, "{work} has one link to {asked}");
    case.cites.into_iter().next().expect("one citation")
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "cases-citing should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

const PARAGRAPH_A_1: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a/paragraph_1";
const SUBSECTION_A: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";

/// Boeing wrote § 174(a)(1). Between the first two printings the words of
/// (a)(1) stayed the same, and the chapeau of (a) above them changed from
/// "specified" to "foreign" research or experimental expenditures. So (a)(1)
/// now speaks of other expenditures.
#[test]
fn should_name_the_provision_above_that_changed_when_the_cited_subdivision_is_unchanged() {
    let boeing = citation_by(SECTION_174, "judicial/case_122262/opinion_9434364");

    let window = &boeing.since.windows[0];
    assert_eq!(window.at, PARAGRAPH_A_1);
    assert!(!window.changed, "the words of (a)(1) did not change");
    assert_eq!(
        window.changed_above,
        vec![reliance::ChangedAbove {
            path: SUBSECTION_A.to_string(),
            fields: vec![TextContentField::Chapeau],
        }],
        "the chapeau of (a) changed above it"
    );

    let unchanged = &boeing.since.windows[1];
    assert!(
        unchanged.changed_above.is_empty(),
        "nothing above (a)(1) changed between 2025-07-30 and 2025-08-14"
    );
}

#[test]
fn should_say_the_chapeau_above_changed_on_the_unchanged_line() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--cites",
        "26 U.S.C. \u{a7} 174(a)(1)",
    ]));

    assert!(
        printed.contains(&format!(
            "2025-07-18 → 2025-07-30: unchanged at {PARAGRAPH_A_1}; \
             the chapeau of {SUBSECTION_A} changed"
        )),
        "got:\n{printed}"
    );
    assert!(
        printed.contains(&format!(
            "2025-07-30 → 2025-08-14: unchanged at {PARAGRAPH_A_1}\n"
        )),
        "got:\n{printed}"
    );
}

#[test]
fn should_carry_the_change_above_the_cited_subdivision_in_json() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        PARAGRAPH_A_1,
        "--json",
    ]));
    let answers: serde_json::Value =
        serde_json::from_str(&printed).unwrap_or_else(|error| panic!("{error}: {printed}"));
    let harris = answers[0]["cases"]
        .as_array()
        .expect("cases is a list")
        .iter()
        .find(|case| case["citing"]["held"]["path"] == "judicial/case_6248/opinion_6248")
        .expect("Harris wrote § 174(a)(1)");

    assert_eq!(
        harris["cites"][0]["since"]["windows"][0]["changed_above"],
        serde_json::json!([{ "path": SUBSECTION_A, "fields": ["chapeau"] }])
    );
}

const SECTION_24: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_A/section_24";

/// No opinion held cites § 24, and Pub. L. 119-21 changed it between the first
/// two printings. `--chain` has no window to walk, and that is not "nothing
/// changed".
#[test]
fn should_say_there_is_no_window_to_walk_and_list_the_changes_when_no_opinion_cites_the_path() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        SECTION_24,
        "--chain",
    ]));

    assert!(
        !printed.contains("Nothing changed"),
        "§ 24 did change, got:\n{printed}"
    );
    assert!(
        printed.contains("No opinion in this dataset cites it, so there is no window to walk."),
        "got:\n{printed}"
    );
    assert!(
        printed.contains(&format!("2025-07-18 → 2025-07-30: CHANGED at {SECTION_24}")),
        "the windows in which the provision changed are listed, got:\n{printed}"
    );
    assert!(
        !printed.contains("2025-07-30 → 2025-08-14: CHANGED"),
        "§ 24 did not change in the second window, got:\n{printed}"
    );
}

const SECTION_250: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VIII/section_250";
const BMW: &str = "judicial/case_10942231/opinion_11409814";

/// BMW was filed on 2026-08-05, and the last printing held is 2025-08-14. The
/// dataset holds no printing after the opinion, and "fewer than two" would let
/// a reader think it holds one.
#[test]
fn should_count_no_printing_after_the_opinion_when_the_opinion_is_later_than_every_printing() {
    let bmw = citation_by(SECTION_250, BMW);

    assert!(bmw.since.windows.is_empty());
    assert_eq!(bmw.since.printings_after, 0);
}

#[test]
fn should_say_it_holds_no_printing_after_the_opinion_when_the_opinion_is_later_than_every_printing()
{
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        SECTION_250,
    ]));

    assert!(
        printed.contains(
            "this dataset holds no printing of the cited work after the opinion, \
             so it cannot say whether the provision changed since"
        ),
        "got:\n{printed}"
    );
    assert!(!printed.contains("fewer than two"), "got:\n{printed}");
}

#[test]
fn should_carry_the_count_of_printings_after_the_opinion_in_json() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        SECTION_250,
        "--json",
    ]));
    let answers: serde_json::Value =
        serde_json::from_str(&printed).unwrap_or_else(|error| panic!("{error}: {printed}"));

    assert_eq!(answers[0]["cases"][0]["citing"]["held"]["path"], BMW);
    assert_eq!(
        answers[0]["cases"][0]["cites"][0]["since"]["printings_after"],
        0
    );
}

/// One printing of title 26, 2025-07-18, and *Snow v. Commissioner*, filed in
/// 1974. The dataset holds one printing after the opinion, and none to compare
/// it with.
fn one_printing_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/cases_citing_one_printing.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "One printing of title 26 and Snow".to_string(),
            description: "One printing, so no window".to_string(),
            author: "words_to_data tests".to_string(),
            license: "Public Domain".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        });
        let (file, date) = TITLE_26[0];
        dataset
            .add_uslm_xml(file, date, None)
            .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
        dataset
            .save_to_sqlite(&path)
            .expect("the fixture should save");

        let added = run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            CACHE,
            "--opinions",
            "109019",
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store Snow, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

#[test]
fn should_say_it_holds_one_printing_after_the_opinion_when_that_is_the_only_printing() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        one_printing_path(),
        "--path",
        SECTION_174,
    ]));

    assert!(
        printed.contains(
            "this dataset holds one printing of the cited work after the opinion, \
             and no printing before it to compare it with, so it cannot say whether \
             the provision changed since"
        ),
        "got:\n{printed}"
    );
}

const MAYO: &str = "judicial/case_10641173/opinion_11107760";
const SECTION_501: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_F/part_I/section_501";

/// Mayo Clinic was filed on 2025-07-25 and cites § 501(c)(3). The window
/// 2025-07-18 → 2025-07-30 starts before the opinion and ends after it, so a
/// change in it can be from before the decision or after it.
#[test]
fn should_mark_a_window_that_starts_before_the_opinion_when_the_opinion_falls_inside_it() {
    let mayo = citation_by(SECTION_501, MAYO);

    let windows: Vec<(&str, &str, bool)> = mayo
        .since
        .windows
        .iter()
        .map(|window| {
            (
                window.from.as_str(),
                window.to.as_str(),
                window.starts_before_opinion,
            )
        })
        .collect();
    assert_eq!(
        windows,
        vec![
            ("2025-07-18", "2025-07-30", true),
            ("2025-07-30", "2025-08-14", false),
        ]
    );
}

#[test]
fn should_carry_whether_a_window_starts_before_the_opinion_in_json() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        SECTION_501,
        "--json",
    ]));
    let answers: serde_json::Value =
        serde_json::from_str(&printed).unwrap_or_else(|error| panic!("{error}: {printed}"));
    let mayo = answers[0]["cases"]
        .as_array()
        .expect("cases is a list")
        .iter()
        .find(|case| case["citing"]["held"]["path"] == MAYO)
        .expect("Mayo cites § 501(c)(3)");

    let windows = &mayo["cites"][0]["since"]["windows"];
    assert_eq!(windows[0]["starts_before_opinion"], true);
    assert_eq!(windows[1]["starts_before_opinion"], false);
}

/// Snow, Boeing, the Boeing dissent and Harris wrote § 174(a)(1). Its words
/// did not change, and the chapeau of (a) above it did. The tally must not
/// read as "nothing changed under these four".
#[test]
fn should_count_apart_the_opinions_whose_subdivision_is_unchanged_under_a_changed_provision() {
    let printed = stdout_of(&run(&[
        "cases-citing",
        dataset_path(),
        "--path",
        PARAGRAPH_A_1,
    ]));

    assert!(
        printed.contains(
            "4 more cite a subdivision whose own words did not change, under a \
             provision above it that changed in a window this dataset covers."
        ),
        "got:\n{printed}"
    );
}
