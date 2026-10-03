//! The OLRC's classification of a section, as `path` and `settle --explain`
//! show it (#259, B.15).
//!
//! The dataset stores `olrc.classified_from` links (#247), and no reader printed
//! them. So a reviewer confirming the link to the new § 174A could not see the
//! authority's own statement that § 70302(a) of Public Law 119-21 made it: the
//! strongest corroboration there is.
//!
//! The dataset is built from the committed corpus: title 26 at two release
//! points, Public Law 119-21, the links `link-by-evidence` wrote (#252), and the
//! classifications `add-classifications` reads off the committed OLRC page.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::link::Link;

const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const PUBLIC_LAW: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// The section Public Law 119-21 § 70302(a) added to the Code.
const SECTION_174A: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174A";

/// The committed dataset with the OLRC's classifications added, written once.
fn dataset_on_disk() -> &'static str {
    static WRITTEN: OnceLock<String> = OnceLock::new();
    WRITTEN.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        for (file, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
            dataset
                .add_uslm_xml(file, date, None)
                .expect("title 26 should load");
        }
        let xml = std::fs::read_to_string(PUBLIC_LAW).expect("the public law is committed");
        let document = roxmltree::Document::parse(&xml).expect("the public law is XML");
        let (expression, _) =
            words_to_data::uslm::bill_parser::bill_expression(&document, "119-hr-1")
                .expect("the public law should parse");
        dataset
            .add_expression(expression)
            .expect("the public law should store");
        for link in evidence_links() {
            if link.subject.name().contains("uscode/title_26") {
                dataset.add_link(link).expect("the link should be added");
            }
        }

        let unclassified = format!(
            "{}/olrc_reader_unclassified.json",
            env!("CARGO_TARGET_TMPDIR")
        );
        let classified = format!("{}/olrc_reader.json", env!("CARGO_TARGET_TMPDIR"));
        dataset
            .save(&unclassified, Format::Compact)
            .expect("the fixture should save");
        run(&[
            "add-classifications",
            &unclassified,
            "--offline",
            "--cache-dir",
            "tests/test_data",
            "--output",
            &classified,
        ]);
        classified
    })
}

fn evidence_links() -> Vec<Link> {
    let json = std::fs::read_to_string("tests/test_data/processed/evidence_links.json")
        .expect("the evidence links fixture should be readable");
    serde_json::from_str(&json).expect("the fixture should parse as links")
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

/// The table puts a note on § 174A too: § 70302(c) of the law, `nt new`. That
/// row is about a note, and a reviewer must not read it as about the text.
#[test]
fn should_say_a_row_classifies_a_note_when_its_descriptions_are_all_notes() {
    let said = run(&["path", dataset_on_disk(), SECTION_174A]);

    assert!(
        said.contains(
            "classified by the OLRC from Pub. L. 119-21 §70302(c), nt new (a note, not the section's text)"
        ),
        "a note-only row should say it classifies a note: {said}"
    );
    assert!(
        said.contains("classified by the OLRC from Pub. L. 119-21 §70302(a), new\n"),
        "and a row about the text should carry no such words: {said}"
    );
}

/// A reviewer confirming the link to § 174A reads the OLRC's row beside it.
#[test]
fn should_show_the_olrc_row_when_a_reviewer_explains_a_link_to_a_classified_section() {
    let link = evidence_links()
        .into_iter()
        .find(|link| link.subject.name().ends_with(SECTION_174A))
        .expect("the fixture holds the link to § 174A");
    let id = link.id();

    let said = run(&[
        "settle",
        dataset_on_disk(),
        "--link",
        &id[..12],
        "--explain",
    ]);

    assert!(
        said.contains("classified by the OLRC from Pub. L. 119-21 §70302(a), new"),
        "the OLRC's statement should be beside the link: {said}"
    );
}

/// Section 161, whose preceding heading § 70302(d) of the law changed: `prec`.
const SECTION_161: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_161";

#[test]
fn should_say_a_row_classifies_a_heading_when_its_description_is_prec() {
    let said = run(&["path", dataset_on_disk(), SECTION_161]);

    assert!(
        said.contains(
            "classified by the OLRC from Pub. L. 119-21 §70302(d), prec (a heading before the section, not the section's text)"
        ),
        "a prec row should say it classifies a heading: {said}"
    );
}
