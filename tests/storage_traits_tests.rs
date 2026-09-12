//! Proves the core storage interface is independent of the legislature
//! extension (#51).
//!
//! `DocumentsOnly` implements `DocumentReader` and nothing else. It holds no
//! bills, no sponsors, no members, and no votes, and it is never asked for any.
//! If this file compiles, a backend for a document class that has no
//! legislature — a court opinion, a state regulation — can be written without
//! implementing methods that make no sense for it.
//!
//! Before the split that was impossible: `DatasetReader` demanded `get_bill`,
//! `get_member`, `get_sponsor_info`, and `get_bill_votes` from every backend.

use words_to_data::dataset::{
    Coverage, Dataset, DatasetError, DatasetMetadata, Expression, ExpressionId, ExpressionInfo,
    SearchResult, WorkId,
};
use words_to_data::diff::TreeDiff;
use words_to_data::storage::{DocumentReader, DocumentWriter, InMemoryStorage};
use words_to_data::uslm::USLMElement;
use words_to_data::uslm::bill_parser::parse_bill_amendments;
use words_to_data::uslm::parser::parse;

const TITLE_9: &str = "tests/test_data/usc/2025-07-18/usc09.xml";
const EARLY: &str = "2025-07-18";
const BILL_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-21";

/// A documents-only backend. It delegates to real storage, so the data under
/// test is the real corpus, not an invention.
struct DocumentsOnly(InMemoryStorage);

impl DocumentReader for DocumentsOnly {
    fn metadata(&self) -> &DatasetMetadata {
        self.0.metadata()
    }

    fn works(&self) -> Result<Vec<WorkId>, DatasetError> {
        self.0.works()
    }

    fn expressions(&self, work: &WorkId) -> Result<Vec<ExpressionInfo>, DatasetError> {
        self.0.expressions(work)
    }

    fn get_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.0.get_expression(id)
    }

    fn next_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.0.next_expression(id)
    }

    fn prev_expression(&self, id: &ExpressionId) -> Result<Option<Expression>, DatasetError> {
        self.0.prev_expression(id)
    }

    fn compute_diff(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<TreeDiff, DatasetError> {
        self.0.compute_diff(from, to)
    }

    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        self.0.search_text(query)
    }

    fn find_element(&self, path: &str) -> Result<Vec<(ExpressionId, USLMElement)>, DatasetError> {
        self.0.find_element(path)
    }

    fn has_element(&self, path: &str) -> Result<bool, DatasetError> {
        self.0.has_element(path)
    }
}

/// Accepts anything that can read documents, and asks for nothing else.
fn count_works(reader: &impl DocumentReader) -> usize {
    reader.works().expect("works should list").len()
}

/// Title 9 as one expression, split out of the parsed tree the same way the
/// writer does it.
fn title_9_expression(label: Option<String>) -> Expression {
    let parsed = parse(TITLE_9, EARLY).expect("the corpus should parse");
    let root = words_to_data::dataset::work_roots(parsed)
        .pop()
        .expect("the file holds one title");
    Expression {
        id: ExpressionId::new(WorkId::new(root.data.path.to_string()), EARLY),
        label,
        element: root,
    }
}

#[test]
fn should_read_documents_from_a_backend_that_implements_no_legislature_methods() {
    let mut storage = InMemoryStorage::new(DatasetMetadata {
        name: "Documents only".to_string(),
        description: "One title, no bills".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    });
    storage
        .add_expression(title_9_expression(Some("Only".to_string())))
        .expect("an expression should be added");

    let documents = DocumentsOnly(storage);

    assert_eq!(count_works(&documents), 1);
    assert!(
        documents
            .get_expression(&ExpressionId::new(WorkId::new("uscode/title_9"), EARLY))
            .expect("the expression should be readable")
            .is_some()
    );
    assert!(
        !documents
            .search_text("arbitration")
            .expect("search should work")
            .is_empty(),
        "title 9 is the Arbitration title"
    );
}

/// Build a dataset holding one release of title 9, and optionally a real bill.
fn dataset_holding(bill: bool) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Door test".to_string(),
        description: "Title 9, with or without a bill".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_9, EARLY, None)
        .expect("the corpus should parse and load");

    if bill {
        let parsed = parse_bill_amendments(BILL_ID, BILL_XML).expect("the public law should parse");
        dataset.add_bill(parsed).expect("the bill should be added");
    }

    dataset
}

/// A dataset of statutes and bills carries legislative material, so the door
/// opens and the caller can read bills through it.
#[test]
fn should_open_the_legislature_door_when_the_dataset_holds_a_bill() {
    let dataset = dataset_holding(true);

    let legislature = dataset
        .legislature()
        .expect("a dataset holding a bill should offer the legislature extension");

    assert_eq!(
        legislature.list_bill_ids().expect("ids should list"),
        vec![BILL_ID.to_string()]
    );
}

/// The point of the door: a dataset that holds only documents reports that it
/// has no legislature, instead of answering "no bills" as though the question
/// made sense for its contents.
#[test]
fn should_close_the_legislature_door_when_the_dataset_holds_no_legislative_material() {
    let dataset = dataset_holding(false);

    assert!(
        dataset.legislature().is_none(),
        "a dataset of documents alone should not offer the legislature extension"
    );
}

/// A dataset holding title 9 covers title 9, and does not cover title 26. The
/// second half is the point: an empty search for a title 26 provision says
/// nothing about the law when the dataset never held title 26.
#[test]
fn should_report_out_of_scope_for_material_the_dataset_never_held() {
    let dataset = dataset_holding(false);
    let scope = dataset.scope().expect("scope should derive");

    assert_eq!(
        scope.works().collect::<Vec<_>>(),
        vec![&WorkId::new("uscode/title_9")]
    );
    assert_eq!(scope.held[0].dates, vec![EARLY.to_string()]);

    assert_eq!(scope.covers("uscode/title_9"), Coverage::InScope);
    assert_eq!(
        scope.covers("uscode/title_9/chapter_1/section_3"),
        Coverage::InScope,
        "a provision inside a held title is in scope"
    );
    assert_eq!(
        scope.covers("uscode"),
        Coverage::InScope,
        "the dataset holds part of the code, so asking about the code is in scope"
    );

    assert_eq!(
        scope.covers("uscode/title_26/subtitle_A/chapter_1/section_174"),
        Coverage::OutOfScope,
        "title 26 was never in this dataset"
    );
}

/// Searching for real text that this dataset cannot contain returns nothing,
/// and the scope explains why. Without that second answer the empty result
/// reads as "no such law".
#[test]
fn should_explain_an_empty_search_with_the_scope() {
    let dataset = dataset_holding(false);

    let hits = dataset
        .search_text("qualified small business stock")
        .expect("search should work");
    assert!(hits.is_empty(), "that phrase belongs to title 26");

    assert_eq!(
        dataset
            .scope()
            .expect("scope should derive")
            .covers("uscode/title_26"),
        Coverage::OutOfScope
    );
}
