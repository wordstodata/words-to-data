//! What `search` does with the quote marks of a query (#439).
//!
//! An agent writes a phrase in quote marks, as for a web search engine. Search
//! read the quote marks as text, so `"Immigration parole fee"` found 0 fields
//! where the bare words found 6. Search now removes one pair of quote marks
//! around the whole query. Text that holds quote marks is still found: see
//! [`words_to_data::search::unquoted`] for the rule.
//!
//! The fixture is real data: title 1 of the US Code at 2025-07-18. The Code
//! prints its defined terms in quote marks, such as `the words "person" and
//! "whoever" include`.

use std::collections::BTreeSet;
use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Title 1 at 2025-07-18, in the format the extension names.
///
/// `name` names this test's file, so tests that run at the same time do not
/// share one.
fn title_1(name: &str, extension: &str) -> String {
    let path = format!(
        "{}/search_quotes_{name}.{extension}",
        env!("CARGO_TARGET_TMPDIR")
    );
    let _ = std::fs::remove_file(&path);
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc01.xml",
            "2025-07-18",
            None,
        )
        .expect("the title should parse");
    match extension {
        "sqlite" => dataset.save_to_sqlite(&path),
        _ => dataset.save(&path, Format::Compact),
    }
    .expect("the fixture should save");
    path
}

/// Run one check on a SQLite dataset and on a compact JSON one.
///
/// SQLite finds a hit in its element index and the in-memory store walks the
/// tree, so each question is asked of both.
fn on_both_backends(name: &str, check: impl Fn(&str)) {
    check(&title_1(name, "sqlite"));
    check(&title_1(name, "json"));
}

/// Where each hit of one search is, as `--json` gives them: path and field.
fn found(dataset: &str, query: &str) -> BTreeSet<(String, String)> {
    let output = run(&["search", dataset, query, "--json"]);
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
        .iter()
        .map(|hit| {
            (
                hit["path"].as_str().unwrap().to_string(),
                hit["field"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn should_find_the_bare_words_when_the_query_is_in_straight_quote_marks() {
    on_both_backends("straight", |dataset| {
        // Six fields of title 1 hold "person", and three of them print it in
        // quote marks, as a defined term.
        let bare = found(dataset, "person");
        assert_eq!(bare.len(), 6);
        assert_eq!(found(dataset, "\"person\""), bare);
    });
}

#[test]
fn should_find_the_bare_words_when_the_query_is_in_curly_quote_marks() {
    on_both_backends("curly", |dataset| {
        assert_eq!(
            found(dataset, "\u{201C}person\u{201D}"),
            found(dataset, "person")
        );
    });
}

/// 1 U.S.C. § 1 says `the words "person" and "whoever" include`. The query
/// starts and ends with a quote mark, but the marks are not one pair around
/// it, so they are text, and only § 1 holds that text.
#[test]
fn should_read_the_quote_marks_as_text_when_the_query_holds_more_than_one_pair() {
    on_both_backends("two_pairs", |dataset| {
        let section_1 = BTreeSet::from([(
            "uscode/title_1/chapter_1/section_1".to_string(),
            "content".to_string(),
        )]);
        assert_eq!(found(dataset, "\"person\" and \"whoever\""), section_1);
    });
}

/// A query that only starts or only ends with a quote mark is read as it is,
/// so a word outside the quote marks finds only the text that prints them.
#[test]
fn should_read_the_quote_marks_as_text_when_a_word_is_outside_them() {
    on_both_backends("outside", |dataset| {
        let section_1 = BTreeSet::from([(
            "uscode/title_1/chapter_1/section_1".to_string(),
            "content".to_string(),
        )]);
        assert_eq!(found(dataset, "and \"whoever\""), section_1);
        assert_eq!(found(dataset, "\"person\" and"), section_1);
    });
}

/// The answer says that the quote marks were not read, so an agent that put
/// them there does not take the hits for text that holds them.
#[test]
fn should_say_that_the_quote_marks_were_removed_when_search_removes_them() {
    let dataset = title_1("notice", "sqlite");

    let output = run(&["search", &dataset, "\"person\""]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().next(),
        Some("Searched without the quote marks: person"),
        "stdout: {stdout}"
    );

    let output = run(&["search", &dataset, "\"person\"", "--json"]);
    let answer: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("--json should print JSON");
    assert_eq!(answer["notice"], "Searched without the quote marks: person");

    let output = run(&["search", &dataset, "\"person\"", "--occurrences"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        stdout.lines().next(),
        Some("Searched without the quote marks: person"),
        "stdout: {stdout}"
    );
}

#[test]
fn should_say_nothing_about_quote_marks_when_the_query_is_read_as_it_is() {
    let dataset = title_1("no_notice", "sqlite");
    for query in ["person", "\"person\" and \"whoever\""] {
        let output = run(&["search", &dataset, query]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(!stdout.contains("quote marks"), "stdout: {stdout}");

        let output = run(&["search", &dataset, query, "--json"]);
        let answer: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("--json should print JSON");
        assert!(answer.get("notice").is_none(), "answer: {answer}");
    }
}

/// The Code is stored with straight quote marks, and text copied from a PDF or
/// a web page has curly ones. Search reads a curly quote mark as the straight
/// one (#453), so the curly form of a query finds what the straight form finds.
#[test]
fn should_find_straight_quote_marks_when_the_query_has_curly_ones() {
    on_both_backends("curly_inside", |dataset| {
        let section_1 = BTreeSet::from([(
            "uscode/title_1/chapter_1/section_1".to_string(),
            "content".to_string(),
        )]);
        assert_eq!(
            found(
                dataset,
                "\u{201C}person\u{201D} and \u{201C}whoever\u{201D}"
            ),
            section_1
        );
        assert_eq!(found(dataset, "and \u{201C}whoever\u{201D}"), section_1);
    });
}

/// 1 U.S.C. § 7 says "that individual's marriage", with a straight apostrophe.
/// A curly apostrophe or single quote mark in the query finds it (#453).
#[test]
fn should_find_a_straight_apostrophe_when_the_query_has_a_curly_one() {
    on_both_backends("curly_single", |dataset| {
        let straight = found(dataset, "individual's marriage");
        assert!(!straight.is_empty(), "title 1 holds the words");
        assert_eq!(found(dataset, "individual\u{2019}s marriage"), straight);
        assert_eq!(
            found(dataset, "individual\u{2018}s marriage"),
            straight,
            "a left single quote mark is read as the straight one too"
        );
    });
}
