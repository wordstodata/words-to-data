//! The readers report the newest review of a link (#415).
//!
//! After `settle --verdict refuted`, `annotations` and `path` still printed the
//! link as `[Pending]`, `annotations --json` gave `"status": "Pending"` and no
//! review, and `contradictions` and the "1 of N causes" count still counted
//! the refuted link. Only `settle --explain` reported the review. ADR 0012
//! says a reader reports the record
//! (`docs/adr/0012-a-review-is-its-own-link-and-a-reader-reports-the-record.md`).
//!
//! The links are the real output of one run of the evidence method over the
//! committed corpus, `tests/test_data/processed/evidence_links.json`. The
//! reviews are recorded through the one door a review goes through,
//! `review::record`, as `settle` records them.

mod common;

use std::collections::BTreeMap;
use std::process::{Command, Output};

use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, Format, WorkId};
use words_to_data::link::{Link, LinkKind};
use words_to_data::review::{self, Review, Verdict};
use words_to_data::storage::{InMemoryStorage, LinkReader};

/// One run of the evidence method over the committed corpus: 1195 links, and
/// five paths that more than one amendment changed.
const EVIDENCE_LINKS: &str = "tests/test_data/processed/evidence_links.json";

/// Every link of the fixture, in a dataset of its own.
fn evidence_dataset() -> Dataset<InMemoryStorage> {
    let json = std::fs::read_to_string(EVIDENCE_LINKS).expect("the fixture should be readable");
    let links: Vec<Link> = serde_json::from_str(&json).expect("the fixture should parse as links");
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for link in links {
        dataset.add_link(link).expect("the link should add");
    }
    dataset
}

/// The amendment links of `dataset`, by the path each one names.
fn amendment_links_by_path(dataset: &Dataset<InMemoryStorage>) -> BTreeMap<String, Vec<Link>> {
    let mut by_path: BTreeMap<String, Vec<Link>> = BTreeMap::new();
    for link in dataset
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("the links should read")
    {
        let path = link.subject.path().expect("an amendment link names a path");
        by_path.entry(path.to_string()).or_default().push(link);
    }
    by_path
}

/// A path that more than one amendment changed, and its links.
fn path_with_several_causes(dataset: &Dataset<InMemoryStorage>) -> (String, Vec<Link>) {
    amendment_links_by_path(dataset)
        .into_iter()
        .find(|(_, links)| links.len() > 1)
        .expect("the fixture holds a path that several amendments changed")
}

/// Record `verdict` on `link`, as `settle` records it, and give the id of the
/// record.
fn settle(dataset: &mut Dataset<InMemoryStorage>, link: &Link, verdict: Verdict) -> String {
    let record = Review {
        verdict,
        reviewer: "agent:tester".to_string(),
        reasoning: Some("the amendment's words are not in the change".to_string()),
        at: time::OffsetDateTime::new_utc(
            time::Date::from_calendar_date(2026, time::Month::October, 26).expect("a real date"),
            time::Time::MIDNIGHT,
        ),
    }
    .about(link);
    let id = record.id();
    review::record(dataset, record).expect("the review should record");
    id
}

/// Write `dataset` to a compact file of its own, named `name`.
fn on_disk(dataset: &Dataset<InMemoryStorage>, name: &str) -> String {
    let path = format!("{}/review_status_{name}.json", env!("CARGO_TARGET_TMPDIR"));
    dataset
        .save(&path, Format::Compact)
        .expect("the dataset should save");
    path
}

fn run(args: &[&str]) -> String {
    let output: Output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run");
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// The `annotations --json` rows at `path` exactly.
fn annotation_rows(file: &str, path: &str) -> Vec<serde_json::Value> {
    let said = run(&["annotations", file, "--path", path, "--exact", "--json"]);
    let answer: serde_json::Value = serde_json::from_str(&said).expect("the answer is JSON");
    answer["annotations"]
        .as_array()
        .expect("a list of annotations")
        .clone()
}

/// The row whose one link is `link`.
fn row_of<'a>(rows: &'a [serde_json::Value], link: &Link) -> &'a serde_json::Value {
    let short = review::short_id(&link.id()).to_string();
    rows.iter()
        .find(|row| row["link_ids"][0] == short.as_str())
        .unwrap_or_else(|| panic!("a row states link {short}: {rows:#?}"))
}

#[test]
fn should_give_rejected_and_the_review_record_when_annotations_json_reads_a_refuted_link() {
    let mut dataset = evidence_dataset();
    let (path, links) = path_with_several_causes(&dataset);
    let refuted = &links[0];
    let record = settle(&mut dataset, refuted, Verdict::Refuted);
    let file = on_disk(&dataset, "refuted_json");

    let rows = annotation_rows(&file, &path);
    let row = row_of(&rows, refuted);

    assert_eq!(
        row["status"], "Rejected",
        "the newest review refutes it: {row}"
    );
    let review = &row["links"][0]["review"];
    assert_eq!(review["verdict"], "refuted", "{row}");
    assert_eq!(review["reviewer"], "agent:tester", "{row}");
    assert_eq!(
        review["record"],
        review::short_id(&record),
        "the review names the record that states it: {row}"
    );
}

