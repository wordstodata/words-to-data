//! `search` inside one long field: every occurrence, and a window around one.
//!
//! A court opinion is one field of 6k to 207k characters. `search` gave one hit
//! for each field, so an opinion that names § 174 five times gave "1 match", and
//! an agent read whole opinions to find the passage it wanted (#306).
//!
//! The fixture is title 1 of the US Code and the real CourtListener records of
//! *Boeing Co. v. United States* (cluster 122262), committed under
//! `tests/test_data/courtlistener`. The case is stored from its split records
//! (#313), and since #314 each writing read from Harvard's field is stored as
//! its paragraphs. The third paragraph that starts on page 444 of the Court's
//! opinion, record 9434364, holds "R&D" three times, at the character offsets
//! 300, 783 and 943.

use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

/// Boeing Co. v. United States, 537 U.S. 437 (2003), named by its combined
/// record. `add-opinions` reads the whole case.
const BOEING: &str = "122262";
/// One paragraph of the Court's opinion in *Boeing*, by Justice Stevens: the
/// third that starts on page 444 of 537 U.S.
const BOEING_MAJORITY: &str = "judicial/case_122262/opinion_9434364/page_444/paragraph_3";

/// What the paragraph holds three times.
const QUERY: &str = "R&D";

/// The character offsets of [`QUERY`] in the paragraph.
const OFFSETS: [u64; 3] = [300, 783, 943];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Title 1 and the Boeing opinion, in the format the extension names.
///
/// `name` names this test's file, so tests that run at the same time do not
/// share one.
fn with_boeing(name: &str, extension: &str) -> String {
    let path = format!(
        "{}/search_occurrences_{name}.{extension}",
        env!("CARGO_TARGET_TMPDIR")
    );
    let _ = std::fs::remove_file(&path);
    // A compact JSON dataset is written whole, so `add-opinions` writes it to a
    // new file rather than back over its input. SQLite grows in place.
    let base = format!("{path}.base.{extension}");
    let _ = std::fs::remove_file(&base);
    let target = if extension == "sqlite" { &path } else { &base };

    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc01.xml",
            "2025-07-18",
            None,
        )
        .expect("title 1 should parse");
    match extension {
        "sqlite" => dataset.save_to_sqlite(target),
        _ => dataset.save(target, Format::Compact),
    }
    .expect("the fixture should save");

    let mut add = vec![
        "add-opinions",
        target,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        BOEING,
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
    path
}

/// Run one check on a SQLite dataset and on a compact JSON one.
///
/// The two back ends find a hit by different means, so each question is asked
/// of both.
fn on_both_backends(name: &str, check: impl Fn(&str)) {
    check(&with_boeing(name, "sqlite"));
    check(&with_boeing(name, "json"));
}

fn json_of(output: &Output) -> serde_json::Value {
    assert!(
        output.status.success(),
        "search should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("--json should print JSON")
}

#[test]
fn should_list_each_occurrence_with_its_offset_when_occurrences_is_asked() {
    on_both_backends("each", |dataset| {
        let answer = json_of(&run(&[
            "search",
            dataset,
            QUERY,
            "--path",
            BOEING_MAJORITY,
            "--occurrences",
            "--json",
        ]));

        let hits = answer["hits"].as_array().expect("hits should be a list");
        let offsets: Vec<u64> = hits
            .iter()
            .map(|hit| hit["offset"].as_u64().expect("each hit has an offset"))
            .collect();
        assert_eq!(offsets, OFFSETS, "in {dataset}");
        for hit in hits {
            assert!(
                hit["snippet"].as_str().unwrap().contains(QUERY),
                "each snippet holds its own occurrence, got {hit}"
            );
        }
        assert_eq!(answer["total"], 3, "in {dataset}");
    });
}

#[test]
fn should_state_the_occurrences_in_each_field_when_occurrences_is_not_asked() {
    on_both_backends("default", |dataset| {
        let printed = stdout_of(&run(&["search", dataset, QUERY, "--path", BOEING_MAJORITY]));

        assert!(
            printed.contains(&format!("{BOEING_MAJORITY}  [content]  3 occurrences")),
            "the hit says how many times its field holds the query, got:\n{printed}"
        );
        assert!(
            printed.contains("1 field, 3 occurrences"),
            "the count says what it counts, so one field is not read as one \
             mention, got:\n{printed}"
        );
    });
}

#[test]
fn should_count_occurrences_against_the_limit_when_occurrences_is_asked() {
    on_both_backends("limit", |dataset| {
        let answer = json_of(&run(&[
            "search",
            dataset,
            QUERY,
            "--path",
            BOEING_MAJORITY,
            "--occurrences",
            "--limit",
            "2",
            "--json",
        ]));
        let offsets: Vec<u64> = answer["hits"]
            .as_array()
            .expect("hits should be a list")
            .iter()
            .map(|hit| hit["offset"].as_u64().unwrap())
            .collect();
        assert_eq!(offsets, OFFSETS[..2], "in {dataset}");
        assert_eq!(answer["total"], 3, "the total counts every occurrence");

        let printed = stdout_of(&run(&[
            "search",
            dataset,
            QUERY,
            "--path",
            BOEING_MAJORITY,
            "--occurrences",
            "--limit",
            "2",
        ]));
        assert!(
            printed.contains("3 occurrences") && printed.contains("1 more not shown"),
            "the run says what the limit left out, got:\n{printed}"
        );
    });
}

/// The hit for the opinion's text. An offset counts characters in one field,
/// and the opinion also has a heading.
fn content_of(answer: &serde_json::Value) -> &serde_json::Value {
    answer["hits"]
        .as_array()
        .expect("hits should be a list")
        .iter()
        .find(|hit| hit["field"] == "content")
        .expect("the opinion's text should be a hit")
}

#[test]
fn should_read_only_the_window_asked_around_an_offset_when_offset_is_given() {
    on_both_backends("window", |dataset| {
        let whole = json_of(&run(&[
            "search",
            dataset,
            "",
            "--path",
            BOEING_MAJORITY,
            "--exact",
            "--snippet",
            "0",
            "--json",
        ]));
        let whole = content_of(&whole)["snippet"].as_str().unwrap().to_string();

        // The third occurrence, as `--occurrences` gave it.
        let window = json_of(&run(&[
            "search",
            dataset,
            "",
            "--path",
            BOEING_MAJORITY,
            "--exact",
            "--offset",
            "943",
            "--snippet",
            "40",
            "--json",
        ]));
        let read = content_of(&window)["snippet"].as_str().unwrap();
        let text = read.trim_matches('…');

        assert_eq!(
            text.chars().count(),
            40,
            "the window holds the characters asked for and no more, got {read:?}"
        );
        assert!(
            text.contains(QUERY),
            "the window holds the text at the offset, got {read:?}"
        );
        assert!(
            whole.contains(text),
            "the window is the field's own text, got {read:?}"
        );
    });
}
