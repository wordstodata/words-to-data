//! A structural path can name more than one provision (#77).
//!
//! The law sometimes numbers two provisions alike. `26 U.S.C. § 45X(d)(4)` is
//! two paragraphs (4); the Code footnotes it "So in original" and the official
//! site renders both. Code that assumes a path is unique within an expression
//! does not fail loudly — it picks one and discards the other.

use words_to_data::inspect::PathMatch;
use words_to_data::uslm::{USLMElement, parser::parse};

const USC26_18: &str = "tests/test_data/usc/2025-07-18/usc26.xml";
const USC26_30: &str = "tests/test_data/usc/2025-07-30/usc26.xml";

/// Both paragraphs (4) of § 45X(d), which the U.S. Code renders in full.
const DUPLICATED: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_45X/subsection_d/paragraph_4";

fn title_26() -> USLMElement {
    parse(USC26_30, "2025-07-30").expect("Error running parser")
}

#[test]
fn should_return_every_provision_sharing_a_path_when_finding_by_path() {
    let doc = title_26();

    let found = doc.find_all(DUPLICATED);

    assert_eq!(
        found.len(),
        2,
        "the law numbers two paragraphs (4) here and both should come back, got {:?}",
        found
            .iter()
            .map(|e| e.data.heading.as_deref())
            .collect::<Vec<_>>()
    );
    // Document order decides which is which, so the order must be the source's.
    assert!(
        found[0]
            .data
            .heading
            .as_deref()
            .is_some_and(|h| h.contains("Sale of")),
        "the first should be the one the document puts first, got {:?}",
        found[0].data.heading.as_deref()
    );
    assert!(
        found[1]
            .data
            .heading
            .as_deref()
            .is_some_and(|h| h.contains("prohibited foreign entities")),
        "the second should be the one the document puts second, got {:?}",
        found[1].data.heading.as_deref()
    );
}

#[test]
fn should_not_panic_when_finding_a_path_that_names_two_provisions() {
    let doc = title_26();

    // `words_to_data path` passes user input straight through, and this
    // aborted the process rather than answering.
    let found = doc.find(DUPLICATED);

    assert!(found.is_some(), "the path exists and should be found");
}

#[test]
fn should_answer_none_when_a_path_merely_shares_a_prefix() {
    let doc = title_26();

    // A near miss is a question with an answer, not a reason to abort. The
    // prefix check used to strip "uscode" and split on "/", leaving one
    // segment, and assert that there were more.
    assert!(doc.find("uscodex").is_none());
    assert!(doc.find("uscode/title_26x").is_none());
    assert!(doc.find("").is_none());
}

