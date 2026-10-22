//! A case is a work: `judicial/case_<cluster>`, with each writing as a child
//! (`docs/adr/0014-a-case-is-a-work-and-a-citation-runs-paragraph-to-subdivision.md`,
//! decisions 1 and 2; #313).
//!
//! *Boeing Co. v. United States*, 537 U.S. 437 (2003), is the case that shows
//! why. CourtListener holds it as cluster 122262 with three records: 122262 is
//! one combined record of the whole case, 9434364 is the Court's opinion by
//! Justice Stevens, and 9434365 is Justice Thomas's dissent. The two split
//! records hold the same words as the combined one, so the split records are the
//! text and the combined one is only a source. Every record here is a real
//! CourtListener response committed under `tests/test_data/courtlistener`.

use std::process::{Command, Output};

use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, Expression};
use words_to_data::document::DocumentNode;
use words_to_data::judicial::{CaseFacts, OpinionFacts};

const CACHE: &str = "tests/test_data/courtlistener";

/// One committed record, as the API returned it.
fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{CACHE}/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

fn opinion(id: u64) -> OpinionRecord {
    OpinionRecord::from_json(&read(&format!("opinion_{id}.json"))).expect("the opinion should read")
}

fn cluster(id: u64) -> ClusterRecord {
    ClusterRecord::from_json(&read(&format!("cluster_{id}.json"))).expect("the cluster should read")
}

/// *Boeing*, built from all three records CourtListener holds for it.
fn boeing() -> Expression {
    case_expression(
        &cluster(122262),
        &[opinion(122262), opinion(9434364), opinion(9434365)],
    )
    .expect("the case should build")
    .expression
}

/// The children of the case root whose node type is `node_type`.
fn children_of_type<'a>(root: &'a DocumentNode, node_type: &str) -> Vec<&'a DocumentNode> {
    root.children
        .iter()
        .filter(|child| child.data.node_type.as_str() == node_type)
        .collect()
}

#[test]
fn should_store_boeing_as_one_case_with_a_majority_and_a_dissent_when_the_cluster_has_split_records()
 {
    let case = boeing();

    assert_eq!(case.id.work.as_str(), "judicial/case_122262");
    assert_eq!(case.id.at, "2003-03-04", "the cluster's filing date");
    assert_eq!(&*case.root.data.path, "judicial/case_122262");

    let writings: Vec<&str> = children_of_type(&case.root, "judicial.opinion")
        .iter()
        .map(|writing| &*writing.data.path)
        .collect();
    assert_eq!(
        writings,
        [
            "judicial/case_122262/opinion_9434364",
            "judicial/case_122262/opinion_9434365",
        ],
        "the two split records are the writings, in the publisher's order, and the \
         combined record 122262 is not a writing"
    );
}

#[test]
fn should_keep_the_reporter_citation_on_the_case_and_the_author_on_the_writing_when_a_case_is_stored()
 {
    let case = boeing();

    let facts = CaseFacts::of(&case.root.data).expect("the case carries judicial facts");
    assert_eq!(facts.case_name, "Boeing Co. v. United States");
    assert_eq!(facts.cluster_id, 122262);
    assert!(
        facts.citations.contains(&"537 U.S. 437".to_string()),
        "a reporter citation belongs to the case, got {:?}",
        facts.citations
    );
    assert_eq!(
        case.root.data.heading.as_deref(),
        Some("Boeing Co. v. United States")
    );
    assert_eq!(case.root.data.node_type.as_str(), "judicial.case");

    let dissent = case
        .root
        .find("judicial/case_122262/opinion_9434365")
        .expect("the dissent is a child of the case");
    let writing = OpinionFacts::of(&dissent.data).expect("the writing carries judicial facts");
    assert_eq!(writing.opinion_type.as_deref(), Some("040dissent"));
    assert_eq!(writing.author.as_deref(), Some("Thomas"));

    let majority = case
        .root
        .find("judicial/case_122262/opinion_9434364")
        .expect("the majority is a child of the case");
    let writing = OpinionFacts::of(&majority.data).expect("the writing carries judicial facts");
    assert_eq!(writing.opinion_type.as_deref(), Some("020lead"));
    assert_eq!(writing.author.as_deref(), Some("Stevens"));
}

/// The cluster's `headmatter` is Harvard's head matter: the parties, the
/// docket number, the dates, the judges line and the attorneys. The source says
/// it is no part of any writing, so it is its own child, with a type that says
/// what it is, and never in a writing's text.
#[test]
fn should_store_the_head_matter_as_its_own_child_when_the_cluster_carries_it() {
    let case = boeing();

    let head = children_of_type(&case.root, "judicial.headmatter");
    assert_eq!(head.len(), 1, "one head matter for the case");
    assert_eq!(&*head[0].data.path, "judicial/case_122262/headmatter");
    let words = head[0]
        .data
        .content
        .as_deref()
        .expect("the head matter carries its words");
    assert!(
        words.contains("argued the cause for petitioners"),
        "the attorneys' lines are head matter, got {words:.300}"
    );
    assert!(
        !words.contains('<'),
        "the markup is stripped, got {words:.300}"
    );
    // A writing's words are in its paragraphs and footnotes, so its whole text
    // is read, as `add-opinions` reads it.
    let built = case_expression(
        &cluster(122262),
        &[opinion(122262), opinion(9434364), opinion(9434365)],
    )
    .expect("the case should build");
    for writing in &built.writings {
        assert!(
            !writing.text.contains("argued the cause for petitioners"),
            "{} holds head matter",
            writing.path
        );
    }
}

// --- Through the command ----------------------------------------------------

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Title 1 and the case of the one opinion named, added with `add-opinions
/// --offline` from the committed records, at its own path per test.
fn with_the_case_of(name: &str, opinion: &str) -> String {
    let path = format!(
        "{}/court_case_work_{name}.sqlite",
        env!("CARGO_TARGET_TMPDIR")
    );
    let _ = std::fs::remove_file(&path);
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc01.xml",
            "2025-07-18",
            None,
        )
        .expect("title 1 should parse");
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    stdout_of(&run(&[
        "add-opinions",
        &path,
        "--offline",
        "--cache-dir",
        "tests/test_data",
        "--opinions",
        opinion,
    ]));
    path
}

/// Justice Thomas's first sentence. It is in his dissent and in the combined
/// record of the whole case, and in nothing else.
const THOMAS: &str = "finger on the law";

#[test]
fn should_find_the_thomas_quote_once_in_the_dissent_when_only_the_dissent_is_named() {
    let dataset = with_the_case_of("thomas", "9434365");

    let answer: serde_json::Value =
        serde_json::from_str(&stdout_of(&run(&["search", &dataset, THOMAS, "--json"])))
            .expect("--json should print JSON");
    let paths: Vec<&str> = answer["hits"]
        .as_array()
        .expect("hits should be a list")
        .iter()
        .map(|hit| hit["path"].as_str().expect("each hit names a path"))
        .collect();

    // The paragraph that holds the words, since #314: the third that starts on
    // page 457 of 537 U.S., where the dissent begins.
    assert_eq!(
        paths,
        ["judicial/case_122262/opinion_9434365/page_457/paragraph_3"],
        "once, from the dissent: the combined record is a source and not text"
    );
}
