//! What `search` counts as the same words: white space, and the spellings of
//! "U.S.C.".
//!
//! Court opinions write one citation in more than one way, and the text of an
//! opinion breaks lines inside phrases. A search that matched the exact
//! characters found 7 of the 9 opinions that cite § 174 when asked for
//! "U.S.C. § 174", and 3 when asked for "U. S. C. § 174" (#307).
//!
//! The fixture is title 1 of the US Code and the ten real CourtListener records
//! committed under `tests/test_data/courtlistener`.

use std::collections::BTreeSet;
use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

/// The ten opinions, as `add-opinions` takes them.
const TEN: &str = "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879";

/// The nine writings that cite 26 U.S.C. § 174, in one spelling or the other.
/// Opinion 2812209 holds no such citation. *Boeing*'s two are its majority,
/// 9434364, and Justice Thomas's dissent, 9434365.
const CITE_174: [&str; 9] = [
    "judicial/case_109019/opinion_109019",
    "judicial/case_122262/opinion_9434364",
    "judicial/case_122262/opinion_9434365",
    "judicial/case_1527901/opinion_1527901",
    "judicial/case_2651100/opinion_2651100",
    "judicial/case_406879/opinion_406879",
    "judicial/case_6248/opinion_6248",
    "judicial/case_7029371/opinion_6931314",
    "judicial/case_8998794/opinion_8991218",
];

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Title 1 and the ten opinions, in the format the extension names.
///
/// `name` names this test's file, so tests that run at the same time do not
/// share one.
fn with_opinions(name: &str, extension: &str) -> String {
    let path = format!(
        "{}/search_matching_{name}.{extension}",
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
        TEN,
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
/// SQLite finds a hit in its element index and the in-memory store walks the
/// tree, so each question is asked of both.
fn on_both_backends(name: &str, check: impl Fn(&str)) {
    check(&with_opinions(name, "sqlite"));
    check(&with_opinions(name, "json"));
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

/// The writings a search found the query in, by path.
fn works_holding(dataset: &str, query: &str) -> BTreeSet<String> {
    hits(&[
        "search", dataset, query, "--path", "judicial", "--all", "--json",
    ])
    .iter()
    .map(|hit| hit["path"].as_str().unwrap().to_string())
    .collect()
}

#[test]
fn should_find_the_same_opinions_when_us_code_is_spelt_with_or_without_spaces() {
    on_both_backends("spellings", |dataset| {
        let expected: BTreeSet<String> = CITE_174.iter().map(|w| w.to_string()).collect();

        assert_eq!(
            works_holding(dataset, "U.S.C. § 174"),
            expected,
            "\"U.S.C.\" also finds \"U. S. C.\", in {dataset}"
        );
        assert_eq!(
            works_holding(dataset, "U. S. C. § 174"),
            expected,
            "\"U. S. C.\" also finds \"U.S.C.\", in {dataset}"
        );
    });
}

/// Opinion 2651100 is read from Harvard's markup, and an inline tag ends a
/// line inside a phrase: Harvard sets the ampersand of "R & D" in `<em>`, so
/// the stored text is "Not all R\n&\nD expenses".
#[test]
fn should_find_a_phrase_when_a_line_break_splits_it() {
    on_both_backends("line_break", |dataset| {
        let found = hits(&[
            "search",
            dataset,
            "Not all R & D expenses are QREs",
            "--path",
            "judicial",
            "--json",
        ]);

        let paths: Vec<&str> = found
            .iter()
            .map(|hit| hit["path"].as_str().unwrap())
            .collect();
        assert_eq!(
            paths,
            ["judicial/case_2651100/opinion_2651100"],
            "in {dataset}"
        );
        let snippet = found[0]["snippet"].as_str().unwrap();
        assert!(
            snippet.contains("R\n&\nD expenses"),
            "the snippet shows the text as the opinion has it, got {snippet:?}"
        );
    });
}