#[test]
fn should_match_a_whole_segment_rather_than_a_path_suffix_when_finding() {
    let doc = title_26();

    // The lookup filtered children with `path.ends_with(segment)`, which asks a
    // different question than "is this the child named by this segment".
    let real = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";
    assert!(
        doc.find(real).is_some(),
        "the real path should still resolve"
    );

    // A segment that is a suffix of a sibling's name must not match it.
    assert!(doc.find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a/paragraph_").is_none());
}

/// Collect every added element in the diff tree, with the path it sits at.
fn added_paths(diff: &words_to_data::diff::TreeDiff, out: &mut Vec<String>) {
    for element in &diff.added {
        out.push(element.path.to_string());
    }
    for child in &diff.child_diffs {
        added_paths(child, out);
    }
}

#[test]
fn should_report_a_new_provision_that_shares_a_path_with_an_existing_one() {
    use words_to_data::diff::TreeDiff;

    let before = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let after = title_26();

    // The earlier expression holds one paragraph (4) at 45X(d); the later holds
    // two. The second is new law, and a diff that keys children by path alone
    // never sees it.
    assert_eq!(
        before.find_all(DUPLICATED).len(),
        1,
        "the earlier expression should hold one paragraph (4)"
    );
    assert_eq!(
        after.find_all(DUPLICATED).len(),
        2,
        "the later expression should hold two"
    );

    let diff = TreeDiff::from_elements(&before, &after);
    let mut added = Vec::new();
    added_paths(&diff, &mut added);

    assert!(
        added.iter().any(|p| p == DUPLICATED),
        "the new paragraph (4) should be reported as added"
    );
}

/// Title 26 at both release points, so a path report has a pair to compare.
fn title_26_both_expressions()
-> words_to_data::dataset::Dataset<words_to_data::storage::InMemoryStorage> {
    use words_to_data::dataset::{Dataset, DatasetMetadata};

    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Duplicate Path Pair".to_string(),
        description: "Title 26, two release points".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(USC26_18, "2025-07-18", None)
        .expect("the earlier fixture should parse");
    dataset
        .add_uslm_xml(USC26_30, "2025-07-30", None)
        .expect("the later fixture should parse");
    dataset
}

fn title_26_at(date: &str) -> words_to_data::dataset::ExpressionId {
    use words_to_data::dataset::{ExpressionId, WorkId};
    ExpressionId::new(WorkId::new("uscode/title_26"), date)
}

#[test]
fn should_say_which_provision_was_added_when_a_path_gains_one() {
    use words_to_data::inspect::{self, Presence};

    let dataset = title_26_both_expressions();
    let (from, to) = (title_26_at("2025-07-18"), title_26_at("2025-07-30"));

    let report = inspect::path_report(&dataset, DUPLICATED, Some((&from, &to)), PathMatch::Subtree)
        .expect("path_report");

    // Each expression is named once, with the number of provisions it holds.
    // Naming the later expression twice was the whole defect (#90).
    let presence: Vec<(&str, usize)> = report
        .present_in
        .iter()
        .map(|p| (p.expression.as_str(), p.provisions))
        .collect();
    assert_eq!(
        presence,
        vec![
            ("uscode/title_26@2025-07-18", 1),
            ("uscode/title_26@2025-07-30", 2),
        ]
    );

    assert_eq!(
        report.provisions.len(),
        2,
        "the later expression holds two paragraphs (4), so the pair has two slots"
    );

    // The first paragraph (4) survived, and it is the one the footnote was
    // added to. Attributing that heading change to the new provision inverts
    // the meaning of the report.
    assert_eq!(report.provisions[0].presence, Presence::InBoth);
    assert_eq!(report.provisions[0].from_position, Some(0));
    assert_eq!(report.provisions[0].to_position, Some(0));
    assert!(
        report.provisions[0]
            .changes
            .iter()
            .any(|c| c.field == "heading"),
        "the surviving provision carries the heading change, got {:?}",
        report.provisions[0].changes
    );

    // The second is new law. It has no counterpart, so it has no changes.
    assert_eq!(report.provisions[1].presence, Presence::Added);
    assert_eq!(report.provisions[1].from_position, None);
    assert_eq!(report.provisions[1].to_position, Some(1));
    assert!(
        report.provisions[1].changes.is_empty(),
        "a provision that did not exist before has nothing to compare against"
    );
}

/// `26 U.S.C. § 6724(d)(2)(JJ)` is two subparagraphs at 2025-07-18 and one at
/// 2025-07-30. The survivor is the first: the one citing section 6226(a)(2).
const LOST_ONE: &str = "uscode/title_26/subtitle_F/chapter_68/subchapter_B/part_II/section_6724/subsection_d/paragraph_2/subparagraph_JJ";
/// `26 U.S.C. § 951A(d)(3)`: two paragraphs at 2025-07-18, none at 2025-07-30.
const LOST_BOTH: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_III/subpart_F/section_951A/subsection_d/paragraph_3";
/// `26 U.S.C. § 45Y(b)(1)(E)`: absent at 2025-07-18, two subparagraphs at 2025-07-30.
const GAINED_BOTH: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_45Y/subsection_b/paragraph_1/subparagraph_E";

#[test]
fn should_say_which_provision_was_removed_when_a_path_loses_one() {
    use words_to_data::inspect::{self, Presence};

    let dataset = title_26_both_expressions();
    let (from, to) = (title_26_at("2025-07-18"), title_26_at("2025-07-30"));

    let report = inspect::path_report(&dataset, LOST_ONE, Some((&from, &to)), PathMatch::Subtree)
        .expect("path_report");

    let presence: Vec<(&str, usize)> = report
        .present_in
        .iter()
        .map(|p| (p.expression.as_str(), p.provisions))
        .collect();
    assert_eq!(
        presence,
        vec![
            ("uscode/title_26@2025-07-18", 2),
            ("uscode/title_26@2025-07-30", 1),
        ]
    );

    assert_eq!(report.provisions.len(), 2);

    // Position pairing keeps the leading provision. The document agrees here:
    // the subparagraph citing section 6226(a)(2) is the one that survived, and
    // it lost the "two subpars. (JJ) have been enacted" footnote with it.
    assert_eq!(report.provisions[0].presence, Presence::InBoth);
    assert_eq!(report.provisions[0].to_position, Some(0));
    let content = report.provisions[0]
        .changes
        .iter()
        .find(|c| c.field == "content")
        .expect("the surviving subparagraph changed its content");
    assert!(
        content
            .old_value
            .contains("Two subpars. (JJ) have been enacted"),
        "the older text should carry the footnote, got {:?}",
        content.old_value
    );
    assert!(
        content.new_value.contains("section 6226(a)(2)")
            && !content.new_value.contains("Two subpars"),
        "the newer text should keep the 6226 citation and drop the footnote, got {:?}",
        content.new_value
    );

    // The second is gone, and a removal has nothing to compare against.
    assert_eq!(report.provisions[1].presence, Presence::Removed);
    assert_eq!(report.provisions[1].from_position, Some(1));
    assert_eq!(report.provisions[1].to_position, None);
    assert!(report.provisions[1].changes.is_empty());
}

#[test]
fn should_report_every_provision_as_removed_when_a_path_disappears() {
    use words_to_data::inspect::{self, Presence};

    let dataset = title_26_both_expressions();
    let (from, to) = (title_26_at("2025-07-18"), title_26_at("2025-07-30"));

    let report = inspect::path_report(&dataset, LOST_BOTH, Some((&from, &to)), PathMatch::Subtree)
        .expect("path_report");

    // Only the earlier expression holds the path at all.
    assert_eq!(report.present_in.len(), 1);
    assert_eq!(
        report.present_in[0].expression,
        "uscode/title_26@2025-07-18"
    );
    assert_eq!(report.present_in[0].provisions, 2);

    assert_eq!(report.provisions.len(), 2);
    for (i, provision) in report.provisions.iter().enumerate() {
        assert_eq!(provision.presence, Presence::Removed);
        assert_eq!(provision.from_position, Some(i));
        assert_eq!(provision.to_position, None);
    }
}

#[test]
fn should_report_every_provision_as_added_when_a_path_is_new() {
    use words_to_data::inspect::{self, Presence};

    let dataset = title_26_both_expressions();
    let (from, to) = (title_26_at("2025-07-18"), title_26_at("2025-07-30"));

    let report = inspect::path_report(
        &dataset,
        GAINED_BOTH,
        Some((&from, &to)),
        PathMatch::Subtree,
    )
    .expect("path_report");

    assert_eq!(report.present_in.len(), 1);
    assert_eq!(
        report.present_in[0].expression,
        "uscode/title_26@2025-07-30"
    );
    assert_eq!(report.present_in[0].provisions, 2);

    assert_eq!(report.provisions.len(), 2);
    for (i, provision) in report.provisions.iter().enumerate() {
        assert_eq!(provision.presence, Presence::Added);
        assert_eq!(provision.from_position, None);
        assert_eq!(provision.to_position, Some(i));
    }
}

#[test]
fn should_report_counts_but_no_provisions_when_no_expression_pair_is_given() {
    use words_to_data::inspect;

    let dataset = title_26_both_expressions();

    let report =
        inspect::path_report(&dataset, DUPLICATED, None, PathMatch::Subtree).expect("path_report");

    // Without a pair there is nothing to compare, but where the path lives is
    // still answerable and is still the question `present_in` exists for.
    assert!(report.provisions.is_empty());
    assert_eq!(
        report
            .present_in
            .iter()
            .map(|p| p.provisions)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

/// Title 26 at one release point, held both ways.
///
/// The caller must keep the returned directory in scope: dropping it removes the
/// SQLite database.
fn dataset_both_backends() -> (
    tempfile::TempDir,
    words_to_data::dataset::Dataset<words_to_data::storage::InMemoryStorage>,
    words_to_data::dataset::Dataset<words_to_data::storage::SqliteStorage>,
) {
    use words_to_data::dataset::{Dataset, DatasetMetadata};

    let mut memory = Dataset::new(DatasetMetadata {
        name: "Duplicate Path Fixture".to_string(),
        description: "Title 26, one release point".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    memory
        .add_uslm_xml(USC26_30, "2025-07-30", None)
        .expect("the fixture should parse");

    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("dataset.sqlite");
    memory.save_to_sqlite(&file).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&file).expect("open sqlite");

    (dir, memory, sqlite)
}

#[test]
fn should_hold_every_provision_sharing_a_path_on_both_backends() {
    let (_dir, memory, sqlite) = dataset_both_backends();

    let from_memory = memory.find_element(DUPLICATED).expect("find in memory");
    let from_sqlite = sqlite.find_element(DUPLICATED).expect("find in sqlite");

    // Both paragraphs (4) are law and both must be reachable. Keying storage by
    // path alone dropped one of them, silently.
    assert_eq!(
        from_memory.len(),
        2,
        "the in-memory backend should hold both provisions"
    );
    assert_eq!(
        from_sqlite.len(),
        2,
        "the SQLite backend should hold both provisions"
    );

    let headings = |found: &[(words_to_data::dataset::ExpressionId, USLMElement)]| -> Vec<String> {
        found
            .iter()
            .map(|(_, e)| e.data.heading.as_deref().unwrap_or("<none>").to_string())
            .collect()
    };
    assert_eq!(
        headings(&from_sqlite),
        headings(&from_memory),
        "both backends should answer alike, in document order"
    );
}

/// A path the fixture does not hold: § 45X(d) has no paragraph (99).
const ABSENT: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_D/section_45X/subsection_d/paragraph_99";

#[test]
fn should_say_whether_a_path_exists_on_both_backends() {
    use words_to_data::storage::DocumentReader;

    let (_dir, memory, sqlite) = dataset_both_backends();

    assert!(
        memory.has_element(DUPLICATED).expect("ask memory"),
        "the in-memory backend holds this path"
    );
    assert!(
        sqlite.has_element(DUPLICATED).expect("ask sqlite"),
        "the SQLite backend holds this path"
    );

    assert!(
        !memory.has_element(ABSENT).expect("ask memory"),
        "the in-memory backend does not hold this path"
    );
    assert!(
        !sqlite.has_element(ABSENT).expect("ask sqlite"),
        "the SQLite backend does not hold this path"
    );
}

#[test]
fn should_say_a_path_exists_when_it_names_more_than_one_provision() {
    use words_to_data::storage::DocumentReader;

    let (_dir, memory, sqlite) = dataset_both_backends();

    // The question is whether at least one provision sits at the path. Two
    // paragraphs (4) sit at this one, and a check which expects a path to name
    // exactly one provision cannot answer for it (ADR 0001, #77, #85).
    assert_eq!(
        memory
            .find_element(DUPLICATED)
            .expect("find in memory")
            .len(),
        2,
        "the fixture must really hold two provisions here"
    );

    assert!(
        memory.has_element(DUPLICATED).expect("ask memory"),
        "two provisions at a path still means the path exists"
    );
    assert!(
        sqlite.has_element(DUPLICATED).expect("ask sqlite"),
        "two provisions at a path still means the path exists"
    );
}
