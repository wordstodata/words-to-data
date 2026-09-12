//! The appendices keep whole bodies of law inside container elements.
//!
//! A USLM appendix does not put its sections under the appendix. It groups them
//! in a container: the Federal Rules sit in `courtRules`, an act reprinted in an
//! appendix sits in `compiledAct`, and the reorganization plans sit in
//! `reorganizationPlans`. The parser did not know those names, dropped each
//! container with the law below it, and said nothing, so four appendices held
//! two to four elements each while the files held thousands (#110).
//!
//! A dropped leaf is correct: a table of contents is not law. A dropped
//! container is data loss, so the parser now reports it.
//!
//! A container that groups law carries no number of its own, and the path
//! segment for it used to fall back to the element's XML uuid. It now comes from
//! the publisher's `identifier`, or from the publisher's heading where there is
//! no identifier (#115).

use rstest::rstest;
use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::inspect;
use words_to_data::uslm::USLMElement;
use words_to_data::uslm::parser::{parse, parse_with_report};

const RELEASE: &str = "2025-07-18";
const USC28A: &str = "tests/test_data/usc/2025-07-18/usc28a.xml";
/// Title 9 is small, holds no unknown container, and is full of the leaves a
/// parse is supposed to drop without a word: `toc`, `note`, `sourceCredit`.
const USC09: &str = "tests/test_data/usc/2025-07-18/usc09.xml";

/// The opening of Rule 1 of the Federal Rules of Civil Procedure, which the
/// dataset did not hold at all before this fix.
const RULE_1_TEXT: &str =
    "These rules govern the procedure in all civil actions and proceedings in the United States";

/// Rule 401 of the Federal Rules of Evidence, which no dataset held while
/// `<article>` was a name the parser did not know (#122).
const RULE_401_TEXT: &str = "Evidence is relevant if";

fn element_count(element: &USLMElement) -> usize {
    1 + element.children.iter().map(element_count).sum::<usize>()
}

fn collect_paths(element: &USLMElement, into: &mut Vec<String>) {
    into.push(element.data.path.to_string());
    for child in &element.children {
        collect_paths(child, into);
    }
}

fn paths_of(file: &str) -> Vec<String> {
    let path = format!("tests/test_data/usc/{RELEASE}/{file}");
    let root = parse(&path, RELEASE).unwrap_or_else(|e| panic!("{file} should parse: {e}"));
    let mut paths = Vec::new();
    collect_paths(&root, &mut paths);
    paths
}

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Appendix Containers".to_string(),
        description: "The title 28 appendix, for the Federal Rules".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    }
}

/// Each appendix held 2 to 4 elements. The floors here are far below the counts
/// the fix produces, so the test fails on a regression rather than on an edit
/// to one rule.
#[rstest]
#[case("usc28a.xml", 3000)]
#[case("usc11a.xml", 2000)]
#[case("usc18a.xml", 1000)]
#[case("usc05A.xml", 100)]
fn should_hold_the_law_of_an_appendix_when_a_container_groups_it(
    #[case] file: &str,
    #[case] floor: usize,
) {
    let path = format!("tests/test_data/usc/{RELEASE}/{file}");
    let root = parse(&path, RELEASE).unwrap_or_else(|e| panic!("{file} should parse: {e}"));

    let count = element_count(&root);
    assert!(
        count > floor,
        "{file} should hold more than {floor} elements, got {count}"
    );
}

#[test]
fn should_find_federal_rules_of_civil_procedure_text_when_the_appendix_is_searched() {
    let mut dataset = Dataset::new(metadata());
    dataset
        .add_uslm_xml(USC28A, RELEASE, None)
        .expect("the title 28 appendix should parse");

    let hits = inspect::search(&dataset, RULE_1_TEXT).expect("search the dataset");

    assert!(
        !hits.is_empty(),
        "Rule 1 of the Federal Rules of Civil Procedure should be searchable"
    );
}

#[test]
fn should_find_federal_rules_of_evidence_text_when_the_appendix_is_searched() {
    let mut dataset = Dataset::new(metadata());
    dataset
        .add_uslm_xml(USC28A, RELEASE, None)
        .expect("the title 28 appendix should parse");

    let hits = inspect::search(&dataset, RULE_401_TEXT).expect("search the dataset");

    assert!(
        !hits.is_empty(),
        "Rule 401 of the Federal Rules of Evidence should be searchable"
    );
}

/// `article` groups the rules of the Federal Rules of Evidence. While the parser
/// did not know the name it dropped each article with the rules below it, and
/// #113's report said so. The name is known now, so nothing is dropped and there
/// is nothing to report (#122).
#[test]
fn should_report_no_dropped_container_when_the_appendix_holds_only_known_containers() {
    let (_root, report) = parse_with_report(USC28A, RELEASE).expect("the appendix should parse");

    assert!(
        report.is_empty(),
        "every container in the title 28 appendix should be known, got {:?}",
        report.dropped_containers
    );
}

