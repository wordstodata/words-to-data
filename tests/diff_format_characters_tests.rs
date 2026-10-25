//! A difference in format characters alone is not a change (#363).
//!
//! The fixture is real: title 13 and title 26 from the committed corpus, at the
//! printings 2025-07-18 and 2025-07-30. Between the two, 13 U.S.C. § 9(b) lost
//! the soft hyphen (U+00AD) in "public rec\u{ad}ords", and nothing else in title
//! 13 changed. In the same window Pub. L. 119-21 changed the chapeau of 26
//! U.S.C. § 174(a).
//!
//! The fixture also holds *Baldrige v. Shapiro*, 455 U.S. 345 (1982), from the
//! committed CourtListener records. It cites "13 U. S. C. §§8 and 9", so it
//! cites § 9 alone, and § 9 holds § 9(b).

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::search::IGNORED;

const PRINTINGS: [(&str, &str); 4] = [
    ("tests/test_data/usc/2025-07-18/usc13.xml", "2025-07-18"),
    ("tests/test_data/usc/2025-07-30/usc13.xml", "2025-07-30"),
    ("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18"),
    ("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30"),
];

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

/// *Baldrige v. Shapiro*, the one opinion of its case.
const BALDRIGE_OPINION: &str = "110653";

const SECTION_9: &str = "uscode/title_13/chapter_1/subchapter_I/section_9";
const SUBSECTION_9_B: &str = "uscode/title_13/chapter_1/subchapter_I/section_9/subsection_b";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// The printings, built once for the file.
fn dataset_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/diff_format_characters.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);

        let mut dataset = Dataset::new(DatasetMetadata {
            name: "Titles 13 and 26 at two printings".to_string(),
            description: "For how a difference in format characters alone is reported".to_string(),
            author: "words_to_data tests".to_string(),
            license: "Public Domain".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        });
        for (file, date) in PRINTINGS {
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
            BALDRIGE_OPINION,
        ]);
        assert!(
            added.status.success(),
            "add-opinions should store Baldrige, stderr: {}",
            String::from_utf8_lossy(&added.stderr)
        );
        path
    })
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn should_not_list_a_provision_as_changed_when_only_a_soft_hyphen_differs() {
    let printed = stdout_of(&run(&[
        "diff",
        dataset_path(),
        "--from",
        "uscode/title_13@2025-07-18",
        "--to",
        "uscode/title_13@2025-07-30",
        "--path",
        SECTION_9,
    ]));

    assert!(!printed.contains(SUBSECTION_9_B), "got:\n{printed}");
    assert!(printed.contains("Changed (0):"), "got:\n{printed}");
}

const SUBSECTION_174_A: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";

/// In the same window the chapeau of § 174(a) changed from "specified" to
/// "foreign" research or experimental expenditures: a change in letters.
#[test]
fn should_still_list_a_provision_as_changed_when_its_letters_differ_in_the_same_window() {
    let printed = stdout_of(&run(&[
        "diff",
        dataset_path(),
        "--from",
        "uscode/title_26@2025-07-18",
        "--to",
        "uscode/title_26@2025-07-30",
        "--path",
        SUBSECTION_174_A,
        "--exact",
    ]));

    assert!(
        printed.contains(&format!("Changed (1):\n  {SUBSECTION_174_A}\n")),
        "got:\n{printed}"
    );
}

#[test]
fn should_state_in_the_help_that_format_characters_alone_are_not_a_change() {
    let printed = stdout_of(&run(&["diff", "--help"]));

    assert!(
        printed.contains("is not listed as changed"),
        "got:\n{printed}"
    );
    // Each character search does not read is named, so the help cannot fall
    // behind the list.
    for c in IGNORED {
        let code = format!("U+{:04X}", u32::from(*c));
        assert!(
            printed.contains(&code),
            "{code} is not named, got:\n{printed}"
        );
    }
}

/// An agent reads the rule from the output: the characters the diff did not
/// read, by code point.
#[test]
fn should_name_the_characters_it_did_not_read_in_json() {
    let printed = stdout_of(&run(&[
        "diff",
        dataset_path(),
        "--from",
        "uscode/title_13@2025-07-18",
        "--to",
        "uscode/title_13@2025-07-30",
        "--json",
    ]));
    let summary: serde_json::Value =
        serde_json::from_str(&printed).unwrap_or_else(|error| panic!("{error}: {printed}"));

    assert_eq!(summary["changed_paths"], serde_json::json!([]));
    let codes: Vec<String> = IGNORED
        .iter()
        .map(|c| format!("U+{:04X}", u32::from(*c)))
        .collect();
    assert_eq!(summary["ignored_characters"], serde_json::json!(codes));
    assert!(
        codes.contains(&"U+00AD".to_string()),
        "the soft hyphen is one of them"
    );
}

/// Baldrige cites § 9. In the window 2025-07-18 to 2025-07-30 the only
/// difference under § 9 is the soft hyphen in § 9(b), so § 9 did not change.
#[test]
fn should_say_unchanged_for_a_window_when_only_a_soft_hyphen_differs_under_the_cited_provision() {
    let printed = stdout_of(&run(&["cases-citing", dataset_path(), "--path", SECTION_9]));

    assert!(
        printed.contains(&format!(
            "2025-07-18 → 2025-07-30: unchanged at {SECTION_9}\n"
        )),
        "got:\n{printed}"
    );
    assert!(!printed.contains("CHANGED"), "got:\n{printed}");
}
