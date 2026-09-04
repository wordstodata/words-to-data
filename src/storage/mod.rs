//! Storage backends for dataset persistence.
//!
//! The traits split three ways, which is the shape the data model needs (#51):
//!
//! - **Documents** are the core. Versions, navigation, diffing, and search work
//!   the same whether the document is a statute, a regulation, or an opinion.
//!   Nothing here names a legislature or a court.
//! - **Links** are also core. A link connects a provision to something else and
//!   carries its provenance. Today the only kind is the change annotation, which
//!   ties a change to the amendment that caused it.
//! - **Legislature** is an extension. Bills, sponsors, members, and votes exist
//!   only in datasets that hold legislative material. A dataset of court
//!   opinions has none of them, and must not have to pretend otherwise.
//!
//! `Storage` is the full set that both first-party backends implement today. A
//! backend that holds only documents implements [`DocumentReader`] alone.

pub mod memory;
pub mod sqlite;

pub use memory::InMemoryStorage;
pub use sqlite::SqliteStorage;

use crate::annotation::ChangeAnnotation;
use crate::congress::{BillVotes, HouseRollCall, Member, SponsorInfo, VotePosition};
use crate::dataset::{DatasetError, DatasetMetadata, SearchResult, VersionSnapshot};
use crate::diff::TreeDiff;
use crate::uslm::USLMElement;
use crate::uslm::bill_parser::Bill;

/// Version info for listing (without full element tree)
#[derive(Debug, Clone)]
pub struct VersionInfo {
    pub date: String,
    pub label: Option<String>,
}

/// Reading the documents a dataset holds.
///
/// This is the core interface. Every document class supports it, so nothing
/// here may name a legislature, a court, or any other extension concept.
pub trait DocumentReader {
    /// List all available versions (date + label, no element tree)
    fn list_versions(&self) -> Result<Vec<VersionInfo>, DatasetError>;

    /// Get a specific version by date
    fn get_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError>;

    /// Get a version by label
    fn get_version_by_label(&self, label: &str) -> Result<Option<VersionSnapshot>, DatasetError>;

    /// Get the next version after the given date
    fn next_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError>;

    /// Get the previous version before the given date
    fn prev_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError>;

    /// Compute diff between two versions
    fn compute_diff(&self, from: &str, to: &str) -> Result<TreeDiff, DatasetError>;

    /// Search text across versions
    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError>;

    /// Find element by path across all versions
    fn find_element(&self, path: &str) -> Result<Vec<(String, USLMElement)>, DatasetError>;
}

/// Reading the links a dataset holds.
///
/// A link connects a provision to something else and carries its provenance.
/// Links are core, not part of any extension, so that a reader which does not
/// know an extension can still see, report, and preserve its links
/// (`docs/adr/0002-links-live-in-the-core.md`).
///
/// Today the only kind is the change annotation. `annotations_for_bill` is a
/// query by link object, and the bill id is an opaque string here: this trait
/// does not need to know what a bill is.
pub trait LinkReader {
    /// Get annotations for a version pair
    fn get_annotations(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError>;

    /// Find all annotations that include the given path (across all version pairs)
    fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError>;

    /// Find all annotations from a specific bill (across all version pairs)
    fn annotations_for_bill(&self, bill_id: &str) -> Result<Vec<ChangeAnnotation>, DatasetError>;

    /// List every `(from_date, to_date)` pair that carries annotations
    fn annotation_pairs(&self) -> Result<Vec<crate::dataset::VersionPair>, DatasetError>;
}

/// Reading the legislature facts a dataset holds.
///
/// An extension, not core. Implement it only for a backend that actually holds
/// bills. A dataset of court opinions does not, and a caller reading a file
/// from another party must be able to find that out rather than get an error
/// from a method that should never have existed for that data.
pub trait LegislatureReader {
    /// Get a bill by ID
    fn get_bill(&self, id: &str) -> Result<Option<Bill>, DatasetError>;

    /// List the IDs of every bill in the dataset
    fn list_bill_ids(&self) -> Result<Vec<String>, DatasetError>;

    /// Get a Congress member by bioguide ID
    fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError>;

    /// Get sponsor info for a bill
    fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError>;

    /// Get votes for a bill
    fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError>;

    /// Get all roll calls where a member voted, with their position
    fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError>;
}

/// Writing documents and dataset metadata.
pub trait DocumentWriter {
    /// Get metadata
    fn metadata(&self) -> &DatasetMetadata;

    /// Set metadata
    fn set_metadata(&mut self, metadata: DatasetMetadata);

    /// Add a version snapshot
    fn add_version(&mut self, snapshot: VersionSnapshot) -> Result<(), DatasetError>;
}

/// Writing links.
pub trait LinkWriter {
    /// Add an annotation for a version pair
    fn add_annotation(
        &mut self,
        from: &str,
        to: &str,
        annotation: ChangeAnnotation,
    ) -> Result<(), DatasetError>;
}

/// Writing legislature facts. An extension, like [`LegislatureReader`].
pub trait LegislatureWriter {
    /// Add a bill
    fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError>;

    /// Add a Congress member
    fn add_member(&mut self, member: Member) -> Result<(), DatasetError>;

    /// Add sponsor info
    fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError>;

    /// Add bill votes
    fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError>;
}

/// The full set of capabilities, which both first-party backends provide.
///
/// Code that needs everything binds to this. Code that needs only documents
/// should bind to [`DocumentReader`] instead, so it keeps working against a
/// dataset that carries no legislative material.
pub trait Storage:
    DocumentReader + LinkReader + LegislatureReader + DocumentWriter + LinkWriter + LegislatureWriter
{
}
