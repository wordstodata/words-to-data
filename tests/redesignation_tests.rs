//! Redesignations: a provision a bill renumbered, and the link that records it.
//!
//! Every case here is read out of the committed corpus. The motivating one is
//! `119-hr-1`: "Section 898(c) is amended by striking paragraph (2) and
//! redesignating paragraph (3) as paragraph (2)."

use std::process::Command;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, Format, WorkId, work_roots,
};
use words_to_data::diff::{Redesignations, TreeDiff};
use words_to_data::legislature::redesignation::{
    Reason, RedesignationReport, Step, read_clause, resolve,
};
use words_to_data::link::{LinkKind, Target, VerificationState};
use words_to_data::storage::{InMemoryStorage, LinkReader};
use words_to_data::uslm::bill_redesignation::redesignations_stated_in_file;
use words_to_data::uslm::parser::parse;

/// The committed public law, which holds every redesignation in the corpus.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-hr-1";

/// Title 26 before and after `119-hr-1` reached the Code.
const TITLE_26_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const TITLE_26_AFTER: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const BEFORE: &str = "2025-07-18";
const AFTER: &str = "2025-07-30";

/// Title 7 before and after the same bill.
///
/// The title that holds a statement whose new path the law never took, which is
/// what the check on the later document exists to catch.
const TITLE_7_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc07.xml";
const TITLE_7_AFTER: &str = "tests/test_data/usc/2025-07-30/usc07.xml";

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
    let after = parse(TITLE_26_AFTER, AFTER).expect("title 26 should parse");

    let report = resolve(&stated, &before, &after);

    let moved = report
        .resolved
        .iter()
        .find(|r| r.from_path == format!("{SUBSECTION_898_C}/paragraph_3"))
        .expect("paragraph (3) of § 898(c) should resolve");
    assert_eq!(moved.to_path, format!("{SUBSECTION_898_C}/paragraph_2"));
}

#[test]
fn should_report_a_redesignation_when_the_new_path_is_absent_after_the_bill() {
    // 119-hr-1: "by redesignating paragraph (1) as subparagraph (A) and
    // indenting appropriately", against title 7 § 9034(b). On 2025-07-30 that
    // subsection still holds a paragraph (1) and a paragraph (2), and holds no
    // subparagraph (A) at all, so the reading names a place the law does not
    // have. Before this check the build recorded the link anyway, because it
    // looked at the earlier document only.
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let earlier = parse(TITLE_7_BEFORE, BEFORE).expect("title 7 should parse");
    let later = parse(TITLE_7_AFTER, AFTER).expect("title 7 should parse");

    let report = resolve(&stated, &earlier, &later);

    let absent =
        "uscode/title_7/chapter_115/subchapter_II/section_9034/subsection_b/subparagraph_A";
    assert!(
        report.resolved.iter().all(|r| r.to_path != absent),
        "a path the later document does not hold is not linked"
    );
    let reported = report
        .unplaced
        .iter()
        .find(|u| {
            u.text
                .contains("redesignating paragraph (1) as subparagraph (A)")
        })
        .expect("the statement is reported rather than dropped");
    assert_eq!(
        reported.reason,
        Reason::RenumberedProvisionNotHeld(absent.to_string())
    );
    // The words a maintainer reads, which must say which end failed.
    assert_eq!(
        reported.reason.to_string(),
        format!("no provision at {absent} after the bill")
    );
}

/// Title 42 before and after the same bill.
const TITLE_42_BEFORE: &str = "tests/test_data/usc/2025-07-18/usc42.xml";
const TITLE_42_AFTER: &str = "tests/test_data/usc/2025-07-30/usc42.xml";

/// § 1397gg(e)(1), where `119-hr-1` shifted a whole run of subparagraphs by one
/// letter: "by redesignating subparagraphs (H) through (U) as subparagraphs (I)
/// through (V), respectively".
const PARAGRAPH_1397GG_E_1: &str =
    "uscode/title_42/chapter_7/subchapter_XXI/section_1397gg/subsection_e/paragraph_1";

