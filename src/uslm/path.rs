//! Path utilities for USLM elements
//!
//! This module contains pure functions for generating structural paths,
//! comparing them, and determining USLM path inclusion for elements.

use super::ElementType;

/// Whether `prefix` names `path` or an ancestor of it.
///
/// Compares whole segments, so a shorter number is not an ancestor of a longer
/// one: `section_16` does not cover `section_163`. Every caller that takes a
/// path and means "this path and everything beneath it" asks here, because a
/// raw string prefix answers that question wrongly.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::path::covers_path;
///
/// // The path itself, and anything beneath it.
/// assert!(covers_path("uscode/title_26/section_163", "uscode/title_26/section_163"));
/// assert!(covers_path(
///     "uscode/title_26/section_163",
///     "uscode/title_26/section_163/subsection_j/paragraph_8"
/// ));
///
/// // A whole segment, not a string prefix.
/// assert!(!covers_path("uscode/title_26/section_16", "uscode/title_26/section_163"));
/// assert!(!covers_path("uscode/title_2", "uscode/title_26"));
/// ```
pub fn covers_path(prefix: &str, path: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// A path segment made from a container's heading.
///
/// Some containers carry no number and no `identifier`, and the only name the
/// publisher gives them is their heading. The U.S. Code prints them that way
/// too, so the heading is what a person would type (#115).
///
/// Keeps ASCII letters and digits, lowercases them, and writes every other run
/// of characters as a single `-`, so that nothing outside `[a-z0-9-]` can reach
/// a path. Returns `None` where the heading holds no letter or digit at all,
/// because an empty segment would name the container's parent instead.
///
/// # Examples
///
/// ```
/// use words_to_data::uslm::path::path_segment_from_heading;
///
/// assert_eq!(
///     path_segment_from_heading("FEDERAL RULES OF BANKRUPTCY PROCEDURE").as_deref(),
///     Some("federal-rules-of-bankruptcy-procedure")
/// );
///
/// // Punctuation and repeated separators collapse to one dash.
/// assert_eq!(
///     path_segment_from_heading("Insolvency, Receivership, and Liquidation").as_deref(),
///     Some("insolvency-receivership-and-liquidation")
/// );
///
/// // A heading with nothing to name by.
/// assert_eq!(path_segment_from_heading("  —  "), None);
/// ```
pub fn path_segment_from_heading(heading: &str) -> Option<String> {
    let mut segment = String::new();
    for character in heading.chars() {
        if character.is_ascii_alphanumeric() {
            segment.push(character.to_ascii_lowercase());
        } else if !segment.ends_with('-') {
            segment.push('-');
        }
    }
    let segment = segment.trim_matches('-');
    match segment.is_empty() {
        true => None,
        false => Some(segment.to_string()),
    }
}

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
/// - For root elements: `"uscode"`
/// - For nested elements: `"uscode/title_26/chapter_1/section_174/level_a/subsection_1"`
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
/// // Child element
/// let section = generate_structural_path(ElementType::Section, "174", Some("uscode/title_26"));
/// assert_eq!(section, "uscode/title_26/section_174");
/// ```
pub fn generate_structural_path(
    element_type: ElementType,
    number_value: &str,
    parent_structural_path: Option<&str>,
) -> String {
    // A frozen list: this word is in every path in every dataset, so changing one
    // renames a provision (`ElementType::path_segment_name`).
    let element_name = element_type.path_segment_name();
    match parent_structural_path {
        Some(parent) => format!("{}/{}_{}", parent, element_name, number_value),
        None => format!("{}_{}", element_name, number_value),
    }
}
