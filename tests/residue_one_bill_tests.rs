//! `residue --bill` matches only the titles of the Code that the bill acts in,
//! and gives the same answers as the whole residue gives for that bill (#398).
//!
//! Before, `--bill` ran the matcher over every title and then kept the bill's
//! rows: 40 s on `eval.sqlite` for a bill with one amendment.
//!
//! Every case reads committed laws and titles. `119-hr-1` (Pub. L. 119-21) and
//! `119-s-1071` (Pub. L. 119-60) both act in title 10, which is held at
//! 2025-08-14 and 2026-01-23. `119-hr-42` (Pub. L. 119-22) acts only in
//! title 43, which is held at 2025-07-18 and 2025-07-30. Nothing is mocked.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::legislature::evidence_matching::{match_bill_by_evidence, match_by_evidence};
use words_to_data::storage::InMemoryStorage;
use words_to_data::uslm::bill_parser::{bill_expression, parse_bill_amendments};

const CACHE: &str = "tests/test_data/congress_client_cache/bill/119";
const BILLS: [&str; 3] = ["119-hr-1", "119-s-1071", "119-hr-42"];

/// A committed law with its metadata, as the Congress client hands it over.
fn download(dir: &str, bill_id: &str) -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{CACHE}/{dir}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: bill_id.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: std::collections::HashMap::new(),
    }
}

/// `119-s-1071`, committed as its law alone: its amendments and its document.
fn add_law_without_metadata(dataset: &mut Dataset<InMemoryStorage>, bill_id: &str, dir: &str) {
    let path = format!("{CACHE}/{dir}/public_law.xml");
    let bill = parse_bill_amendments(bill_id, &path).expect("the bill should parse");
    dataset.add_bill(bill).expect("the bill should add");
    let xml = std::fs::read_to_string(&path).expect("the law should read");
    let document = roxmltree::Document::parse(&xml).expect("the law should parse");
    let (law, _) = bill_expression(&document, bill_id).expect("the law should read");
    dataset.add_expression(law).expect("the law should add");
}

/// Three laws, two of them in one title, and the titles they act in.
fn laws() -> &'static Dataset<InMemoryStorage> {
    static DATASET: OnceLock<Dataset<InMemoryStorage>> = OnceLock::new();
    DATASET.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        dataset
            .load_bill_download(&download("hr/1", "119-hr-1"))
            .expect("the law should load");
        dataset
            .load_bill_download(&download("hr/42", "119-hr-42"))
            .expect("the law should load");
        add_law_without_metadata(&mut dataset, "119-s-1071", "s/1071");
        for (date, title) in [
            ("2025-08-14", "usc10.xml"),
            ("2026-01-23", "usc10.xml"),
            ("2025-07-18", "usc43.xml"),
            ("2025-07-30", "usc43.xml"),
        ] {
            dataset
                .add_uslm_xml(&format!("tests/test_data/usc/{date}/{title}"), date, None)
                .expect("the title should parse");
        }
        dataset
    })
}

/// A value as JSON, so that two answers compare field by field.
fn json<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value).expect("the value should serialize")
}

#[test]
fn should_give_each_bill_the_answers_the_whole_run_gives_it_when_matched_for_that_bill_alone() {
    let whole = match_by_evidence(laws()).expect("the matcher should run");

    for bill in BILLS {
        let of_bill: Vec<_> = whole
            .matches
            .iter()
            .filter(|amendment| amendment.bill_id == bill)
            .collect();
        assert!(!of_bill.is_empty(), "{bill} should state amendments");

        let alone = match_bill_by_evidence(laws(), bill).expect("the matcher should run");

        assert_eq!(json(&alone), json(&of_bill), "the answers for {bill}");
    }
}

/// The same dataset, saved as a database for the binary to open.
fn laws_on_disk() -> &'static Path {
    static PATH: OnceLock<PathBuf> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = PathBuf::from(format!(
            "{}/residue_one_bill.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        ));
        let _ = std::fs::remove_file(&path);
        laws()
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        path
    })
}

/// What `residue --json` prints, with `args` after the dataset.
fn residue_json(args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .arg("residue")
        .arg(laws_on_disk())
        .args(args)
        .arg("--json")
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "residue should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("--json should emit json")
}

#[test]
fn should_list_the_rows_the_whole_residue_lists_for_a_bill_when_residue_is_asked_for_that_bill() {
    let whole = residue_json(&[]);

    for bill in BILLS {
        let of_bill: Vec<&serde_json::Value> = whole["rows"]
            .as_array()
            .expect("a rows array")
            .iter()
            .filter(|row| row["bill_id"] == bill)
            .collect();
        assert!(!of_bill.is_empty(), "{bill} should have rows");

        let alone = residue_json(&["--bill", bill]);

        assert_eq!(alone["rows"], json(&of_bill), "the rows for {bill}");
    }
}
