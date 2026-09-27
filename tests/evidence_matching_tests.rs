//! Each amendment of a public law, linked to the change it made from its
//! address, its window and the words it quotes, with no model call (#250).
//!
//! Stages 3 and 4 of
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`.
//!
//! Every case is read out of the committed corpus: the public law `119-hr-1`
//! and title 7 of the Code at its three committed release points. Title 7 is
//! the smaller of the two titles the bill amends most, and 82 of the bill's
//! amendments act in it.

use std::process::Command;
use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{
    Dataset, DatasetMetadata, ExpressionId, WorkId, adjacent_expressions,
};
use words_to_data::citation::resolve::SectionPaths;
use words_to_data::legislature::evidence_matching::{
    AmendmentMatch, OlrcClassification, Outcome, Stage, evidence_method, match_by_evidence,
    olrc_classification,
};
use words_to_data::olrc::{ClassificationTable, classify};
use words_to_data::link::{Link, LinkKind, Target, VerificationState, amendment_reference};
use words_to_data::storage::LinkReader;
use words_to_data::matching::matching_method;
use words_to_data::storage::InMemoryStorage;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";
const RELEASE_POINTS: [&str; 3] = ["2025-07-18", "2025-07-30", "2025-08-14"];
const TITLE_7: &str = "uscode/title_7";

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

/// The bill and title 7 at each committed release point, read once for every
/// test in this file.
///
/// Built the way `build-dataset` builds one: after everything is loaded, the
/// renumberings the bill states are recorded over every window the dataset
/// holds. That includes the second window, where the bill changed nothing,
/// which is how the maintainer's own dataset came to hold changes there that
/// the Code's text does not show (#218).
fn dataset() -> &'static Dataset<InMemoryStorage> {
    static DATASET: OnceLock<Dataset<InMemoryStorage>> = OnceLock::new();
    DATASET.get_or_init(|| {
        let mut dataset = Dataset::new(DatasetMetadata::default());
        dataset
            .load_bill_download(&committed_bill_download())
            .expect("the committed bill should load");
        for date in RELEASE_POINTS {
            dataset
                .add_uslm_xml(&format!("tests/test_data/usc/{date}/usc07.xml"), date, None)
                .expect("title 7 should parse");
        }
        let windows = adjacent_expressions(&dataset).expect("the windows should list");
        let bill = dataset
            .bill_document(BILL_ID)
            .expect("the dataset should answer for the bill")
            .expect("the bill is held as a document");
        dataset
            .record_redesignations_over(BILL_ID, &bill.root, &windows)
            .expect("the renumberings should record");
        dataset
    })
}

/// What the matcher says about every amendment, worked out once.
fn matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        match_by_evidence(dataset())
            .expect("the matcher should run")
            .matches
    })
}

/// The matcher's answer for one amendment, by the start of its id.
fn match_of(id_start: &str) -> &'static AmendmentMatch {
    let found: Vec<&AmendmentMatch> = matches()
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect();
    assert_eq!(found.len(), 1, "one amendment starts with {id_start}");
    found[0]
}

fn title_7_at(date: &str) -> ExpressionId {
    ExpressionId::new(WorkId::new(TITLE_7), date)
}

#[test]
fn should_link_the_one_change_under_the_address_when_an_amendment_has_a_single_change() {
    // Section 19(a)(2)(A)(ii) of the Food and Nutrition Act of 2008
    // (7 U.S.C. 2028(a)(2)(A)(ii)) is amended by striking "section 3(u)(4)"
    // and inserting "section 3(u)(3)".
    let found = match_of("bf20b9a41d6c");

    let Outcome::Linked(linked) = &found.outcome else {
        panic!("the amendment should be linked: {:?}", found.outcome);
    };
    assert_eq!(linked.from, title_7_at("2025-07-18"));
    assert_eq!(linked.to, title_7_at("2025-07-30"));
    assert_eq!(
        linked.paths(),
        vec![
            "uscode/title_7/chapter_51/section_2028/subsection_a/paragraph_2/subparagraph_A/clause_ii"
                .to_string()
        ]
    );
}

/// Subsection (o) of 7 U.S.C. 2015, where two amendments of the bill share one
/// address.
const SECTION_2015_O: &str = "uscode/title_7/chapter_51/section_2015/subsection_o";

/// The paths an amendment was linked to, or a failure that says why not.
fn linked_paths(id_start: &str) -> Vec<String> {
    match &match_of(id_start).outcome {
        Outcome::Linked(linked) => linked.paths(),
        other => panic!("{id_start} should be linked: {other:?}"),
    }
}

#[test]
fn should_assign_each_change_by_the_words_the_bill_quotes_when_two_amendments_share_an_address() {
    // Both amendments are addressed to section 6(o) of the Food and Nutrition
    // Act of 2008 (7 U.S.C. 2015(o)). The first strikes paragraph (3) and
    // enacts a new one; the second redesignates paragraph (7) and enacts a new
    // paragraph (7) after paragraph (6). The address alone cannot tell their
    // changes apart. The text each one enacts can.
    let paragraph_3 = format!("{SECTION_2015_O}/paragraph_3");
    let paragraph_7 = format!("{SECTION_2015_O}/paragraph_7");
    let under = |path: &str, container: &str| {
        path == container || path.starts_with(&format!("{container}/"))
    };

    let new_paragraph_3 = linked_paths("f217bfa18755");
    assert!(new_paragraph_3.contains(&paragraph_3));
    assert!(
        new_paragraph_3.iter().all(|path| under(path, &paragraph_3)),
        "the new paragraph (3) causes only changes inside paragraph (3): {new_paragraph_3:?}"
    );

    let new_paragraph_7 = linked_paths("d624331f459d");
    assert!(
        new_paragraph_7.contains(&paragraph_7),
        "the new paragraph (7) is linked: {new_paragraph_7:?}"
    );
    assert!(
        !new_paragraph_7.iter().any(|path| under(path, &paragraph_3)),
        "the change to paragraph (3) has one cause, and it is not this amendment: {new_paragraph_7:?}"
    );
}

#[test]
fn should_write_one_amended_by_link_per_changed_path_in_the_shape_every_reader_knows() {
    // Section 508(e)(2)(H)(i) of the Federal Crop Insurance Act
    // (7 U.S.C. 1508(e)(2)(H)(i)) is amended by striking "65" and inserting
    // "80".
    let found = match_of("179c37ff9497");

    let links = found.links();

    assert_eq!(links.len(), 1, "one changed path, one link");
    let link = &links[0];
    assert_eq!(link.kind, LinkKind::new(LinkKind::AMENDED_BY));
    assert_eq!(
        link.subject,
        Target::Change {
            work: WorkId::new(TITLE_7),
            path: "uscode/title_7/chapter_36/subchapter_I/section_1508/subsection_e/paragraph_2/subparagraph_H/clause_i"
                .to_string(),
            from_date: "2025-07-18".to_string(),
            to_date: "2025-07-30".to_string(),
        }
    );
    let Target::External { reference, .. } = &link.object else {
        panic!("the object is the amendment: {:?}", link.object);
    };
    assert_eq!(*reference, amendment_reference(BILL_ID, &found.amendment_id));

    // The method has its own name and a version a person chose, and it is
    // not the model method it replaces (#179, decision 10).
    assert_eq!(link.provenance.method, Some(evidence_method()));
    assert_ne!(Some(evidence_method()), Some(matching_method()));
    assert_eq!(
        link.provenance.verification,
        VerificationState::MachineSuggested
    );

    // The evidence says where, when and by which words, so a reviewer can
    // check each step.
    let reasoning = link
        .provenance
        .evidence
        .as_ref()
        .and_then(|evidence| evidence.reasoning.as_deref())
        .expect("the link carries its reasoning");
    for part in [
        "/us/usc/t7/s1508(e)(2)(H)(i)",
        "2025-07-04",
        "2025-07-18",
        "struck \"65\"",
        "inserted \"80\"",
    ] {
        assert!(
            reasoning.contains(part),
            "the reasoning names {part}: {reasoning}"
        );
    }
    // No model answered, so no reply and no model are named.
    let evidence = link.provenance.evidence.as_ref().unwrap();
    assert_eq!((&evidence.reply, &evidence.model), (&None, &None));

    // The payload is the one `match-amendments` writes, so every reader of it
    // works unchanged.
    let payload = link.payload.as_ref().expect("the link carries its payload");
    assert_eq!(payload.namespace, "legislature");
    assert_eq!(payload.value["bill_id"], BILL_ID);
    assert_eq!(payload.value["amendment_id"], found.amendment_id.as_str());
    assert!(payload.value.get("operation").is_some());
}

#[test]
fn should_write_every_link_and_record_the_method_over_each_window_when_the_command_runs() {
    let dir = tempfile::tempdir().expect("a temporary directory");
    let path = dir.path().join("evidence.sqlite");
    dataset()
        .save_to_sqlite(&path)
        .expect("the fixture should save");

    let run = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["link-by-evidence", path.to_str().expect("a UTF-8 path")])
        .output()
        .expect("the binary should run");
    assert!(
        run.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&run.stderr)
    );

    let written = Dataset::open_sqlite(&path).expect("the dataset should open");
    let ours: Vec<Link> = written
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("the links should read")
        .into_iter()
        .filter(|link| link.provenance.method == Some(evidence_method()))
        .collect();
    let expected: usize = matches().iter().map(|found| found.links().len()).sum();
    assert!(expected > 0, "the corpus gives links");
    assert_eq!(ours.len(), expected, "every link the matcher finds is written");

    // The method ran over the window its links sit in, and a reader can say
    // so without counting links (#179, decision 11).
    assert!(written.method_runs().iter().any(|run| {
        run.method == evidence_method()
            && run.covers(&WorkId::new(TITLE_7), "2025-07-18", "2025-07-30")
    }));

    // The answer is the same on every run, so a second run adds nothing.
    let again = Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(["link-by-evidence", path.to_str().expect("a UTF-8 path")])
        .output()
        .expect("the binary should run");
    assert!(again.status.success());
    let written = Dataset::open_sqlite(&path).expect("the dataset should open");
    let after_again = written
        .links_by_kind(LinkKind::AMENDED_BY)
        .expect("the links should read")
        .into_iter()
        .filter(|link| link.provenance.method == Some(evidence_method()))
        .count();
    assert_eq!(after_again, expected);
}

/// The OLRC's classification of the committed table, as `olrc.classified_from`
/// links against title 7, the way `add-classifications` states them (#247).
fn olrc_links() -> &'static [Link] {
    static LINKS: OnceLock<Vec<Link>> = OnceLock::new();
    LINKS.get_or_init(|| {
        let html = std::fs::read_to_string("tests/test_data/olrc/classification/tbl119pl_1st.htm")
            .expect("the committed table should read");
        let table = ClassificationTable::parse(&html).expect("the committed table should parse");
        let mut paths = SectionPaths::new();
        for date in RELEASE_POINTS {
            let title_7 = dataset()
                .get_expression(&title_7_at(date))
                .expect("storage should answer")
                .expect("title 7 is held");
            paths.add_work(&title_7.root);
        }
        let scope = dataset().scope().expect("the scope should derive");
        classify(&table.rows, &scope, &paths, "olrc:tbl119pl_1st.htm").links
    })
}

#[test]
fn should_not_count_a_note_as_the_olrc_classifying_a_change_to_a_sections_text() {
    // Section 10601(f) of the law is classified to 7 U.S.C. 8351 as `nt`: a
    // note under the section. The dataset holds no notes, and a note says
    // nothing about the section's own text.
    let section_8351 = "uscode/title_7/chapter_109A/section_8351";
    assert_eq!(
        olrc_classification(olrc_links(), section_8351, "119-21"),
        OlrcClassification::OnlyNotes(vec!["10601(f)".to_string()])
    );

    // Two sections of the law amend 7 U.S.C. 2025 itself.
    let section_2025 = "uscode/title_7/chapter_51/section_2025";
    assert_eq!(
        olrc_classification(olrc_links(), section_2025, "119-21"),
        OlrcClassification::Text(vec!["10101(b)(1)".to_string(), "10106".to_string()])
    );

    // A section no row of the law names.
    let section_2 = "uscode/title_7/chapter_1/section_2";
    assert_eq!(
        olrc_classification(olrc_links(), section_2, "119-21"),
        OlrcClassification::Nothing
    );
}

#[test]
fn should_name_the_olrc_classification_in_the_evidence_and_change_no_answer_when_it_is_held() {
    let mut with_olrc = Dataset::with_storage(dataset().storage().clone());
    for link in olrc_links() {
        with_olrc
            .add_link(link.clone())
            .expect("the link should add");
    }
    let classified = match_by_evidence(&with_olrc)
        .expect("the matcher should run")
        .matches;

    // The same answer for every amendment. The OLRC resolves to a section and
    // no lower, so it cannot choose a change, and the matcher works the same
    // without it.
    let paths_of = |found: &AmendmentMatch| match &found.outcome {
        Outcome::Linked(linked) => linked.paths(),
        Outcome::Residue(_) => Vec::new(),
    };
    assert_eq!(classified.len(), matches().len());
    for (with, without) in classified.iter().zip(matches()) {
        assert_eq!(with.amendment_id, without.amendment_id);
        assert_eq!(paths_of(with), paths_of(without));
    }

    // Section 16(a) of the Food and Nutrition Act of 2008 (7 U.S.C. 2025(a)),
    // amended by section 10106 of the law. The table places 10101(b)(1) and
    // 10106 at 7 U.S.C. 2025, and the evidence says so.
    let found = classified
        .iter()
        .find(|found| found.amendment_id.starts_with("285e9181329c"))
        .expect("the amendment is stated");
    let reasoning = found.links()[0]
        .provenance
        .evidence
        .as_ref()
        .and_then(|evidence| evidence.reasoning.clone())
        .expect("the link carries its reasoning");
    assert!(
        reasoning.contains(
            "OLRC: the classification table places Pub. L. 119-21 § 10101(b)(1), § 10106 at this section."
        ),
        "the evidence names the classification: {reasoning}"
    );
}

#[test]
fn should_link_only_in_the_first_window_when_the_address_also_changed_in_a_later_one() {
    // Section 6(o) of the Food and Nutrition Act of 2008 (7 U.S.C. 2015(o)) is
    // amended by redesignating paragraph (7) as paragraph (8), and by
    // inserting a new paragraph (7). The law landed in the first window. The
    // renumbering is recorded over the second window as well, as
    // `build-dataset` records it, so something under the address changed there
    // too.
    let found = match_of("d624331f459d");

    let Outcome::Linked(linked) = &found.outcome else {
        panic!("the amendment should be linked: {:?}", found.outcome);
    };
    assert_eq!(linked.from, title_7_at("2025-07-18"));
    assert_eq!(linked.to, title_7_at("2025-07-30"));

    // The second window is never a second link. It is named for a reviewer.
    let later: Vec<(&ExpressionId, &ExpressionId)> = linked
        .later_windows
        .iter()
        .map(|window| (&window.from, &window.to))
        .collect();
    assert_eq!(
        later,
        vec![(&title_7_at("2025-07-30"), &title_7_at("2025-08-14"))]
    );
    assert!(
        linked.later_windows[0]
            .changes
            .iter()
            .all(|path| path.starts_with(SECTION_2015_O)),
        "the later window names the changes under the address: {:?}",
        linked.later_windows[0].changes
    );
}

#[test]
fn should_give_a_renumbering_to_the_amendment_its_redesignation_link_names() {
    // "(1) by redesignating paragraph (7) as paragraph (8); and (2) by
    // inserting after paragraph (6) the following: (7) ..." The renumbering
    // quotes no words, and the dataset already says which amendment made it:
    // the `legislature.redesignated_as` link the bill's reading recorded.
    let paragraph_8 = format!("{SECTION_2015_O}/paragraph_8");

    let caused = linked_paths("d624331f459d");

    assert!(
        caused.contains(&paragraph_8),
        "the renumbered paragraph is this amendment's change: {caused:?}"
    );
    assert!(
        !linked_paths("f217bfa18755").contains(&paragraph_8),
        "and no other amendment's"
    );
}

#[test]
fn should_say_the_dataset_does_not_hold_the_title_when_an_address_is_outside_it() {
    // Section 1240O(b) of the Food Security Act of 1985 (16 U.S.C.
    // 3839bb-2(b)), and this dataset holds title 7 alone. That is a limit of the dataset, not
    // a statement that nothing changed, and the reason must not read as one.
    let found = match_of("c0b8fadcd360");

    let Outcome::Residue(residue) = &found.outcome else {
        panic!("the amendment should be residue: {:?}", found.outcome);
    };
    assert_eq!(residue.stage, Stage::Window);
    assert_eq!(
        residue.reason,
        "the dataset holds no expression of uscode/title_16"
    );
}

#[test]
fn should_leave_changes_as_residue_when_the_quoted_words_cannot_place_them() {
    // Section 27(a)(2) of the Food and Nutrition Act of 2008
    // (7 U.S.C. 2036(a)(2)) is amended by striking "section 3(u)(4)" each place
    // it appears and inserting "section 3(u)(3)".
    //
    // The Code does not print the Act's own section numbers: it prints
    // "section 2012(u)(4) of this title". So neither quoted string shows in
    // either change under the address, and with two changes there, the
    // matcher has nothing to choose by. It must not guess.
    let found = match_of("a9fd405d5415");

    let Outcome::Residue(residue) = &found.outcome else {
        panic!("the amendment should be residue: {:?}", found.outcome);
    };
    assert_eq!(residue.stage, Stage::Resolve);
    assert!(
        residue.reason.contains("show in none"),
        "the reason says the quoted words were not found: {}",
        residue.reason
    );
    // An agent picks the residue up from here, so it is told where to look.
    assert_eq!(residue.from.as_ref(), Some(&title_7_at("2025-07-18")));
    assert_eq!(residue.to.as_ref(), Some(&title_7_at("2025-07-30")));
    let paragraph_2 = "uscode/title_7/chapter_51/section_2036/subsection_a/paragraph_2";
    assert_eq!(
        residue.changes,
        vec![
            format!("{paragraph_2}/subparagraph_C"),
            format!("{paragraph_2}/subparagraph_E"),
        ]
    );
}
