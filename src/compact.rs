//! Compact serialization format with string deduplication
//!
//! Stores strings in a table, uses indices in data structures.
//! Reduces file size ~5x and eliminates duplicate allocations on load.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::congress::{BillVotes, Member, SponsorInfo};
use crate::dataset::{DatasetMetadata, Expression, ExpressionId, WorkId};
use crate::intern::StringInterner;
/// Custom serializer for HashMap with tuple keys (JSON doesn't support non-string keys)
use crate::link::Link;
use crate::storage::{InMemoryStorage, SCHEMA_VERSION, memory::ExpressionsByWork};
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

/// Compact Expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionCompact {
    pub work: StrIdx,
    pub date: StrIdx,
    pub label: Option<StrIdx>,
    pub element: USLMElementCompact,
}

/// Compact Dataset for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatasetCompact {
    /// The schema this file was written by.
    ///
    /// Serialized first, and read on its own by [`schema_version_of`] before
    /// the rest of the file is parsed. Most fields below default to empty, so
    /// without a guard an older file would load as a dataset that simply holds
    /// nothing — an empty answer that actually means "wrong schema".
    ///
    /// `#[serde(default)]` so a file written before the field existed reads 0
    /// rather than failing to parse.
    #[serde(default)]
    pub schema_version: i32,
    pub string_table: StringTable,
    pub metadata: DatasetMetadata,
    pub expressions: Vec<ExpressionCompact>,
    /// Bills (stored as-is, no dedup needed)
    #[serde(default)]
    pub bills: HashMap<String, Bill>,
    /// Every link, by its content-hash id (stored as-is)
    #[serde(default)]
    pub links: std::collections::BTreeMap<String, Link>,
    /// Every verbatim model reply, by the hash of its own text
    #[serde(default)]
    pub replies: std::collections::BTreeMap<String, String>,
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

/// Just the schema version, read without parsing the rest of the file.
///
/// The check has to come first. Every other field changes shape between
/// schemas, so parsing the whole file to reach the version fails on one of
/// those fields instead, and reports a serde error about a type mismatch deep
/// in the document rather than the one thing the reader needs to be told: this
/// file was written by another build, rebuild it.
#[derive(Deserialize)]
struct SchemaProbe {
    #[serde(default)]
    schema_version: i32,
}

/// Refuse a compact-JSON file this build cannot read.
///
/// The same guard SQLite has had since #68. Without it the JSON path fails the
/// quiet way instead: an older file parses, every collection defaults to empty,
/// and the dataset reports that it holds nothing.
pub fn check_schema_version(json: &str) -> Result<(), crate::dataset::DatasetError> {
    let probe: SchemaProbe = serde_json::from_str(json)?;
    if probe.schema_version != SCHEMA_VERSION {
        return Err(crate::dataset::DatasetError::SchemaVersionMismatch {
            found: probe.schema_version,
            expected: SCHEMA_VERSION,
        });
    }
    Ok(())
}

impl DatasetCompact {
    /// Convert from InMemoryStorage
    pub fn from_storage(storage: &InMemoryStorage) -> Self {
        let mut table = StringTable::new();

        let expressions: Vec<ExpressionCompact> = storage
            .all_expressions()
            .map(|e| Self::compact_expression(e, &mut table))
            .collect();

        Self {
            schema_version: SCHEMA_VERSION,
            string_table: table,
            metadata: storage.metadata.clone(),
            expressions,
            bills: storage.bills.clone(),
            links: storage.links.clone(),
            replies: storage.replies.clone(),
            members: storage.members.clone(),
            sponsors: storage.sponsors.clone(),
            bill_votes: storage.bill_votes.clone(),
        }
    }

    fn compact_expression(expression: &Expression, table: &mut StringTable) -> ExpressionCompact {
        ExpressionCompact {
            work: table.intern(expression.id.work.as_str()),
            date: table.intern(&expression.id.at),
            label: expression.label.as_ref().map(|l| table.intern(l)),
            element: Self::compact_element(&expression.element, table),
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

    /// Convert to InMemoryStorage
    pub fn into_storage(self) -> InMemoryStorage {
        let mut interner = StringInterner::new();

        // Pre-populate interner from string table (single pass)
        for s in &self.string_table.strings {
            interner.intern(s);
        }

        let mut expressions = ExpressionsByWork::new();
        for compact in self.expressions {
            let expression = Self::expand_expression(compact, &self.string_table, &mut interner);
            expressions
                .entry(expression.id.work.clone())
                .or_default()
                .insert(expression.id.at.clone(), expression);
        }

        InMemoryStorage::from_parts(
            self.metadata,
            expressions,
            self.bills,
            self.links,
            self.replies,
            self.members,
            self.sponsors,
            self.bill_votes,
            interner,
        )
    }

    fn expand_expression(
        compact: ExpressionCompact,
        table: &StringTable,
        interner: &mut StringInterner,
    ) -> Expression {
        Expression {
            id: ExpressionId::new(
                WorkId::new(table.get(compact.work)),
                table.get(compact.date),
            ),
            label: compact.label.map(|i| table.get(i).to_string()),
            element: Self::expand_element(compact.element, table, interner),
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
