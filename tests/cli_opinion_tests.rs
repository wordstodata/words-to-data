//! End-to-end tests over the two commands #53 added, driving the real binary.
//!
//! The fixture is title 1 of the US Code — the smallest title in the corpus — and
//! the ten real CourtListener records committed under
//! `tests/test_data/courtlistener`. Title 26 is deliberately **not** in it, so
//! every citation to § 174 is out of scope, which is the answer these tests exist
//! to pin: a dataset that does not carry the title must say so, and never "not
//! found".
//!
//! The deep answer — nine of the ten cite § 174, and the provision changed — needs
//! both printings of title 26 and is asserted in
//! `tests/court_opinion_citation_tests.rs`. This file pins the wiring: that the
//! commands run, that `--offline` spends no request, and that the honest answer
//! reaches the terminal.

use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata};

/// The ten opinions, as the command takes them.
const TEN: &str = "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879";

/// Where the committed CourtListener records are, as a cache directory.
///
/// The client keys on `courtlistener/<kind>_<id>.json`, which is the name the
/// committed files already have, so a test and a live run read the same file.
const CACHE: &str = "tests/test_data";

const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// A dataset holding title 1 and nothing else, at its own path per test.
fn title_1_only(name: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Title 1 only".to_string(),
        description: "One title, so a citation to title 26 is out of scope".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc01.xml",
            "2025-07-18",
            None,
        )
        .expect("title 1 should parse");
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

fn stdout_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn should_store_ten_opinions_and_spend_no_request_when_offline() {
    let dataset = title_1_only("cli_opinions_added");
    let output = run(&[
        "add-opinions",
        &dataset,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        TEN,
    ]);

    assert!(
        output.status.success(),
        "add-opinions should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = stdout_of(&output);

    assert!(
        printed.contains("CourtListener requests spent: 0"),
        "an offline run reads the committed records and spends nothing, got:\n{printed}"
    );
    for work in [
        "judicial/opinion_2812209 @ 2015-06-26",
        "judicial/opinion_109019 @ 1974-05-13",
        "judicial/opinion_9434365 @ 2003-03-04",
    ] {
        assert!(
            printed.contains(work),
            "the run should name {work} at the date the court filed it, got:\n{printed}"
        );
    }
    assert!(
        printed.contains("out of scope") && printed.contains("statement about the law"),
        "a citation into a title this dataset does not carry is reported as out of \
         scope, got:\n{printed}"
    );

    // And the opinions really are in the file afterwards.
    let listed = run(&["expressions", &dataset]);
    let listed = stdout_of(&listed);
    assert!(
        listed.contains("judicial/opinion_2812209@2015-06-26"),
        "{listed}"
    );
}

#[test]
fn should_say_out_of_scope_when_asked_about_a_title_the_dataset_does_not_carry() {
    let dataset = title_1_only("cli_opinions_out_of_scope");
    run(&[
        "add-opinions",
        &dataset,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        TEN,
    ]);

    let output = run(&["cases-citing", &dataset, "--cites", "26 U.S.C. \u{a7} 174"]);
    assert!(
        output.status.success(),
        "cases-citing should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = stdout_of(&output);

    assert!(
        printed.contains("out of scope"),
        "the dataset holds title 1, so it can say nothing about title 26, got:\n{printed}"
    );
    assert!(
        printed.contains("This is not \"not found\"."),
        "and it must say which answer it is giving, got:\n{printed}"
    );
    assert!(
        !printed.contains("No opinion in this dataset cites it"),
        "an out-of-scope question is not answered with an empty list, got:\n{printed}"
    );
}

/// Asked by path rather than by citation, the command answers about the path and
/// does not pretend the dataset holds it.
#[test]
fn should_report_no_citing_opinion_when_the_dataset_holds_no_link_to_the_path() {
    let dataset = title_1_only("cli_opinions_by_path");
    run(&[
        "add-opinions",
        &dataset,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        TEN,
    ]);

    let output = run(&["cases-citing", &dataset, "--path", SECTION_174, "--chain"]);
    assert!(output.status.success(), "cases-citing should exit zero");
    let printed = stdout_of(&output);

    assert!(
        printed.contains("No opinion in this dataset cites it"),
        "no link was written into material the dataset does not hold, got:\n{printed}"
    );
    assert!(
        printed.contains("This dataset holds 10 opinion(s)"),
        "and the opinions it does hold are still counted, got:\n{printed}"
    );
    assert!(
        printed.contains("no chain to walk"),
        "nothing changed in a covered window, so there is no chain, got:\n{printed}"
    );
}

/// A record that is neither cached nor fetchable is named, not skipped.
///
/// A run that quietly stored nine of ten opinions would report success for a job
/// it did not do.
#[test]
fn should_fail_by_name_when_a_record_is_not_cached_and_the_client_is_offline() {
    let dataset = title_1_only("cli_opinions_missing");
    let output = run(&[
        "add-opinions",
        &dataset,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        "999999999",
    ]);

    assert!(
        !output.status.success(),
        "a missing record is not a success"
    );
    let complaint = String::from_utf8_lossy(&output.stderr);
    assert!(
        complaint.contains("999999999") && complaint.contains("offline"),
        "the failure should name the record and say why, got: {complaint}"
    );
}
