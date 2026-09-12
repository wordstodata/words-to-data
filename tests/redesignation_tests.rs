//! Redesignations: a provision a bill renumbered, and the link that records it.
//!
//! Every case here is read out of the committed corpus. The motivating one is
//! `119-hr-1`: "Section 898(c) is amended by striking paragraph (2) and
//! redesignating paragraph (3) as paragraph (2)."

use std::process::Command;

use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, Format, WorkId, work_roots,
};
use words_to_data::diff::{Redesignations, TreeDiff};
use words_to_data::legislature::redesignation::{RedesignationReport, Step, read_clause, resolve};
use words_to_data::link::{LinkKind, Target, VerificationState};
use words_to_data::storage::{InMemoryStorage, LinkReader};
use words_to_data::uslm::bill_redesignation::redesignations_stated_in_file;
use words_to_data::uslm::parser::parse;

/// The committed public law, which holds every redesignation in the corpus.
const BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-hr-1";

/// Title 26 before and after `119-hr-1` reached the Code.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// § 898(c), the provision the diff reported wrongly before this existed.
const SUBSECTION_898_C: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_II/subpart_D/section_898/subsection_c";

#[test]
fn should_read_one_renumbering_when_the_clause_names_one_paragraph() {
    // 119-hr-1, § 70352(a). The motivating case.
    let clause = "Section 898(c) is amended by striking paragraph (2) and \
                  redesignating paragraph (3) as paragraph (2).";

    let renumberings = read_clause(clause).expect("the clause should be readable");

    let written: Vec<String> = renumberings.iter().map(ToString::to_string).collect();
    assert_eq!(written, vec!["paragraph_3 -> paragraph_2"]);
}

#[test]
fn should_name_the_section_under_amendment_when_the_bill_states_a_redesignation() {
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");

    let found = stated
        .iter()
        .find(|s| s.text.contains("Section 898(c)"))
        .expect("the bill states a redesignation in section 898(c)");

    assert_eq!(found.section.as_deref(), Some("/us/usc/t26/s898"));
    assert_eq!(found.container, vec![Step::numbered("c")]);
    let written: Vec<String> = found.renumberings.iter().map(ToString::to_string).collect();
    assert_eq!(written, vec!["paragraph_3 -> paragraph_2"]);
}

#[test]
fn should_resolve_the_paragraph_898_c_renumbered_when_title_26_is_in_hand() {
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let before = parse(TITLE_26_BEFORE, BEFORE).expect("title 26 should parse");

    let report = resolve(&stated, &before);

    let moved = report
        .resolved
        .iter()
        .find(|r| r.from_path == format!("{SUBSECTION_898_C}/paragraph_3"))
        .expect("paragraph (3) of § 898(c) should resolve");
    assert_eq!(moved.to_path, format!("{SUBSECTION_898_C}/paragraph_2"));
}

/// The paths of the children one diff node reports as removed.
fn removed_paths(at: &words_to_data::diff::TreeDiff) -> Vec<String> {
    at.removed
        .iter()
        .map(|node| node.path.to_string())
        .collect()
}

#[test]
fn should_report_a_renumbered_paragraph_as_rewritten_when_no_redesignation_is_known() {
    // What the diff said before this existed, and what it still says with no
    // redesignation in hand. Old (2) was struck and old (3) took its number, so
    // pairing by position reads old (2) against a paragraph that is really old
    // (3): a large rewrite, plus children appearing out of nowhere.
    let before = parse(TITLE_26_BEFORE, BEFORE).expect("title 26 should parse");
    let after = parse(TITLE_26_AFTER, AFTER).expect("title 26 should parse");

    let diff = TreeDiff::from_nodes(&before, &after);
    let at = diff.find(SUBSECTION_898_C).expect("§ 898(c) changed");

    let rewritten = at
        .child_diffs
        .iter()
        .find(|child| child.root_path.ends_with("/paragraph_2"))
        .expect("paragraph (2) reads as rewritten");
    assert!(!rewritten.changes.is_empty(), "its words read as changed");
    assert!(
        !rewritten.added.is_empty(),
        "and it reads as gaining children"
    );
    assert_eq!(
        removed_paths(at),
        vec![format!("{SUBSECTION_898_C}/paragraph_3")],
        "and paragraph (3) reads as simply gone"
    );
    assert!(at.moved.is_empty(), "nothing is reported as moved");
}

