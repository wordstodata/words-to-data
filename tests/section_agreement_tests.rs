//! Does an amendment's own text name the section its link points into?
//!
//! A cheap deterministic check, and a queue rather than a verdict. An amendment
//! that names § 263A and whose link points inside § 263 is suspect on its face,
//! and nothing reported it before (#239).
//!
//! **Three outcomes, not two.** A citation form the extractor declined is its
//! own case and never a disagreement: reporting it as one would manufacture a
//! false fault out of an extractor limitation (#140).
//!
//! Every link here is real. They are the committed output of one matching run
//! over the real corpus, so no model runs and nothing is invented. Only the
//! links are loaded: the check reads an amendment's words and a path, so the two
//! release points of title 26 are 112 MB of XML it never opens.

use std::process::Command;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::legislature::section_agreement::{self, Outcome};
use words_to_data::link::{Link, Named};
use words_to_data::query::LinkQuery;
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// The committed output of one real matching run.
const REAL_ANNOTATIONS: &str = "tests/test_data/processed/annotations.json";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// § 263(a)(1)(B), which the bill reached by an amendment to § 263A(c)(2).
///
/// The disagreement this check exists to find: the amendment names 263A and the
/// link points inside 263, and the two are different sections of the Code.
const SECTION_263_A_1_B: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_IX/section_263/subsection_a/paragraph_1/subparagraph_B";

/// Every amendment link the committed run recorded, over one window.
///
/// Unfiltered by title: the check reads a path and an amendment's words, so a
/// link into title 7 or title 20 is as checkable as one into title 26. The
/// annotations name thirteen titles and the third outcome only happens outside
/// title 26, so filtering would hide it.
fn amendment_links() -> Dataset<InMemoryStorage> {
    let json = std::fs::read_to_string(REAL_ANNOTATIONS).expect("the fixture should be readable");
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&json).expect("the fixture should parse as annotations");

    let mut dataset = Dataset::new(DatasetMetadata::default());
    for annotation in &annotations {
        // A link's subject is a change to one work, so each annotation is
        // recorded against the work its own paths sit in.
        for path in &annotation.paths {
            let Some(work) = work_of(path) else { continue };
            let from = ExpressionId::new(work.clone(), BEFORE);
            let to = ExpressionId::new(work, AFTER);
            let mut one = annotation.clone();
            one.paths = vec![path.clone()];
            for link in Link::from_annotation(&one, &from, &to) {
                dataset.add_link(link).expect("the link should be added");
            }
        }
    }
    dataset
}

/// `uscode/title_26` out of a path inside title 26.
fn work_of(path: &str) -> Option<WorkId> {
    let mut segments = path.split('/');
    let container = segments.next()?;
    let title = segments.next()?;
    Some(WorkId::new(format!("{container}/{title}")))
}

/// The disagreement the ticket names: § 263A cited, § 263 pointed at.
#[test]
fn should_report_the_link_when_the_amendment_names_a_different_section_from_its_path() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let row = report
        .rows
        .iter()
        .find(|row| row.path == SECTION_263_A_1_B)
        .expect("the run recorded a link at § 263(a)(1)(B)");

    assert_eq!(row.outcome, Outcome::Disagrees);
    assert_eq!(row.named_section.as_deref(), Some("263A"));
    assert_eq!(row.path_section.as_deref(), Some("263"));
}

/// § 1070a(b)(7)(A)(iii), which the bill reached through the Higher Education
/// Act of 1965.
///
/// The third outcome, on real data. The amendment names *section 401 of the
/// Act*, and an Act's own numbering is not the Code's, so the section is only
/// knowable from the citation beside it — `20 U.S.C. 1070a(b)(7)(A)(iii)`,
/// which the extractor declines because it carries no section marker. Reading
/// the bare `401` and comparing it against `1070a` would manufacture a fault out
/// of that limitation (#140).
const SECTION_1070A_B_7_A_III: &str = "uscode/title_20/chapter_28/subchapter_IV/part_A/subpart_1/section_1070a/subsection_b/paragraph_7/subparagraph_A/clause_iii";

/// A declined citation is its own case, and never a disagreement.
#[test]
fn should_report_its_own_case_when_the_extractor_declined_the_amendments_citation() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let row = report
        .rows
        .iter()
        .find(|row| row.path == SECTION_1070A_B_7_A_III)
        .expect("the run recorded a link at § 1070a(b)(7)(A)(iii)");

    assert_eq!(row.outcome, Outcome::CouldNotBeRead);
    // Why, in the extractor's own words, so a reviewer meets a named extractor
    // limit rather than a silent gap.
    let reason = row.reason.as_deref().expect("a third case says why");
    assert!(
        reason.contains("20 U.S.C. 1070a") && reason.contains("no section marker"),
        "the reason should name the declined citation and the extractor's own \
         words, found {reason:?}"
    );
}

