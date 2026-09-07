//! The legislature domain: what a bill does to existing law.
//!
//! These types describe legislative behaviour, not a markup language. An
//! amendment strikes, inserts, or redesignates, and it does so whether the bill
//! arrived as USLM XML, as a scanned PDF, or from a state legislature that
//! publishes in some other format entirely.
//!
//! They lived in `uslm` until now, which confused a format with a domain. That
//! mattered as soon as a second format appeared: a reader for a state code
//! needs these types and has no USLM to speak of, and a court opinion needs
//! none of them at all.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::uslm::USLMError;

/// Types of amendments that can be made to existing law via a bill
///
/// When a bill modifies existing United States Code, it uses specific
/// amending actions to describe the type of change being made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmendingAction {
    /// Modify existing text
    Amend,
    /// Add new text or sections
    Add,
    /// Remove existing text or sections
    Delete,
    /// Insert new text at a specific location
    Insert,
    /// Change the designation or numbering of sections
    Redesignate,
    /// Remove an entire section or provision from the law
    Repeal,
    /// Relocate an element (may include redesignation)
    Move,
    /// Remove specific text within an element (finer than Delete)
    Strike,
    /// Remove specific text and replace with new text
    StrikeAndInsert,
}

impl FromStr for AmendingAction {
    type Err = USLMError;

    /// Parse an amending action from its string representation
    ///
    /// This implementation is case-insensitive. Returns an error if the
    /// action type is not recognized.
    fn from_str(s: &str) -> std::result::Result<Self, <Self as std::str::FromStr>::Err> {
        match s.to_lowercase().as_str() {
            "amend" => Ok(AmendingAction::Amend),
            "add" => Ok(AmendingAction::Add),
            "delete" => Ok(AmendingAction::Delete),
            "insert" => Ok(AmendingAction::Insert),
            "redesignate" => Ok(AmendingAction::Redesignate),
            "repeal" => Ok(AmendingAction::Repeal),
            "move" => Ok(AmendingAction::Move),
            "strike" => Ok(AmendingAction::Strike),
            "strikeandinsert" | "strike_and_insert" => Ok(AmendingAction::StrikeAndInsert),
            _ => Err(USLMError::UnknownAmendingAction(s.to_lowercase())),
        }
    }
}

impl AmendingAction {
    /// Extract all text from a node and its descendants
    #[allow(dead_code)]
    fn extract_all_text(node: &roxmltree::Node) -> String {
        let mut text = String::new();
        for descendant in node.descendants() {
            if let Some(t) = descendant.text() {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(t);
            }
        }
        text
    }
}

/// A reference to a USC section found in a bill
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct UscReference {
    /// The USLM path being referenced (e.g., "/us/usc/t7/s2025/c/1/A/ii")
    pub path: String,
    /// The human-readable text of the reference (e.g., "7 U.S.C. 2025(c)(1)(A)(ii)")
    pub display_text: String,
}

/// An amending action found in a bill
///
/// Not `Eq` or `Hash`: its provenance carries a raw model score, which is a
/// float. It was never used as a key — only as a map value — so nothing is
/// lost. Its identity is `id`, a content hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillAmendment {
    /// Content-based ID: sha256("{bill_id}:{amending_text}")
    /// This provides a stable, deterministic identifier that works regardless of source format.
    pub id: String,

    /// Type of action (amend, add, delete, insert, redesignate, repeal)
    pub action_types: Vec<AmendingAction>,

    /// The text of the change
    pub amending_text: String,

    /// List of word-level changes that an amendment enacts
    pub changes: Vec<BillDiff>,

    /// Where `changes` came from.
    ///
    /// The amending text is parsed from the bill and is a fact from a source.
    /// The word-level changes are not: a model produced them, and until this
    /// existed nothing recorded that. Evidence without a verification state
    /// reads as corroboration, so the whole provenance is carried rather than
    /// the evidence alone (#58).
    ///
    /// `None` means nothing was recorded, which is every amendment extracted
    /// before this was built.
    #[serde(default)]
    pub provenance: Option<crate::link::Provenance>,
}

impl BillAmendment {
    pub fn update_changes(&self, changes: &[BillDiff]) -> Self {
        BillAmendment {
            id: self.id.clone(),
            action_types: self.action_types.clone(),
            amending_text: self.amending_text.clone(),
            changes: changes.to_vec(),
            provenance: self.provenance.clone(),
        }
    }
}

/// Actions caused by a bill amendment
///
/// This is designed to exist as single entries for every logical
/// amending action. For example, given the following amending text:
/// ```ignore
///(B)
/// in subsection (b)--
///
///   (i)
///   by striking "specified research" and inserting "foreign research",
///
///
///   (ii)
///   by inserting "and which are attributable to foreign research (within the meaning of section 41(d)(4)(F))" before the period at the end, and
/// ```
/// we would annotate that with two Bill Diffs:
/// ```ignore
/// {
///  "removed": ["specified"],
///  "added": ["foreign"]
/// }
/// ```
/// and
/// ```ignore
/// {
///  "removed": [],
///  "added": [
///    "which",
///    "attributable",
///    "foreign",
///    "research",
///    "(within",
///    "meaning",
///    "section",
///    "41(d)(4)(F))"
///  ]
///}
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct BillDiff {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}