#[test]
fn should_report_a_renumbered_paragraph_as_moved_when_the_bill_said_so() {
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let before = parse(TITLE_26_BEFORE, BEFORE).expect("title 26 should parse");
    let after = parse(TITLE_26_AFTER, AFTER).expect("title 26 should parse");
    let known = Redesignations::from_pairs(
        resolve(&stated, &before)
            .resolved
            .iter()
            .map(|r| (r.from_path.clone(), r.to_path.clone())),
    );

    let diff = TreeDiff::from_nodes_with(&before, &after, &known);
    let at = diff.find(SUBSECTION_898_C).expect("§ 898(c) changed");

    // Paragraph (2) was struck.
    assert_eq!(
        removed_paths(at),
        vec![format!("{SUBSECTION_898_C}/paragraph_2")]
    );
    // Paragraph (3) became paragraph (2), with its words untouched.
    let moved: Vec<(String, String)> = at
        .moved
        .iter()
        .map(|m| (m.from.path.to_string(), m.to.path.to_string()))
        .collect();
    assert_eq!(
        moved,
        vec![(
            format!("{SUBSECTION_898_C}/paragraph_3"),
            format!("{SUBSECTION_898_C}/paragraph_2"),
        )]
    );
    assert!(
        at.moved[0].changes.is_empty(),
        "the renumbering changed no words"
    );
    // And nothing reads as a rewrite of paragraph (2), which is the false
    // statement this whole change exists to remove.
    assert!(at.child_diffs.is_empty(), "no child reads as changed");
    assert!(at.added.is_empty(), "no child reads as added");
}

#[test]
fn should_pair_by_position_when_no_bill_redesignated_the_path() {
    // § 6724(d)(2), where two subparagraphs (JJ) became one between these two
    // release points and no bill in the corpus redesignated anything in the
    // section. Its pairing must be untouched, whether or not redesignations are
    // known elsewhere in the title
    // (`docs/adr/0001-structural-paths-locate-not-identify.md`).
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let before = parse(TITLE_26_BEFORE, BEFORE).expect("title 26 should parse");
    let after = parse(TITLE_26_AFTER, AFTER).expect("title 26 should parse");
    let known = Redesignations::from_pairs(
        resolve(&stated, &before)
            .resolved
            .iter()
            .map(|r| (r.from_path.clone(), r.to_path.clone())),
    );

    let by_position = TreeDiff::from_nodes(&before, &after);
    let over_links = TreeDiff::from_nodes_with(&before, &after, &known);

    let by_position_at = find_section(&by_position, "section_6724");
    let over_links_at = find_section(&over_links, "section_6724");
    assert!(
        !by_position_at.is_empty(),
        "§ 6724 changed between these release points"
    );
    assert_eq!(by_position_at, over_links_at);
}

/// One section's diff, as JSON, found by the last segment of its path.
///
/// By segment rather than by whole path, so a test does not have to spell out
/// every subtitle, chapter and part above the section.
fn find_section(diff: &TreeDiff, segment: &str) -> String {
    if diff.root_path.ends_with(segment) {
        return serde_json::to_string(diff).expect("a diff serializes");
    }
    diff.child_diffs
        .iter()
        .map(|child| find_section(child, segment))
        .find(|found| !found.is_empty())
        .unwrap_or_default()
}

/// Title 26 at both release points, in a dataset, with `119-hr-1`'s
/// redesignations recorded as links.
fn dataset_with_redesignations() -> (
    Dataset<InMemoryStorage>,
    ExpressionId,
    ExpressionId,
    RedesignationReport,
) {
    let work = WorkId::new("uscode/title_26");
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (path, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
        let parsed = parse(path, date).expect("title 26 should parse");
        let root = work_roots(parsed).pop().expect("the file holds one title");
        dataset
            .add_expression(Expression {
                id: ExpressionId::new(work.clone(), date),
                label: None,
                root,
            })
            .expect("the expression should store");
    }
    let from = ExpressionId::new(work.clone(), BEFORE);
    let to = ExpressionId::new(work, AFTER);

    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let report = dataset
        .record_redesignations(BILL_ID, &stated, &from, &to)
        .expect("the redesignations should record");
    (dataset, from, to, report)
}

