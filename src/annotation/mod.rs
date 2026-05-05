//! Annotation types for linking code changes to their legislative source.
//!
//! These types are used by `Dataset` to track how bill amendments
//! caused changes in the US Code.

use serde::{Deserialize, Serialize};

use crate::uslm::AmendingAction;

/// An annotation linking a change to its legal cause
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeAnnotation {
    /// The type of legal operation that caused this change
    pub operation: AmendingAction,

    /// Reference to the bill that enacted the change
    pub source_bill: BillReference,

    /// Structural paths of changes in the TreeDiff
    pub paths: Vec<String>,

    /// Metadata about the annotation itself
    pub metadata: AnnotationMetadata,
}

/// A reference to a bill instruction that caused a change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillReference {
    /// The bill identifier (e.g., "119-21" for Pub. L. 119-21)
    pub bill_id: String,
    /// The amendment ID (content-hash) that links back to the BillAmendment
    pub amendment_id: String,
    /// Text of the amending instruction from the bill (may be a substring of the full amendment)
    pub causative_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationMetadata {
    /// Current verification status of this annotation
    pub status: AnnotationStatus,

    /// Confidence score for AI-generated annotations (0.0 - 1.0)
    /// None for human annotations
    pub confidence: Option<f32>,

    /// Identifier for who/what created this annotation
    /// Convention: "human:<username>" or "model:<model-id>"
    pub annotator: String,

    /// When this annotation was created
    pub timestamp: time::OffsetDateTime,

    /// Freeform notes
    pub notes: Option<String>,

    /// Explanation of how/why this annotation was determined
    /// Distinct from `notes` - reasoning explains the annotation process,
    /// notes provide additional context about the change itself
    pub reasoning: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnnotationStatus {
    /// Not yet reviewed
    Pending,
    /// Human confirmed as correct
    Verified,
    /// Flagged for review due to uncertainty or disagreement
    Disputed,
    /// Determined to be incorrect
    Rejected,
}
