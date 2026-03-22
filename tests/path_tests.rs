use words_to_data::uslm::ElementType;
use words_to_data::uslm::path::{generate_structural_path, should_include_in_uslm_path};

#[test]
fn test_generate_structural_path_root_element() {
    let path = generate_structural_path(ElementType::USCodeDocument, "26", None);
    assert_eq!(path, "uscodedocument_26");
}

#[test]
fn test_generate_structural_path_nested_element() {
    let path = generate_structural_path(
        ElementType::Section,
        "174",
        Some("uscodedocument_26/title_26"),
    );
    assert_eq!(path, "uscodedocument_26/title_26/section_174");
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
