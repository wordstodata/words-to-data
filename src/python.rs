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
}

// ============================================================================
// Bill parsing types
// ============================================================================

/// A reference to a USC section found in a bill
#[pyclass(from_py_object)]
#[derive(Clone)]
struct UscReference {
    inner: crate::uslm::UscReference,
}

impl UscReference {
    fn from(rust_ref: &crate::uslm::UscReference) -> Self {
        UscReference {
            inner: rust_ref.clone(),
        }
    }
}

#[pymethods]
impl UscReference {
    #[getter]
    fn path(&self) -> String {
        self.inner.path.clone()
    }

    #[getter]
    fn display_text(&self) -> String {
        self.inner.display_text.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "UscReference(path='{}', display_text='{}')",
            self.inner.path, self.inner.display_text
        )
    }
}

/// An amendment found in a bill that modifies the US Code
#[pyclass(from_py_object)]
#[derive(Clone)]
struct BillAmendment {
    inner: crate::uslm::BillAmendment,
    target_paths: Vec<UscReference>,
}

impl BillAmendment {
    fn from(rust_amendment: &crate::uslm::BillAmendment) -> Self {
        let target_paths = rust_amendment
            .target_paths
            .iter()
            .map(UscReference::from)
            .collect();

        BillAmendment {
            inner: rust_amendment.clone(),
            target_paths,
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
    fn target_paths(&self) -> Vec<UscReference> {
        self.target_paths.clone()
    }

    #[getter]
    fn source_path(&self) -> String {
        self.inner.source_path.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "BillAmendment(source_path='{}', target_paths={}, action_types={:?})",
            self.inner.source_path,
            self.target_paths.len(),
            self.action_types()
        )
    }
}

/// Data extracted from a bill document
#[pyclass(from_py_object)]
#[derive(Clone)]
struct AmendmentData {
    bill_id: String,
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
            bill_id: rust_data.bill_id,
            amendments,
        }
    }
}

#[pymethods]
impl AmendmentData {
    #[getter]
    fn bill_id(&self) -> String {
        self.bill_id.clone()
    }

    #[getter]
    fn amendments(&self) -> Vec<BillAmendment> {
        self.amendments.clone()
    }

    fn __repr__(&self) -> String {
        format!(
            "AmendmentData(bill_id='{}', amendments={})",
            self.bill_id,
            self.amendments.len()
        )
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
    m.add_class::<UscReference>()?;
    Ok(())
}