#[test]
fn should_record_a_link_naming_the_bill_when_a_redesignation_resolves() {
    let (dataset, _, _, _) = dataset_with_redesignations();

    let links = dataset
        .links_for_path(&format!("{SUBSECTION_898_C}/paragraph_3"))
        .expect("links should read");
    let redesignation = links
        .iter()
        .find(|link| link.kind.0 == LinkKind::REDESIGNATED_AS)
        .expect("paragraph (3) carries a redesignation link");

    // The object names where the provision went, and carries the dates, so the
    // statement is not read as holding for all time.
    assert_eq!(
        redesignation.object,
        Target::Change {
            work: WorkId::new("uscode/title_26"),
            path: format!("{SUBSECTION_898_C}/paragraph_2"),
            from_date: BEFORE.to_string(),
            to_date: AFTER.to_string(),
        }
    );
    // And the provenance names the bill that said so.
    let payload = redesignation.payload.as_ref().expect("a payload");
    assert_eq!(payload.value["bill_id"], BILL_ID);
    assert_eq!(
        redesignation.provenance.verification,
        VerificationState::MachineSuggested
    );
    assert!(
        redesignation
            .provenance
            .evidence
            .as_ref()
            .and_then(|e| e.reasoning.as_deref())
            .is_some_and(|words| words.contains("redesignating paragraph (3)")),
        "the words the statement was read out of travel with it"
    );
}

#[test]
fn should_report_that_paragraph_2_was_formerly_paragraph_3_when_asked_for_its_history() {
    let (dataset, _, _, _) = dataset_with_redesignations();

    let history = dataset
        .provision_history(&format!("{SUBSECTION_898_C}/paragraph_2"))
        .expect("a history should read");

    assert_eq!(
        history.earliest(),
        format!("{SUBSECTION_898_C}/paragraph_3")
    );
    assert_eq!(history.latest(), format!("{SUBSECTION_898_C}/paragraph_2"));
    assert!(history.covers(&format!("{SUBSECTION_898_C}/paragraph_3")));
    assert_eq!(history.steps.len(), 1);
    assert_eq!(history.steps[0].from_date, BEFORE);
    assert_eq!(history.steps[0].to_date, AFTER);
}

#[test]
fn should_report_no_renumbering_when_a_provision_kept_its_number() {
    let (dataset, _, _, _) = dataset_with_redesignations();
    let untouched = "uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041/subsection_d";

    let history = dataset
        .provision_history(untouched)
        .expect("a history should read");

    // An answer, not a failure: the provision has always been where it is.
    assert!(history.steps.is_empty());
    assert_eq!(history.earliest(), untouched);
    assert_eq!(history.paths(), vec![untouched]);
}

#[test]
fn should_diff_over_the_links_when_a_dataset_holds_redesignations() {
    let (dataset, from, to, _) = dataset_with_redesignations();

    let diff = dataset.compute_diff(&from, &to).expect("a diff");
    let at = diff.find(SUBSECTION_898_C).expect("§ 898(c) changed");

    assert_eq!(
        removed_paths(at),
        vec![format!("{SUBSECTION_898_C}/paragraph_2")]
    );
    assert_eq!(at.moved.len(), 1);
    assert_eq!(
        at.moved[0].to.path.to_string(),
        format!("{SUBSECTION_898_C}/paragraph_2")
    );
    assert!(at.child_diffs.is_empty());
}

