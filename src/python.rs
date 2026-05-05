//! Python bindings for words_to_data
//!
//! This module provides a minimal Python interface using JSON serialization.
use pyo3::exceptions::{PyOSError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pythonize::pythonize;
use serde_json;

use crate::congress::{
    BillDownload as RustBillDownload, Chamber as RustChamber, CongressClient as RustCongressClient,
    CongressError, CosponsorRecord as RustCosponsorRecord, Member as RustMember,
    MemberTerm as RustMemberTerm, Party as RustParty, SponsorInfo as RustSponsorInfo,
};
use crate::dataset::{
    Dataset as RustDataset, DatasetError, DatasetMetadata as RustDatasetMetadata,
    SearchResult as RustSearchResult, VersionSnapshot as RustVersionSnapshot,
};
use crate::diff::{
    AmendmentSimilarity as RustAmendmentSimilarity, MentionMatch as RustMentionMatch,
    TreeDiff as RustTreeDiff,
};
use crate::uslm::parser::ParseError;

#[pyclass(from_py_object)]
#[derive(Clone)]
struct USLMElement {
    pub inner: crate::uslm::USLMElement,

    /// Child elements in document order
    pub children: Vec<USLMElement>,
}

impl USLMElement {
    pub fn from(rust_elem: &crate::uslm::USLMElement) -> Self {
        let children = rust_elem.children.iter().map(Self::from).collect();
        USLMElement {
            inner: rust_elem.clone(),
            children,
        }
    }
}

#[pymethods]
impl USLMElement {
    #[getter]
    fn data(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.data)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[getter]
    fn children(&self) -> PyResult<Vec<USLMElement>> {
        Ok(self.children.clone())
    }

    fn find(&self, path: &str) -> Option<USLMElement> {
        self.inner.find(path).map(USLMElement::from)
    }

    fn __repr__(&self) -> String {
        format!(
            "USLMElement(path='{}', element_type={:?}, children={})",
            self.inner.data.path,
            self.inner.data.element_type,
            self.children.len()
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    fn merge_children(&mut self, other: &mut USLMElement) {
        self.inner.merge_children_mut(&mut other.inner);
        self.children.append(&mut other.children.clone());
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: crate::uslm::USLMElement = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
    }
}

// ============================================================================
// TreeDiff and related types
// ============================================================================

/// A single word-level change within a text field
#[pyclass(from_py_object)]
#[derive(Clone)]
struct TextChange {
    inner: crate::diff::TextChange,
}

#[pymethods]
impl TextChange {
    #[getter]
    fn value(&self) -> String {
        self.inner.value.clone()
    }

    #[getter]
    fn old_index(&self) -> Option<i32> {
        self.inner.old_index
    }

    #[getter]
    fn new_index(&self) -> Option<i32> {
        self.inner.new_index
    }

    #[getter]
    fn tag(&self) -> String {
        format!("{:?}", self.inner.tag).to_lowercase()
    }

    fn __repr__(&self) -> String {
        let value = &self.inner.value;
        format!(
            "TextChange(tag='{}', value='{}')",
            self.tag(),
            &value[..value.len().min(20)]
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: crate::diff::TextChange = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self { inner })
    }
}

/// A change detected in a single text content field
#[pyclass(from_py_object)]
#[derive(Clone)]
struct FieldChangeEvent {
    inner: crate::diff::FieldChangeEvent,
    changes: Vec<TextChange>,
}

#[pymethods]
impl FieldChangeEvent {
    #[getter]
    fn field_name(&self) -> String {
        format!("{:?}", self.inner.field_name).to_lowercase()
    }

    #[getter]
    #[allow(clippy::wrong_self_convention)]
    fn from_date(&self) -> String {
        self.inner.from_date.to_string()
    }

    #[getter]
    fn to_date(&self) -> String {
        self.inner.to_date.to_string()
    }

    #[getter]
    fn old_value(&self) -> String {
        self.inner.old_value.clone()
    }

    #[getter]
    fn new_value(&self) -> String {
        self.inner.new_value.clone()
    }

    #[getter]
    fn changes(&self) -> Vec<TextChange> {
        self.changes.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "FieldChangeEvent(field='{}', changes={})",
            self.field_name(),
            self.changes.len()
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: crate::diff::FieldChangeEvent = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        let changes = inner
            .changes
            .iter()
            .map(|tc| TextChange { inner: tc.clone() })
            .collect();
        Ok(Self { inner, changes })
    }
}

/// A hierarchical diff between two versions of a USLM document tree
#[pyclass(from_py_object)]
#[derive(Clone)]
struct TreeDiff {
    inner: RustTreeDiff,
    changes: Vec<FieldChangeEvent>,
    child_diffs: Vec<TreeDiff>,
}

impl TreeDiff {
    fn from(rust_diff: &RustTreeDiff) -> Self {
        let changes = rust_diff
            .changes
            .iter()
            .map(|change| {
                let text_changes = change
                    .changes
                    .iter()
                    .map(|tc| TextChange { inner: tc.clone() })
                    .collect();
                FieldChangeEvent {
                    inner: change.clone(),
                    changes: text_changes,
                }
            })
            .collect();

        let child_diffs = rust_diff.child_diffs.iter().map(Self::from).collect();

        TreeDiff {
            inner: rust_diff.clone(),
            changes,
            child_diffs,
        }
    }
}

#[pymethods]
impl TreeDiff {
    #[getter]
    fn root_path(&self) -> String {
        self.inner.root_path.clone()
    }

    #[getter]
    fn changes(&self) -> Vec<FieldChangeEvent> {
        self.changes.clone()
    }

    #[getter]
    #[allow(clippy::wrong_self_convention)]
    fn from_element(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.from_element)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[getter]
    fn to_element(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.to_element)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[getter]
    fn added(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.inner
            .added
            .iter()
            .map(|elem| {
                let data = serde_json::to_value(elem).map_err(|e| {
                    PyRuntimeError::new_err(format!("JSON serialization error: {}", e))
                })?;
                pythonize(py, &data)
                    .map(|obj| obj.unbind())
                    .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
            })
            .collect()
    }

    #[getter]
    fn removed(&self, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
        self.inner
            .removed
            .iter()
            .map(|elem| {
                let data = serde_json::to_value(elem).map_err(|e| {
                    PyRuntimeError::new_err(format!("JSON serialization error: {}", e))
                })?;
                pythonize(py, &data)
                    .map(|obj| obj.unbind())
                    .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
            })
            .collect()
    }

    #[getter]
    fn child_diffs(&self) -> Vec<TreeDiff> {
        self.child_diffs.clone()
    }

    fn find(&self, path: &str) -> Option<TreeDiff> {
        self.inner.find(path).map(TreeDiff::from)
    }

    /// Return a shallow copy of this TreeDiff without children
    fn shallow(&self) -> TreeDiff {
        TreeDiff::from(&self.inner.shallow())
    }

    fn __repr__(&self) -> String {
        format!(
            "TreeDiff(path='{}', changes={}, added={}, removed={}, child_diffs={})",
            self.root_path(),
            self.changes.len(),
            self.inner.added.len(),
            self.inner.removed.len(),
            self.child_diffs.len()
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustTreeDiff = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
    }

    /// Calculate similarity between this TreeDiff and amendment data from a bill
    ///
    /// Returns a dictionary mapping TreeDiff paths to their best-matching amendment
    fn calculate_amendment_similarities(
        &self,
        amendment_data: &AmendmentData,
    ) -> PyResult<Vec<AmendmentSimilarity>> {
        let similarities = self
            .inner
            .calculate_amendment_similarities(&amendment_data.inner);
        Ok(similarities
            .into_values()
            .map(|s| AmendmentSimilarity { inner: s })
            .collect())
    }

    /// Scan all amendment texts for mentions of changed sections.
    ///
    /// Uses regexes generated from this TreeDiff to find section mentions
    /// in each amendment's amending_text.
    ///
    /// Returns a dictionary mapping amendment_id to list of MentionMatch objects.
    fn scan_for_mentions(
        &self,
        amendment_data: &AmendmentData,
    ) -> PyResult<std::collections::HashMap<String, Vec<MentionMatch>>> {
        let mentions = self.inner.scan_for_mentions(&amendment_data.inner);
        Ok(mentions
            .into_iter()
            .map(|(k, v)| (k, v.iter().map(MentionMatch::from).collect()))
            .collect())
    }
}

// ============================================================================
// AmendmentSimilarity
// ============================================================================

/// Similarity between a TreeDiff and a bill amendment
#[pyclass(from_py_object)]
#[derive(Clone)]
struct AmendmentSimilarity {
    inner: RustAmendmentSimilarity,
}

#[pymethods]
impl AmendmentSimilarity {
    #[getter]
    fn tree_diff_path(&self) -> String {
        self.inner.tree_diff_path.clone()
    }

    #[getter]
    fn amendment_id(&self) -> String {
        self.inner.amendment_id.clone()
    }

    #[getter]
    fn score(&self) -> f32 {
        self.inner.score
    }

    #[getter]
    fn precision(&self) -> f32 {
        self.inner.precision
    }

    #[getter]
    fn recall(&self) -> f32 {
        self.inner.recall
    }

    #[getter]
    fn matched_words(&self) -> i32 {
        self.inner.matched_words
    }

    #[getter]
    fn tree_diff_words(&self) -> i32 {
        self.inner.tree_diff_words
    }

    fn __repr__(&self) -> String {
        format!(
            "AmendmentSimilarity(path='{}', score={:.3}, precision={:.3}, recall={:.3})",
            self.inner.tree_diff_path, self.inner.score, self.inner.precision, self.inner.recall
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustAmendmentSimilarity = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self { inner })
    }
}

// ============================================================================
// MentionMatch
// ============================================================================

/// A match found when scanning amendment text for section mentions
#[pyclass(from_py_object)]
#[derive(Clone)]
struct MentionMatch {
    inner: RustMentionMatch,
}

impl MentionMatch {
    fn from(rust_match: &RustMentionMatch) -> Self {
        MentionMatch {
            inner: rust_match.clone(),
        }
    }
}

#[pymethods]
impl MentionMatch {
    #[getter]
    fn tree_diff_path(&self) -> String {
        self.inner.tree_diff_path.clone()
    }

    #[getter]
    fn matched_text(&self) -> String {
        self.inner.matched_text.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "MentionMatch(path='{}', matched_text='{}')",
            self.inner.tree_diff_path, self.inner.matched_text
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustMentionMatch = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self { inner })
    }
}

// ============================================================================
// LegalDiff types
// ============================================================================

use crate::legal_diff::{
    AnnotationMetadata as RustAnnotationMetadata, AnnotationStatus as RustAnnotationStatus,
    BillReference as RustBillReference, ChangeAnnotation as RustChangeAnnotation,
    LegalDiff as RustLegalDiff,
};
use crate::uslm::AmendingAction;
use std::str::FromStr;

/// A reference to a bill that caused a change
#[pyclass(from_py_object)]
#[derive(Clone)]
struct BillReference {
    inner: RustBillReference,
}

impl BillReference {
    fn from(rust_ref: &RustBillReference) -> Self {
        BillReference {
            inner: rust_ref.clone(),
        }
    }
}

#[pymethods]
impl BillReference {
    #[new]
    fn new(bill_id: &str, amendment_id: &str, causative_text: &str) -> Self {
        BillReference {
            inner: RustBillReference {
                bill_id: bill_id.to_string(),
                amendment_id: amendment_id.to_string(),
                causative_text: causative_text.to_string(),
            },
        }
    }

    #[getter]
    fn bill_id(&self) -> String {
        self.inner.bill_id.clone()
    }

    #[getter]
    fn amendment_id(&self) -> String {
        self.inner.amendment_id.clone()
    }

    #[getter]
    fn causative_text(&self) -> String {
        self.inner.causative_text.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "BillReference(bill_id='{}', amendment_id='{}')",
            self.inner.bill_id, &self.inner.amendment_id
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustBillReference = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
    }
}

/// Metadata about an annotation
#[pyclass(from_py_object)]
#[derive(Clone)]
struct AnnotationMetadata {
    inner: RustAnnotationMetadata,
}

impl AnnotationMetadata {
    fn from(rust_meta: &RustAnnotationMetadata) -> Self {
        AnnotationMetadata {
            inner: rust_meta.clone(),
        }
    }
}

#[pymethods]
impl AnnotationMetadata {
    #[getter]
    fn status(&self) -> String {
        match self.inner.status {
            RustAnnotationStatus::Pending => "pending".to_string(),
            RustAnnotationStatus::Verified => "verified".to_string(),
            RustAnnotationStatus::Disputed => "disputed".to_string(),
            RustAnnotationStatus::Rejected => "rejected".to_string(),
        }
    }

    #[getter]
    fn confidence(&self) -> Option<f32> {
        self.inner.confidence
    }

    #[getter]
    fn annotator(&self) -> String {
        self.inner.annotator.clone()
    }

    #[getter]
    fn timestamp(&self) -> String {
        self.inner
            .timestamp
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_else(|_| "invalid timestamp".to_string())
    }

    #[getter]
    fn notes(&self) -> Option<String> {
        self.inner.notes.clone()
    }

    #[getter]
    fn reasoning(&self) -> Option<String> {
        self.inner.reasoning.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "AnnotationMetadata(status='{}', annotator='{}')",
            self.status(),
            self.inner.annotator
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustAnnotationMetadata = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
    }
}

/// An annotation linking a change to its legal cause
#[pyclass(from_py_object)]
#[derive(Clone)]
struct ChangeAnnotation {
    inner: RustChangeAnnotation,
}

impl ChangeAnnotation {
    fn from(rust_ann: &RustChangeAnnotation) -> Self {
        ChangeAnnotation {
            inner: rust_ann.clone(),
        }
    }
}

#[pymethods]
#[allow(clippy::too_many_arguments)]
impl ChangeAnnotation {
    #[new]
    #[pyo3(signature = (operation, bill_id, amendment_id, causative_text, annotator, paths, confidence=None, notes=None, reasoning=None))]
    fn new(
        operation: &str,
        bill_id: &str,
        amendment_id: &str,
        causative_text: &str,
        annotator: &str,
        paths: Vec<String>,
        confidence: Option<f32>,
        notes: Option<String>,
        reasoning: Option<String>,
    ) -> PyResult<Self> {
        let action = AmendingAction::from_str(operation)
            .map_err(|e| PyValueError::new_err(format!("Invalid operation: {}", e)))?;

        let metadata = RustAnnotationMetadata {
            status: RustAnnotationStatus::Pending,
            confidence,
            annotator: annotator.to_string(),
            timestamp: time::OffsetDateTime::now_utc(),
            notes,
            reasoning,
        };

        let source_bill = RustBillReference {
            bill_id: bill_id.to_string(),
            amendment_id: amendment_id.to_string(),
            causative_text: causative_text.to_string(),
        };

        Ok(ChangeAnnotation {
            inner: RustChangeAnnotation {
                operation: action,
                source_bill,
                paths,
                metadata,
            },
        })
    }

    #[getter]
    fn operation(&self) -> String {
        format!("{:?}", self.inner.operation).to_lowercase()
    }

    #[getter]
    fn source_bill(&self) -> BillReference {
        BillReference::from(&self.inner.source_bill)
    }

    #[getter]
    fn metadata(&self) -> AnnotationMetadata {
        AnnotationMetadata::from(&self.inner.metadata)
    }

    fn __repr__(&self) -> String {
        format!(
            "ChangeAnnotation(operation='{}', bill_id='{}')",
            self.operation(),
            self.inner.source_bill.bill_id
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustChangeAnnotation = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
    }
}

/// A legal diff combining word-level changes with semantic annotations
#[pyclass(from_py_object)]
#[derive(Clone)]
struct LegalDiff {
    inner: RustLegalDiff,
}

impl LegalDiff {
    fn from(rust_diff: &RustLegalDiff) -> Self {
        LegalDiff {
            inner: rust_diff.clone(),
        }
    }
}

#[pymethods]
impl LegalDiff {
    #[new]
    fn new(tree_diff: &TreeDiff) -> Self {
        let rust_legal_diff = RustLegalDiff::new(&tree_diff.inner);
        LegalDiff {
            inner: rust_legal_diff,
        }
    }

    #[getter]
    fn tree_diff(&self) -> TreeDiff {
        TreeDiff::from(&self.inner.tree_diff)
    }

    #[getter]
    fn annotations_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.annotations)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[getter]
    fn amendments_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.amendments)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    /// Add an annotation for a specific structural path
    fn add_annotation(&mut self, annotation: &ChangeAnnotation) {
        self.inner.add_annotation(annotation.inner.clone());
    }

    /// Get all annotations for a specific path
    fn get_annotations(&self, path: &str) -> Vec<ChangeAnnotation> {
        self.inner
            .get_annotations(path)
            .into_iter()
            .map(|annotation| ChangeAnnotation {
                inner: annotation.clone(),
            })
            .collect()
    }

    /// Get the TreeDiff node for a specific path
    fn get_diff_node(&self, path: &str) -> Option<TreeDiff> {
        self.inner.get_diff_node(path).map(TreeDiff::from)
    }

    /// Get all paths that have annotations
    fn annotated_paths(&self) -> Vec<String> {
        self.inner.annotated_paths().iter().cloned().collect()
    }

    /// Get all paths in the TreeDiff that lack annotations
    fn unannotated_paths(&self) -> Vec<String> {
        self.inner.unannotated_paths()
    }

    fn __repr__(&self) -> String {
        format!(
            "LegalDiff(root_path='{}', annotations={})",
            self.inner.tree_diff.root_path,
            self.inner.annotations.len()
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustLegalDiff = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
    }
}

// ============================================================================
// Bill parsing types
// ============================================================================

/// Word-level changes from a bill amendment instruction
#[pyclass(from_py_object)]
#[derive(Clone)]
struct BillDiff {
    inner: crate::uslm::BillDiff,
}

impl BillDiff {
    fn from(rust_diff: &crate::uslm::BillDiff) -> Self {
        BillDiff {
            inner: rust_diff.clone(),
        }
    }
}

#[pymethods]
impl BillDiff {
    #[new]
    fn new(added: Vec<String>, removed: Vec<String>) -> Self {
        BillDiff {
            inner: crate::uslm::BillDiff { added, removed },
        }
    }

    #[getter]
    fn added(&self) -> Vec<String> {
        self.inner.added.clone()
    }

    #[getter]
    fn removed(&self) -> Vec<String> {
        self.inner.removed.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "BillDiff(added={:?}, removed={:?})",
            self.inner.added, self.inner.removed
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: crate::uslm::BillDiff = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
    }
}

#[pyclass(from_py_object)]
#[derive(Clone)]
struct BillAmendment {
    inner: crate::uslm::BillAmendment,
}

impl BillAmendment {
    fn from(rust_amendment: &crate::uslm::BillAmendment) -> Self {
        BillAmendment {
            inner: rust_amendment.clone(),
        }
    }
}

#[pymethods]
impl BillAmendment {
    #[getter]
    fn id(&self) -> String {
        self.inner.id.clone()
    }

    #[getter]
    fn action_types(&self) -> Vec<String> {
        self.inner
            .action_types
            .iter()
            .map(|action| format!("{:?}", action).to_lowercase())
            .collect()
    }

    #[getter]
    fn amending_text(&self) -> String {
        self.inner.amending_text.clone()
    }

    #[getter]
    fn changes(&self) -> Vec<BillDiff> {
        self.inner.changes.iter().map(BillDiff::from).collect()
    }

    fn __repr__(&self) -> String {
        let text_preview = if self.inner.amending_text.len() > 50 {
            format!("{}...", &self.inner.amending_text[..50])
        } else {
            self.inner.amending_text.clone()
        };
        format!(
            "BillAmendment(id='{}', action_types={:?}, changes={}, amending_text='{}')",
            &self.inner.id[..self.inner.id.len().min(12)],
            self.action_types(),
            self.inner.changes.len(),
            text_preview
        )
    }

    /// Create a new BillAmendment with updated changes
    ///
    /// Returns a new BillAmendment with the same id, action_types, and amending_text,
    /// but with the provided changes.
    fn update_changes(&self, changes: Vec<BillDiff>) -> BillAmendment {
        let rust_changes: Vec<crate::uslm::BillDiff> =
            changes.iter().map(|c| c.inner.clone()).collect();
        BillAmendment {
            inner: self.inner.update_changes(&rust_changes),
        }
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: crate::uslm::BillAmendment = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
    }
}

/// Data extracted from a bill document
#[pyclass(from_py_object)]
#[derive(Clone)]
struct AmendmentData {
    inner: crate::uslm::bill_parser::AmendmentData,
}

impl AmendmentData {
    fn from(rust_data: crate::uslm::bill_parser::AmendmentData) -> Self {
        AmendmentData { inner: rust_data }
    }
}

#[pymethods]
impl AmendmentData {
    #[new]
    fn new(bill_id: String, amendments: Vec<BillAmendment>) -> Self {
        let amendments_map: std::collections::HashMap<String, crate::uslm::BillAmendment> =
            amendments
                .iter()
                .map(|a| (a.inner.id.clone(), a.inner.clone()))
                .collect();

        let inner = crate::uslm::bill_parser::AmendmentData {
            bill_id,
            amendments: amendments_map,
        };

        Self::from(inner)
    }

    #[getter]
    fn bill_id(&self) -> String {
        self.inner.bill_id.clone()
    }

    #[getter]
    fn amendments(&self) -> std::collections::HashMap<String, BillAmendment> {
        self.inner
            .amendments
            .iter()
            .map(|(k, v)| (k.clone(), BillAmendment::from(v)))
            .collect()
    }

    fn get_amendment(&self, amendment_id: &str) -> Option<BillAmendment> {
        self.inner
            .amendments
            .get(amendment_id)
            .map(BillAmendment::from)
    }

    fn __repr__(&self) -> String {
        format!(
            "AmendmentData(bill_id='{}', amendments={})",
            self.inner.bill_id,
            self.inner.amendments.len()
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    fn to_dict(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))?;
        pythonize(py, &data)
            .map(|obj| obj.unbind())
            .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: crate::uslm::bill_parser::AmendmentData = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(inner))
    }
}

fn parse_error_to_py(err: ParseError) -> PyErr {
    match err {
        ParseError::Xml(e) => PyValueError::new_err(format!("XML parsing error: {}", e)),
        ParseError::Io(e) => PyOSError::new_err(format!("File error: {}", e)),
        ParseError::Utf8(e) => PyValueError::new_err(format!("Invalid UTF-8: {}", e)),
        ParseError::USLMDataError(e) => PyValueError::new_err(format!("USLM error: {}", e)),
        ParseError::UnsupportedDocumentType(s) => {
            PyValueError::new_err(format!("Unsupported document type: {}", s))
        }
        ParseError::InvalidDate => PyValueError::new_err("Invalid date format. Use YYYY-MM-DD"),
        ParseError::SerializationError(e) => {
            PyRuntimeError::new_err(format!("JSON serialization error: {}", e))
        }
        ParseError::UnableToParseElement(s) => {
            PyValueError::new_err(format!("Unable to parse element: {}", s))
        }
        ParseError::UnknownElement => PyValueError::new_err("Unknown element type"),
        ParseError::RepealedElement => PyValueError::new_err("Repealed element (not supported)"),
        ParseError::ReservedElement => PyValueError::new_err("Reserved element (not supported)"),
    }
}

#[pyfunction]
fn parse_uslm_xml(path: &str, date: &str) -> PyResult<USLMElement> {
    let element = crate::utils::parse_uslm_xml(path, date).map_err(parse_error_to_py)?;
    Ok(USLMElement::from(&element))
}

/// Compute word-level diff between two USLM documents
///
/// Args:
///     old_element: The original (older) version of the element
///     new_element: The new (newer) version of the element
///
/// Returns:
///     TreeDiff containing all detected changes
///
/// Raises:
///     ValueError: If the two elements don't have the same structural path
#[pyfunction]
fn compute_diff(old_element: &USLMElement, new_element: &USLMElement) -> PyResult<TreeDiff> {
    if old_element.inner.data.path != new_element.inner.data.path {
        return Err(PyValueError::new_err(format!(
            "Document paths don't match: '{}' vs '{}'",
            old_element.inner.data.path, new_element.inner.data.path
        )));
    }

    let diff = RustTreeDiff::from_elements(&old_element.inner, &new_element.inner);
    Ok(TreeDiff::from(&diff))
}

/// Parse a Public Law bill and extract amendments to the US Code
///
/// Args:
///     bill_id: The bill identifier (e.g., "119-21")
///     path: Path to the Public Law XML file
///
/// Returns:
///     AmendmentData containing the bill ID and all extracted amendments
///
/// Raises:
///     ValueError: If the XML is invalid
///     OSError: If the file cannot be read
#[pyfunction]
fn parse_bill_amendments(bill_id: &str, path: &str) -> PyResult<AmendmentData> {
    let data = crate::uslm::bill_parser::parse_bill_amendments(bill_id, path)
        .map_err(parse_error_to_py)?;
    Ok(AmendmentData::from(data))
}

/// Parse a Public Law bill XML string and extract amendments to the US Code
///
/// Args:
///     bill_id: The bill identifier (e.g., "119-21")
///     xml_str: The Public Law XML content as a string
///
/// Returns:
///     AmendmentData containing the bill ID and all extracted amendments
///
/// Raises:
///     ValueError: If the XML is invalid
#[pyfunction]
fn parse_bill_amendments_from_str(bill_id: &str, xml_str: &str) -> PyResult<AmendmentData> {
    let data = crate::uslm::bill_parser::parse_bill_amendments_from_str(bill_id, xml_str)
        .map_err(parse_error_to_py)?;
    Ok(AmendmentData::from(data))
}

/// Load and merge all USLM XML files from a folder into a single element.
///
/// Reads all .xml files from the folder, parses them in parallel, and merges
/// all parsed elements' children into a single root element. Useful for loading
/// a complete US Code title that may be split across multiple XML files.
///
/// Args:
///     path: Path to directory containing USLM XML files
///     date: Publication date in YYYY-MM-DD format
///
/// Returns:
///     Merged USLMElement tree, or None if the folder is empty or unreadable
#[pyfunction]
fn load_uslm_folder(folder_path: &str, date: &str) -> PyResult<Option<USLMElement>> {
    let result = crate::utils::load_uslm_folder(folder_path, date);
    match result {
        None => Ok(None),
        Some(element) => Ok(Some(USLMElement::from(&element))),
    }
}

// ============================================================================
// Dataset types
// ============================================================================

fn dataset_error_to_py(err: DatasetError) -> PyErr {
    match err {
        DatasetError::Io(e) => PyOSError::new_err(format!("IO error: {}", e)),
        DatasetError::Json(e) => PyValueError::new_err(format!("JSON error: {}", e)),
        DatasetError::VersionNotFound(v) => {
            PyValueError::new_err(format!("Version not found: {}", v))
        }
        DatasetError::FolderLoadFailed(p) => PyOSError::new_err(format!(
            "Failed to load folder '{}': empty or unreadable",
            p
        )),
    }
}

/// Metadata for a Dataset
#[pyclass(from_py_object)]
#[derive(Clone)]
struct DatasetMetadata {
    inner: RustDatasetMetadata,
}

#[pymethods]
impl DatasetMetadata {
    #[new]
    fn new(
        name: String,
        description: String,
        author: String,
        source_urls: Vec<String>,
        license: String,
        version: String,
    ) -> Self {
        DatasetMetadata {
            inner: RustDatasetMetadata {
                name,
                description,
                author,
                source_urls,
                license,
                version,
            },
        }
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[getter]
    fn description(&self) -> String {
        self.inner.description.clone()
    }

    #[getter]
    fn author(&self) -> String {
        self.inner.author.clone()
    }

    #[getter]
    fn source_urls(&self) -> Vec<String> {
        self.inner.source_urls.clone()
    }

    #[getter]
    fn license(&self) -> String {
        self.inner.license.clone()
    }

    #[getter]
    fn version(&self) -> String {
        self.inner.version.clone()
    }

    fn __repr__(&self) -> String {
        format!("DatasetMetadata(name='{}')", self.inner.name)
    }
}

/// A version snapshot in a Dataset
#[pyclass(from_py_object)]
#[derive(Clone)]
struct VersionSnapshot {
    inner: RustVersionSnapshot,
}

impl VersionSnapshot {
    fn from(rust: &RustVersionSnapshot) -> Self {
        VersionSnapshot {
            inner: rust.clone(),
        }
    }
}

#[pymethods]
impl VersionSnapshot {
    #[new]
    fn new(date: String, element: &USLMElement, label: Option<String>) -> Self {
        VersionSnapshot {
            inner: RustVersionSnapshot {
                date,
                label,
                element: element.inner.clone(),
            },
        }
    }

    #[getter]
    fn date(&self) -> String {
        self.inner.date.clone()
    }

    #[getter]
    fn label(&self) -> Option<String> {
        self.inner.label.clone()
    }

    #[getter]
    fn element(&self) -> USLMElement {
        USLMElement::from(&self.inner.element)
    }

    fn __repr__(&self) -> String {
        format!(
            "VersionSnapshot(date='{}', label={:?})",
            self.inner.date, self.inner.label
        )
    }
}

/// A search result from Dataset.search_text
#[pyclass(from_py_object)]
#[derive(Clone)]
struct SearchResult {
    inner: RustSearchResult,
}

impl SearchResult {
    fn from(rust: &RustSearchResult) -> Self {
        SearchResult {
            inner: rust.clone(),
        }
    }
}

#[pymethods]
impl SearchResult {
    #[getter]
    fn date(&self) -> String {
        self.inner.date.clone()
    }

    #[getter]
    fn path(&self) -> String {
        self.inner.path.clone()
    }

    #[getter]
    fn field(&self) -> String {
        self.inner.field.clone()
    }

    #[getter]
    fn snippet(&self) -> String {
        self.inner.snippet.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "SearchResult(date='{}', path='{}', field='{}')",
            self.inner.date, self.inner.path, self.inner.field
        )
    }
}

/// A versioned collection of legal documents with bill annotations
#[pyclass(from_py_object)]
#[derive(Clone)]
struct Dataset {
    inner: RustDataset,
}

#[pymethods]
impl Dataset {
    #[new]
    fn new(metadata: &DatasetMetadata) -> Self {
        Dataset {
            inner: RustDataset::new(metadata.inner.clone()),
        }
    }

    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        let inner = RustDataset::load(path).map_err(dataset_error_to_py)?;
        Ok(Dataset { inner })
    }

    fn save(&self, path: &str) -> PyResult<()> {
        self.inner.save(path).map_err(dataset_error_to_py)
    }

    #[getter]
    fn metadata(&self) -> DatasetMetadata {
        DatasetMetadata {
            inner: self.inner.metadata.clone(),
        }
    }

    #[getter]
    fn versions(&self) -> Vec<VersionSnapshot> {
        self.inner
            .versions
            .iter()
            .map(VersionSnapshot::from)
            .collect()
    }

    #[getter]
    fn bills(&self) -> Vec<AmendmentData> {
        self.inner
            .bills
            .iter()
            .map(|b| AmendmentData::from(b.clone()))
            .collect()
    }

    fn get_annotations(&self, from_date: &str, to_date: &str) -> Option<Vec<ChangeAnnotation>> {
        self.inner
            .get_annotations(from_date, to_date)
            .map(|annotations| annotations.iter().map(ChangeAnnotation::from).collect())
    }

    fn add_annotation(&mut self, from_date: &str, to_date: &str, annotation: &ChangeAnnotation) {
        self.inner
            .add_annotation(from_date, to_date, annotation.inner.clone());
    }

    fn annotated_paths(&self, from_date: &str, to_date: &str) -> Vec<String> {
        self.inner.annotated_paths(from_date, to_date)
    }

    fn unannotated_paths(&self, from_date: &str, to_date: &str) -> PyResult<Vec<String>> {
        self.inner
            .unannotated_paths(from_date, to_date)
            .map_err(dataset_error_to_py)
    }

    fn add_version(&mut self, snapshot: &VersionSnapshot) {
        self.inner.add_version(snapshot.inner.clone());
    }

    fn get_version(&self, date: &str) -> Option<VersionSnapshot> {
        self.inner.get_version(date).map(VersionSnapshot::from)
    }

    fn get_version_by_label(&self, label: &str) -> Option<VersionSnapshot> {
        self.inner
            .get_version_by_label(label)
            .map(VersionSnapshot::from)
    }

    fn next_version(&self, date: &str) -> Option<VersionSnapshot> {
        self.inner.next_version(date).map(VersionSnapshot::from)
    }

    fn prev_version(&self, date: &str) -> Option<VersionSnapshot> {
        self.inner.prev_version(date).map(VersionSnapshot::from)
    }

    fn compute_diff(&self, from_date: &str, to_date: &str) -> PyResult<TreeDiff> {
        let diff = self
            .inner
            .compute_diff(from_date, to_date)
            .map_err(dataset_error_to_py)?;
        Ok(TreeDiff::from(&diff))
    }

    fn add_bill(&mut self, bill: &AmendmentData) {
        self.inner.add_bill(bill.inner.clone());
    }

    fn get_bill(&self, bill_id: &str) -> Option<AmendmentData> {
        self.inner
            .get_bill(bill_id)
            .map(|b| AmendmentData::from(b.clone()))
    }

    fn annotations_for_path(&self, path: &str) -> Vec<ChangeAnnotation> {
        self.inner
            .annotations_for_path(path)
            .into_iter()
            .map(ChangeAnnotation::from)
            .collect()
    }

    fn annotations_for_bill(&self, bill_id: &str) -> Vec<ChangeAnnotation> {
        self.inner
            .annotations_for_bill(bill_id)
            .into_iter()
            .map(ChangeAnnotation::from)
            .collect()
    }

    fn search_text(&self, query: &str) -> Vec<SearchResult> {
        self.inner
            .search_text(query)
            .iter()
            .map(SearchResult::from)
            .collect()
    }

    fn find_element(&self, path: &str) -> Vec<(String, USLMElement)> {
        self.inner
            .find_element(path)
            .into_iter()
            .map(|(date, elem)| (date.to_string(), USLMElement::from(elem)))
            .collect()
    }

    /// Parse a USLM XML file and add it as a version snapshot
    #[pyo3(signature = (xml_path, date, label=None))]
    fn add_uslm_xml(&mut self, xml_path: &str, date: &str, label: Option<String>) -> PyResult<()> {
        self.inner
            .add_uslm_xml(xml_path, date, label)
            .map_err(parse_error_to_py)
    }

    /// Load all USLM XML files from a folder and add as a merged version snapshot
    #[pyo3(signature = (folder_path, date, label=None))]
    fn add_uslm_folder(
        &mut self,
        folder_path: &str,
        date: &str,
        label: Option<String>,
    ) -> PyResult<()> {
        self.inner
            .add_uslm_folder(folder_path, date, label)
            .map_err(dataset_error_to_py)
    }

    fn __repr__(&self) -> String {
        format!(
            "Dataset(name='{}', versions={}, bills={})",
            self.inner.metadata.name,
            self.inner.versions.len(),
            self.inner.bills.len()
        )
    }

    pub fn add_changes_to_amendment(&mut self, amendment_id: &str, bill_diff: &BillDiff) {
        self.inner
            .add_changes_to_amendment(amendment_id, &bill_diff.inner.clone());
    }

    // Congress data methods

    fn add_member(&mut self, member: &Member) {
        self.inner.add_member(member.inner.clone());
    }

    fn get_member(&self, bioguide_id: &str) -> Option<Member> {
        self.inner
            .get_member(bioguide_id)
            .map(|m| Member { inner: m.clone() })
    }

    fn add_sponsor_info(&mut self, info: &SponsorInfo) {
        self.inner.add_sponsor_info(info.inner.clone());
    }

    fn get_sponsor_info(&self, bill_id: &str) -> Option<SponsorInfo> {
        self.inner
            .get_sponsor_info(bill_id)
            .map(|s| SponsorInfo { inner: s.clone() })
    }

    #[getter]
    fn members(&self) -> Vec<Member> {
        self.inner
            .members
            .values()
            .map(|m| Member { inner: m.clone() })
            .collect()
    }

    #[getter]
    fn sponsors(&self) -> Vec<SponsorInfo> {
        self.inner
            .sponsors
            .values()
            .map(|s| SponsorInfo { inner: s.clone() })
            .collect()
    }

    fn load_bill_download(&mut self, download: &BillDownload) -> PyResult<String> {
        self.inner
            .load_bill_download(&download.inner)
            .map_err(dataset_error_to_py)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
    }

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustDataset = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Dataset { inner })
    }
}

