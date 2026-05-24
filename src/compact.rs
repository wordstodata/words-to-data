//! Compact serialization format with string deduplication
//!
//! Stores strings in a table, uses indices in data structures.
//! Reduces file size ~5x and eliminates duplicate allocations on load.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::annotation::ChangeAnnotation;
use crate::congress::{BillVotes, Member, SponsorInfo};
use crate::dataset::{Dataset, DatasetMetadata, VersionPair, VersionSnapshot};
use crate::intern::StringInterner;
use crate::uslm::bill_parser::Bill;
use crate::uslm::{DocumentType, ElementData, ElementType, RefPair, SourceCredit, USLMElement};

/// Index into string table (u32 = 4B vs Arc = 8B + allocation)
type StrIdx = u32;

/// String table for deduplication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StringTable {
    strings: Vec<String>,
    #[serde(skip)]
    lookup: HashMap<String, StrIdx>,
}

impl StringTable {
    pub fn new() -> Self {
        Self {
            strings: Vec::new(),
            lookup: HashMap::new(),
        }
    }

    /// Intern a string, return its index
    pub fn intern(&mut self, s: &str) -> StrIdx {
        if let Some(&idx) = self.lookup.get(s) {
            idx
        } else {
            let idx = self.strings.len() as StrIdx;
            self.strings.push(s.to_string());
            self.lookup.insert(s.to_string(), idx);
            idx
        }
    }

    /// Intern optional string
    pub fn intern_option(&mut self, s: Option<&str>) -> Option<StrIdx> {
        s.map(|v| self.intern(v))
    }

    /// Get string by index
    pub fn get(&self, idx: StrIdx) -> &str {
        &self.strings[idx as usize]
    }

    /// Build lookup table after deserialization
    pub fn rebuild_lookup(&mut self) {
        self.lookup.clear();
        for (idx, s) in self.strings.iter().enumerate() {
            self.lookup.insert(s.clone(), idx as StrIdx);
        }
    }

    /// Convert to interner (for runtime use)
    pub fn into_interner(self) -> StringInterner {
        let mut interner = StringInterner::new();
        for s in &self.strings {
            interner.intern(s);
        }
        interner
    }

    /// Get Arc<str> for index, using interner
    pub fn get_arc(&self, idx: StrIdx, interner: &mut StringInterner) -> Arc<str> {
        interner.intern(self.get(idx))
    }

    /// Get optional Arc<str> for index
    pub fn get_arc_option(
        &self,
        idx: Option<StrIdx>,
        interner: &mut StringInterner,
    ) -> Option<Arc<str>> {
        idx.map(|i| self.get_arc(i, interner))
    }
}

impl Default for StringTable {
    fn default() -> Self {
        Self::new()
    }
}

/// Compact ElementData with string indices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementDataCompact {
    pub path: StrIdx,
    pub element_type: ElementType,
    pub document_type: DocumentType,
    pub date: StrIdx, // Store date as string index too
    pub number_value: StrIdx,
    pub number_display: StrIdx,
    pub verbose_name: StrIdx,
    pub heading: Option<StrIdx>,
    pub chapeau: Option<StrIdx>,
    pub proviso: Option<StrIdx>,
    pub content: Option<StrIdx>,
    pub continuation: Option<StrIdx>,
    pub uslm_id: Option<StrIdx>,
    pub uslm_uuid: Option<StrIdx>,
    pub source_credits: Vec<SourceCreditCompact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefPairCompact {
    pub ref_id: StrIdx,
    pub description: StrIdx,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceCreditCompact {
    pub ref_pairs: Vec<RefPairCompact>,
}

/// Compact USLMElement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct USLMElementCompact {
    pub data: ElementDataCompact,
    pub children: Vec<USLMElementCompact>,
}

/// Compact VersionSnapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionSnapshotCompact {
    pub date: StrIdx,
    pub label: Option<StrIdx>,
    pub element: USLMElementCompact,
}

/// Compact Dataset for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetCompact {
    pub string_table: StringTable,
    pub metadata: DatasetMetadata,
    pub versions: Vec<VersionSnapshotCompact>,
    /// Bills (stored as-is, no dedup needed)
    #[serde(default)]
    pub bills: HashMap<String, Bill>,
    /// Annotations per version-pair (stored as-is)
    #[serde(default)]
    pub diff_annotations: HashMap<VersionPair, Vec<ChangeAnnotation>>,
    /// Congress members (stored as-is)
    #[serde(default)]
    pub members: HashMap<String, Member>,
    /// Sponsor info (stored as-is)
    #[serde(default)]
    pub sponsors: HashMap<String, SponsorInfo>,
    /// Roll call votes (stored as-is)
    #[serde(default)]
    pub bill_votes: HashMap<String, BillVotes>,
}

