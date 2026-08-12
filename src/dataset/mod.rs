//! Dataset module for versioned US Code data with bill annotations
//!
//! This module provides the `Dataset` struct for storing and exploring
//! versioned legal documents, designed for the SLEUTH Tauri app.

mod error;

pub use error::DatasetError;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::annotation::ChangeAnnotation;
use crate::congress::{
    BillDownload, BillVotes, CosponsorRecord, HouseRollCall, Member, SponsorInfo, VotePosition,
};
use crate::diff::TreeDiff;
use crate::storage::{
    DatasetReader, DatasetWriter, InMemoryStorage, SqliteStorage, Storage, VersionInfo,
};
use crate::uslm::bill_parser::Bill;
use crate::uslm::parser::ParseError;
use crate::uslm::{BillDiff, USLMElement};
use crate::utils::{load_uslm_folder, parse_uslm_xml};

/// On-disk serialization format for in-memory datasets.
///
/// SQLite is a backend, not a serialization format. To work with SQLite-backed
/// datasets, use [`Dataset::open_sqlite`] / [`Dataset::new_sqlite`] (lazy access)
/// or [`Dataset::save_to_sqlite`] (dump in-memory data to a SQLite file).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Format {
    /// Compact format with deduplicated string table (smaller, faster)
    #[default]
    Compact,
    /// Raw JSON with full data (larger, for debugging/interop)
    Json,
}

/// Metadata describing a dataset
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatasetMetadata {
    pub name: String,
    pub description: String,
    pub author: String,
    pub source_urls: Vec<String>,
    pub license: String,
    pub version: String,
}

/// A snapshot of a USLMElement at a specific point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSnapshot {
    /// Date in "YYYY-MM-DD" format
    pub date: String,
    /// Optional human-readable label (e.g., "Pre-Tax Cuts Act")
    pub label: Option<String>,
    /// The element tree at this version
    pub element: USLMElement,
}

/// A search result from text search
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub date: String,
    pub path: String,
    pub field: String,
    pub snippet: String,
}

/// Key for diff_annotations HashMap: (from_date, to_date)
pub type VersionPair = (String, String);

/// A collection of versioned legal documents with bill annotations
///
/// Generic over storage backend. Use `Dataset<InMemoryStorage>` for in-memory
/// or `Dataset<SqliteStorage>` for database-backed storage.
pub struct Dataset<S: Storage> {
    storage: S,
}

impl<S: Storage> Dataset<S> {
    /// Create a dataset with the given storage backend
    pub fn with_storage(storage: S) -> Self {
        Self { storage }
    }

    /// Get reference to underlying storage
    pub fn storage(&self) -> &S {
        &self.storage
    }

    /// Get mutable reference to underlying storage
    pub fn storage_mut(&mut self) -> &mut S {
        &mut self.storage
    }

    // --- Delegate DatasetReader methods ---

    pub fn list_versions(&self) -> Result<Vec<VersionInfo>, DatasetError> {
        self.storage.list_versions()
    }