/// The eleven articles of the Federal Rules of Evidence, each holding its rules.
#[test]
fn should_hold_the_articles_of_the_federal_rules_of_evidence_when_the_appendix_is_parsed() {
    let root = parse(USC28A, RELEASE).expect("the appendix should parse");

    let evidence = root
        .find("uscode/appendix_28a/level_Evid")
        .expect("the Federal Rules of Evidence should be in the tree");

    assert_eq!(
        evidence.children.len(),
        11,
        "the Federal Rules of Evidence hold eleven articles"
    );
    assert!(
        root.find("uscode/appendix_28a/level_Evid/level_IV/level_401")
            .is_some(),
        "Rule 401 should sit under Article IV"
    );
}

/// The publisher writes `identifier="/us/usc/t28a/courtRules/Civil"` on the
/// container that holds the Federal Rules of Civil Procedure, and no `<num>`.
/// Rule 1 used to sit under `level_id2e47c0a6-b17c-11ef-b971-e82c9e4f66ce`.
#[test]
fn should_name_a_container_from_the_publisher_identifier_when_it_carries_no_number() {
    let root = parse(USC28A, RELEASE).expect("the appendix should parse");

    let rule_1 = root
        .find("uscode/appendix_28a/level_Civil/title_I/level_1")
        .expect("Rule 1 of the Federal Rules of Civil Procedure should be at a typable path");

    assert_eq!(
        rule_1.data.heading.as_deref().map(str::trim),
        Some("Scope and Purpose")
    );
}

/// The container holding the Federal Rules of Bankruptcy Procedure carries no
/// number and no `identifier`. Its heading is the only name the publisher gives
/// it, and the U.S. Code prints it that way, so the path segment comes from
/// there.
#[test]
fn should_name_a_container_from_its_heading_when_the_publisher_supplies_no_identifier() {
    let root = parse(
        &format!("tests/test_data/usc/{RELEASE}/usc11a.xml"),
        RELEASE,
    )
    .expect("the title 11 appendix should parse");

    assert!(
        root.find("uscode/appendix_11a/level_federal-rules-of-bankruptcy-procedure")
            .is_some(),
        "the Federal Rules of Bankruptcy Procedure should be at a typable path"
    );
}

/// A heading can carry a footnote, and the footnote is not part of the name. One
/// heading in the title 28 appendix runs to 313 characters with its footnote
/// attached, which is no more typable than the uuid it replaces.
#[test]
fn should_leave_a_footnote_out_of_a_segment_taken_from_a_heading() {
    let root = parse(USC28A, RELEASE).expect("the appendix should parse");

    assert!(
        root.find(
            "uscode/appendix_28a/level_Civil/\
             level_supplemental-rules-for-admiralty-or-maritime-claims-and-asset-forfeiture-actions"
        )
        .is_some(),
        "the supplemental admiralty rules should be named without their footnote"
    );
}

/// The four appendices carry most of the numberless containers, and nine
/// ordinary titles carry the rest. Titles 12, 29 and 38 hold the largest share
/// of the ordinary-title cases, and none of them is an obscure corner.
#[rstest]
#[case("usc28a.xml")]
#[case("usc11a.xml")]
#[case("usc18a.xml")]
#[case("usc05A.xml")]
#[case("usc12.xml")]
#[case("usc29.xml")]
#[case("usc38.xml")]
fn should_hold_no_uuid_path_when_a_release_point_is_parsed(#[case] file: &str) {
    let uuid_paths: Vec<String> = paths_of(file)
        .into_iter()
        .filter(|path| path.contains("level_id"))
        .take(3)
        .collect();

    assert!(
        uuid_paths.is_empty(),
        "{file} should hold no uuid path, got {uuid_paths:?}"
    );
}

/// Title 26 is the control. It holds no numberless container the parser reaches
/// and no `<article>`, so neither change may touch it. 57,391 elements is the
/// count the dataset holds at this release point.
#[test]
fn should_leave_a_title_with_no_numberless_container_unchanged() {
    let root = parse(&format!("tests/test_data/usc/{RELEASE}/usc26.xml"), RELEASE)
        .expect("title 26 should parse");

    // The `uscode` container is not one of the title's own elements.
    let count = element_count(&root) - 1;
    assert_eq!(count, 57_391, "title 26 at {RELEASE} holds 57,391 elements");
}

#[test]
fn should_stay_silent_when_an_unknown_element_holds_no_structural_children() {
    let (_root, report) = parse_with_report(USC09, RELEASE).expect("title 9 should parse");

    assert!(
        report.is_empty(),
        "dropping a leaf is correct and should be quiet, got {:?}",
        report.dropped_containers
    );
}
