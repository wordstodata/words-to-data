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

use words_to_data::dataset::{DatasetError, DatasetMetadata, SearchResult, VersionSnapshot};
use words_to_data::diff::TreeDiff;
use words_to_data::storage::{DocumentReader, DocumentWriter, InMemoryStorage, VersionInfo};
use words_to_data::uslm::USLMElement;
use words_to_data::uslm::parser::parse;

const TITLE_9: &str = "tests/test_data/usc/2025-07-18/usc09.xml";

/// A documents-only backend. It delegates to real storage, so the data under
/// test is the real corpus, not an invention.
struct DocumentsOnly(InMemoryStorage);

impl DocumentReader for DocumentsOnly {
    fn list_versions(&self) -> Result<Vec<VersionInfo>, DatasetError> {
        self.0.list_versions()
    }

    fn get_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.0.get_version(date)
    }

    fn get_version_by_label(&self, label: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.0.get_version_by_label(label)
    }

    fn next_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.0.next_version(date)
    }

    fn prev_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.0.prev_version(date)
    }

    fn compute_diff(&self, from: &str, to: &str) -> Result<TreeDiff, DatasetError> {
        self.0.compute_diff(from, to)
    }

    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        self.0.search_text(query)
    }

    fn find_element(&self, path: &str) -> Result<Vec<(String, USLMElement)>, DatasetError> {
        self.0.find_element(path)
    }
}

/// Accepts anything that can read documents, and asks for nothing else.
fn count_versions(reader: &impl DocumentReader) -> usize {
    reader.list_versions().expect("versions should list").len()
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
    });
    storage
        .add_version(VersionSnapshot {
            date: "2025-07-18".to_string(),
            label: Some("Only".to_string()),
            element: parse(TITLE_9, "2025-07-18").expect("the corpus should parse"),
        })
        .expect("a version should be added");

    let documents = DocumentsOnly(storage);

    assert_eq!(count_versions(&documents), 1);
    assert!(
        documents
            .get_version("2025-07-18")
            .expect("the version should be readable")
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
