//! `words_to_data opinions`: the list of court opinions a dataset holds (#308).
//!
//! Before this command an agent saw `judicial/opinion_<id>@date` in
//! `expressions` and nothing else, so it wrote opinion ids from memory and could
//! not tell that two works were the majority and the dissent of one case.
//!
//! The fixture is title 26 of the U.S. Code as it read on 2025-07-18, and the ten
//! real CourtListener records committed under `tests/test_data/courtlistener`,
//! added with `add-opinions --offline`. Title 26 is in it so the opinions'
//! citations to § 174 resolve and become `judicial.cites` links.

use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::link::LinkKind;
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// The ten opinions, as `add-opinions` takes them.
const TEN: &str = "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879";

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

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

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Title 26 and ten opinions".to_string(),
        description: "The opinions cite section 174, and title 26 holds it".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    }
}

/// A SQLite dataset at its own path per test, holding title 26 and no opinions.
fn title_26_only(name: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let mut dataset: Dataset<InMemoryStorage> = Dataset::new(metadata());
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc26.xml",
            "2025-07-18",
            None,
        )
        .expect("title 26 should parse");
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

/// The same dataset with the ten committed opinions added to it.
fn with_the_ten(name: &str) -> String {
    let path = title_26_only(name);
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

/// The `--json` rows of `opinions`, read back.
fn rows_of(dataset: &str) -> Vec<serde_json::Value> {
    let printed = stdout_of(&run(&["opinions", dataset, "--json"]));
    let answer: serde_json::Value =
        serde_json::from_str(&printed).expect("--json should print JSON");
    answer["opinions"]
        .as_array()
        .expect("the answer should hold an `opinions` array")
        .clone()
}

#[test]
fn should_list_ten_opinions_with_their_case_names_when_the_ten_are_added() {
    let dataset = with_the_ten("opinions_ten");

    let rows = rows_of(&dataset);

    assert_eq!(rows.len(), 10, "one row for each held opinion: {rows:#?}");
    let obergefell = rows
        .iter()
        .find(|row| row["work"] == "judicial/opinion_2812209")
        .expect("Obergefell should be listed");
    assert_eq!(obergefell["date"], "2015-06-26");
    assert_eq!(obergefell["case_name"], "Obergefell v. Hodges");
}

#[test]
fn should_show_one_cluster_with_two_writing_types_when_a_case_has_a_majority_and_a_dissent() {
    let dataset = with_the_ten("opinions_boeing");

    let rows = rows_of(&dataset);

    // Boeing Co. v. United States: 122262 is the combined opinion and 9434365
    // is Justice Thomas's dissent. The committed records give both cluster 122262.
    let at = |work: &str| {
        rows.iter()
            .position(|row| row["work"] == work)
            .unwrap_or_else(|| panic!("{work} should be listed: {rows:#?}"))
    };
    let (majority, dissent) = (
        at("judicial/opinion_122262"),
        at("judicial/opinion_9434365"),
    );

    assert_eq!(rows[majority]["cluster_id"], 122262);
    assert_eq!(rows[dissent]["cluster_id"], 122262);
    assert_eq!(rows[majority]["opinion_type"], "010combined");
    assert_eq!(rows[dissent]["opinion_type"], "040dissent");
    assert_eq!(rows[dissent]["author"], "Thomas");
    assert_eq!(
        majority.abs_diff(dissent),
        1,
        "the two writings of one case should be next to each other: {rows:#?}"
    );
}

#[test]
fn should_count_the_citation_links_each_opinion_makes_when_the_dataset_holds_them() {
    let dataset = with_the_ten("opinions_cites");

    let rows = rows_of(&dataset);

    // The links, read from the file through the library rather than the command.
    let stored = Dataset::open_sqlite(&dataset).expect("the dataset should open");
    let links = stored
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read");
    assert!(!links.is_empty(), "the fixture should hold citation links");

    for row in &rows {
        let work = row["work"].as_str().expect("a work path");
        let made = links
            .iter()
            .filter(|link| link.subject.path() == Some(work))
            .count();
        assert_eq!(
            row["cites"], made,
            "{work} is the subject of {made} judicial.cites link(s): {row:#}"
        );
    }
    let total: u64 = rows.iter().map(|row| row["cites"].as_u64().unwrap()).sum();
    assert_eq!(
        total as usize,
        links.len(),
        "every citation link has a held opinion"
    );

    // Obergefell v. Hodges cites no section of title 26.
    let obergefell = rows
        .iter()
        .find(|row| row["work"] == "judicial/opinion_2812209")
        .expect("Obergefell should be listed");
    assert_eq!(obergefell["cites"], 0);
}

#[test]
fn should_name_the_field_each_text_was_read_from_when_the_records_differ() {
    let dataset = with_the_ten("opinions_text_field");

    let rows = rows_of(&dataset);

    let field_of = |work: &str| {
        rows.iter()
            .find(|row| row["work"] == work)
            .unwrap_or_else(|| panic!("{work} should be listed: {rows:#?}"))["text_field"]
            .clone()
    };
    // Shami: a court PDF. Snow: the Lawbox donation. The Boeing dissent:
    // Harvard's OCR. See docs/adr/0008.
    assert_eq!(field_of("judicial/opinion_2651100"), "plain_text");
    assert_eq!(field_of("judicial/opinion_109019"), "html_lawbox");
    assert_eq!(field_of("judicial/opinion_9434365"), "xml_harvard");
}

#[test]
fn should_say_the_court_is_not_recorded_when_the_stored_record_does_not_hold_it() {
    let dataset = with_the_ten("opinions_court");

    // The stored facts hold the docket and not the court (#53), and the command
    // fetches nothing to fill the gap.
    for row in rows_of(&dataset) {
        assert!(row["court"].is_null(), "no court is stored: {row:#}");
    }
    let printed = stdout_of(&run(&["opinions", &dataset]));
    let boeing = printed
        .lines()
        .skip_while(|line| !line.starts_with("judicial/opinion_122262@2003-03-04"))
        .take(2)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        boeing.contains("Boeing") && boeing.contains("court not recorded"),
        "the row should name the case and say the court is not recorded, got:\n{printed}"
    );
}

#[test]
fn should_say_it_holds_0_opinions_when_the_dataset_holds_none() {
    let dataset = title_26_only("opinions_none");

    let printed = stdout_of(&run(&["opinions", &dataset]));

    assert!(
        printed.contains("This dataset holds 0 opinions"),
        "an empty answer is said, not left blank, got:\n{printed}"
    );
}

#[test]
fn should_say_how_many_it_did_not_show_when_the_list_stops_at_a_limit() {
    let dataset = with_the_ten("opinions_limit");

    let printed = stdout_of(&run(&["opinions", &dataset, "--limit", "3"]));

    let shown = printed
        .lines()
        .filter(|line| line.starts_with("judicial/"))
        .count();
    assert_eq!(shown, 3, "three rows, got:\n{printed}");
    assert!(
        printed.contains("This dataset holds 10 opinions")
            && printed.contains("7 more not shown; pass --all for every one of them"),
        "the run says the whole count and what it left out, got:\n{printed}"
    );

    let all = stdout_of(&run(&["opinions", &dataset, "--all"]));
    let shown = all
        .lines()
        .filter(|line| line.starts_with("judicial/"))
        .count();
    assert_eq!(shown, 10, "--all shows every row, got:\n{all}");
}
