use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use time::Date;

use crate::uslm::{ElementData, TextContentField, USLMElement};

/// A change detected in a single text content field between two document versions
///
/// This struct captures the complete details of a change to one of the five text
/// content fields (Heading, Chapeau, Proviso, Content, or Continuation) in a
/// legislative element.
///
/// The changes are computed at word-level granularity using a diff algorithm,
/// allowing precise identification of which words were inserted, deleted, or
/// remained unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FieldChangeEvent {
    /// Which text content field changed
    pub field_name: TextContentField,

    /// The publication date of the original version
    pub from_date: Date,

    /// The publication date of the new version
    pub to_date: Date,

    /// The complete original text of the field
    pub old_value: String,

    /// The complete new text of the field
    pub new_value: String,

    /// Word-level changes showing insertions, deletions, and unchanged portions
    pub changes: Vec<TextChange>,
}

/// A single word-level change within a text field
///
/// Represents one unit of change in a diff, typically a word or whitespace token.
/// Each change has a type (Insert, Delete, or Equal) and position indices in
/// the old and new text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TextChange {
    /// The text value of this change (a word or whitespace)
    pub value: String,

    /// Position in the original text (None for insertions)
    pub old_index: Option<i32>,

    /// Position in the new text (None for deletions)
    pub new_index: Option<i32>,

    /// The type of change
    pub tag: TextChangeType,
}

/// The type of change for a text fragment
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextChangeType {
    /// This text was added in the new version
    Insert,
    /// This text was removed from the old version
    Delete,
    /// This text is unchanged between versions
    Equal,
}

/// A hierarchical diff between two versions of a USLM document tree
///
/// This struct captures all changes between two versions of the same legislative
/// element and its children. It mirrors the tree structure of `USLMElement`,
/// with diffs computed recursively for all matching children.
///
/// # Structure
///
/// The diff includes:
/// - **Field changes**: Text modifications to the element's own content fields
/// - **Added elements**: New child elements in the new version
/// - **Removed elements**: Child elements that existed in the old version but not the new
/// - **Child diffs**: Recursive diffs for child elements that exist in both versions
///
/// # Examples
///
/// ```
/// use words_to_data::{diff::TreeDiff, uslm::parser::parse};
///
/// // Parse two versions of a document
/// let old_doc = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
/// let new_doc = parse("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30").unwrap();
///
/// // Compute the diff
/// let diff = TreeDiff::from_elements(&old_doc, &new_doc);
///
/// // Examine changes
/// println!("Field changes: {}", diff.changes.len());
/// println!("Elements added: {}", diff.added.len());
/// println!("Elements removed: {}", diff.removed.len());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TreeDiff {
    /// The structural path of the element being compared
    pub root_path: String,

    /// Text content field changes for this element
    pub changes: Vec<FieldChangeEvent>,

    /// Metadata from the original version of this element
    pub from_element: ElementData,

    /// Metadata from the new version of this element
    pub to_element: ElementData,

    /// Child elements that were added in the new version
    pub added: Vec<ElementData>,

    /// Child elements that were removed from the old version
    pub removed: Vec<ElementData>,

    /// Recursive diffs for child elements present in both versions
    pub child_diffs: Vec<TreeDiff>,
}

