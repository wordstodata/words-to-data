//! The words of a provision that a window holds at one end only (#259, B.8).
//!
//! A provision new in the window has no older end to compare against, so a
//! field diff says nothing about it. Before this, `path` said only
//! "added — now at position 0" and `settle --explain` said the dataset "does
//! not hold both ends". The new § 174A, which treats software development as
//! research, is the case a reviewer met: the link was right, and nothing on
//! screen showed it.
//!
//! Every dataset here is built from the committed corpus: title 26 at two
//! release points, and the links `link-by-evidence` wrote over them (#252).

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::link::Link;

const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// The section Public Law 119-21 § 70302(a) added to the Code.
const SECTION_174A: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174A";

/// Title 26 at both release points, with the committed evidence links, written
/// once as a compact dataset. Every test here only reads it.
fn dataset_on_disk() -> &'static str {
    static WRITTEN: OnceLock<String> = OnceLock::new();
    WRITTEN.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        for (file, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
            dataset
                .add_uslm_xml(file, date, None)
                .expect("title 26 should load");
        }
        let json = std::fs::read_to_string("tests/test_data/processed/evidence_links.json")
            .expect("the evidence links fixture should be readable");
        let links: Vec<Link> =
            serde_json::from_str(&json).expect("the fixture should parse as links");
        for link in links {
            if link.subject.name().contains("uscode/title_26") {
                dataset.add_link(link).expect("the link should be added");
            }
        }
        let path = format!("{}/one_end_words.json", env!("CARGO_TARGET_TMPDIR"));
        dataset
            .save(&path, Format::Compact)
            .expect("the fixture should save");
        path
    })
}

/// One run of the CLI, which must succeed.
fn run(args: &[&str]) -> String {
    let output: Output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "the command should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// `path` over the one window the corpus holds.
fn path_over(at_path: &str) -> String {
    run(&[
        "path",
        dataset_on_disk(),
        at_path,
        "--from",
        &format!("uscode/title_26@{BEFORE}"),
        "--to",
        &format!("uscode/title_26@{AFTER}"),
    ])
}

#[test]
fn should_show_the_words_added_when_path_reads_a_provision_new_in_the_window() {
    let said = path_over(SECTION_174A);

    assert!(
        said.contains("The words added"),
        "the reader should say the words shown are the words added: {said}"
    );
    assert!(
        said.contains("Software development"),
        "the heading of (d)(3) should be shown: {said}"
    );
    assert!(
        said.contains(
            "any amount paid or incurred in connection with the development of any software"
        ),
        "and the words of (d)(3): {said}"
    );
}