/// The measure every redesignation link records, named so a receiving party can
/// recompute the figure instead of trusting it.
const MEASURE: &str = "similar::TextDiff::from_words ratio over heading, chapeau, proviso, \
                       content, continuation, joined by one space";

/// The link one resolved redesignation of title 42 becomes.
fn link_for(from_path: &str, report: &RedesignationReport) -> words_to_data::link::Link {
    report
        .resolved
        .iter()
        .find(|r| r.from_path == from_path)
        .unwrap_or_else(|| panic!("{from_path} should resolve"))
        .link(&WorkId::new("uscode/title_42"), BEFORE, AFTER, BILL_ID)
}

#[test]
fn should_corroborate_a_link_with_the_words_at_its_two_ends() {
    // The M to N step of the § 1397gg(e)(1) run. Both subparagraph (M) and
    // subparagraph (N) are in the law on **both** dates, because the run shifted
    // by one letter, so the two existence checks pass whatever letter a misread
    // landed on. Old (M) and new (N) hold one sentence, character for character,
    // and that identity is the evidence
    // (`docs/adr/0010-two-readers-one-resolver-a-model-never-writes-a-path.md`).
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let earlier = parse(TITLE_42_BEFORE, BEFORE).expect("title 42 should parse");
    let later = parse(TITLE_42_AFTER, AFTER).expect("title 42 should parse");

    let report = resolve(&stated, &earlier, &later);
    let link = link_for(&format!("{PARAGRAPH_1397GG_E_1}/subparagraph_M"), &report);

    let corroboration = link
        .provenance
        .corroboration
        .as_ref()
        .expect("a resolved redesignation carries a corroboration");
    assert_eq!(corroboration.method, MEASURE);
    assert_eq!(corroboration.score, 1.0);
    assert_eq!(
        corroboration.detail,
        vec![("own_text".to_string(), 1.0), ("subtree".to_string(), 1.0)],
        "both parts are named, so the headline figure can be checked"
    );
    // And the figure does not raise how far the link can be trusted. A machine's
    // proposal that scores well is still a machine's proposal (`CONTEXT.md`).
    assert_eq!(
        link.provenance.verification,
        VerificationState::MachineSuggested
    );
}

#[test]
fn should_still_record_the_link_when_the_bill_renumbered_and_rewrote_at_once() {
    // The S to T step of the same § 1397gg(e)(1) run, and the same clause. Old
    // (S) listed four subsections of § 1396u–2; new (T) is one sentence about
    // § 1396r–1a. The bill renumbered and rewrote in one breath, which is
    // ordinary, so the words at the two ends share almost nothing.
    //
    // The link is recorded all the same. Different words are no evidence against
    // a renumbering, and refusing here would delete the record of what the bill
    // said. `MachineSuggested` beside a low figure is the honest account: the
    // bill said this, and the words do not back it up.
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let earlier = parse(TITLE_42_BEFORE, BEFORE).expect("title 42 should parse");
    let later = parse(TITLE_42_AFTER, AFTER).expect("title 42 should parse");

    let report = resolve(&stated, &earlier, &later);
    let link = link_for(&format!("{PARAGRAPH_1397GG_E_1}/subparagraph_S"), &report);

    let corroboration = link
        .provenance
        .corroboration
        .as_ref()
        .expect("the link is recorded, and it carries the figure");
    assert!(
        (corroboration.score - 0.218_75).abs() < 1e-4,
        "the figure is low and it is recorded: {}",
        corroboration.score
    );
    assert_eq!(
        link.object,
        Target::Change {
            work: WorkId::new("uscode/title_42"),
            path: format!("{PARAGRAPH_1397GG_E_1}/subparagraph_T"),
            from_date: BEFORE.to_string(),
            to_date: AFTER.to_string(),
        }
    );
    assert_eq!(
        link.provenance.verification,
        VerificationState::MachineSuggested,
        "a low figure lowers nothing either"
    );
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
        resolve(&stated, &before, &after)
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
        resolve(&stated, &before, &after)
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
/// Title 26 at both release points, and the pair of expressions they name.
///
/// The law as it read before and after `119-hr-1`, with no links recorded yet.
fn dataset_holding_title_26() -> (Dataset<InMemoryStorage>, ExpressionId, ExpressionId) {
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
    (dataset, from, to)
}

/// The bill as the Congress client would hand it over, read from the committed
/// cache.
///
/// The same four files a download leaves behind, so a test exercises the path a
/// build really takes rather than a shape invented for the test.
fn committed_bill_download() -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: BILL_ID.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: std::collections::HashMap::new(),
    }
}

