//! Compact serialization format with string deduplication
//!
//! Stores strings in a table, uses indices in data structures.
//! Reduces file size ~5x and eliminates duplicate allocations on load.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::congress::{BillVotes, Member, SponsorInfo};
use crate::dataset::{DatasetMetadata, Expression, ExpressionId, WorkId};
use crate::document::{ClassPayload, DocumentNode, NodeData, NodeType};
use crate::intern::StringInterner;
/// Custom serializer for HashMap with tuple keys (JSON doesn't support non-string keys)
use crate::link::{Link, Provenance};
use crate::storage::{InMemoryStorage, SCHEMA_VERSION, memory::ExpressionsByWork};
use crate::uslm::bill_parser::Bill;

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

/// Compact [`NodeData`] with string indices
///
/// Every field is a string index or an option of one, including the class
/// payload. A payload is interned whole, as the JSON text it is: the compact
/// format never looks inside a payload any more than the rest of the core does,
/// and it does not need to in order to store it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDataCompact {
    pub path: StrIdx,
    pub node_type: StrIdx,
    pub date: StrIdx, // Store date as string index too
    pub heading: Option<StrIdx>,
    pub chapeau: Option<StrIdx>,
    pub proviso: Option<StrIdx>,
    pub content: Option<StrIdx>,
    pub continuation: Option<StrIdx>,
    /// Where the node's text came from. Stored whole rather than interned: most
    /// nodes carry none, and a `Provenance` is a record rather than a string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Box<Provenance>>,
    /// The namespace of the class payload, and its JSON text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<ClassPayloadCompact>,
}

/// Compact [`ClassPayload`]: the namespace and the facts, both interned.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassPayloadCompact {
    pub namespace: StrIdx,
    pub value: StrIdx,
}

/// Compact [`DocumentNode`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentNodeCompact {
    pub data: NodeDataCompact,
    pub children: Vec<DocumentNodeCompact>,
}

/// Compact Expression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpressionCompact {
    pub work: StrIdx,
    pub date: StrIdx,
    pub label: Option<StrIdx>,
    pub root: DocumentNodeCompact,
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
            root: Self::compact_node(&expression.root, table),
        }
    }

    fn compact_node(node: &DocumentNode, table: &mut StringTable) -> DocumentNodeCompact {
        let data = &node.data;
        DocumentNodeCompact {
            data: NodeDataCompact {
                path: table.intern(&data.path),
                node_type: table.intern(data.node_type.as_str()),
                date: table.intern(&data.date.to_string()),
                heading: data.heading.as_ref().map(|s| table.intern(s)),
                chapeau: data.chapeau.as_ref().map(|s| table.intern(s)),
                proviso: data.proviso.as_ref().map(|s| table.intern(s)),
                content: data.content.as_ref().map(|s| table.intern(s)),
                continuation: data.continuation.as_ref().map(|s| table.intern(s)),
                provenance: data.provenance.clone(),
                payload: data.payload.as_ref().map(|payload| ClassPayloadCompact {
                    namespace: table.intern(&payload.namespace),
                    value: table.intern(&payload.value),
                }),
            },
            children: node
                .children
                .iter()
                .map(|c| Self::compact_node(c, table))
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
            root: Self::expand_node(compact.root, table, interner),
        }
    }

    fn expand_node(
        node: DocumentNodeCompact,
        table: &StringTable,
        interner: &mut StringInterner,
    ) -> DocumentNode {
        let data = node.data;
        let date_str = table.get(data.date);

        DocumentNode {
            data: NodeData {
                path: table.get_arc(data.path, interner),
                node_type: NodeType::new(table.get(data.node_type)),
                date: crate::date::date_str_to_date(date_str).unwrap_or_else(|_| {
                    time::Date::from_calendar_date(1970, time::Month::January, 1).unwrap()
                }),
                heading: table.get_arc_option(data.heading, interner),
                chapeau: table.get_arc_option(data.chapeau, interner),
                proviso: table.get_arc_option(data.proviso, interner),
                content: table.get_arc_option(data.content, interner),
                continuation: table.get_arc_option(data.continuation, interner),
                provenance: data.provenance,
                payload: data.payload.map(|payload| ClassPayload {
                    namespace: table.get_arc(payload.namespace, interner),
                    value: table.get(payload.value).into(),
                }),
            },
            children: node
                .children
                .into_iter()
                .map(|c| Self::expand_node(c, table, interner))
                .collect(),
        }
    }
}