/// The lines of `said` that start with `start`, after their indent.
fn lines_starting(said: &str, start: &str) -> Vec<String> {
    said.lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with(start))
        .map(str::to_string)
        .collect()
}

#[test]
fn should_print_verified_and_who_confirmed_it_when_annotations_reads_a_confirmed_link() {
    let mut dataset = evidence_dataset();
    let (path, links) = path_with_several_causes(&dataset);
    let confirmed = &links[0];
    settle(&mut dataset, confirmed, Verdict::Confirmed);
    let file = on_disk(&dataset, "confirmed_text");

    let said = run(&["annotations", &file, "--path", &path, "--exact"]);

    let short = review::short_id(&confirmed.id()).to_string();
    let link_line = lines_starting(&said, &format!("link {short}"));
    assert_eq!(link_line.len(), 1, "one line names the link: {said}");
    assert!(
        link_line[0].contains("confirmed by agent:tester on 2026-10-26"),
        "the link line says who confirmed it and when: {said}"
    );
    let headers = lines_starting(&said, "[Verified]");
    assert_eq!(
        headers.len(),
        1,
        "the confirmed record reads Verified: {said}"
    );
    assert_eq!(
        lines_starting(&said, "[Pending]").len(),
        links.len() - 1,
        "the other records are not reviewed: {said}"
    );
}

#[test]
fn should_leave_a_refuted_link_out_of_the_causes_when_annotations_counts_the_other_links() {
    let mut dataset = evidence_dataset();
    let (path, links) = path_with_several_causes(&dataset);
    settle(&mut dataset, &links[0], Verdict::Refuted);
    let file = on_disk(&dataset, "refuted_causes");

    let rows = annotation_rows(&file, &path);

    for standing in &links[1..] {
        let row = row_of(&rows, standing);
        assert_eq!(
            row["links"][0]["causes"],
            links.len() - 1,
            "the refuted link is not a cause of the change: {row}"
        );
    }
}

#[test]
fn should_print_rejected_and_who_refuted_it_when_path_reads_a_refuted_link() {
    let mut dataset = evidence_dataset();
    let (path, links) = path_with_several_causes(&dataset);
    let refuted = &links[0];
    settle(&mut dataset, refuted, Verdict::Refuted);
    let file = on_disk(&dataset, "refuted_path");

    let said = run(&["path", &file, &path, "--exact"]);

    assert_eq!(
        lines_starting(&said, "[Rejected]").len(),
        1,
        "the refuted record reads Rejected: {said}"
    );
    let short = review::short_id(&refuted.id()).to_string();
    let link_line = lines_starting(&said, &format!("link {short}"));
    assert!(
        link_line
            .iter()
            .any(|line| line.contains("refuted by agent:tester on 2026-10-26")),
        "the link line says who refuted it and when: {said}"
    );
}

/// A path one link names, and that link beside a second from another maker:
/// an agent who linked another amendment of the fixture there, with
/// `link-amendment`. Two makers with different answers, which is a
/// disagreement.
fn disagreement_by_two_makers(dataset: &mut Dataset<InMemoryStorage>) -> (String, Link) {
    let by_path = amendment_links_by_path(dataset);
    let (path, batch_link) = by_path
        .iter()
        .find(|(_, links)| links.len() == 1)
        .map(|(path, links)| (path.clone(), links[0].clone()))
        .expect("the fixture holds a path that one amendment changed");
    let other_amendment = by_path
        .values()
        .flatten()
        .find(|link| link.object != batch_link.object)
        .expect("the fixture holds more than one amendment");
    let mut agent_link = batch_link.clone();
    agent_link.object = other_amendment.object.clone();
    agent_link.payload = other_amendment.payload.clone();
    agent_link.provenance.source = "agent:tester".to_string();
    agent_link.provenance.method = None;
    dataset.add_link(agent_link).expect("the link should add");
    (path, batch_link)
}

fn disagreeing_subjects(dataset: &Dataset<InMemoryStorage>) -> Vec<String> {
    words_to_data::inspect::contradictions(dataset)
        .expect("contradictions should be read")
        .disagreement
        .into_iter()
        .map(|group| group.subject)
        .collect()
}

#[test]
fn should_not_report_a_disagreement_when_the_other_link_at_the_subject_is_refuted() {
    let mut dataset = evidence_dataset();
    let (path, batch_link) = disagreement_by_two_makers(&mut dataset);
    let subject = batch_link.subject.name();
    // Guard: before the review, the two makers disagree.
    assert!(
        disagreeing_subjects(&dataset).contains(&subject),
        "two makers name different amendments at {path}"
    );

    settle(&mut dataset, &batch_link, Verdict::Refuted);

    assert!(
        !disagreeing_subjects(&dataset).contains(&subject),
        "the refuted link is settled against, so one answer stands at {path}"
    );
}

#[test]
fn should_say_how_many_refuted_links_it_left_out_when_contradictions_answers_a_person() {
    let mut dataset = evidence_dataset();
    let (_, batch_link) = disagreement_by_two_makers(&mut dataset);
    settle(&mut dataset, &batch_link, Verdict::Refuted);
    let file = on_disk(&dataset, "refuted_contradictions");

    let said = run(&["contradictions", &file]);

    assert!(
        said.contains("Refuted:      1 link(s) left out"),
        "the report says what it did not read as an answer: {said}"
    );
}