impl TreeDiff {
    /// Compute the diff between two USLM element trees
    ///
    /// Compares two versions of the same legislative element and computes all
    /// changes at both the current level and recursively through all children.
    ///
    /// # Arguments
    ///
    /// * `from_element` - The original (older) version of the element
    /// * `to_element` - The new (newer) version of the element
    ///
    /// # Panics
    ///
    /// Panics if the two elements don't have the same structural path, as they
    /// must represent the same logical element in different versions.
    ///
    /// # Returns
    ///
    /// A `TreeDiff` containing all detected changes between the two versions.
    ///
    /// # Examples
    ///
    /// ```
    /// # use words_to_data::{diff::TreeDiff, uslm::parser::parse};
    /// let old = parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").unwrap();
    /// let new = parse("tests/test_data/usc/2025-07-30/usc07.xml", "2025-07-30").unwrap();
    ///
    /// let diff = TreeDiff::from_elements(&old, &new);
    /// assert_eq!(diff.root_path, old.data.path);
    /// ```
    pub fn from_elements(from_element: &USLMElement, to_element: &USLMElement) -> TreeDiff {
        assert!(from_element.data.path == to_element.data.path);
        let root_path = from_element.data.path.clone();
        // 1. Diff the root element's fields
        let changes = diff_elements(from_element, to_element);

        // 2. Build HashMaps of children by path
        let children_a: HashMap<String, &USLMElement> = from_element
            .children
            .iter()
            .map(|child| (child.data.path.clone(), child))
            .collect();
        let children_b: HashMap<String, &USLMElement> = to_element
            .children
            .iter()
            .map(|child| (child.data.path.clone(), child))
            .collect();

        // 3. Find added, removed, matched
        let mut added = vec![];
        let mut removed = vec![];
        let mut child_diffs = vec![];
        // Iterate once through A - handle matched and removed
        for (path, child_a) in &children_a {
            match children_b.get(path) {
                Some(child_b) => {
                    // Matched - recurse
                    let child_diff = TreeDiff::from_elements(child_a, child_b);
                    if !child_diff.child_diffs.is_empty() || !child_diff.changes.is_empty() {
                        child_diffs.push(child_diff);
                    }
                }
                None => {
                    // Removed
                    removed.push(child_a.data.clone()); //ElementSnapshot::from(child_a));
                }
            }
        }

        // Iterate through B for added only
        for (path, child_b) in &children_b {
            if !children_a.contains_key(path) {
                added.push(child_b.data.clone()); //ElementSnapshot::from(child_b));
            }
        }

        TreeDiff {
            changes,
            root_path,
            from_element: from_element.data.clone(),
            to_element: to_element.data.clone(),
            added,
            removed,
            child_diffs,
        }
    }
}

/// Compute field-level changes between two elements
///
/// Compares all five text content fields (Heading, Chapeau, Proviso, Content,
/// Continuation) between two versions of the same element and returns change
/// events for any fields that differ.
///
/// # Arguments
///
/// * `element_a` - The original version of the element
/// * `element_b` - The new version of the element
///
/// # Returns
///
/// A vector of `FieldChangeEvent` for each field that has changes.
/// Fields that are identical in both versions are omitted.
///
/// # Panics
///
/// Panics if the elements have different paths or types.
pub fn diff_elements(element_a: &USLMElement, element_b: &USLMElement) -> Vec<FieldChangeEvent> {
    assert!(element_a.data.path == element_b.data.path);
    assert!(element_a.data.element_type == element_b.data.element_type);
    let mut changes: Vec<FieldChangeEvent> = Vec::new();
    for field_name in [
        TextContentField::Heading,
        TextContentField::Chapeau,
        TextContentField::Proviso,
        TextContentField::Content,
        TextContentField::Continuation,
    ]
    .into_iter()
    {
        let field_changes = diff_field(element_a, element_b, field_name);
        if !field_changes.changes.is_empty() {
            changes.push(field_changes);
        }
    }
    changes
}

// databases don't like usizes, make it an i32
// text content will never exceed the i32 range
fn rewrap_usize(s: Option<usize>) -> Option<i32> {
    s.map(|val| val as i32)
}

fn diff_field(
    element_a: &USLMElement,
    element_b: &USLMElement,
    field_name: TextContentField,
) -> FieldChangeEvent {
    let a = element_a
        .data
        .get_text_content(field_name)
        .unwrap_or_default();
    let b = element_b
        .data
        .get_text_content(field_name)
        .unwrap_or_default();

    let diff = TextDiff::from_words(a.as_str(), b.as_str());
    let changes: Vec<TextChange> = diff
        .iter_all_changes()
        // Remove non-changes
        .filter(|c| c.tag() != ChangeTag::Equal)
        // Case to our own diff stucture
        .map(|c| {
            let tag = match c.tag() {
                ChangeTag::Delete => TextChangeType::Delete,
                ChangeTag::Insert => TextChangeType::Insert,
                ChangeTag::Equal => TextChangeType::Equal,
            };
            TextChange {
                value: String::from(c.value()),
                old_index: rewrap_usize(c.old_index()),
                new_index: rewrap_usize(c.new_index()),
                tag,
            }
        })
        .collect();
    FieldChangeEvent {
        field_name,
        from_date: element_a.data.date,
        to_date: element_b.data.date,
        old_value: a,
        new_value: b,
        changes,
    }
}
