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
//!
//! `DocumentsAndLinks` goes further and implements the whole of `Storage`, which
//! `DocumentsOnly` cannot: until #127 the `Storage` bound itself required the
//! legislature extension, so a full backend had to answer six questions about
//! bills to say nothing about bills. It names no bill, member, sponsor or vote,
//! and the only thing it says about the legislature is that it has none.

use std::collections::BTreeMap;

use words_to_data::dataset::{
    Coverage, Dataset, DatasetError, DatasetMetadata, Expression, ExpressionId, ExpressionInfo,
    ExpressionPair, SearchResult, WorkId,
};
use words_to_data::diff::TreeDiff;
use words_to_data::link::{Link, LinkKind, Provenance, Target, VerificationState};
use words_to_data::storage::{
    DocumentReader, DocumentWriter, EvidenceReader, EvidenceWriter, InMemoryStorage,
    LegislatureReader, LinkReader, LinkWriter, Storage,
};
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

/// A backend that holds documents and links, and nothing else.
///
/// This is the shape a judicial backend has: an opinion cites a provision, so
/// links are needed, and there is no bill, member, sponsor or vote anywhere in
/// the data. It implements every trait the `Storage` bound asks for, and it
/// mentions the legislature exactly once — to say it holds none.
///
/// It delegates to real storage, so the data under test is the real corpus.
struct DocumentsAndLinks(InMemoryStorage);

impl DocumentReader for DocumentsAndLinks {
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

impl LinkReader for DocumentsAndLinks {
    fn links_for_path(&self, path: &str) -> Result<Vec<Link>, DatasetError> {
        self.0.links_for_path(path)
    }

    fn links_for_pair(
        &self,
        from: &ExpressionId,
        to: &ExpressionId,
    ) -> Result<Vec<Link>, DatasetError> {
        self.0.links_for_pair(from, to)
    }

    fn links_by_kind(&self, kind: &str) -> Result<Vec<Link>, DatasetError> {
        self.0.links_by_kind(kind)
    }

    fn links_by_namespace(&self, namespace: &str) -> Result<Vec<Link>, DatasetError> {
        self.0.links_by_namespace(namespace)
    }

    fn links_for_object_prefix(&self, prefix: &str) -> Result<Vec<Link>, DatasetError> {
        self.0.links_for_object_prefix(prefix)
    }

    fn link_pairs(&self) -> Result<Vec<ExpressionPair>, DatasetError> {
        self.0.link_pairs()
    }

    fn count_links_by_kind(&self) -> Result<BTreeMap<String, usize>, DatasetError> {
        self.0.count_links_by_kind()
    }
}

impl EvidenceReader for DocumentsAndLinks {
    fn get_reply(&self, id: &str) -> Result<Option<String>, DatasetError> {
        self.0.get_reply(id)
    }

    fn replies(&self) -> Result<Vec<String>, DatasetError> {
        self.0.replies()
    }

    fn count_replies(&self) -> Result<usize, DatasetError> {
        self.0.count_replies()
    }
}

impl DocumentWriter for DocumentsAndLinks {
    fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.0.set_metadata(metadata)
    }

    fn add_expression(&mut self, expression: Expression) -> Result<(), DatasetError> {
        self.0.add_expression(expression)
    }
}

impl LinkWriter for DocumentsAndLinks {
    fn add_link(&mut self, link: Link) -> Result<(), DatasetError> {
        self.0.add_link(link)
    }
}

impl EvidenceWriter for DocumentsAndLinks {
    fn add_reply(&mut self, reply: &str) -> Result<String, DatasetError> {
        self.0.add_reply(reply)
    }
}

impl Storage for DocumentsAndLinks {
    /// No legislature at all, which is not the same answer as "no bills".
    fn legislature(&self) -> Option<&dyn LegislatureReader> {
        None
    }
}

/// One opinion citing a provision of title 9, as a link of a kind the core
/// stores and does not interpret.
fn citation_link() -> Link {
    Link {
        subject: Target::Provision("uscode/title_9/chapter_1/section_3".to_string()),
        kind: LinkKind::new(LinkKind::CITES),
        object: Target::External {
            reference: "judicial.opinion:us/570/1".to_string(),
            display: "Opinion".to_string(),
        },
        provenance: Provenance {
            source: "human:words_to_data tests".to_string(),
            method: None,
            verification: VerificationState::Asserted,
            evidence: None,
            raw_score: None,
            timestamp: None,
            corroboration: None,
        },
        payload: None,
    }
}

/// The compiling witness for #127: a full `Storage` backend that says nothing
/// about bills.
///
/// The assertions matter less than the fact that this file builds. `Dataset<S>`
/// and the generic `inspect` readers both bind to `Storage`, so the test also
/// proves that a legislature-free backend can be handed to them.
#[test]
fn should_implement_storage_without_the_legislature_extension() {
    let mut storage = InMemoryStorage::new(DatasetMetadata {
        name: "Documents and links".to_string(),
        description: "One title and one citation, no bills".to_string(),
        author: "words_to_data tests".to_string(),
        source_urls: vec![],
        license: "MIT".to_string(),
        version: "1.0.0".to_string(),
        ..Default::default()
    });
    storage
        .add_expression(title_9_expression(None))
        .expect("an expression should be added");

    let mut dataset = Dataset::with_storage(DocumentsAndLinks(storage));
    dataset
        .add_link(citation_link())
        .expect("the citation should be added");

    // The core readers answer, through the generic code that used to demand the
    // legislature extension as well.
    assert_eq!(
        words_to_data::inspect::expressions(&dataset, None)
            .expect("expressions should list")
            .len(),
        1
    );
    assert_eq!(
        dataset
            .links_by_namespace("judicial")
            .expect("links should list")
            .len(),
        1,
        "a link of a kind the core does not interpret is still stored and returned"
    );

    // And the one thing it says about the legislature is that it has none.
    assert!(
        dataset.legislature().is_none(),
        "a documents-and-links backend holds no legislative material"
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