    pub fn get_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.storage.get_version(date)
    }

    pub fn get_version_by_label(
        &self,
        label: &str,
    ) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.storage.get_version_by_label(label)
    }

    pub fn next_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.storage.next_version(date)
    }

    pub fn prev_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.storage.prev_version(date)
    }

    pub fn get_bill(&self, bill_id: &str) -> Result<Option<Bill>, DatasetError> {
        self.storage.get_bill(bill_id)
    }

    pub fn get_annotations(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError> {
        self.storage.get_annotations(from, to)
    }

    pub fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError> {
        self.storage.get_member(bioguide_id)
    }

    pub fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError> {
        self.storage.get_sponsor_info(bill_id)
    }

    pub fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError> {
        self.storage.get_bill_votes(bill_id)
    }

    pub fn compute_diff(&self, from_date: &str, to_date: &str) -> Result<TreeDiff, DatasetError> {
        self.storage.compute_diff(from_date, to_date)
    }

    pub fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        self.storage.search_text(query)
    }

    pub fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        self.storage.annotations_for_path(path)
    }

    pub fn annotations_for_bill(
        &self,
        bill_id: &str,
    ) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        self.storage.annotations_for_bill(bill_id)
    }

    pub fn find_element(&self, path: &str) -> Result<Vec<(String, USLMElement)>, DatasetError> {
        self.storage.find_element(path)
    }

    pub fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError> {
        self.storage.votes_by_member(bioguide_id)
    }

    // --- Delegate DatasetWriter methods ---

    pub fn metadata(&self) -> &DatasetMetadata {
        self.storage.metadata()
    }

    pub fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.storage.set_metadata(metadata)
    }

    pub fn add_version(&mut self, snapshot: VersionSnapshot) -> Result<(), DatasetError> {
        self.storage.add_version(snapshot)
    }

    pub fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError> {
        self.storage.add_bill(bill)
    }

    pub fn add_annotation(
        &mut self,
        from: &str,
        to: &str,
        annotation: ChangeAnnotation,
    ) -> Result<(), DatasetError> {
        self.storage.add_annotation(from, to, annotation)
    }

    pub fn add_member(&mut self, member: Member) -> Result<(), DatasetError> {
        self.storage.add_member(member)
    }

    pub fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError> {
        self.storage.add_sponsor_info(info)
    }

    pub fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError> {
        self.storage.add_bill_votes(votes)
    }

    /// Get paths that have annotations for a version pair
    pub fn annotated_paths(&self, from: &str, to: &str) -> Vec<String> {
        self.get_annotations(from, to)
            .ok()
            .flatten()
            .map(|annotations| {
                annotations
                    .iter()
                    .flat_map(|a| a.paths.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get paths with changes that lack annotations for a version pair
    pub fn unannotated_paths(&self, from: &str, to: &str) -> Result<Vec<String>, DatasetError> {
        let diff = self.compute_diff(from, to)?;
        let annotated = self.annotated_paths(from, to);
        let annotated_set: std::collections::HashSet<_> = annotated.into_iter().collect();

        let mut paths_with_changes = Vec::new();
        Self::collect_paths_with_changes(&diff, &mut paths_with_changes);

        Ok(paths_with_changes
            .into_iter()
            .filter(|p| !annotated_set.contains(p))
            .collect())
    }

    fn collect_paths_with_changes(diff: &TreeDiff, paths: &mut Vec<String>) {
        if !diff.changes.is_empty() || !diff.added.is_empty() || !diff.removed.is_empty() {
            paths.push(diff.root_path.clone());
        }
        for child in &diff.child_diffs {
            Self::collect_paths_with_changes(child, paths);
        }
    }
}

// --- InMemoryStorage-specific methods ---

impl Clone for Dataset<InMemoryStorage> {
    fn clone(&self) -> Self {
        Self {
            storage: self.storage.clone(),
        }
    }
}

impl Dataset<InMemoryStorage> {
    /// Create a new in-memory dataset with the given metadata
    pub fn new(metadata: DatasetMetadata) -> Self {
        Self::with_storage(InMemoryStorage::new(metadata))
    }

    /// Add changes to an amendment in any bill
    pub fn add_changes_to_amendment(&mut self, amendment_id: &str, bill_diff: &BillDiff) {
        for bill in self.storage.bills.values_mut() {
            if let Some(amendment) = bill.amendments.get_mut(amendment_id) {
                amendment.changes.push(bill_diff.clone());
                return;
            }
        }
    }

    /// Parse a USLM XML file and add it as a version
    pub fn add_uslm_xml(
        &mut self,
        xml_path: &str,
        date: &str,
        label: Option<String>,
    ) -> Result<(), ParseError> {
        let result = parse_uslm_xml(xml_path, date)?;
        self.add_version(VersionSnapshot {
            date: date.to_string(),
            label,
            element: result,
        })
        .map_err(|e| ParseError::Io(std::io::Error::other(e)))
    }

    /// Load and merge all USLM XML files from a folder
    pub fn add_uslm_folder(
        &mut self,
        folder_path: &str,
        date: &str,
        label: Option<String>,
    ) -> Result<(), DatasetError> {
        let element = load_uslm_folder(folder_path, date)
            .ok_or_else(|| DatasetError::FolderLoadFailed(folder_path.to_string()))?;
        self.add_version(VersionSnapshot {
            date: date.to_string(),
            label,
            element,
        })
    }

    /// Save to file in specified format
    pub fn save(&self, path: &str, format: Format) -> Result<(), DatasetError> {
        match format {
            Format::Compact => {
                use crate::compact::DatasetCompact;
                let compact = DatasetCompact::from_storage(self.storage());
                let json = serde_json::to_string(&compact)?;
                fs::write(path, json)?;
            }
            Format::Json => {
                let json = serde_json::to_string_pretty(self.storage())?;
                fs::write(path, json)?;
            }
        }
        Ok(())
    }

    /// Load from file in specified format
    pub fn load(path: &str, format: Format) -> Result<Self, DatasetError> {
        match format {
            Format::Compact => {
                use crate::compact::DatasetCompact;
                let json = fs::read_to_string(path)?;
                let compact: DatasetCompact = serde_json::from_str(&json)?;
                Ok(Self::with_storage(compact.into_storage()))
            }
            Format::Json => {
                let json = fs::read_to_string(path)?;
                let mut storage: InMemoryStorage = serde_json::from_str(&json)?;
                storage.intern_strings();
                Ok(Self::with_storage(storage))
            }
        }
    }

    /// Dump this in-memory dataset to a SQLite file at `path`.
    ///
    /// To then query the file lazily (without re-loading into memory),
    /// use [`Dataset::open_sqlite`].
    pub fn save_to_sqlite<P: AsRef<Path>>(&self, path: P) -> Result<(), DatasetError> {
        let mut sqlite = SqliteStorage::open(path)?;
        sqlite.save_from_memory(self.storage())?;
        Ok(())
    }

    /// Load bill data from a BillDownload
    pub fn load_bill_download(&mut self, download: &BillDownload) -> Result<String, DatasetError> {
        use crate::uslm::bill_parser;
        use serde_json::Value;

        let bill =
            bill_parser::parse_bill_amendments_from_str(&download.bill_id, &download.bill_xml)
                .map_err(|e| {
                    DatasetError::Json(serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    )))
                })?;
        let bill_id = bill.bill_id.clone();
        self.add_bill(bill)?;

        // Parse sponsor from metadata
        let sponsors_v: Value = serde_json::from_str(&download.bill_metadata_json)?;
        let sponsor_id = sponsors_v["bill"]["sponsors"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|s| s["bioguideId"].as_str())
            .unwrap_or("")
            .to_string();

        // Parse cosponsors
        let cosponsors_v: Value = serde_json::from_str(&download.cosponsors_json)?;
        let mut cosponsors = Vec::new();
        if let Some(arr) = cosponsors_v["cosponsors"].as_array() {
            for c in arr {
                cosponsors.push(CosponsorRecord {
                    bioguide_id: c["bioguideId"].as_str().unwrap_or("").to_string(),
                    date: c["sponsorshipDate"].as_str().unwrap_or("").to_string(),
                    withdrawn: c["sponsorshipWithdrawnDate"].as_str().is_some(),
                });
            }
        }

        self.add_sponsor_info(SponsorInfo {
            bill_id: bill_id.clone(),
            sponsor: sponsor_id,
            cosponsors,
        })?;

        // Parse and add members
        for json in download.member_jsons.values() {
            if let Ok(member) = Member::from_api_response(json) {
                self.add_member(member)?;
            }
        }

        // Parse votes
        if let Some(ref votes_json) = download.votes_json
            && let Ok(roll_calls) = serde_json::from_str::<Vec<HouseRollCall>>(votes_json)
        {
            self.add_bill_votes(BillVotes {
                bill_id: bill_id.clone(),
                roll_calls,
            })?;
        }

        Ok(bill_id)
    }
}

// --- SqliteStorage-specific methods ---

impl Dataset<SqliteStorage> {
    /// Open a SQLite-backed dataset
    pub fn open_sqlite<P: AsRef<Path>>(path: P) -> Result<Self, DatasetError> {
        Ok(Self::with_storage(SqliteStorage::open(path)?))
    }

    /// Create a new SQLite-backed dataset in memory
    pub fn new_sqlite(metadata: DatasetMetadata) -> Result<Self, DatasetError> {
        Ok(Self::with_storage(SqliteStorage::new_with_metadata(
            metadata,
        )?))
    }

    /// Materialize the full SQLite dataset as an in-memory dataset.
    pub fn to_memory(&self) -> Result<Dataset<InMemoryStorage>, DatasetError> {
        Ok(Dataset::with_storage(self.storage.to_memory()?))
    }

    /// Load just the version pair `[from, to]` (and any annotations between them)
    /// into an in-memory dataset, without materializing the rest of the database.
    pub fn load_window(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Dataset<InMemoryStorage>, DatasetError> {
        Ok(Dataset::with_storage(self.storage.load_window(from, to)?))
    }
}

// --- Implement traits for Dataset<S> ---

impl<S: Storage> DatasetReader for Dataset<S> {
    fn list_versions(&self) -> Result<Vec<VersionInfo>, DatasetError> {
        self.storage.list_versions()
    }

    fn get_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.storage.get_version(date)
    }

    fn get_bill(&self, id: &str) -> Result<Option<Bill>, DatasetError> {
        self.storage.get_bill(id)
    }

    fn get_annotations(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError> {
        self.storage.get_annotations(from, to)
    }

    fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError> {
        self.storage.get_member(bioguide_id)
    }

    fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError> {
        self.storage.get_sponsor_info(bill_id)
    }

    fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError> {
        self.storage.get_bill_votes(bill_id)
    }

    fn compute_diff(&self, from: &str, to: &str) -> Result<TreeDiff, DatasetError> {
        self.storage.compute_diff(from, to)
    }

    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        self.storage.search_text(query)
    }

    fn get_version_by_label(&self, label: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.storage.get_version_by_label(label)
    }

    fn next_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.storage.next_version(date)
    }

    fn prev_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        self.storage.prev_version(date)
    }

    fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        self.storage.annotations_for_path(path)
    }

    fn annotations_for_bill(&self, bill_id: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        self.storage.annotations_for_bill(bill_id)
    }

    fn find_element(&self, path: &str) -> Result<Vec<(String, USLMElement)>, DatasetError> {
        self.storage.find_element(path)
    }

    fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError> {
        self.storage.votes_by_member(bioguide_id)
    }
}

impl<S: Storage> DatasetWriter for Dataset<S> {
    fn metadata(&self) -> &DatasetMetadata {
        self.storage.metadata()
    }

    fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.storage.set_metadata(metadata)
    }

    fn add_version(&mut self, snapshot: VersionSnapshot) -> Result<(), DatasetError> {
        self.storage.add_version(snapshot)
    }

    fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError> {
        self.storage.add_bill(bill)
    }

    fn add_annotation(
        &mut self,
        from: &str,
        to: &str,
        annotation: ChangeAnnotation,
    ) -> Result<(), DatasetError> {
        self.storage.add_annotation(from, to, annotation)
    }

    fn add_member(&mut self, member: Member) -> Result<(), DatasetError> {
        self.storage.add_member(member)
    }

    fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError> {
        self.storage.add_sponsor_info(info)
    }

    fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError> {
        self.storage.add_bill_votes(votes)
    }
}

impl<S: Storage> Storage for Dataset<S> {}
