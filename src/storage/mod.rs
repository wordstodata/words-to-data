//! Storage backends for dataset persistence

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

/// Trait for querying dataset contents
///
/// Implemented by both `Dataset` (in-memory) and `SqliteStorage` (database).
/// Allows consumer code to work with either backend.
pub trait DatasetReader {
    /// List all available versions (date + label, no element tree)
    fn list_versions(&self) -> Result<Vec<VersionInfo>, DatasetError>;

    /// Get a specific version by date
    fn get_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError>;

    /// Get a bill by ID
    fn get_bill(&self, id: &str) -> Result<Option<Bill>, DatasetError>;

    /// Get annotations for a version pair
    fn get_annotations(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError>;

    /// Get a Congress member by bioguide ID
    fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError>;

    /// Get sponsor info for a bill
    fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError>;

    /// Get votes for a bill
    fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError>;

    /// Compute diff between two versions
    fn compute_diff(&self, from: &str, to: &str) -> Result<TreeDiff, DatasetError>;

    /// Search text across versions
    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError>;

    // --- Navigation methods ---

    /// Get a version by label
    fn get_version_by_label(&self, label: &str) -> Result<Option<VersionSnapshot>, DatasetError>;

    /// Get the next version after the given date
    fn next_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError>;

    /// Get the previous version before the given date
    fn prev_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError>;

    // --- Cross-reference queries ---

    /// Find all annotations that include the given path (across all version pairs)
    fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError>;

    /// Find all annotations from a specific bill (across all version pairs)
    fn annotations_for_bill(&self, bill_id: &str) -> Result<Vec<ChangeAnnotation>, DatasetError>;

    /// Find element by path across all versions
    fn find_element(&self, path: &str) -> Result<Vec<(String, USLMElement)>, DatasetError>;

    /// Get all roll calls where a member voted, with their position
    fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError>;
}

/// Trait for writing dataset contents
pub trait DatasetWriter {
    /// Get metadata
    fn metadata(&self) -> &DatasetMetadata;

    /// Set metadata
    fn set_metadata(&mut self, metadata: DatasetMetadata);

    /// Add a version snapshot
    fn add_version(&mut self, snapshot: VersionSnapshot) -> Result<(), DatasetError>;

    /// Add a bill
    fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError>;

    /// Add an annotation for a version pair
    fn add_annotation(
        &mut self,
        from: &str,
        to: &str,
        annotation: ChangeAnnotation,
    ) -> Result<(), DatasetError>;

    /// Add a Congress member
    fn add_member(&mut self, member: Member) -> Result<(), DatasetError>;

    /// Add sponsor info
    fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError>;

    /// Add bill votes
    fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError>;
}

/// Combined storage trait for full dataset access
pub trait Storage: DatasetReader + DatasetWriter {}
