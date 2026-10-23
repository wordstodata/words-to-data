//! What `search` reads as the same characters, beyond case and white space
//! (#338).
//!
//! The stored text holds forms that an agent does not type: a soft hyphen
//! inside a word of the Code, a ligature in a court opinion, "§7" for "§ 7",
//! "U. S." for "U.S." in a reporter cite, and a hyphen that a line break left
//! inside a word of a slip opinion. A search that compared the characters as
//! they are did not find words that the dataset holds, and an agent then said
//! "the text is not in the dataset".
//!
//! The fixtures are real data: title 13 of the US Code at 2025-07-18, and
//! CourtListener records committed under `tests/test_data/courtlistener`.

use std::collections::BTreeSet;
use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// A dataset of one release-point file, in the format the extension names.
///
/// `name` names this test's file, so tests that run at the same time do not
/// share one.
fn with_title(name: &str, extension: &str, xml: &str) -> String {
    let path = format!(
        "{}/search_folding_{name}.{extension}",
        env!("CARGO_TARGET_TMPDIR")
    );
    let _ = std::fs::remove_file(&path);
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .add_uslm_xml(xml, "2025-07-18", None)
        .expect("the title should parse");
    match extension {
        "sqlite" => dataset.save_to_sqlite(&path),
        _ => dataset.save(&path, Format::Compact),
    }
    .expect("the fixture should save");
    path
}

/// Title 1 and the court cases of `opinions`, in the format the extension
/// names.
fn with_opinions(name: &str, extension: &str, opinions: &str) -> String {
    let path = format!(
        "{}/search_folding_{name}.{extension}",
        env!("CARGO_TARGET_TMPDIR")
    );
    let _ = std::fs::remove_file(&path);
    // A compact JSON dataset is written whole, so `add-opinions` writes it to a
    // new file rather than back over its input. SQLite grows in place.
    let base = with_title(
        &format!("{name}_base"),
        extension,
        "tests/test_data/usc/2025-07-18/usc01.xml",
    );
    let target = if extension == "sqlite" { &base } else { &path };

    let mut add = vec![
        "add-opinions",
        &base,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        opinions,
    ];
    if extension != "sqlite" {
        add.extend(["--output", &path]);
    }
    let added = run(&add);
    assert!(
        added.status.success(),
        "add-opinions should exit zero, stderr: {}",
        String::from_utf8_lossy(&added.stderr)
    );
    target.to_string()
}

/// Run one check on a SQLite dataset and on a compact JSON one.
///
/// SQLite finds a hit in its element index and the in-memory store walks the
/// tree, so each question is asked of both.
fn on_both_backends(make: impl Fn(&str) -> String, check: impl Fn(&str)) {
    check(&make("sqlite"));
    check(&make("json"));
}

/// The hits of one search, as `--json` gives them.
fn hits(args: &[&str]) -> Vec<serde_json::Value> {
    let output = run(args);
    assert!(
        output.status.success(),
        "search should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answer: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should print JSON");
    answer["hits"]
        .as_array()
        .expect("hits should be a list")
        .clone()
}

/// The paths of the hits of one search.
fn paths_of(found: &[serde_json::Value]) -> BTreeSet<String> {
    found
        .iter()
        .map(|hit| hit["path"].as_str().unwrap().to_string())
        .collect()
}

/// 13 U.S.C. § 9(b) ends "public rec\u{ad}ords": the Code puts a soft hyphen
/// (U+00AD) inside the word. About 600 fields of each printing hold one.
#[test]
fn should_find_a_word_when_a_soft_hyphen_splits_it() {
    let title_13 = |extension: &str| {
        with_title(
            "soft_hyphen",
            extension,
            "tests/test_data/usc/2025-07-18/usc13.xml",
        )
    };
    on_both_backends(title_13, |dataset| {
        let found = hits(&[
            "search",
            dataset,
            "public records",
            "--path",
            "uscode/title_13/chapter_1/subchapter_I/section_9",
            "--json",
        ]);

        assert_eq!(
            paths_of(&found),
            BTreeSet::from(["uscode/title_13/chapter_1/subchapter_I/section_9/subsection_b".to_string()]),
            "the subsection that holds the words, in {dataset}"
        );
        let snippet = found[0]["snippet"].as_str().unwrap();
        assert!(
            snippet.contains("public rec\u{ad}ords"),
            "the snippet shows the text as the Code has it, got {snippet:?}"
        );
    });
}

