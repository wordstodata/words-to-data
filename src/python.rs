//! Python bindings for words_to_data
//!
//! This module provides a minimal Python interface using JSON serialization.
use pyo3::exceptions::{PyOSError, PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pythonize::pythonize;
use serde_json;

use crate::diff::TreeDiff as RustTreeDiff;
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
    fn data(&self) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.data).unwrap();
        let result = Python::attach(|py| {
            let obj = pythonize(py, &data).unwrap();
            obj.unbind()
        });
        Ok(result)
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
        format!(
            "TextChange(tag='{}', value='{}')",
            self.tag(),
            &self.value()[..self.value().len().min(20)]
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
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
    fn from_element(&self) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.from_element).unwrap();
        let result = Python::attach(|py| {
            let obj = pythonize(py, &data).unwrap();
            obj.unbind()
        });
        Ok(result)
    }

    #[getter]
    fn to_element(&self) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.to_element).unwrap();
        let result = Python::attach(|py| {
            let obj = pythonize(py, &data).unwrap();
            obj.unbind()
        });
        Ok(result)
    }

    #[getter]
    fn added(&self, py: Python) -> PyResult<Vec<Py<PyAny>>> {
        self.inner
            .added
            .iter()
            .map(|elem| {
                let data = serde_json::to_value(elem).unwrap();
                pythonize(py, &data)
                    .map(|obj| obj.unbind())
                    .map_err(|e| PyRuntimeError::new_err(format!("Conversion error: {}", e)))
            })
            .collect()
    }

    #[getter]
    fn removed(&self, py: Python) -> PyResult<Vec<Py<PyAny>>> {
        self.inner
            .removed
            .iter()
            .map(|elem| {
                let data = serde_json::to_value(elem).unwrap();
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

    #[staticmethod]
    fn from_json(json_str: &str) -> PyResult<Self> {
        let inner: RustTreeDiff = serde_json::from_str(json_str)
            .map_err(|e| PyValueError::new_err(format!("JSON deserialization error: {}", e)))?;
        Ok(Self::from(&inner))
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
    fn new(bill_id: &str, causative_text: &str) -> Self {
        BillReference {
            inner: RustBillReference {
                bill_id: bill_id.to_string(),
                causative_text: causative_text.to_string(),
            },
        }
    }

    #[getter]
    fn bill_id(&self) -> String {
        self.inner.bill_id.clone()
    }

    #[getter]
    fn causative_text(&self) -> String {
        self.inner.causative_text.clone()
    }

    fn __repr__(&self) -> String {
        format!("BillReference(bill_id='{}')", self.inner.bill_id)
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
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
impl ChangeAnnotation {
    #[new]
    #[pyo3(signature = (operation, bill_id, causative_text, annotator, confidence=None, notes=None, reasoning=None, related_paths=None))]
    fn new(
        operation: &str,
        bill_id: &str,
        causative_text: &str,
        annotator: &str,
        confidence: Option<f32>,
        notes: Option<String>,
        reasoning: Option<String>,
        related_paths: Option<Vec<String>>,
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
            causative_text: causative_text.to_string(),
        };

        Ok(ChangeAnnotation {
            inner: RustChangeAnnotation {
                operation: action,
                source_bill,
                related_paths: related_paths.unwrap_or_default(),
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
    fn related_paths(&self) -> Vec<String> {
        self.inner.related_paths.clone()
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
    fn annotations_dict(&self) -> PyResult<Py<PyAny>> {
        let data = serde_json::to_value(&self.inner.annotations).unwrap();
        let result = Python::attach(|py| {
            let obj = pythonize(py, &data).unwrap();
            obj.unbind()
        });
        Ok(result)
    }

    /// Add an annotation for a specific structural path
    fn add_annotation(&mut self, path: &str, annotation: &ChangeAnnotation) {
        self.inner
            .add_annotation(path.to_string(), annotation.inner.clone());
    }

    /// Get all annotations for a specific path
    fn get_annotations(&self, path: &str) -> Option<Vec<ChangeAnnotation>> {
        self.inner
            .get_annotations(path)
            .map(|anns| anns.iter().map(ChangeAnnotation::from).collect())
    }

    /// Get the TreeDiff node for a specific path
    fn get_diff_node(&self, path: &str) -> Option<TreeDiff> {
        self.inner.get_diff_node(path).map(TreeDiff::from)
    }

    /// Find all annotations that reference a given path in their related_paths
    fn find_related_annotations(&self, path: &str) -> Vec<(String, ChangeAnnotation)> {
        self.inner
            .find_related_annotations(path)
            .into_iter()
            .map(|(p, ann)| (p.clone(), ChangeAnnotation::from(ann)))
            .collect()
    }

    /// Get all paths that have annotations
    fn annotated_paths(&self) -> Vec<String> {
        self.inner.annotated_paths().cloned().collect()
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

    fn __repr__(&self) -> String {
        let text_preview = if self.inner.amending_text.len() > 50 {
            format!("{}...", &self.inner.amending_text[..50])
        } else {
            self.inner.amending_text.clone()
        };
        format!(
            "BillAmendment(action_types={:?}, amending_text='{}')",
            self.action_types(),
            text_preview
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
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
    amendments: Vec<BillAmendment>,
}

impl AmendmentData {
    fn from(rust_data: crate::uslm::bill_parser::AmendmentData) -> Self {
        let amendments = rust_data
            .amendments
            .iter()
            .map(BillAmendment::from)
            .collect();

        AmendmentData {
            inner: rust_data,
            amendments,
        }
    }
}

#[pymethods]
impl AmendmentData {
    #[getter]
    fn bill_id(&self) -> String {
        self.inner.bill_id.clone()
    }

    #[getter]
    fn amendments(&self) -> Vec<BillAmendment> {
        self.amendments.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "AmendmentData(bill_id='{}', amendments={})",
            self.inner.bill_id,
            self.amendments.len()
        )
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string(&self.inner)
            .map_err(|e| PyRuntimeError::new_err(format!("JSON serialization error: {}", e)))
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
///     path: Path to the Public Law XML file
///
/// Returns:
///     AmendmentData containing the bill ID and all extracted amendments
///
/// Raises:
///     ValueError: If the XML is invalid or not a Public Law document
///     OSError: If the file cannot be read
#[pyfunction]
fn parse_bill_amendments(path: &str) -> PyResult<AmendmentData> {
    let data = crate::uslm::bill_parser::parse_bill_amendments(path).map_err(parse_error_to_py)?;
    Ok(AmendmentData::from(data))
}

/// Python module definition
#[pymodule]
fn words_to_data(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(parse_uslm_xml, m)?)?;
    m.add_function(wrap_pyfunction!(compute_diff, m)?)?;
    m.add_function(wrap_pyfunction!(parse_bill_amendments, m)?)?;
    m.add_class::<USLMElement>()?;
    m.add_class::<TreeDiff>()?;
    m.add_class::<FieldChangeEvent>()?;
    m.add_class::<TextChange>()?;
    m.add_class::<AmendmentData>()?;
    m.add_class::<BillAmendment>()?;
    // legal_diff types
    m.add_class::<LegalDiff>()?;
    m.add_class::<ChangeAnnotation>()?;
    m.add_class::<BillReference>()?;
    m.add_class::<AnnotationMetadata>()?;
    Ok(())
}