// ============================================================================
// Congress types
// ============================================================================

#[pyclass]
struct Party {
    inner: RustParty,
}

#[pymethods]
impl Party {
    #[staticmethod]
    fn democrat() -> Self {
        Party {
            inner: RustParty::Democrat,
        }
    }

    #[staticmethod]
    fn republican() -> Self {
        Party {
            inner: RustParty::Republican,
        }
    }

    #[staticmethod]
    fn independent() -> Self {
        Party {
            inner: RustParty::Independent,
        }
    }

    #[staticmethod]
    fn other(name: String) -> Self {
        Party {
            inner: RustParty::Other(name),
        }
    }

    fn is_democrat(&self) -> bool {
        matches!(self.inner, RustParty::Democrat)
    }

    fn is_republican(&self) -> bool {
        matches!(self.inner, RustParty::Republican)
    }

    fn is_independent(&self) -> bool {
        matches!(self.inner, RustParty::Independent)
    }

    fn name(&self) -> String {
        match &self.inner {
            RustParty::Democrat => "Democrat".to_string(),
            RustParty::Republican => "Republican".to_string(),
            RustParty::Independent => "Independent".to_string(),
            RustParty::Other(s) => s.clone(),
        }
    }

    fn __repr__(&self) -> String {
        format!("Party({})", self.name())
    }
}

