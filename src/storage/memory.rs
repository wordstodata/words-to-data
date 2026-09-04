//! In-memory storage backend

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::annotation::ChangeAnnotation;
use crate::congress::{BillVotes, HouseRollCall, Member, SponsorInfo, VotePosition};
use crate::dataset::{DatasetError, DatasetMetadata, SearchResult, VersionPair, VersionSnapshot};
use crate::diff::TreeDiff;
use crate::intern::StringInterner;
use crate::storage::{
    DocumentReader, DocumentWriter, LegislatureReader, LegislatureWriter, LinkReader, LinkWriter,
    Storage, VersionInfo,
};
use crate::uslm::USLMElement;
use crate::uslm::bill_parser::Bill;

/// Custom serializer for HashMap with tuple keys (JSON doesn't support non-string keys)
mod tuple_key_map {
    use super::*;

    pub fn serialize<S>(
        map: &HashMap<VersionPair, Vec<ChangeAnnotation>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let vec: Vec<_> = map.iter().collect();
        vec.serialize(serializer)
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<HashMap<VersionPair, Vec<ChangeAnnotation>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec: Vec<(VersionPair, Vec<ChangeAnnotation>)> = Vec::deserialize(deserializer)?;
        Ok(vec.into_iter().collect())
    }
}

/// In-memory storage backend
///
/// Stores all data in memory using HashMaps. Good for small datasets
/// or when you need fast access without persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InMemoryStorage {
    pub metadata: DatasetMetadata,
    pub versions: Vec<VersionSnapshot>,
    pub bills: HashMap<String, Bill>,
    #[serde(with = "tuple_key_map")]
    pub diff_annotations: HashMap<VersionPair, Vec<ChangeAnnotation>>,
    pub members: HashMap<String, Member>,
    pub sponsors: HashMap<String, SponsorInfo>,
    pub bill_votes: HashMap<String, BillVotes>,
    #[serde(skip)]
    interner: StringInterner,
}

impl InMemoryStorage {
    pub fn new(metadata: DatasetMetadata) -> Self {
        Self {
            metadata,
            versions: Vec::new(),
            bills: HashMap::new(),
            diff_annotations: HashMap::new(),
            members: HashMap::new(),
            sponsors: HashMap::new(),
            bill_votes: HashMap::new(),
            interner: StringInterner::new(),
        }
    }

    pub fn intern_strings(&mut self) {
        for version in self.versions.iter_mut() {
            version.element.intern_strings(&mut self.interner);
        }
    }

    /// Create from parts (used by loaders)
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        metadata: DatasetMetadata,
        versions: Vec<VersionSnapshot>,
        bills: HashMap<String, Bill>,
        diff_annotations: HashMap<VersionPair, Vec<ChangeAnnotation>>,
        members: HashMap<String, Member>,
        sponsors: HashMap<String, SponsorInfo>,
        bill_votes: HashMap<String, BillVotes>,
        interner: StringInterner,
    ) -> Self {
        Self {
            metadata,
            versions,
            bills,
            diff_annotations,
            members,
            sponsors,
            bill_votes,
            interner,
        }
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
                    path: element.data.path.to_string(),
                    field: field_name.to_string(),
                    snippet: text.to_string(),
                });
            }
        }

        for child in &element.children {
            Self::search_element(child, date, query, results);
        }
    }
}

impl DocumentReader for InMemoryStorage {
    fn list_versions(&self) -> Result<Vec<VersionInfo>, DatasetError> {
        Ok(self
            .versions
            .iter()
            .map(|v| VersionInfo {
                date: v.date.clone(),
                label: v.label.clone(),
            })
            .collect())
    }

    fn get_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        Ok(self.versions.iter().find(|v| v.date == date).cloned())
    }

    fn compute_diff(&self, from: &str, to: &str) -> Result<TreeDiff, DatasetError> {
        let from_v = self
            .versions
            .iter()
            .find(|v| v.date == from)
            .ok_or_else(|| DatasetError::VersionNotFound(from.to_string()))?;
        let to_v = self
            .versions
            .iter()
            .find(|v| v.date == to)
            .ok_or_else(|| DatasetError::VersionNotFound(to.to_string()))?;
        Ok(TreeDiff::from_elements(&from_v.element, &to_v.element))
    }

    fn search_text(&self, query: &str) -> Result<Vec<SearchResult>, DatasetError> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for version in &self.versions {
            Self::search_element(&version.element, &version.date, &query_lower, &mut results);
        }

        Ok(results)
    }

    fn get_version_by_label(&self, label: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        Ok(self
            .versions
            .iter()
            .find(|v| v.label.as_deref() == Some(label))
            .cloned())
    }

    fn next_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        let pos = self.versions.iter().position(|v| v.date == date);
        Ok(pos.and_then(|p| self.versions.get(p + 1).cloned()))
    }

    fn prev_version(&self, date: &str) -> Result<Option<VersionSnapshot>, DatasetError> {
        let pos = self.versions.iter().position(|v| v.date == date);
        Ok(pos.and_then(|p| {
            if p == 0 {
                None
            } else {
                self.versions.get(p - 1).cloned()
            }
        }))
    }

    fn find_element(&self, path: &str) -> Result<Vec<(String, USLMElement)>, DatasetError> {
        Ok(self
            .versions
            .iter()
            .filter_map(|v| v.element.find(path).map(|e| (v.date.clone(), e.clone())))
            .collect())
    }
}