/// *Little Sandy Coal v. Commissioner* (opinion 9377344) is stored from a PDF,
/// and its text writes "qualified" with the ligature "\u{fb01}" (U+FB01).
#[test]
fn should_find_a_word_when_a_ligature_spells_two_of_its_letters() {
    let little_sandy_coal =
        |extension: &str| with_opinions("ligature", extension, "9377344");
    on_both_backends(little_sandy_coal, |dataset| {
        let search = |query: &str| {
            hits(&[
                "search",
                dataset,
                query,
                "--path",
                "judicial/case_9381868",
                "--all",
                "--json",
            ])
        };
        let as_stored = search("quali\u{fb01}ed research expenses");
        assert!(
            !as_stored.is_empty(),
            "the opinion holds the words with the ligature, in {dataset}"
        );

        let as_typed = search("qualified research expenses");

        assert_eq!(
            paths_of(&as_typed),
            paths_of(&as_stored),
            "\"fi\" finds the paragraphs that write \"\u{fb01}\", in {dataset}"
        );
        let snippet = as_typed[0]["snippet"].as_str().unwrap();
        assert!(
            snippet.contains("quali\u{fb01}ed"),
            "the snippet shows the text as the opinion has it, got {snippet:?}"
        );
    });
}

/// *BMW (US) Holding Corp. v. United States* (opinion 11409814) writes both
/// "IRC §7508(a)" and "26 U.S.C. § 7508(a)".
#[test]
fn should_find_the_same_paragraphs_when_a_section_sign_is_written_with_or_without_a_space() {
    let one_case = |extension: &str| with_opinions("section_sign", extension, "11409814");
    on_both_backends(one_case, |dataset| {
        let search = |query: &str| {
            paths_of(&hits(&[
                "search",
                dataset,
                query,
                "--path",
                "judicial/case_10942231",
                "--all",
                "--json",
            ]))
        };
        let spaced = search("§ 7508(a)");
        let closed = search("§7508(a)");

        assert_eq!(
            spaced, closed,
            "\"§ 7508(a)\" and \"§7508(a)\" find the same paragraphs, in {dataset}"
        );
        assert_eq!(
            closed,
            search("IRC §7508(a)")
                .union(&search("U.S.C. § 7508(a)"))
                .cloned()
                .collect(),
            "both written forms are found, in {dataset}"
        );
    });
}

/// A reporter cite is written "308 U. S. 488" in one opinion and
/// "308 U.S. 488" in another: *Snow* (109019) and *Encyclopaedia Britannica*
/// (406879) cite *Deputy v. du Pont* each in its own way.
#[test]
fn should_find_the_same_paragraphs_when_us_reports_is_spelt_with_or_without_spaces() {
    let two_cases = |extension: &str| with_opinions("us_reports", extension, "109019,406879");
    on_both_backends(two_cases, |dataset| {
        let search = |query: &str| {
            paths_of(&hits(&[
                "search", dataset, query, "--path", "judicial", "--all", "--json",
            ]))
        };
        let closed = search("308 U.S. 488");
        let spaced = search("308 U. S. 488");

        assert_eq!(
            closed, spaced,
            "\"U.S.\" and \"U. S.\" find the same paragraphs, in {dataset}"
        );
        let cases: BTreeSet<&str> = closed
            .iter()
            .map(|path| path.split('/').nth(1).unwrap())
            .collect();
        assert_eq!(
            cases,
            BTreeSet::from(["case_109019", "case_406879"]),
            "both opinions cite the case, in {dataset}"
        );
    });
}

/// The text of *Little Sandy Coal* keeps the hyphen where a line of the PDF
/// broke a word: "substan-tially", "experimenta-tion" (#338).
#[test]
fn should_find_a_word_when_a_line_break_left_a_hyphen_inside_it() {
    let little_sandy_coal =
        |extension: &str| with_opinions("broken_word", extension, "9377344");
    on_both_backends(little_sandy_coal, |dataset| {
        let search = |query: &str| {
            hits(&[
                "search",
                dataset,
                query,
                "--path",
                "judicial/case_9381868",
                "--all",
                "--json",
            ])
        };
        let as_stored = paths_of(&search("substan-tially"));
        assert!(
            !as_stored.is_empty(),
            "the opinion holds the broken word, in {dataset}"
        );

        let as_typed = search("substantially");

        assert!(
            paths_of(&as_typed).is_superset(&as_stored),
            "\"substantially\" finds each paragraph that writes \"substan-tially\", in {dataset}"
        );
        assert!(
            as_typed
                .iter()
                .any(|hit| hit["snippet"].as_str().unwrap().contains("substan-tially")),
            "a snippet shows the text as the opinion has it, in {dataset}"
        );
    });
}