#[pyclass]
struct Chamber {
    inner: RustChamber,
}

#[pymethods]
impl Chamber {
    #[staticmethod]
    fn senate() -> Self {
        Chamber {
            inner: RustChamber::Senate,
        }
    }

    #[staticmethod]
    fn house() -> Self {
        Chamber {
            inner: RustChamber::House,
        }
    }

    fn is_senate(&self) -> bool {
        matches!(self.inner, RustChamber::Senate)
    }

    fn is_house(&self) -> bool {
        matches!(self.inner, RustChamber::House)
    }

    fn __repr__(&self) -> String {
        match self.inner {
            RustChamber::Senate => "Chamber(Senate)".to_string(),
            RustChamber::House => "Chamber(House)".to_string(),
        }
    }
}

#[pyclass]
struct MemberTerm {
    inner: RustMemberTerm,
}

#[pymethods]
impl MemberTerm {
    #[getter]
    fn congress(&self) -> u16 {
        self.inner.congress
    }

    #[getter]
    fn chamber(&self) -> Chamber {
        Chamber {
            inner: self.inner.chamber,
        }
    }

    #[getter]
    fn state(&self) -> String {
        self.inner.state.clone()
    }

    #[getter]
    fn district(&self) -> Option<u8> {
        self.inner.district
    }

