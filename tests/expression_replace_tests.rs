//! Replacing an expression that a dataset already holds.
//!
//! The element index of a SQLite dataset is a derivation of the stored trees
//! (`docs/adr/0007`), so after a replacement it must describe the tree that is
//! stored and nothing else. The in-memory backend holds the trees themselves,
//! so it is the reference for the answer.
//!
//! The corpus holds Title 8 at two release points. The 30 July tree has
//! chapter 16, IMMIGRATION FEES; the 18 July tree does not. The two trees are
//! therefore a real replacement that drops provisions, and no test below has
//! to invent one.

use words_to_data::dataset::{
    Dataset, DatasetMetadata, Expression, ExpressionId, WorkId, work_roots,
};
use words_to_data::document::DocumentNode;
use words_to_data::storage::DocumentReader;
use words_to_data::uslm::parser::parse;

/// Aliens and Nationality. One work, which the corpus holds twice.
const TITLE_8: &str = "uscode/title_8";

/// A section of the chapter that only the 30 July tree has.
const DROPPED_SECTION: &str = "uscode/title_8/chapter_16/section_1801";

/// The date that keys the expression. Both trees below go in under this one
/// key, because a replacement is what this file is about: an operator stored
/// the wrong release point and then writes the correct one over it.
const KEYED_AT: &str = "2025-07-18";

fn metadata() -> DatasetMetadata {
    DatasetMetadata {
        name: "Expression replacement".to_string(),
        description: "Title 8 at two release points".to_string(),
        author: "Test".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "0.1.0".to_string(),
        ..Default::default()
    }
}

fn title_8_as_published(date: &str) -> DocumentNode {
    let path = format!("tests/test_data/usc/{date}/usc08.xml");
    let parsed = parse(&path, date).expect("the corpus holds title 8 at this release point");
    work_roots(parsed).pop().expect("the file holds one title")
}

/// The one keyed expression, carrying whichever published tree is named.
fn expression_from(published: &str) -> Expression {
    Expression {
        id: ExpressionId::new(WorkId::new(TITLE_8), KEYED_AT),
        label: None,
        root: title_8_as_published(published),
    }
}

#[test]
fn should_report_no_node_when_a_replacing_tree_drops_it() {
    let mut dataset = Dataset::new_sqlite(metadata()).expect("a SQLite dataset");
    dataset
        .add_expression(expression_from("2025-07-30"))
        .expect("the first tree is stored");
    assert!(
        dataset.has_node(DROPPED_SECTION).unwrap(),
        "the 30 July tree holds section 1801"
    );

    dataset
        .add_expression(expression_from("2025-07-18"))
        .expect("the second tree replaces the first");

    assert!(
        !dataset.has_node(DROPPED_SECTION).unwrap(),
        "section 1801 is not in the tree that the dataset now holds"
    );
}
