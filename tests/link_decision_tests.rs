//! How a link was made, as the readers print it (#179).
//!
//! `link-by-evidence` records in each `legislature.amended_by` link how it
//! decided the link: the address and the source that gave it, the window, and
//! how the change was chosen. A reviewer, and a playbook that sends the weaker
//! kinds of decision to review first, must be able to read that without
//! reading the matcher.
//!
//! Every case reads the committed corpus: the public law `119-hr-1`, title 26
//! of the Code at its three committed release points, and the committed OLRC
//! table for the 119th Congress, 1st session. The links are the ones
//! `link-by-evidence` writes over them today. Nothing is mocked.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::annotation::ChangeAnnotation;
use words_to_data::citation::resolve::SectionPaths;
use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, ExpressionId, Format, WorkId, adjacent_expressions,
};
use words_to_data::legislature::evidence_matching::Recorded;
use words_to_data::link::{Link, LinkKind};
use words_to_data::olrc::{ClassificationTable, classify};
use words_to_data::storage::LinkReader;

const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";
const RELEASE_POINTS: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];
const TITLE_26: &str = "uscode/title_26";

/// 26 U.S.C. § 6041(a), which five amendments of the law edited in place.
const SECTION_6041_A: &str = "uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041/subsection_a";

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

/// The bill and title 26 at each committed release point, with the
/// renumberings recorded as `build-dataset` records them and the OLRC table
/// stated as `add-classifications` states it, and then linked by
/// `link-by-evidence`. Written once as a compact dataset; every test here
/// only reads it.
fn linked_dataset() -> &'static str {
    static WRITTEN: OnceLock<String> = OnceLock::new();
    WRITTEN.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        dataset
            .load_bill_download(&committed_bill_download())
            .expect("the committed bill should load");
        for date in RELEASE_POINTS {
            dataset
                .add_uslm_xml(&format!("tests/test_data/usc/{date}/usc26.xml"), date, None)
                .expect("title 26 should parse");
        }
        let windows = adjacent_expressions(&dataset).expect("the windows should list");
        let bill = dataset
            .bill_document(BILL_ID)
            .expect("the dataset should answer for the bill")
            .expect("the bill is held as a document");
        dataset
            .record_redesignations_over(BILL_ID, &bill.root, &windows)
            .expect("the renumberings should record");

        let html = std::fs::read_to_string("tests/test_data/olrc/classification/tbl119pl_1st.htm")
            .expect("the committed table should read");
        let table = ClassificationTable::parse(&html).expect("the committed table should parse");
        let mut paths = SectionPaths::new();
        for date in RELEASE_POINTS {
            let expression = dataset
                .get_expression(&ExpressionId::new(WorkId::new(TITLE_26), date))
                .expect("storage should answer")
                .expect("title 26 is held");
            paths.add_work(&expression.root);
        }
        let scope = dataset.scope().expect("the scope should derive");
        for link in classify(&table.rows, &scope, &paths, "olrc:tbl119pl_1st.htm").links {
            dataset.add_link(link).expect("the link should add");
        }

        let dir = env!("CARGO_TARGET_TMPDIR");
        let unlinked = format!("{dir}/link_decision_unlinked.json");
        let linked = format!("{dir}/link_decision_linked.json");
        dataset
            .save(&unlinked, Format::Compact)
            .expect("the dataset should save");
        run(&["link-by-evidence", &unlinked, "--output", &linked]);
        linked
    })
}

/// Every amendment link `link-by-evidence` wrote.
fn amendment_links() -> &'static [Link] {
    static LINKS: OnceLock<Vec<Link>> = OnceLock::new();
    LINKS.get_or_init(|| {
        let dataset = Dataset::load(linked_dataset(), Format::Compact)
            .expect("the linked dataset should load");
        dataset
            .links_by_kind(LinkKind::AMENDED_BY)
            .expect("the links should read")
    })
}