    #[getter]
    fn start_year(&self) -> u16 {
        self.inner.start_year
    }

    #[getter]
    fn end_year(&self) -> Option<u16> {
        self.inner.end_year
    }
}

#[pyclass]
struct Member {
    inner: RustMember,
}

#[pymethods]
impl Member {
    #[new]
    #[pyo3(signature = (bioguide_id, name, first_name, last_name, party, state, district, chamber))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        bioguide_id: String,
        name: String,
        first_name: String,
        last_name: String,
        party: &Party,
        state: String,
        district: Option<u8>,
        chamber: &Chamber,
    ) -> Self {
        Member {
            inner: RustMember {
                bioguide_id,
                name,
                first_name,
                last_name,
                party: party.inner.clone(),
                state,
                district,
                chamber: chamber.inner,
                terms: vec![],
            },
        }
    }

    #[getter]
    fn bioguide_id(&self) -> String {
        self.inner.bioguide_id.clone()
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[getter]
    fn first_name(&self) -> String {
        self.inner.first_name.clone()
    }

    #[getter]
    fn last_name(&self) -> String {
        self.inner.last_name.clone()
    }

    #[getter]
    fn party(&self) -> Party {
        Party {
            inner: self.inner.party.clone(),
        }
    }

    #[getter]
    fn state(&self) -> String {
        self.inner.state.clone()
    }

    #[getter]
    fn district(&self) -> Option<u8> {
        self.inner.district
    }

    #[getter]
    fn chamber(&self) -> Chamber {
        Chamber {
            inner: self.inner.chamber,
        }
    }

    #[getter]
    fn terms(&self) -> Vec<MemberTerm> {
        self.inner
            .terms
            .iter()
            .map(|t| MemberTerm { inner: t.clone() })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "Member(bioguide_id='{}', name='{}')",
            self.inner.bioguide_id, self.inner.name
        )
    }
}