/// The figure is reported per window, because the split between windows is the
/// signal that filed the ticket: one dataset showed 4% of its first window's
/// links disagreeing and 29% of its second window's.
///
/// **The committed corpus holds amendment links in one window only.** It holds
/// three release points, so it holds two windows, and the committed matching run
/// covers `2025-07-18 -> 2025-07-30` alone. So this proves the tally is taken
/// per window and that each window's share is computed from that window's own
/// links; reproducing the 4%-versus-29% split needs a dataset with a second
/// matching run in it.
#[test]
fn should_report_a_share_for_each_window_when_the_dataset_holds_amendment_links() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    let [window] = &report.windows[..] else {
        panic!(
            "the committed run covers one window, found {}",
            report.windows.len()
        );
    };
    assert_eq!(window.window.from_date, BEFORE);
    assert_eq!(window.window.to_date, AFTER);

    // Three counts that measure three different things, so the share is taken
    // from this window's own links and not from the dataset's total.
    assert_eq!(
        window.checked,
        window.agrees + window.disagrees + window.could_not_be_read
    );
    assert!(window.disagrees > 0, "the corpus holds disagreements");
    assert!(
        (window.disagreeing_share - window.disagrees as f64 / window.checked as f64).abs()
            < f64::EPSILON,
        "the share should be this window's disagreements over its own links"
    );
}

/// The report is a queue, ordered so a reviewer meets the suspect links first,
/// and each row names the link a reviewer would act on.
///
/// The id leads because this list is the review queue and the id is the one
/// field a reviewer copies out of it, into `settle` (#227). A link that agrees
/// is counted in its window's tally and is not queued: it asks nothing of
/// anybody.
#[test]
fn should_queue_the_disagreements_first_and_name_each_link_when_the_report_is_read() {
    let dataset = amendment_links();
    let report = section_agreement::section_agreement(&dataset, &LinkQuery::new())
        .expect("the check should read the dataset");

    assert!(
        report.rows.iter().all(|row| row.outcome != Outcome::Agrees),
        "a link that agrees asks nothing of a reviewer and is not queued"
    );

    let first_unread = report
        .rows
        .iter()
        .position(|row| row.outcome == Outcome::CouldNotBeRead)
        .expect("the corpus holds links whose citation was declined");
    assert!(
        report.rows[..first_unread]
            .iter()
            .all(|row| row.outcome == Outcome::Disagrees),
        "every disagreement should come before the first case that could not be read"
    );
    assert!(
        report.rows[first_unread..]
            .iter()
            .all(|row| row.outcome == Outcome::CouldNotBeRead),
        "nothing should follow the cases that could not be read"
    );

    // The short id `settle` accepts, so a reviewer copies the row's own field.
    let row = &report.rows[0];
    let link = dataset
        .link_by_id_prefix(&row.id)
        .expect("the id should look up");
    assert!(
        matches!(link, Named::One(_)),
        "a row's id should name exactly one link, found {link:?}"
    );
}

/// The command a reviewer runs, over a dataset saved to disk.
///
/// `--json` is the surface an agent reads, and it survives a change to the
/// human-readable text.
#[test]
fn should_report_the_queue_and_each_windows_share_when_the_command_runs() {
    let path = format!("{}/section_agreement.json", env!("CARGO_TARGET_TMPDIR"));
    amendment_links()
        .save(&path, Format::Compact)
        .expect("the fixture should save");

    let run = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["section-agreement", &path, "--json"])
        .output()
        .expect("the binary should run");
    assert!(
        run.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let report: serde_json::Value =
        serde_json::from_slice(&run.stdout).expect("--json should emit json");

    let window = &report["windows"][0];
    assert_eq!(window["window"]["from_date"], BEFORE);
    assert_eq!(window["window"]["to_date"], AFTER);
    assert!(
        window["disagrees"].as_u64().expect("a count") > 0,
        "the corpus holds disagreements"
    );
    assert!(
        window["disagreeing_share"].as_f64().expect("a share") > 0.0,
        "the share is the figure the ticket asks for, per window"
    );

    let rows = report["rows"].as_array().expect("the queue is an array");
    assert_eq!(rows[0]["outcome"], "disagrees", "the suspect links lead");
    assert!(
        rows.iter().all(|row| row["outcome"] != "agrees"),
        "a link that agrees asks nothing of a reviewer and is not queued"
    );
}