#[test]
fn should_give_mixed_and_each_links_own_status_when_one_link_of_a_record_is_refuted() {
    let mut dataset = evidence_dataset();
    // One amendment, one window, one maker, several paths: one record of
    // several links.
    let mut by_record: BTreeMap<(String, Option<String>), Vec<Link>> = BTreeMap::new();
    for links in amendment_links_by_path(&dataset).into_values() {
        for link in links {
            let window = link.subject.window().map(|window| window.to_string());
            by_record
                .entry((link.object.name(), window))
                .or_default()
                .push(link);
        }
    }
    let record = by_record
        .into_values()
        .find(|links| links.len() > 1)
        .expect("the fixture holds an amendment linked at more than one path");
    let refuted = &record[0];
    settle(&mut dataset, refuted, Verdict::Refuted);
    let file = on_disk(&dataset, "refuted_mixed");

    let short = review::short_id(&refuted.id()).to_string();
    let said = run(&["annotations", &file, "--bill", "119-hr-1", "--json"]);
    let answer: serde_json::Value = serde_json::from_str(&said).expect("the answer is JSON");
    let whole = answer["annotations"]
        .as_array()
        .expect("a list of annotations")
        .iter()
        .find(|row| {
            row["link_ids"]
                .as_array()
                .is_some_and(|ids| ids.len() > 1 && ids.iter().any(|id| id == short.as_str()))
        })
        .expect("the whole record is listed for its bill");

    assert_eq!(whole["status"], "Mixed", "its links differ: {whole}");
    for link in whole["links"].as_array().expect("links") {
        let expected = if link["id"] == short.as_str() {
            "Rejected"
        } else {
            "Pending"
        };
        assert_eq!(link["status"], expected, "{link}");
    }
}

#[test]
fn should_list_the_change_as_unclaimed_when_coverage_reads_a_path_whose_only_link_is_refuted() {
    let mut dataset = evidence_dataset();
    let (before, after) = ("2025-07-18", "2025-07-30");
    for date in [before, after] {
        common::add_uslm_xml(
            &mut dataset,
            &format!("tests/test_data/usc/{date}/usc26.xml"),
            date,
        );
    }
    let from = ExpressionId::new(WorkId::new("uscode/title_26"), before);
    let to = ExpressionId::new(WorkId::new("uscode/title_26"), after);
    let unclaimed = |dataset: &Dataset<InMemoryStorage>| {
        words_to_data::inspect::coverage(dataset, &from, &to)
            .expect("coverage should be read")
            .unannotated_paths
    };
    let unclaimed_before = unclaimed(&dataset);
    let diff = words_to_data::inspect::diff(&dataset, &from, &to).expect("the diff should read");
    let changed = |path: &String| {
        diff.changed_paths.contains(path)
            || diff.added_paths.contains(path)
            || diff.removed_paths.contains(path)
    };
    let (path, only_link) = amendment_links_by_path(&dataset)
        .into_iter()
        .find(|(path, links)| {
            links.len() == 1
                && path.starts_with("uscode/title_26/")
                && links[0]
                    .subject
                    .window()
                    .is_some_and(|w| w.from_date == before)
                && changed(path)
                && !unclaimed_before.contains(path)
        })
        .map(|(path, links)| (path, links[0].clone()))
        .expect("the fixture claims a change of the window with one link");

    settle(&mut dataset, &only_link, Verdict::Refuted);

    assert!(
        unclaimed(&dataset).contains(&path),
        "a refuted link claims no change, so {path} is unclaimed again"
    );
}

/// The short ids of the links `section-agreement --json` queues.
fn queued(file: &str) -> Vec<String> {
    let said = run(&["section-agreement", file, "--json"]);
    let report: serde_json::Value = serde_json::from_str(&said).expect("the answer is JSON");
    report["rows"]
        .as_array()
        .expect("the queue is a list")
        .iter()
        .map(|row| {
            row["id"]
                .as_str()
                .expect("a row names its link")
                .to_string()
        })
        .collect()
}

/// A review queue that keeps a refuted link asks a reviewer to review it again
/// (#415). The links here name a bill the fixture holds no document for, so
/// every one of them is queued as "could not be read" until it is refuted.
#[test]
fn should_leave_a_refuted_link_out_of_the_queue_when_section_agreement_reads_it() {
    let mut dataset = evidence_dataset();
    let (_, links) = path_with_several_causes(&dataset);
    let refuted = &links[0];
    let short = review::short_id(&refuted.id()).to_string();
    let before = queued(&on_disk(&dataset, "agreement_before"));
    assert!(before.contains(&short), "queued before the review");

    settle(&mut dataset, refuted, Verdict::Refuted);
    let after = queued(&on_disk(&dataset, "agreement_after"));

    assert!(
        !after.contains(&short),
        "{short} is refuted, and still queued"
    );
    assert_eq!(after.len(), before.len() - 1);
}