#[pyclass]
struct CosponsorRecord {
    inner: RustCosponsorRecord,
}

#[pymethods]
impl CosponsorRecord {
    #[new]
    fn new(bioguide_id: String, date: String, withdrawn: bool) -> Self {
        CosponsorRecord {
            inner: RustCosponsorRecord {
                bioguide_id,
                date,
                withdrawn,
            },
        }
    }

    #[getter]
    fn bioguide_id(&self) -> String {
        self.inner.bioguide_id.clone()
    }

    #[getter]
    fn date(&self) -> String {
        self.inner.date.clone()
    }

    #[getter]
    fn withdrawn(&self) -> bool {
        self.inner.withdrawn
    }

    fn __repr__(&self) -> String {
        format!("CosponsorRecord(bioguide_id='{}')", self.inner.bioguide_id)
    }
}

#[pyclass]
struct SponsorInfo {
    inner: RustSponsorInfo,
}

#[pymethods]
impl SponsorInfo {
    #[getter]
    fn bill_id(&self) -> String {
        self.inner.bill_id.clone()
    }

    #[getter]
    fn sponsor(&self) -> String {
        self.inner.sponsor.clone()
    }

    #[getter]
    fn cosponsors(&self) -> Vec<CosponsorRecord> {
        self.inner
            .cosponsors
            .iter()
            .map(|c| CosponsorRecord { inner: c.clone() })
            .collect()
    }