/// The first amendment link whose recorded reasoning holds `words`.
fn link_whose_reasoning_says(words: &str) -> &'static Link {
    amendment_links()
        .iter()
        .find(|link| {
            link.provenance
                .evidence
                .as_ref()
                .and_then(|evidence| evidence.reasoning.as_deref())
                .is_some_and(|reasoning| reasoning.contains(words))
        })
        .unwrap_or_else(|| panic!("a link's reasoning says {words}"))
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

/// `settle --explain` on one link.
fn explain(link: &Link) -> String {
    run(&[
        "settle",
        linked_dataset(),
        "--link",
        &link.id()[..16],
        "--explain",
    ])
}

#[test]
fn should_print_the_source_and_the_method_with_its_version_when_a_reviewer_explains_a_link() {
    let link = link_whose_reasoning_says("read from the bill's markup");

    let said = explain(link);

    assert!(
        said.contains("Source: rule:evidence_matching"),
        "the source is named: {said}"
    );
    assert!(
        said.contains("Method: address, window and quoted words@3"),
        "the method and its version are named: {said}"
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

/// Section 70431(a)(5)(C) of the law amends "Sections 1202(b)(2),
/// 1202(g)(2)(A), and 1202(j)(1)(A)". The markup reads no section out of a
/// plural, and the OLRC table classifies 70431(a)(5) to 26 U.S.C. 1202.
#[test]
fn should_name_the_address_and_say_the_olrc_table_gave_it_when_the_markup_named_no_section() {
    let link = link_whose_reasoning_says("read from the OLRC classification table");

    let said = explain(link);

    assert_eq!(
        lines_starting(&said, "Address:"),
        vec!["Address: /us/usc/t26/s1202, from the OLRC classification table".to_string()],
        "the address and its source are on one line: {said}"
    );
    assert!(
        said.contains("Pub. L. 119-21 § 70431(a)(5)"),
        "the row that gave the address is named: {said}"
    );
}

/// A change no amendment's quoted words placed, given to the one amendment
/// whose address still wants it. The weakest way the matcher chooses, so a
/// reviewer must see it at a glance.
#[test]
fn should_say_the_change_was_chosen_by_elimination_when_no_quoted_words_placed_it() {
    let link = link_whose_reasoning_says("no other amendment addressed there takes it");

    let said = explain(link);

    assert_eq!(
        lines_starting(&said, "Chosen:"),
        vec!["Chosen: by elimination".to_string()],
        "the kind of decision is on one line: {said}"
    );
}

/// The window is the first one after the law's enactment in which something
/// under the address changed: Public Law 119-21 was enacted on 2025-07-04,
/// and the first committed release point after it is 2025-07-18.
#[test]
fn should_name_the_window_the_matcher_linked_in_when_a_reviewer_explains_a_link() {
    let link = link_whose_reasoning_says("read from the bill's markup");

    let said = explain(link);

    assert_eq!(
        lines_starting(&said, "Window:"),
        vec!["Window: uscode/title_26@2025-07-18 to 2025-07-30".to_string()],
        "the window is on one line: {said}"
    );
}

/// 26 U.S.C. § 6041(a) is one paragraph the diff reports as one change, and
/// five amendments of the law edited it in place. Each link is one of five
/// causes, and a reviewer must know the others are there.
#[test]
fn should_say_the_link_is_one_of_several_causes_when_other_amendments_edited_the_same_provision() {
    let link = amendment_links()
        .iter()
        .find(|link| link.subject.path() == Some(SECTION_6041_A))
        .expect("§ 6041(a) carries an amendment link");

    let said = explain(link);

    assert_eq!(
        lines_starting(&said, "Causes:"),
        vec!["Causes: one of 5 amendments linked to this change".to_string()],
        "the number of causes is on one line: {said}"
    );
}

/// How much choosing the matcher did: an address under which one thing
/// changed left it nothing to tell apart. § 6041(a) is one paragraph with
/// nothing below it, so its window holds one change under the address.
#[test]
fn should_say_how_many_changes_the_address_held_in_the_window_when_a_reviewer_explains_a_link() {
    let link = amendment_links()
        .iter()
        .find(|link| link.subject.path() == Some(SECTION_6041_A))
        .expect("§ 6041(a) carries an amendment link");

    let said = explain(link);

    assert_eq!(
        lines_starting(&said, "Changes under the address:"),
        vec!["Changes under the address: 1".to_string()],
        "the number of changes the matcher chose from is on one line: {said}"
    );
}

/// The matcher writes its evidence as text, and the readers read it back into
/// parts. A new sentence in the matcher that the readers do not know would
/// leave a link with no decision shown, so every link it writes must read.
#[test]
fn should_read_every_part_of_the_evidence_when_the_matcher_wrote_the_link() {
    let unread: Vec<&str> = amendment_links()
        .iter()
        .filter_map(|link| {
            link.provenance
                .evidence
                .as_ref()
                .and_then(|evidence| evidence.reasoning.as_deref())
        })
        .filter(|reasoning| {
            Recorded::read(reasoning)
                .is_none_or(|recorded| recorded.changes_under_address.is_none())
        })
        .collect();

    assert!(!amendment_links().is_empty(), "the matcher wrote links");
    assert!(unread.is_empty(), "every link reads: {unread:#?}");
}

/// `annotations --json` gives each link how it was made, so an agent can pick
/// out the weaker kinds of decision without explaining every link.
#[test]
fn should_carry_how_each_link_was_made_when_annotations_answer_in_json() {
    let said = run(&[
        "annotations",
        linked_dataset(),
        "--path",
        SECTION_6041_A,
        "--exact",
        "--json",
    ]);

    let answer: serde_json::Value = serde_json::from_str(&said).expect("the answer is JSON");
    let rows = answer["annotations"]
        .as_array()
        .expect("a list of annotations");
    assert_eq!(rows.len(), 5, "five amendments edited § 6041(a): {said}");
    for row in rows {
        let links = row["links"]
            .as_array()
            .expect("each record lists its links");
        assert_eq!(links.len(), 1, "one link for the one path: {row}");
        let link = &links[0];
        assert_eq!(
            link["id"], row["link_ids"][0],
            "the link is named by its id"
        );
        assert_eq!(link["path"], SECTION_6041_A);
        assert_eq!(link["source"], "rule:evidence_matching");
        assert_eq!(link["method"], "address, window and quoted words@3");
        assert_eq!(link["causes"], 5, "one of five causes: {link}");
        assert_eq!(link["recorded"]["address_source"], "markup");
        assert_eq!(link["recorded"]["changes_under_address"], 1);
        assert_eq!(link["recorded"]["chosen"], "quoted_words", "{link}");
    }
}

/// A person reading `annotations` sees the method beside the maker, and the
/// kind of decision beside each link, in a few words.
#[test]
fn should_print_the_method_and_each_links_kind_of_decision_when_annotations_answer_a_person() {
    let said = run(&[
        "annotations",
        linked_dataset(),
        "--path",
        SECTION_6041_A,
        "--exact",
    ]);

    let headers = lines_starting(&said, "[");
    assert_eq!(headers.len(), 5, "five records: {said}");
    for header in &headers {
        assert!(
            header.ends_with("by rule:evidence_matching, address, window and quoted words@3)"),
            "the method follows the maker: {header}"
        );
    }
    let links = lines_starting(&said, "link ");
    assert_eq!(links.len(), 5, "one link for each record: {said}");
    for link in &links {
        assert!(
            link.ends_with(&format!("{SECTION_6041_A}  [quoted_words, 1 of 5 causes]")),
            "the kind of decision follows the path: {link}"
        );
    }
}

/// The first title 26 annotation of the model method removed in #252, as the
/// link it is stored as, in a dataset of its own written to `name`.
///
/// `tests/test_data/processed/annotations.json` is the real output of that
/// method over the real corpus.
fn model_link_on_disk(name: &str) -> (Link, String) {
    let json = std::fs::read_to_string("tests/test_data/processed/annotations.json")
        .expect("the annotations fixture should be readable");
    let annotations: Vec<ChangeAnnotation> =
        serde_json::from_str(&json).expect("the fixture should parse as annotations");
    let annotation = annotations
        .into_iter()
        .find(|annotation| {
            annotation
                .paths
                .iter()
                .any(|path| path.starts_with(TITLE_26))
        })
        .expect("the fixture holds a title 26 annotation");
    let from = ExpressionId::new(WorkId::new(TITLE_26), RELEASE_POINTS[0]);
    let to = ExpressionId::new(WorkId::new(TITLE_26), RELEASE_POINTS[1]);
    let link = Link::from_annotation(&annotation, &from, &to)
        .into_iter()
        .next()
        .expect("the annotation states a link");

    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset.add_link(link.clone()).expect("the link should add");
    let path = format!("{}/{name}", env!("CARGO_TARGET_TMPDIR"));
    dataset
        .save(&path, Format::Compact)
        .expect("the dataset should save");
    (link, path)
}

/// A link the model method made has no recorded address or window: only its
/// source and the model's own reasoning. A reviewer must still see both.
#[test]
fn should_print_the_source_and_the_stored_reasoning_when_a_model_made_the_link() {
    let (link, dataset) = model_link_on_disk("link_decision_model.json");

    let said = run(&["settle", &dataset, "--link", &link.id()[..16], "--explain"]);

    assert!(
        said.contains("Source: model:deepseek-v4-pro"),
        "the source is named: {said}"
    );
    assert!(
        said.contains("Method: not recorded"),
        "a link with no method says so: {said}"
    );
    assert_eq!(
        lines_starting(&said, "Reasoning:"),
        vec!["Reasoning:".to_string()],
        "the reasoning has a heading: {said}"
    );
    let flat = said.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("Section reference matches exactly (Section 163(j)(8)(A)(v))"),
        "the model's reasoning is printed: {said}"
    );
}

/// A model link made after #58 names the model that answered and the reply it
/// answered with, which the dataset keeps once under the hash of its text.
/// `tests/test_data/processed/model_replies.json` holds real recorded replies.
#[test]
fn should_name_the_model_and_its_reply_when_the_link_records_them() {
    let (model_link, _) = model_link_on_disk("link_decision_model_reply_unused.json");
    let json = std::fs::read_to_string("tests/test_data/processed/model_replies.json")
        .expect("the replies fixture should be readable");
    let replies: Vec<serde_json::Value> =
        serde_json::from_str(&json).expect("the fixture should parse");
    let reply = replies[0]["reply"].as_str().expect("a recorded reply");

    let mut dataset = Dataset::new(DatasetMetadata::default());
    let reply_id = dataset.add_reply(reply).expect("the reply should store");
    let mut link = model_link;
    let evidence = link
        .provenance
        .evidence
        .as_mut()
        .expect("the model gave its reasoning");
    evidence.reply = Some(reply_id.clone());
    evidence.model = Some("deepseek-v4-pro".to_string());
    dataset.add_link(link.clone()).expect("the link should add");
    let path = format!(
        "{}/link_decision_model_reply.json",
        env!("CARGO_TARGET_TMPDIR")
    );
    dataset
        .save(&path, Format::Compact)
        .expect("the dataset should save");

    let said = run(&["settle", &path, "--link", &link.id()[..16], "--explain"]);

    assert_eq!(
        lines_starting(&said, "Model:"),
        vec!["Model: deepseek-v4-pro".to_string()],
        "the model is named: {said}"
    );
    assert_eq!(
        lines_starting(&said, "Reply:"),
        vec![format!("Reply: {reply_id}")],
        "the reply is named by its id: {said}"
    );
}