impl DatasetCompact {
    /// Convert from Dataset
    pub fn from_dataset(dataset: &Dataset) -> Self {
        let mut table = StringTable::new();

        let versions: Vec<VersionSnapshotCompact> = dataset
            .versions
            .iter()
            .map(|v| Self::compact_version(v, &mut table))
            .collect();

        Self {
            string_table: table,
            metadata: dataset.metadata.clone(),
            versions,
            bills: dataset.bills.clone(),
            diff_annotations: dataset.diff_annotations.clone(),
            members: dataset.members.clone(),
            sponsors: dataset.sponsors.clone(),
            bill_votes: dataset.bill_votes.clone(),
        }
    }

    fn compact_version(
        version: &VersionSnapshot,
        table: &mut StringTable,
    ) -> VersionSnapshotCompact {
        VersionSnapshotCompact {
            date: table.intern(&version.date),
            label: version.label.as_ref().map(|l| table.intern(l)),
            element: Self::compact_element(&version.element, table),
        }
    }

    fn compact_element(element: &USLMElement, table: &mut StringTable) -> USLMElementCompact {
        let data = &element.data;
        USLMElementCompact {
            data: ElementDataCompact {
                path: table.intern(&data.path),
                element_type: data.element_type,
                document_type: data.document_type.clone(),
                date: table.intern(&data.date.to_string()),
                number_value: table.intern(&data.number_value),
                number_display: table.intern(&data.number_display),
                verbose_name: table.intern(&data.verbose_name),
                heading: data.heading.as_ref().map(|s| table.intern(s)),
                chapeau: data.chapeau.as_ref().map(|s| table.intern(s)),
                proviso: data.proviso.as_ref().map(|s| table.intern(s)),
                content: data.content.as_ref().map(|s| table.intern(s)),
                continuation: data.continuation.as_ref().map(|s| table.intern(s)),
                uslm_id: data.uslm_id.as_ref().map(|s| table.intern(s)),
                uslm_uuid: data.uslm_uuid.as_ref().map(|s| table.intern(s)),
                source_credits: data
                    .source_credits
                    .iter()
                    .map(|sc| SourceCreditCompact {
                        ref_pairs: sc
                            .ref_pairs
                            .iter()
                            .map(|rp| RefPairCompact {
                                ref_id: table.intern(&rp.ref_id),
                                description: table.intern(&rp.description),
                            })
                            .collect(),
                    })
                    .collect(),
            },
            children: element
                .children
                .iter()
                .map(|c| Self::compact_element(c, table))
                .collect(),
        }
    }

    /// Convert to Dataset
    pub fn into_dataset(self) -> Dataset {
        let mut interner = StringInterner::new();

        // Pre-populate interner from string table (single pass)
        for s in &self.string_table.strings {
            interner.intern(s);
        }

        let versions: Vec<VersionSnapshot> = self
            .versions
            .into_iter()
            .map(|v| Self::expand_version(v, &self.string_table, &mut interner))
            .collect();

        Dataset::from_parts(
            self.metadata,
            versions,
            self.bills,
            self.diff_annotations,
            self.members,
            self.sponsors,
            self.bill_votes,
            interner,
        )
    }

    fn expand_version(
        version: VersionSnapshotCompact,
        table: &StringTable,
        interner: &mut StringInterner,
    ) -> VersionSnapshot {
        VersionSnapshot {
            date: table.get(version.date).to_string(),
            label: version.label.map(|i| table.get(i).to_string()),
            element: Self::expand_element(version.element, table, interner),
        }
    }

    fn expand_element(
        element: USLMElementCompact,
        table: &StringTable,
        interner: &mut StringInterner,
    ) -> USLMElement {
        let data = element.data;
        let date_str = table.get(data.date);

        USLMElement {
            data: ElementData {
                path: table.get_arc(data.path, interner),
                element_type: data.element_type,
                document_type: data.document_type,
                date: crate::date::date_str_to_date(date_str).unwrap_or_else(|_| {
                    time::Date::from_calendar_date(1970, time::Month::January, 1).unwrap()
                }),
                number_value: table.get_arc(data.number_value, interner),
                number_display: table.get_arc(data.number_display, interner),
                verbose_name: table.get_arc(data.verbose_name, interner),
                heading: table.get_arc_option(data.heading, interner),
                chapeau: table.get_arc_option(data.chapeau, interner),
                proviso: table.get_arc_option(data.proviso, interner),
                content: table.get_arc_option(data.content, interner),
                continuation: table.get_arc_option(data.continuation, interner),
                uslm_id: table.get_arc_option(data.uslm_id, interner),
                uslm_uuid: table.get_arc_option(data.uslm_uuid, interner),
                source_credits: data
                    .source_credits
                    .into_iter()
                    .map(|sc| SourceCredit {
                        ref_pairs: sc
                            .ref_pairs
                            .into_iter()
                            .map(|rp| RefPair {
                                ref_id: table.get(rp.ref_id).to_string(),
                                description: table.get(rp.description).to_string(),
                            })
                            .collect(),
                    })
                    .collect(),
            },
            children: element
                .children
                .into_iter()
                .map(|c| Self::expand_element(c, table, interner))
                .collect(),
        }
    }
}