    fn __repr__(&self) -> String {
        format!(
            "SponsorInfo(bill_id='{}', sponsor='{}')",
            self.inner.bill_id, self.inner.sponsor
        )
    }
}

#[pyclass]
struct BillDownload {
    inner: RustBillDownload,
}

#[pymethods]
impl BillDownload {
    #[new]
    #[pyo3(signature = (bill_id, bill_xml, sponsors_json, cosponsors_json, votes_json, member_jsons))]
    fn new(
        bill_id: String,
        bill_xml: String,
        sponsors_json: String,
        cosponsors_json: String,
        votes_json: Option<String>,
        member_jsons: std::collections::HashMap<String, String>,
    ) -> Self {
        BillDownload {
            inner: RustBillDownload {
                bill_id,
                bill_xml,
                sponsors_json,
                cosponsors_json,
                votes_json,
                member_jsons,
            },
        }
    }

    #[getter]
    fn bill_id(&self) -> String {
        self.inner.bill_id.clone()
    }

    #[getter]
    fn bill_xml(&self) -> String {
        self.inner.bill_xml.clone()
    }

    #[getter]
    fn sponsors_json(&self) -> String {
        self.inner.sponsors_json.clone()
    }

    #[getter]
    fn cosponsors_json(&self) -> String {
        self.inner.cosponsors_json.clone()
    }

