//! Works and expressions, over the real corpus.
//!
//! The point of this shape is that it survives a document class with no release
//! cycle. These tests use statutes, because that is the corpus we have, but each
//! one is written so it would still make sense for a court opinion: a work with
//! a single expression is the normal case here, not a degenerate one.

use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, VersionSnapshot, WorkId};
use words_to_data::uslm::parser::parse;

const EARLY: &str = "2025-07-18";
const LATE: &str = "2025-07-30";

/// Title 51 gained two sections between the two release points.
const AMENDED: &str = "usc51";
/// Title 9 did not change.
const UNCHANGED: &str = "usc09";

fn dataset_of(title: &str, dates: &[&str]) -> Dataset<words_to_data::storage::InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Works".to_string(),
        description: format!("{title} at {} release points", dates.len()),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
    });
    for date in dates {
        dataset
            .add_version(VersionSnapshot {
                date: date.to_string(),
                label: None,
                element: parse(&format!("tests/test_data/usc/{date}/{title}.xml"), date)
                    .expect("the corpus should parse"),
            })
            .expect("a version should be added");
    }
    dataset
}

#[test]
fn should_name_every_work_the_dataset_holds() {
    let dataset = dataset_of(AMENDED, &[EARLY, LATE]);

    assert_eq!(
        dataset.works().expect("works should list"),
        vec![WorkId::new("uscode/title_51")],
        "two releases of one title are one work, not two"
    );
}

#[test]
fn should_list_an_expression_for_each_date_a_work_was_published() {
    let dataset = dataset_of(AMENDED, &[EARLY, LATE]);
    let work = WorkId::new("uscode/title_51");

    assert_eq!(
        dataset.expressions(&work).expect("expressions should list"),
        vec![
            ExpressionId::new(work.clone(), EARLY),
            ExpressionId::new(work.clone(), LATE),
        ]
    );
}

/// A work published once has one expression. For a court opinion that is the
/// only case there will ever be, so it must be ordinary rather than special.
#[test]
fn should_give_one_expression_for_a_work_published_once() {
    let dataset = dataset_of(UNCHANGED, &[EARLY]);
    let work = WorkId::new("uscode/title_9");

    let expressions = dataset.expressions(&work).expect("expressions should list");

    assert_eq!(expressions, vec![ExpressionId::new(work, EARLY)]);
}

#[test]
fn should_read_the_text_of_one_expression() {
    let dataset = dataset_of(AMENDED, &[EARLY, LATE]);
    let id = ExpressionId::new(WorkId::new("uscode/title_51"), LATE);

    let element = dataset
        .get_expression(&id)
        .expect("reading should work")
        .expect("the expression should be there");

    assert_eq!(element.data.path.to_string(), "uscode/title_51");
}

#[test]
fn should_find_no_expression_for_a_date_the_work_was_not_published() {
    let dataset = dataset_of(AMENDED, &[EARLY, LATE]);
    let id = ExpressionId::new(WorkId::new("uscode/title_51"), "1999-01-01");

    assert!(
        dataset
            .get_expression(&id)
            .expect("reading should work")
            .is_none()
    );
}

/// The identifier prints in the form Akoma Ntoso and ELI use, so it can be
/// written in a citation or a URL without translation.
#[test]
fn should_print_an_expression_as_work_at_date() {
    let id = ExpressionId::new(WorkId::new("uscode/title_9"), EARLY);

    assert_eq!(id.to_string(), "uscode/title_9@2025-07-18");
}
