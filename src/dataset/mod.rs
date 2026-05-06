//! Dataset module for versioned US Code data with bill annotations
//!
//! This module provides the `Dataset` struct for storing and exploring
//! versioned legal documents, designed for the SLEUTH Tauri app.

mod error;

pub use error::DatasetError;

use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::HashMap;
use std::fs;

use crate::annotation::ChangeAnnotation;
use crate::congress::{BillDownload, CosponsorRecord, Member, SponsorInfo};
use crate::diff::TreeDiff;
use crate::uslm::bill_parser::Bill;
use crate::uslm::parser::ParseError;
use crate::uslm::{BillDiff, USLMElement};
use crate::utils::{load_uslm_folder, parse_uslm_xml};

/// Metadata describing a dataset
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub metadata: DatasetMetadata,

    /// Chronologically sorted version snapshots
    pub versions: Vec<VersionSnapshot>,

    /// Bills that caused changes in this dataset
    pub bills: Vec<Bill>,

    /// Annotations per version-pair
    #[serde_as(as = "Vec<(_, _)>")]
    pub diff_annotations: HashMap<VersionPair, Vec<ChangeAnnotation>>,

    /// Congress members by bioguide ID
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(default)]
    pub members: HashMap<String, Member>,

    /// Sponsor info by bill ID
    #[serde_as(as = "Vec<(_, _)>")]
    #[serde(default)]
    pub sponsors: HashMap<String, SponsorInfo>,
}

impl Dataset {
    /// Create a new empty dataset with the given metadata
    pub fn new(metadata: DatasetMetadata) -> Self {
        Dataset {
            metadata,
            versions: Vec::new(),
            bills: Vec::new(),
            diff_annotations: HashMap::new(),
            members: HashMap::new(),
            sponsors: HashMap::new(),
        }
    }

    /// Add a version snapshot, maintaining chronological order by date
    pub fn add_version(&mut self, snapshot: VersionSnapshot) {
        let pos = self
            .versions
            .binary_search_by(|v| v.date.cmp(&snapshot.date))
            .unwrap_or_else(|pos| pos);
        self.versions.insert(pos, snapshot);
    }

    /// Get a version snapshot by exact date
    pub fn get_version(&self, date: &str) -> Option<&VersionSnapshot> {
        self.versions.iter().find(|v| v.date == date)
    }

    /// Get a version snapshot by label
    pub fn get_version_by_label(&self, label: &str) -> Option<&VersionSnapshot> {
        self.versions
            .iter()
            .find(|v| v.label.as_deref() == Some(label))
    }

    /// Get the version after the given date
    pub fn next_version(&self, date: &str) -> Option<&VersionSnapshot> {
        let pos = self.versions.iter().position(|v| v.date == date)?;
        self.versions.get(pos + 1)
    }

    /// Get the version before the given date
    pub fn prev_version(&self, date: &str) -> Option<&VersionSnapshot> {
        let pos = self.versions.iter().position(|v| v.date == date)?;
        if pos == 0 {
            None
        } else {
            self.versions.get(pos - 1)
        }
    }