    #[getter]
    fn votes_json(&self) -> Option<String> {
        self.inner.votes_json.clone()
    }

    #[getter]
    fn member_jsons(&self) -> std::collections::HashMap<String, String> {
        self.inner.member_jsons.clone()
    }

    fn __repr__(&self) -> String {
        format!("BillDownload(bill_id='{}')", self.inner.bill_id)
    }
}

fn congress_error_to_py(err: CongressError) -> PyErr {
    match err {
        CongressError::Http(msg) => PyRuntimeError::new_err(format!("HTTP error: {}", msg)),
        CongressError::Parse(msg) => PyValueError::new_err(format!("Parse error: {}", msg)),
        CongressError::NotFound(what) => PyValueError::new_err(format!("Not found: {}", what)),
        CongressError::RateLimited => PyRuntimeError::new_err("Rate limited by Congress.gov API"),
        CongressError::InvalidApiKey => PyValueError::new_err("Invalid API key"),
        CongressError::Io(e) => PyOSError::new_err(format!("IO error: {}", e)),
        CongressError::Json(e) => PyValueError::new_err(format!("JSON error: {}", e)),
    }
}

#[pyclass]
struct CongressClient {
    inner: RustCongressClient,
}

#[pymethods]
impl CongressClient {
    #[new]
    fn new(api_key: String) -> Self {
        CongressClient {
            inner: RustCongressClient::new(api_key),
        }
    }

