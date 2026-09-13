use words_to_data::uslm::ElementType;
use words_to_data::uslm::path::{
    generate_structural_path, path_segment_from_heading, should_include_in_uslm_path,
};

#[test]
fn test_generate_structural_path_nested_element() {
    let path = generate_structural_path(ElementType::Section, "174", Some("uscode/title_26"));
    assert_eq!(path, "uscode/title_26/section_174");
}

#[test]
fn test_should_include_in_uslm_path_section() {
    assert!(should_include_in_uslm_path(ElementType::Section));
}

#[test]
fn test_should_include_in_uslm_path_level() {
    assert!(!should_include_in_uslm_path(ElementType::Level));
}

#[test]
fn test_should_include_in_uslm_path_unknown() {
    assert!(!should_include_in_uslm_path(ElementType::Unknown));
}

/// A segment goes into a path, so nothing outside `[a-z0-9-]` may survive. The
/// section symbol and the curly apostrophe both appear in real headings.
#[test]
fn should_keep_only_path_safe_characters_when_a_heading_becomes_a_segment() {
    let segment = path_segment_from_heading(
        "SUPPLEMENTAL RULES FOR SOCIAL SECURITY ACTIONS UNDER 42 U.S.C. \u{a7} 405(g)",
    )
    .expect("the heading names the container");

    assert_eq!(
        segment,
        "supplemental-rules-for-social-security-actions-under-42-u-s-c-405-g"
    );
}

/// An empty segment would name the container's parent, so there has to be none.
#[test]
fn should_give_no_segment_when_a_heading_holds_no_letter_or_digit() {
    assert_eq!(path_segment_from_heading("\u{2014} \u{2014}"), None);
}
