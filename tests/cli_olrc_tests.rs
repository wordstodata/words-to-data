//! `words_to_data add-classifications`, driven as a subprocess (#247).
//!
//! The dataset under test holds the real Public Law 119-21 and title 26 of the
//! 2025-07-30 release point. The table is the real OLRC page committed under
//! `tests/test_data/olrc/classification`, read with `--offline` so that no test
//! reaches the network.

use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::link::{Link, LinkKind};
use words_to_data::storage::LinkReader;

/// Build a SQLite dataset holding Public Law 119-21 and title 26.
fn public_law_and_title_26(name: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "OLRC classification fixture".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30", None)
        .expect("title 26 should parse");

    let xml =
        std::fs::read_to_string("tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml")
            .expect("the public law is committed");
    let document = roxmltree::Document::parse(&xml).expect("the public law is XML");
    let (expression, _) = words_to_data::uslm::bill_parser::bill_expression(&document, "119-hr-1")
        .expect("the public law should parse");
    dataset
        .add_expression(expression)
        .expect("the public law should store");

    dataset.save_to_sqlite(&path).expect("the fixture should save");
    path
}

/// Run the CLI the way a shell would, reading only the committed page.
fn add_classifications(dataset: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "add-classifications",
            dataset,
            "--offline",
            "--cache-dir",
            "tests/test_data",
        ])
        .output()
        .expect("the binary should run")
}

/// Every classification link the dataset at `path` holds.
fn classifications(path: &str) -> Vec<Link> {
    let dataset = Dataset::open_sqlite(path).expect("the dataset should open");
    dataset
        .storage()
        .links_by_kind(LinkKind::CLASSIFIED_FROM)
        .expect("the links should read")
}

#[test]
fn should_store_the_same_links_once_when_the_command_runs_twice() {
    let dataset = public_law_and_title_26("olrc_twice");

    assert!(add_classifications(&dataset).status.success());
    let mut first: Vec<String> = classifications(&dataset).iter().map(Link::id).collect();
    assert!(add_classifications(&dataset).status.success());
    let mut second: Vec<String> = classifications(&dataset).iter().map(Link::id).collect();

    first.sort();
    second.sort();
    assert!(!first.is_empty());
    assert_eq!(first, second, "a link is identified by what it says");
}

#[test]
fn should_store_the_classifications_of_a_public_law_when_the_dataset_holds_it() {
    let dataset = public_law_and_title_26("olrc_store");

    let output = add_classifications(&dataset);
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stdout: {said}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let links = classifications(&dataset);
    assert!(!links.is_empty(), "title 26 holds sections 119-21 classified");
    assert!(
        links
            .iter()
            .all(|link| link.object.name().starts_with("olrc.classification:119-21:")),
        "every link names a section of Public Law 119-21"
    );
    assert!(
        said.contains("119-21: 635 row(s)"),
        "the run says how many rows it read for the law: {said}"
    );
}
