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

fn element_count(element: &USLMElement) -> usize {
    1 + element.children.iter().map(element_count).sum::<usize>()
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
#[case("usc28a.xml", 2000)]
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
fn should_report_a_dropped_container_when_an_unknown_element_holds_structural_children() {
    let (_root, report) = parse_with_report(USC28A, RELEASE).expect("the appendix should parse");

    // `article` groups the Federal Rules of Evidence. The parser does not know
    // the name, so it drops the article and the law below it — and now says so.
    let article = report
        .dropped_containers
        .iter()
        .find(|dropped| dropped.element_name == "article")
        .unwrap_or_else(|| {
            panic!(
                "the dropped articles should be reported, got {:?}",
                report.dropped_containers
            )
        });

    assert!(
        article.structural_children > 0,
        "a reported container should say how much law went with it"
    );
    assert!(
        article.parent_path.starts_with("uscode/appendix_28a"),
        "a reported container should say where it sat, got {}",
        article.parent_path
    );
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