fn dataset_with_redesignations() -> (
    Dataset<InMemoryStorage>,
    ExpressionId,
    ExpressionId,
    RedesignationReport,
) {
    let (mut dataset, from, to) = dataset_holding_title_26();

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
    // The bill as well, because the command reads it from the dataset rather
    // than from a file beside it (#196).
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the bill should load");
    dataset
        .save(&path, Format::Compact)
        .expect("the fixture should save");

    let output = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args([
            "redesignations",
            &path,
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

/// One title at both release points, by its file name.
fn release_pair(
    file: &str,
) -> (
    words_to_data::document::DocumentNode,
    words_to_data::document::DocumentNode,
) {
    let read = |date: &str| {
        let path = format!("tests/test_data/usc/{date}/{file}");
        parse(&path, date).unwrap_or_else(|e| panic!("{path} should parse: {e}"))
    };
    (read(BEFORE), read(AFTER))
}

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
            let (earlier, later) = release_pair(file);
            resolve(&stated, &earlier, &later)
        })
        .collect();
    let report = RedesignationReport::across_works(per_work);

    // What the sweep found, printed so the numbers in the pull request can be
    // read straight off a run.
    let with_figure = report
        .resolved
        .iter()
        .filter(|r| r.corroboration.detail.len() == 2)
        .count();
    println!(
        "stated={} resolved={} unplaced={} with_figure={}",
        stated.len(),
        report.resolved.len(),
        report.unplaced.len(),
        with_figure
    );
    for unplaced in &report.unplaced {
        println!("  {}: {}", unplaced.reason, unplaced.text);
    }
    for resolved in &report.resolved {
        println!(
            "  figure={:.4} {} -> {}",
            resolved.corroboration.score, resolved.from_path, resolved.to_path
        );
    }

    // Every link the corpus resolves carries a figure a receiving party can
    // recompute. A link without one is a statement nobody can check.
    assert_eq!(
        with_figure,
        report.resolved.len(),
        "every resolved link carries a corroboration"
    );

    // Every statement is accounted for: one that resolved nowhere carries a
    // reason. This is the assertion that keeps a silent parse from passing.
    let placed: std::collections::HashSet<(&str, &str)> = report
        .resolved
        .iter()
        .map(|r| (r.amendment_id.as_str(), r.text.as_str()))
        .collect();
    let named: std::collections::HashSet<(&str, &str)> = report
        .unplaced
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

/// The three numbers a person reading the sweep wants (#166).
///
/// One clause states many renumberings — "redesignating subparagraphs (H)
/// through (U) as subparagraphs (I) through (V)" is one statement and fourteen
/// links — so a count of links is not a count of statements, and the sum of the
/// two counts neither.
#[test]
fn should_count_statements_links_and_unplaced_statements_when_it_sweeps_the_corpus() {
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");

    let per_work: Vec<RedesignationReport> = TITLES_NAMED
        .iter()
        .map(|file| {
            let (earlier, later) = release_pair(file);
            resolve(&stated, &earlier, &later)
        })
        .collect();
    let report = RedesignationReport::across_works(per_work);

    assert_eq!(
        report.statements(),
        57,
        "the bill states 57 redesignations, and one clause counts once"
    );
    assert_eq!(
        report.links(),
        81,
        "those statements become 81 renumberings this build can place"
    );
    assert_eq!(
        report.statements_unplaced(),
        17,
        "17 statements stay unplaced, and each one is reported"
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

#[test]
fn should_record_the_redesignations_a_bill_states_when_the_bill_is_loaded() {
    // #150. Loading the bill is what records them, so no caller can build a
    // dataset that silently holds none. The maintainer rebuilt the corpus after
    // #93 merged and got 889 `legislature.amended_by` links and zero
    // redesignations, because the only sign of the gap was an absence.
    let (mut dataset, _, _) = dataset_holding_title_26();

    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the bill should load");

    let links = dataset
        .links_for_path(&format!("{SUBSECTION_898_C}/paragraph_3"))
        .expect("links should read");
    let redesignation = links
        .iter()
        .find(|link| link.kind.0 == LinkKind::REDESIGNATED_AS)
        .expect("loading the bill records that § 898(c)(3) became (2)");

    assert_eq!(
        redesignation.object,
        Target::Change {
            work: WorkId::new("uscode/title_26"),
            path: format!("{SUBSECTION_898_C}/paragraph_2"),
            from_date: BEFORE.to_string(),
            to_date: AFTER.to_string(),
        }
    );
}

#[test]
fn should_shorten_the_clause_when_it_reports_a_statement_it_could_not_place() {
    // #164. A clause that renumbers a section can also enact a whole new one,
    // and then the clause carries the whole of the enacted text: § 1062 of
    // `119-hr-1` runs past six thousand characters. Printed in full, nineteen of
    // these buried the ten real lines of a rebuild's output.
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let earlier = parse(TITLE_26_BEFORE, BEFORE).expect("title 26 should parse");
    let later = parse(TITLE_26_AFTER, AFTER).expect("title 26 should parse");

    let report = resolve(&stated, &earlier, &later);

    let longest = report
        .unplaced
        .iter()
        .max_by_key(|u| u.text.chars().count())
        .expect("the corpus states something this build cannot place");
    assert!(
        longest.text.chars().count() > 1_000,
        "the fixture is a clause that enacts text, and this one is {} characters",
        longest.text.chars().count()
    );

    let line = format!("{longest}");
    assert!(
        line.chars().count() < 260,
        "one statement stays one line, and this one is {} characters: {line}",
        line.chars().count()
    );
    assert!(
        line.ends_with('…'),
        "a shortened clause says that it was shortened: {line}"
    );
    // The reason is what a maintainer acts on, so it is never shortened.
    assert!(
        line.contains(&longest.reason.to_string()),
        "the reason survives whole: {line}"
    );
}

#[test]
fn should_name_the_bill_and_all_three_counts_when_it_summarises_a_sweep() {
    // A rebuild printed nineteen warnings and no count, so the report read as a
    // failure rather than as work done. The summary is the line that says what
    // was recorded (#164). The count of statements is the third number, which
    // the type could not give until #166.
    let stated = redesignations_stated_in_file(BILL_ID, BILL).expect("the bill should parse");
    let earlier = parse(TITLE_26_BEFORE, BEFORE).expect("title 26 should parse");
    let later = parse(TITLE_26_AFTER, AFTER).expect("title 26 should parse");

    let report = resolve(&stated, &earlier, &later);

    assert_eq!(
        report.summary(BILL_ID),
        format!(
            "{BILL_ID}: 57 statement(s), {} link(s) recorded, {} statement(s) not placed",
            report.links(),
            report.statements_unplaced()
        ),
        "the line names the bill, the statements it read, the links it recorded, \
         and what it could not place"
    );
}