    /// Save dataset to a JSON file
    pub fn save(&self, path: &str) -> Result<(), DatasetError> {
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Load dataset from a JSON file
    pub fn load(path: &str) -> Result<Self, DatasetError> {
        let json = fs::read_to_string(path)?;
        let dataset = serde_json::from_str(&json)?;
        Ok(dataset)
    }

    /// Compute diff between two versions by date
    pub fn compute_diff(&self, from_date: &str, to_date: &str) -> Result<TreeDiff, DatasetError> {
        let from = self
            .get_version(from_date)
            .ok_or_else(|| DatasetError::VersionNotFound(from_date.to_string()))?;
        let to = self
            .get_version(to_date)
            .ok_or_else(|| DatasetError::VersionNotFound(to_date.to_string()))?;
        Ok(TreeDiff::from_elements(&from.element, &to.element))
    }

    /// Add a bill to the dataset
    pub fn add_bill(&mut self, bill: Bill) {
        self.bills.push(bill);
    }

    /// Get a bill by its ID
    pub fn get_bill(&self, bill_id: &str) -> Option<&Bill> {
        self.bills.iter().find(|b| b.bill_id == bill_id)
    }

    pub fn add_changes_to_amendment(&mut self, amendment_id: &str, bill_diff: &BillDiff) {
        for bill in self.bills.iter_mut() {
            if let Some(amendment) = bill.amendments.get_mut(amendment_id) {
                amendment.changes.push(bill_diff.clone());
                return;
            }
        }
    }

    /// Get annotations for a specific version pair
    pub fn get_annotations(&self, from: &str, to: &str) -> Option<&Vec<ChangeAnnotation>> {
        self.diff_annotations
            .get(&(from.to_string(), to.to_string()))
    }

    /// Get mutable annotations for a specific version pair
    pub fn get_annotations_mut(&mut self, from: &str, to: &str) -> &mut Vec<ChangeAnnotation> {
        self.diff_annotations
            .entry((from.to_string(), to.to_string()))
            .or_default()
    }

    /// Add an annotation for a specific version pair
    pub fn add_annotation(&mut self, from: &str, to: &str, annotation: ChangeAnnotation) {
        self.get_annotations_mut(from, to).push(annotation);
    }

    /// Get all annotations that include the given path (searches all version pairs)
    pub fn annotations_for_path(&self, path: &str) -> Vec<&ChangeAnnotation> {
        self.diff_annotations
            .values()
            .flatten()
            .filter(|a| a.paths.iter().any(|p| p == path))
            .collect()
    }

    /// Get all annotations associated with the given bill ID (searches all version pairs)
    pub fn annotations_for_bill(&self, bill_id: &str) -> Vec<&ChangeAnnotation> {
        self.diff_annotations
            .values()
            .flatten()
            .filter(|a| a.source_bill.bill_id == bill_id)
            .collect()
    }

    /// Get paths that have annotations for a version pair
    pub fn annotated_paths(&self, from: &str, to: &str) -> Vec<String> {
        self.get_annotations(from, to)
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

    /// Find an element by path across all versions
    ///
    /// Returns tuples of (date, element) for each version containing the path
    pub fn find_element(&self, path: &str) -> Vec<(&str, &USLMElement)> {
        self.versions
            .iter()
            .filter_map(|v| v.element.find(path).map(|e| (v.date.as_str(), e)))
            .collect()
    }

    /// Parse a USLM XML file into a USLMElement tree and add it to the dataset as a snapshot
    ///
    /// # Arguments
    ///
    /// * `xml_path` - Path to the USLM XML file
    /// * `date` - Publication date string in "YYYY-MM-DD" format
    /// * `label` - Optional label for the snapshot
    ///
    /// # Returns
    ///
    /// The OK(()), or a `ParseError` if parsing fails.
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
        });
        Ok(())
    }

    /// Load and merge all USLM XML files from a folder into a single element and add it to
    /// the dataset as a snapshot
    ///
    /// Reads all .xml files from the folder, parses them in parallel using Rayon,
    /// and merges all parsed elements' children into a single root element. This is
    /// useful for loading a complete US Code title that may be split across multiple
    /// XML files.
    ///
    /// # Arguments
    ///
    /// * `folder_path` - Path to directory containing USLM XML files
    /// * `date` - Publication date string in "YYYY-MM-DD" format
    /// * `label` - Optional label for the snapshot
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
        });
        Ok(())
    }

    /// Search for text across all versions
    pub fn search_text(&self, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for version in &self.versions {
            Self::search_element(&version.element, &version.date, &query_lower, &mut results);
        }

        results
    }

    fn search_element(
        element: &USLMElement,
        date: &str,
        query: &str,
        results: &mut Vec<SearchResult>,
    ) {
        let fields = [
            ("heading", &element.data.heading),
            ("chapeau", &element.data.chapeau),
            ("content", &element.data.content),
            ("proviso", &element.data.proviso),
            ("continuation", &element.data.continuation),
        ];

        for (field_name, field_value) in fields {
            if let Some(text) = field_value
                && text.to_lowercase().contains(query)
            {
                results.push(SearchResult {
                    date: date.to_string(),
                    path: element.data.path.clone(),
                    field: field_name.to_string(),
                    snippet: text.clone(),
                });
            }
        }

        for child in &element.children {
            Self::search_element(child, date, query, results);
        }
    }

    // --- Congress data methods ---

    /// Add a Congress member to the dataset
    pub fn add_member(&mut self, member: Member) {
        self.members.insert(member.bioguide_id.clone(), member);
    }

    /// Get a member by bioguide ID
    pub fn get_member(&self, bioguide_id: &str) -> Option<&Member> {
        self.members.get(bioguide_id)
    }

    /// Add sponsor info for a bill
    pub fn add_sponsor_info(&mut self, info: SponsorInfo) {
        self.sponsors.insert(info.bill_id.clone(), info);
    }

    /// Get sponsor info by bill ID
    pub fn get_sponsor_info(&self, bill_id: &str) -> Option<&SponsorInfo> {
        self.sponsors.get(bill_id)
    }

    /// Get members who sponsored or cosponsored bills affecting a path
    pub fn sponsors_for_path(&self, path: &str) -> Vec<&Member> {
        let bill_ids: Vec<_> = self
            .annotations_for_path(path)
            .iter()
            .map(|a| &a.source_bill.bill_id)
            .collect();

        let mut member_ids: Vec<&str> = Vec::new();

        for bill_id in &bill_ids {
            if let Some(info) = self.sponsors.get(*bill_id) {
                member_ids.push(&info.sponsor);
                for cosponsor in &info.cosponsors {
                    member_ids.push(&cosponsor.bioguide_id);
                }
            }
        }

        member_ids
            .into_iter()
            .filter_map(|id| self.members.get(id))
            .collect()
    }

    /// Load bill data from a BillDownload (raw downloaded data)
    ///
    /// Parses the XML and JSON, stores bill, sponsors, and members.
    /// Returns the canonical bill_id (from parsed XML, e.g., "119-21")
    pub fn load_bill_download(&mut self, download: &BillDownload) -> Result<String, DatasetError> {
        use crate::uslm::bill_parser;
        use serde_json::Value;

        // Parse bill XML to get AmendmentData
        let bill =
            bill_parser::parse_bill_amendments_from_str(&download.bill_id, &download.bill_xml)
                .map_err(|e| {
                    DatasetError::Json(serde_json::Error::io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        e.to_string(),
                    )))
                })?;
        let bill_id = bill.bill_id.clone();
        self.add_bill(bill);

        // Parse sponsors JSON
        let sponsors_v: Value = serde_json::from_str(&download.sponsors_json)?;
        let sponsor_id = sponsors_v["bill"]["sponsors"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|s| s["bioguideId"].as_str())
            .unwrap_or("")
            .to_string();

        // Parse cosponsors JSON
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

        // Use canonical bill_id from parsed XML
        self.add_sponsor_info(SponsorInfo {
            bill_id: bill_id.clone(),
            sponsor: sponsor_id,
            cosponsors,
        });

        // Parse and add members
        for json in download.member_jsons.values() {
            if let Ok(member) = Member::from_api_response(json) {
                self.add_member(member);
            }
        }

        // TODO: Parse votes_json when available

        Ok(bill_id)
    }
}
