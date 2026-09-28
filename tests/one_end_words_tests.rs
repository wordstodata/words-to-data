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

/// The scholarship credit Public Law 119-21 added: 65 text fields, the longest
/// 392 characters. Too large for a screen, so it is what a bound is tested on.
const SECTION_25F: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_A/section_25F";

#[test]
fn should_stop_at_a_screenful_and_say_what_it_left_out_when_an_added_provision_is_large() {
    let said = path_over(SECTION_25F);

    assert!(
        said.contains("The words added (65 fields)"),
        "the reader should say how many fields the provision holds: {said}"
    );
    assert!(
        said.contains("45 more fields not shown"),
        "and how many it did not print, after a screenful of 20: {said}"
    );
    assert!(
        !said.contains("for purposes of administering the requirements of this section"),
        "the last field is past the screenful: {said}"
    );
    // Subsection (a) runs to 331 characters. It is cut, and the cut is marked.
    assert!(
        said.contains("In the case of an individual who is a citizen or resident"),
        "a long field starts as written: {said}"
    );
    assert!(
        !said.contains("aggregate amount of qualified contributions made by the taxpayer"),
        "and is cut before its end: {said}"
    );
    assert!(
        said.contains("qualified contributions made by the ta…"),
        "where the cut is marked: {said}"
    );
    assert!(
        said.contains("--json"),
        "the reader is told where the rest is: {said}"
    );
}

#[test]
fn should_carry_every_field_in_full_when_path_answers_in_json() {
    let said = run(&[
        "path",
        dataset_on_disk(),
        SECTION_25F,
        "--from",
        &format!("uscode/title_26@{BEFORE}"),
        "--to",
        &format!("uscode/title_26@{AFTER}"),
        "--json",
    ]);
    let report: serde_json::Value = serde_json::from_str(&said).expect("--json emits json");
    let words = report["provisions"][0]["words"]
        .as_array()
        .expect("an added provision carries its words");

    assert_eq!(words.len(), 65, "every field is carried: {said}");
    assert!(
        words.iter().any(|w| w["text"]
            .as_str()
            .is_some_and(|t| t.ends_with(
                "aggregate amount of qualified contributions made by the taxpayer during the taxable year."
            ))),
        "and each field is carried whole"
    );
}

/// The paragraph Public Law 119-21 struck from § 132(f): the suspension of the
/// bicycle commuting exclusion.
const PARAGRAPH_132_F_8: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_III/section_132/subsection_f/paragraph_8";

#[test]
fn should_show_the_words_removed_when_path_reads_a_provision_gone_in_the_window() {
    let said = path_over(PARAGRAPH_132_F_8);

    assert!(
        said.contains("The words removed"),
        "the reader should say the words shown are the words removed: {said}"
    );
    assert!(
        said.contains("Suspension of qualified bicycle commuting reimbursement exclusion"),
        "and show them: {said}"
    );
}

/// The committed evidence link whose subject is `path`.
fn link_at(path: &str) -> Link {
    let json = std::fs::read_to_string("tests/test_data/processed/evidence_links.json")
        .expect("the evidence links fixture should be readable");
    let links: Vec<Link> = serde_json::from_str(&json).expect("the fixture should parse as links");
    links
        .into_iter()
        .find(|link| link.subject.name().ends_with(path))
        .unwrap_or_else(|| panic!("the fixture holds a link at {path}"))
}

/// `settle --explain` on the committed link at `path`.
fn explain(path: &str) -> String {
    let id = link_at(path).id();
    run(&[
        "settle",
        dataset_on_disk(),
        "--link",
        &id[..12],
        "--explain",
    ])
}

#[test]
fn should_show_the_words_added_when_a_reviewer_explains_a_link_to_a_new_provision() {
    let said = explain(SECTION_174A);

    assert!(
        !said.contains("does not hold both ends"),
        "the dataset holds the end that matters: {said}"
    );
    assert!(
        said.contains("The words added"),
        "the reader should say the words shown are the words added: {said}"
    );
    assert!(
        said.contains(
            "any amount paid or incurred in connection with the development of any software"
        ),
        "and show the words of (d)(3): {said}"
    );
}

#[test]
fn should_show_the_words_removed_when_a_reviewer_explains_a_link_to_a_struck_provision() {
    let said = explain(PARAGRAPH_132_F_8);

    assert!(
        said.contains("The words removed"),
        "the reader should say the words shown are the words removed: {said}"
    );
    assert!(
        said.contains("Paragraph (1)(D) shall not apply to any taxable year"),
        "and show them: {said}"
    );
}