    #[getter]
    fn api_key(&self) -> String {
        self.inner.api_key().to_string()
    }

    fn download_bill(&self, bill_id: &str) -> PyResult<BillDownload> {
        let download = self
            .inner
            .download_bill(bill_id)
            .map_err(congress_error_to_py)?;
        Ok(BillDownload { inner: download })
    }

    fn __repr__(&self) -> String {
        "CongressClient(...)".to_string()
    }
}

/// Python module definition
#[pymodule]
fn words_to_data(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_uslm_xml, m)?)?;
    m.add_function(wrap_pyfunction!(compute_diff, m)?)?;
    m.add_function(wrap_pyfunction!(parse_bill_amendments, m)?)?;
    m.add_function(wrap_pyfunction!(parse_bill_amendments_from_str, m)?)?;
    m.add_function(wrap_pyfunction!(load_uslm_folder, m)?)?;
    m.add_class::<USLMElement>()?;
    m.add_class::<TreeDiff>()?;
    m.add_class::<FieldChangeEvent>()?;
    m.add_class::<TextChange>()?;
    m.add_class::<AmendmentData>()?;
    m.add_class::<BillAmendment>()?;
    m.add_class::<BillDiff>()?;
    m.add_class::<AmendmentSimilarity>()?;
    m.add_class::<MentionMatch>()?;
    // legal_diff types
    m.add_class::<LegalDiff>()?;
    m.add_class::<ChangeAnnotation>()?;
    m.add_class::<BillReference>()?;
    m.add_class::<AnnotationMetadata>()?;
    // dataset types
    m.add_class::<Dataset>()?;
    m.add_class::<DatasetMetadata>()?;
    m.add_class::<VersionSnapshot>()?;
    m.add_class::<SearchResult>()?;
    // congress types
    m.add_class::<Party>()?;
    m.add_class::<Chamber>()?;
    m.add_class::<MemberTerm>()?;
    m.add_class::<Member>()?;
    m.add_class::<CosponsorRecord>()?;
    m.add_class::<SponsorInfo>()?;
    m.add_class::<BillDownload>()?;
    m.add_class::<CongressClient>()?;
    Ok(())
}
