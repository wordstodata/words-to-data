//! A structural path can name more than one provision (#77).
//!
//! The law sometimes numbers two provisions alike. `26 U.S.C. § 45X(d)(4)` is
//! two paragraphs (4); the Code footnotes it "So in original" and the official
//! site renders both. Code that assumes a path is unique within an expression
//! does not fail loudly — it picks one and discards the other.

use words_to_data::uslm::{USLMElement, parser::parse};

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

/// Title 26 at one release point, held both ways.
fn dataset_both_backends(
    name: &str,
) -> (
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
    });
    memory
        .add_uslm_xml(USC26_30, "2025-07-30", None)
        .expect("the fixture should parse");

    let dir = std::path::Path::new("target/duplicate_path_dbs");
    std::fs::create_dir_all(dir).expect("create sqlite test dir");
    let file = dir.join(format!("{name}.sqlite"));
    std::fs::remove_file(&file).ok();
    memory.save_to_sqlite(&file).expect("save to sqlite");
    let sqlite = Dataset::open_sqlite(&file).expect("open sqlite");

    (memory, sqlite)
}

#[test]
fn should_hold_every_provision_sharing_a_path_on_both_backends() {
    let (memory, sqlite) = dataset_both_backends("find_element");

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
