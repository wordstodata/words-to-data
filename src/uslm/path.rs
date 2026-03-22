//! Path generation utilities for USLM elements
//!
//! This module contains pure functions for generating structural paths
//! and determining USLM path inclusion for elements.

use super::ElementType;

/// Determines if an element type should be included in the USLM path
///
/// Returns true for elements that are part of the official USLM identifier scheme,
/// false for structural-only elements like Level and Unknown.
///
/// # Arguments
///
/// * `element_type` - The type of element to check
///
/// # Returns
///
/// `true` if the element should be included in USLM paths, `false` otherwise.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::path::should_include_in_uslm_path;
/// use words_to_data::uslm::ElementType;
///
/// assert!(should_include_in_uslm_path(ElementType::Section));
/// assert!(!should_include_in_uslm_path(ElementType::Level));
/// assert!(!should_include_in_uslm_path(ElementType::Unknown));
/// ```
pub fn should_include_in_uslm_path(element_type: ElementType) -> bool {
    !matches!(element_type, ElementType::Level | ElementType::Unknown)
}

/// Generate a full structural path for an element
///
/// Creates a hierarchical path string that includes all elements in the document
/// structure, including structural-only elements like Level that are not part of
/// the USLM identifier scheme.
///
/// # Format
///
/// Paths use the format `elementtype_value` with `/` separators:
/// - For root elements: `"uscodedocument_26"`
/// - For nested elements: `"uscodedocument_26/title_26/chapter_1/section_174/level_a/subsection_1"`
///
/// # Arguments
///
/// * `element_type` - The type of element (Section, Paragraph, etc.)
/// * `number_value` - The identifier/number for this element (e.g., "174", "a", "1")
/// * `parent_structural_path` - The structural path of the parent element, or None for root
///
/// # Returns
///
/// A string containing the full structural path for this element.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::path::generate_structural_path;
/// use words_to_data::uslm::ElementType;
///
/// // Root element
/// let root = generate_structural_path(ElementType::USCodeDocument, "26", None);
/// assert_eq!(root, "uscodedocument_26");
///
/// // Child element
/// let section = generate_structural_path(ElementType::Section, "174", Some("uscodedocument_26/title_26"));
/// assert_eq!(section, "uscodedocument_26/title_26/section_174");
/// ```
pub fn generate_structural_path(
    element_type: ElementType,
    number_value: &str,
    parent_structural_path: Option<&str>,
) -> String {
    let element_name = format!("{:?}", element_type).to_lowercase();
    match parent_structural_path {
        Some(parent) => format!("{}/{}_{}", parent, element_name, number_value),
        None => format!("{}_{}", element_name, number_value),
    }
}
