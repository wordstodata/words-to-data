use std::{collections::HashSet, hash::Hash};

use serde::{Deserialize, Serialize};

use crate::{diff::TreeDiff, uslm::AmendingAction};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalDiff {
    /// The underlying word-level diffs
    pub tree_diff: TreeDiff,

    /// List of annotated changes
    pub annotations: Vec<ChangeAnnotation>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
/// A reference to a bill instruction that caused a change.
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

impl LegalDiff {
    /// Create a new LegalDiff from an existing TreeDiff with no annotations
    pub fn new(tree_diff: &TreeDiff) -> Self {
        LegalDiff {
            tree_diff: tree_diff.clone(),
            annotations: Vec::new(),
        }
    }

    /// Add an annotation for a specific structural path
    pub fn add_annotation(&mut self, annotation: ChangeAnnotation) {
        self.annotations.push(annotation);
    }

    /// Get all annotations for a specific path
    pub fn get_annotations(&self, path: &str) -> Vec<&ChangeAnnotation> {
        let path_string = String::from(path);

        let matches: Vec<&ChangeAnnotation> = self
            .annotations
            .iter()
            .filter(|annotation| annotation.paths.contains(&path_string))
            .collect();
        matches
    }

    /// Get the TreeDiff node for a specific path
    /// Delegates to TreeDiff::find()
    pub fn get_diff_node(&self, path: &str) -> Option<&TreeDiff> {
        self.tree_diff.find(path)
    }

    /// Get all paths that have annotations
    pub fn annotated_paths(&self) -> HashSet<String> {
        self.annotations
            .iter()
            .flat_map(|annotation| annotation.paths.clone())
            .collect()
    }

    /// Get all paths in the TreeDiff that lack annotations
    /// Useful for finding unannotated changes
    pub fn unannotated_paths(&self) -> Vec<String> {
        let a_paths = self.annotated_paths();
        let mut paths_with_changes = Vec::new();
        Self::collect_paths_with_changes(&self.tree_diff, &mut paths_with_changes);
        paths_with_changes
            .into_iter()
            .filter(|p| !a_paths.contains(p))
            .collect()
    }

    /// Helper to recursively collect all paths that have changes
    fn collect_paths_with_changes(diff: &TreeDiff, paths: &mut Vec<String>) {
        // If this node has any changes, add its path
        if !diff.changes.is_empty() || !diff.added.is_empty() || !diff.removed.is_empty() {
            paths.push(diff.root_path.clone());
        }
        // Recursively process child diffs
        for child in &diff.child_diffs {
            Self::collect_paths_with_changes(child, paths);
        }
    }
}