#[test]
fn should_print_the_statements_it_could_not_place_when_the_command_runs() {
    // Silence is the failure mode: a run that resolved nothing and said nothing
    // would read as a corpus with no redesignations in it (#110).
    let path = format!("{}/redesignations.json", env!("CARGO_TARGET_TMPDIR"));
    let mut dataset = Dataset::new(DatasetMetadata::default());
    for (file, date) in [(TITLE_26_BEFORE, BEFORE), (TITLE_26_AFTER, AFTER)] {
        dataset
            .add_uslm_xml(file, date, None)
            .expect("title 26 should load");
    }
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "redesignations",
            &path,
            "--bill-xml",
            BILL,
            "--bill-id",
            BILL_ID,
            "--between",
            BEFORE,
            AFTER,
            // Beside the input, not over it, so the fixture stays free of links
            // and the run can be repeated.
            "--output",
            &format!("{}/redesignations_linked.json", env!("CARGO_TARGET_TMPDIR")),
        ])
        .output()
        .expect("the binary should run");

    assert!(output.status.success(), "the command should succeed");
    let said = String::from_utf8_lossy(&output.stdout);
    assert!(
        said.contains("states 57 redesignation(s)"),
        "it says how many the bill states: {said}"
    );
    assert!(
        said.contains("could not be placed"),
        "it says which it could not place: {said}"
    );
    assert!(
        said.contains("a table of sections, not a provision"),
        "and why, in words: {said}"
    );
}

/// The titles the corpus's redesignations name, as release-point file names.
///
/// Only the titles needed. Sweeping all fifty-seven files to place fifty-seven
/// statements would spend minutes proving the same thing.
const TITLES_NAMED: [&str; 7] = [
    "usc05.xml",
    "usc07.xml",
    "usc10.xml",
    "usc15.xml",
    "usc20.xml",
    "usc26.xml",
    "usc42.xml",
];

/// The whole corpus's redesignations, resolved against every title they name.
///
/// This is the number the issue asks for: how many of the statements in the
/// corpus become links, and how many are reported instead.
#[test]
fn should_resolve_most_of_the_corpus_and_report_the_rest() {
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");

    let per_work: Vec<RedesignationReport> = TITLES_NAMED
        .iter()
        .map(|file| {
            let path = format!("tests/test_data/usc/{BEFORE}/{file}");
            let root = parse(&path, BEFORE).unwrap_or_else(|e| panic!("{path} should parse: {e}"));
            resolve(&stated, &root)
        })
        .collect();
    let report = RedesignationReport::across_works(per_work);

    // What the sweep found, printed so the numbers in the pull request can be
    // read straight off a run.
    println!(
        "stated={} resolved={} unresolved={}",
        stated.len(),
        report.resolved.len(),
        report.unresolved.len()
    );
    for unresolved in &report.unresolved {
        println!("  {}: {}", unresolved.reason, unresolved.text);
    }

    // Every statement is accounted for: one that resolved nowhere carries a
    // reason. This is the assertion that keeps a silent parse from passing.
    let placed: std::collections::HashSet<(&str, &str)> = report
        .resolved
        .iter()
        .map(|r| (r.amendment_id.as_str(), r.text.as_str()))
        .collect();
    let named: std::collections::HashSet<(&str, &str)> = report
        .unresolved
        .iter()
        .map(|u| (u.amendment_id.as_str(), u.text.as_str()))
        .collect();
    for statement in &stated {
        let key = (statement.amendment_id.as_str(), statement.text.as_str());
        assert!(
            placed.contains(&key) || named.contains(&key),
            "every statement is either resolved or reported: {}",
            statement.text
        );
    }

    // A run that resolves nothing is the failure this test exists to catch.
    assert!(
        report.resolved.len() >= 60,
        "the corpus's redesignations should mostly resolve, and {} did",
        report.resolved.len()
    );
}

#[test]
fn should_report_every_redesignation_the_corpus_states() {
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");

    // 57 `redesignate` actions sit in the committed public law. Each one is a
    // statement, and none may be dropped: a statement this build cannot place
    // carries the reason instead.
    assert_eq!(stated.len(), 57);
    assert!(stated.iter().all(|s| !s.text.is_empty()));
    assert!(
        stated
            .iter()
            .all(|s| s.unreadable.is_some() || !s.renumberings.is_empty()),
        "a statement says what it found or why it found nothing"
    );
}
