//! `words_to_data path` on a case or a writing: every fact the dataset stores
//! about it (#327).
//!
//! `opinions` names each writing and its author, and stops there. The panel,
//! per curiam, the precedential status and the reporter citations were stored
//! and no command showed them: a SLEUTH run could not tell who wrote Boeing's
//! combined record and read the payload with SQL. `path` reports everything
//! about one path, so on a judicial path it now reports the stored facts too.
//!
//! The fixture is an empty dataset with the ten real CourtListener records
//! committed under `tests/test_data/courtlistener`, added with
//! `add-opinions --offline`. *Boeing Co. v. United States* is cluster 122262:
//! 9434364 is the Court's opinion by Justice Stevens and 9434365 is Justice
//! Thomas's dissent. Its combined record 122262 is kept as a source only.

use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::storage::InMemoryStorage;

/// The ten opinions, as `add-opinions` takes them.
const TEN: &str = "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879";

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

const BOEING: &str = "judicial/case_122262";
const BOEING_DISSENT: &str = "judicial/case_122262/opinion_9434365";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// A SQLite dataset at its own path per test, holding the ten opinions.
fn with_the_ten(name: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let dataset: Dataset<InMemoryStorage> = Dataset::new(DatasetMetadata {
        name: "Ten opinions".to_string(),
        description: "The ten committed CourtListener records".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    stdout_of(&run(&[
        "add-opinions",
        &path,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        TEN,
    ]));
    path
}

/// The `judicial` part of `path --json`.
fn judicial_of(dataset: &str, path: &str) -> serde_json::Value {
    let printed = stdout_of(&run(&["path", dataset, path, "--json"]));
    let report: serde_json::Value =
        serde_json::from_str(&printed).expect("--json should print JSON");
    report["judicial"].clone()
}

#[test]
fn should_give_the_dissent_facts_and_its_case_facts_when_path_names_boeings_dissent() {
    let dataset = with_the_ten("facts_boeing_dissent_json");

    let judicial = judicial_of(&dataset, BOEING_DISSENT);

    let writing = &judicial["writing"];
    assert_eq!(writing["path"], BOEING_DISSENT, "{judicial:#}");
    assert_eq!(writing["opinion_type"], "040dissent");
    assert_eq!(writing["author"], "Thomas");
    assert_eq!(writing["per_curiam"], false);

    let case = &judicial["case"];
    assert_eq!(case["path"], BOEING);
    assert_eq!(case["case_name"], "Boeing Co. v. United States");
    assert_eq!(case["cluster_id"], 122262);
    assert_eq!(
        case["panel"],
        "Stevens, Rehnquist, O'Connor, Kennedy, Souter, Ginsburg, Breyer, Thomas, Scalia"
    );
    assert_eq!(case["precedential_status"], "Published");
    assert_eq!(
        case["citations"][0], "537 U.S. 437",
        "the reporter citations as stored: {case:#}"
    );
    assert_eq!(case["combined_records"], serde_json::json!([122262]));
}

/// What `path` prints for a person.
fn printed(dataset: &str, path: &str) -> String {
    stdout_of(&run(&["path", dataset, path]))
}

#[test]
fn should_print_the_dissent_facts_and_say_not_recorded_when_a_fact_is_missing() {
    let dataset = with_the_ten("facts_boeing_dissent_text");

    let out = printed(&dataset, BOEING_DISSENT);

    for line in [
        "Writing judicial/case_122262/opinion_9434365:",
        "  writing type: 040dissent",
        "  author: Thomas",
        "  per curiam: no",
        // The split records carry no page count and no download URL, and the
        // publisher sent a blank sha1 for them.
        "  page count: not recorded",
        "  download URL: not recorded",
        "  sha1: not recorded",
        "  text from: courtlistener:opinion/9434365:xml_harvard (ocr)",
        "Case judicial/case_122262@2003-03-04:",
        "  case name: Boeing Co. v. United States",
        "  panel: Stevens, Rehnquist, O'Connor, Kennedy, Souter, Ginsburg, Breyer, Thomas, Scalia",
        "  precedential status: Published",
        "  citations: 537 U.S. 437; 123 S. Ct. 1099; 155 L. Ed. 2d 17; 2003 U.S. LEXIS 1947",
        "  court: not recorded",
    ] {
        assert!(
            out.lines().any(|printed| printed == line),
            "`path` should print {line:?}:\n{out}"
        );
    }
}

#[test]
fn should_print_the_case_facts_and_one_line_per_writing_when_path_names_boeings_case() {
    let dataset = with_the_ten("facts_boeing_case_text");

    let out = printed(&dataset, BOEING);

    for line in [
        "Case judicial/case_122262@2003-03-04:",
        "  precedential status: Published",
        "  combined records kept as a source, not as text: 122262",
        "Writings of this case (2):",
        "  judicial/case_122262/opinion_9434364  020lead · author Stevens · per curiam no",
        "  judicial/case_122262/opinion_9434365  040dissent · author Thomas · per curiam no",
    ] {
        assert!(
            out.lines().any(|printed| printed == line),
            "`path` should print {line:?}:\n{out}"
        );
    }
    assert!(
        !out.lines()
            .any(|printed| printed.starts_with("Writing judicial/")),
        "the case path names no one writing, so no writing is given in full:\n{out}"
    );
}

#[test]
fn should_print_page_count_download_url_and_sha1_when_the_record_carries_them() {
    let dataset = with_the_ten("facts_obergefell");

    // The Obergefell record is a combined record that also carries the
    // Court's own PDF, so it carries the facts that Boeing's split records do
    // not.
    let out = printed(&dataset, "judicial/case_2812209/opinion_2812209");

    for line in [
        "  writing type: 010combined",
        "  author: Kennedy",
        "  page count: 103",
        "  download URL: http://www.supremecourt.gov/opinions/14pdf/14-556_3204.pdf",
        "  sha1: 0304bb66d174e0fd2cddceb66f790f70dad355ff",
        // Harvard's field comes first since ADR 0014, decision 3.
        "  text from: courtlistener:opinion/2812209:xml_harvard (ocr)",
        "  combined records kept as a source, not as text: none",
    ] {
        assert!(
            out.lines().any(|printed| printed == line),
            "`path` should print {line:?}:\n{out}"
        );
    }
}

#[test]
fn should_give_no_court_facts_when_the_path_is_not_judicial() {
    let dataset = with_the_ten("facts_not_judicial");

    let path = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";
    let judicial = judicial_of(&dataset, path);
    let out = printed(&dataset, path);

    assert!(judicial.is_null(), "{judicial:#}");
    assert!(
        !out.lines().any(|line| line.starts_with("Case ")),
        "a Code path has no case:\n{out}"
    );
}
