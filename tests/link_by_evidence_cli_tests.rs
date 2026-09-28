//! `words_to_data link-by-evidence` on a dataset whose public law was stored
//! before the bill kept its quoted strings (#257, #259).
//!
//! The fixture `tests/test_data/processed/bill_stored_before_257.zip` is a
//! real dataset, made by the code as it stood before #257 (the parent of
//! commit `6c86733`): the committed public law `119-hr-1`, loaded with
//! `Dataset::load_bill_download` and saved as compact JSON. It holds no
//! release point, because only the stored bill matters here. It is zipped to
//! keep the repository small.

use std::io::Read;
use std::process::Command;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, Format};

const FIXTURE: &str = "tests/test_data/processed/bill_stored_before_257.zip";
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";

/// The fixture, unzipped into `dir`, and its path.
fn dataset_stored_before_257(dir: &std::path::Path) -> std::path::PathBuf {
    let file = std::fs::File::open(FIXTURE).expect("the fixture should be committed");
    let mut archive = zip::ZipArchive::new(file).expect("the fixture should be a zip");
    let mut entry = archive.by_index(0).expect("the zip holds the dataset");
    let mut json = String::new();
    entry
        .read_to_string(&mut json)
        .expect("the dataset should read");
    let path = dir.join("stored_before_257.json");
    std::fs::write(&path, json).expect("the dataset should write");
    path
}

fn link_by_evidence(input: &std::path::Path, output: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "link-by-evidence",
            input.to_str().expect("a UTF-8 path"),
            "--output",
            output.to_str().expect("a UTF-8 path"),
        ])
        .output()
        .expect("the binary should run")
}

#[test]
fn should_warn_and_name_the_law_and_the_fix_when_a_public_law_is_stored_with_no_quoted_strings() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let input = dataset_stored_before_257(dir.path());

    let run = link_by_evidence(&input, &dir.path().join("linked.json"));

    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "the command still runs: {stderr}");
    assert!(
        stderr.contains("warning:") && stderr.contains("119-21"),
        "the warning names the law: {stderr}"
    );
    assert!(
        stderr.contains("build-dataset"),
        "the warning says how to fix it: {stderr}"
    );
}

#[test]
fn should_not_warn_when_the_public_law_is_stored_with_its_quoted_strings() {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&BillDownload {
            bill_id: "119-hr-1".to_string(),
            bill_xml: read("public_law.xml"),
            bill_metadata_json: read("metadata.json"),
            cosponsors_json: read("cosponsors.json"),
            votes_json: None,
            member_jsons: std::collections::HashMap::new(),
        })
        .expect("the committed bill should load");
    let dir = tempfile::tempdir().expect("a temporary directory");
    let input = dir.path().join("current.json");
    dataset
        .save(input.to_str().expect("a UTF-8 path"), Format::Compact)
        .expect("the dataset should save");

    let run = link_by_evidence(&input, &dir.path().join("linked.json"));

    let stderr = String::from_utf8_lossy(&run.stderr);
    assert!(run.status.success(), "the command runs: {stderr}");
    assert!(!stderr.contains("warning:"), "no warning: {stderr}");
}