impl LinkReader for InMemoryStorage {
    fn get_annotations(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Option<Vec<ChangeAnnotation>>, DatasetError> {
        let key = (from.to_string(), to.to_string());
        Ok(self.diff_annotations.get(&key).cloned())
    }

    fn annotations_for_path(&self, path: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        Ok(self
            .diff_annotations
            .values()
            .flatten()
            .filter(|a| a.paths.iter().any(|p| p == path))
            .cloned()
            .collect())
    }

    fn annotations_for_bill(&self, bill_id: &str) -> Result<Vec<ChangeAnnotation>, DatasetError> {
        Ok(self
            .diff_annotations
            .values()
            .flatten()
            .filter(|a| a.source_bill.bill_id == bill_id)
            .cloned()
            .collect())
    }

    fn annotation_pairs(&self) -> Result<Vec<VersionPair>, DatasetError> {
        Ok(self.diff_annotations.keys().cloned().collect())
    }
}

impl LegislatureReader for InMemoryStorage {
    fn get_bill(&self, id: &str) -> Result<Option<Bill>, DatasetError> {
        Ok(self.bills.get(id).cloned())
    }

    fn list_bill_ids(&self) -> Result<Vec<String>, DatasetError> {
        Ok(self.bills.keys().cloned().collect())
    }

    fn get_member(&self, bioguide_id: &str) -> Result<Option<Member>, DatasetError> {
        Ok(self.members.get(bioguide_id).cloned())
    }

    fn get_sponsor_info(&self, bill_id: &str) -> Result<Option<SponsorInfo>, DatasetError> {
        Ok(self.sponsors.get(bill_id).cloned())
    }

    fn get_bill_votes(&self, bill_id: &str) -> Result<Option<BillVotes>, DatasetError> {
        Ok(self.bill_votes.get(bill_id).cloned())
    }

    fn votes_by_member(
        &self,
        bioguide_id: &str,
    ) -> Result<Vec<(HouseRollCall, VotePosition)>, DatasetError> {
        let mut results = Vec::new();
        for bill_votes in self.bill_votes.values() {
            for roll_call in &bill_votes.roll_calls {
                for mv in &roll_call.member_votes {
                    if mv.bioguide_id == bioguide_id {
                        results.push((roll_call.clone(), mv.position));
                    }
                }
            }
        }
        Ok(results)
    }
}

impl DocumentWriter for InMemoryStorage {
    fn metadata(&self) -> &DatasetMetadata {
        &self.metadata
    }

    fn set_metadata(&mut self, metadata: DatasetMetadata) {
        self.metadata = metadata;
    }

    fn add_version(&mut self, snapshot: VersionSnapshot) -> Result<(), DatasetError> {
        let pos = self
            .versions
            .binary_search_by(|v| v.date.cmp(&snapshot.date))
            .unwrap_or_else(|pos| pos);
        self.versions.insert(pos, snapshot);
        Ok(())
    }
}

impl LinkWriter for InMemoryStorage {
    fn add_annotation(
        &mut self,
        from: &str,
        to: &str,
        annotation: ChangeAnnotation,
    ) -> Result<(), DatasetError> {
        self.diff_annotations
            .entry((from.to_string(), to.to_string()))
            .or_default()
            .push(annotation);
        Ok(())
    }
}

impl LegislatureWriter for InMemoryStorage {
    fn add_bill(&mut self, bill: Bill) -> Result<(), DatasetError> {
        self.bills.insert(bill.bill_id.clone(), bill);
        Ok(())
    }

    fn add_member(&mut self, member: Member) -> Result<(), DatasetError> {
        self.members.insert(member.bioguide_id.clone(), member);
        Ok(())
    }

    fn add_sponsor_info(&mut self, info: SponsorInfo) -> Result<(), DatasetError> {
        self.sponsors.insert(info.bill_id.clone(), info);
        Ok(())
    }

    fn add_bill_votes(&mut self, votes: BillVotes) -> Result<(), DatasetError> {
        self.bill_votes.insert(votes.bill_id.clone(), votes);
        Ok(())
    }
}

impl Storage for InMemoryStorage {
    fn legislature(&self) -> Option<&dyn LegislatureReader> {
        let holds_legislature = !self.bills.is_empty()
            || !self.members.is_empty()
            || !self.sponsors.is_empty()
            || !self.bill_votes.is_empty();
        holds_legislature.then_some(self as &dyn LegislatureReader)
    }
}
