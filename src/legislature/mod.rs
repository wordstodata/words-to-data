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

pub mod redesignation;

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::uslm::USLMError;

/// What a bill says it does to a provision of existing law.
///
/// This is the publisher's vocabulary, and nothing else. `AmendingActionTypeEnum`
/// in `uslm-2.0.17.xsd` (around line 610) allows twelve values, a conforming bill
/// writes one of them in an `amendingAction/@type` attribute, and there is one
/// variant here for each. The variants are in the schema's own order, and each
/// doc comment says what the schema says.
///
/// A drafter's prose word is not one of these. "By striking X and inserting Y"
/// is how the law reads, and the markup for it is `delete` and `insert`. A word
/// out of the prose — as a model answers it — comes in through
/// [`AmendingAction::from_prose`], which maps the prose onto the schema, so that
/// this type holds one vocabulary and not two.
///
/// # Reading what was stored before
///
/// Until #156 this enum also carried `Strike` and `StrikeAndInsert`, and a real
/// sweep stored 150 annotations that use them
/// (`tests/test_data/processed/annotations.json`). Those are records, so the
/// serde aliases below read them as the schema's word for the same act, exactly
/// as [`AmendingAction::from_prose`] does. Nothing writes the old word again:
/// there is no variant for it, so serialization always gives the schema's word.
///
/// `move` gets no alias. The schema has no action for a relocation, `redesignate`
/// is a different fact, and nothing in the repo's recorded data holds the word,
/// so there is no record to keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmendingAction {
    /// Enacts a law.
    Enact,
    /// Adds a provision to existing law.
    Add,
    /// Modifies an existing provision in the law.
    Amend,
    /// Replaces an existing provision in the law.
    ///
    /// The alias reads a record written before #156, where "by striking X and
    /// inserting Y" was stored in the drafter's words.
    #[serde(alias = "strike_and_insert", alias = "strikeandinsert")]
    Substitute,
    /// Changes the number of an existing provision in the law.
    Redesignate,
    /// Repeals a provision of law or regulation.
    Repeal,
    /// Repeals a provision and reserves its location.
    RepealAndReserve,
    /// Adds text to a proposed provision to the law.
    Insert,
    /// Removes text from a proposed provision to the law.
    ///
    /// The alias reads a record written before #156, where "by striking X" was
    /// stored in the drafter's word.
    #[serde(alias = "strike")]
    Delete,
    /// Makes the text the same as the defined replacement text.
    Conform,
    /// No change is directed, as in "The authority... continues to read...".
    NoChange,
    /// An action the publisher has not yet defined.
    Unknown,
}

impl FromStr for AmendingAction {
    type Err = USLMError;

    /// Read the value of an `amendingAction/@type` attribute.
    ///
    /// Case-insensitive, and it accepts the serde form of a two-word value
    /// (`repeal_and_reserve`) beside the schema's own (`repealAndReserve`), so a
    /// value that was stored and read back gives the variant it came from.
    ///
    /// Any other word is an error rather than a silent nothing. `Unknown` is the
    /// schema's own value for an undefined action and is not a bin for a word
    /// this build cannot read: telling those two apart is the whole of #156.
    fn from_str(s: &str) -> std::result::Result<Self, <Self as std::str::FromStr>::Err> {
        match s.to_lowercase().as_str() {
            "enact" => Ok(AmendingAction::Enact),
            "add" => Ok(AmendingAction::Add),
            "amend" => Ok(AmendingAction::Amend),
            "substitute" => Ok(AmendingAction::Substitute),
            "redesignate" => Ok(AmendingAction::Redesignate),
            "repeal" => Ok(AmendingAction::Repeal),
            "repealandreserve" | "repeal_and_reserve" => Ok(AmendingAction::RepealAndReserve),
            "insert" => Ok(AmendingAction::Insert),
            "delete" => Ok(AmendingAction::Delete),
            "conform" => Ok(AmendingAction::Conform),
            "nochange" | "no_change" => Ok(AmendingAction::NoChange),
            "unknown" => Ok(AmendingAction::Unknown),
            _ => Err(USLMError::UnknownAmendingAction(s.to_lowercase())),
        }
    }
}

impl AmendingAction {
    /// Read a word for an action out of prose, as a model answers it.
    ///
    /// A bill's markup writes the publisher's word. A bill's *prose* does not: it
    /// says "by striking 'or' and inserting 'and'", and a model asked what an
    /// amendment did answers in those words. Real recorded replies do:
    /// `tests/test_data/processed/model_replies.json` holds `strikeandinsert`,
    /// and 150 of the 753 annotations in `tests/test_data/processed/annotations.json`
    /// were stored as `strike` or `strike_and_insert`.
    ///
    /// Those words are mapped onto the schema's word for the same act, rather
    /// than refused. What the model said is kept in its own right — the reasoning
    /// and the reply go into the link's [`crate::link::Provenance`], which is what
    /// a reading is recorded with — so the action may be the publisher's word
    /// without anything being lost.
    ///
    /// | Prose | Schema | Why |
    /// | --- | --- | --- |
    /// | `strike` | `delete` | the schema's word for removing text |
    /// | `strike and insert` | `substitute` | the schema's `substitute` "replaces an existing provision" |
    ///
    /// `move` is **not** mapped. The schema has no action for a relocation, and
    /// `redesignate` is a different fact: it renumbers a provision that stays
    /// where it is. A caller gets an error and reports it, the same as for any
    /// other word this does not know.
    ///
    /// ```
    /// use words_to_data::legislature::AmendingAction;
    ///
    /// // The drafter's words, mapped onto the publisher's.
    /// assert_eq!(AmendingAction::from_prose("strike").unwrap(), AmendingAction::Delete);
    /// assert_eq!(
    ///     AmendingAction::from_prose("strikeandinsert").unwrap(),
    ///     AmendingAction::Substitute
    /// );
    /// // The publisher's own words read too.
    /// assert_eq!(AmendingAction::from_prose("repeal").unwrap(), AmendingAction::Repeal);
    /// // Relocation has no action, in either vocabulary.
    /// assert!(AmendingAction::from_prose("move").is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`USLMError::UnknownAmendingAction`] when the word is neither a
    /// value of the schema nor a prose word with a value to map onto.
    pub fn from_prose(s: &str) -> std::result::Result<Self, USLMError> {
        match s.to_lowercase().as_str() {
            "strike" => Ok(AmendingAction::Delete),
            "strikeandinsert" | "strike_and_insert" => Ok(AmendingAction::Substitute),
            _ => AmendingAction::from_str(s),
        }
    }

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

    /// Every action the bill's markup states, in the order the markup states it.
    ///
    /// A bag rather than a set, and one entry for each `amendingAction` in the
    /// instruction's subtree, so the same action can appear twice.
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
