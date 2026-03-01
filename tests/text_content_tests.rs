use time::Date;
use words_to_data::uslm::{DocumentType, ElementData, ElementType, TextContentField, USCType};

/// Helper function to create a test ElementData with specified text fields
fn create_test_element_data(
    heading: Option<&str>,
    chapeau: Option<&str>,
    proviso: Option<&str>,
    content: Option<&str>,
    continuation: Option<&str>,
) -> ElementData {
    ElementData {
        path: "test_path/section_1".to_string(),
        element_type: ElementType::Section,
        document_type: DocumentType::USCode {
            usc_type: USCType::Title,
        },
        date: Date::from_calendar_date(2025, time::Month::July, 18).unwrap(),
        number_value: "1".to_string(),
        number_display: "1".to_string(),
        verbose_name: "Section 1".to_string(),
        heading: heading.map(|s| s.to_string()),
        chapeau: chapeau.map(|s| s.to_string()),
        proviso: proviso.map(|s| s.to_string()),
        content: content.map(|s| s.to_string()),
        continuation: continuation.map(|s| s.to_string()),
        uslm_id: Some("/us/usc/t1/s1".to_string()),
        uslm_uuid: None,
        source_credits: vec![],
    }
}

#[test]
fn test_all_fields_populated() {
    let element = create_test_element_data(
        Some("Heading"),
        Some("Chapeau"),
        Some("Proviso"),
        Some("Content"),
        Some("Continuation"),
    );

    // Verify all fields are retrievable
    assert_eq!(
        element.get_text_content(TextContentField::Heading).unwrap(),
        "Heading"
    );
    assert_eq!(
        element.get_text_content(TextContentField::Chapeau).unwrap(),
        "Chapeau"
    );
    assert_eq!(
        element.get_text_content(TextContentField::Proviso).unwrap(),
        "Proviso"
    );
    assert_eq!(
        element.get_text_content(TextContentField::Content).unwrap(),
        "Content"
    );
    assert_eq!(
        element
            .get_text_content(TextContentField::Continuation)
            .unwrap(),
        "Continuation"
    );
}

#[test]
fn test_mixed_some_none_fields() {
    let element = create_test_element_data(
        Some("Present"),
        None,
        Some("Also present"),
        None,
        Some("Here too"),
    );

    // Present fields
    assert!(
        element
            .get_text_content(TextContentField::Heading)
            .is_some()
    );
    assert!(
        element
            .get_text_content(TextContentField::Proviso)
            .is_some()
    );
    assert!(
        element
            .get_text_content(TextContentField::Continuation)
            .is_some()
    );

    // Absent fields
    assert!(
        element
            .get_text_content(TextContentField::Chapeau)
            .is_none()
    );
    assert!(
        element
            .get_text_content(TextContentField::Content)
            .is_none()
    );
}

#[test]
fn test_all_fields_none() {
    let element = create_test_element_data(None, None, None, None, None);

    // All fields should return None
    assert!(
        element
            .get_text_content(TextContentField::Heading)
            .is_none()
    );
    assert!(
        element
            .get_text_content(TextContentField::Chapeau)
            .is_none()
    );
    assert!(
        element
            .get_text_content(TextContentField::Proviso)
            .is_none()
    );
    assert!(
        element
            .get_text_content(TextContentField::Content)
            .is_none()
    );
    assert!(
        element
            .get_text_content(TextContentField::Continuation)
            .is_none()
    );
}

#[test]
fn test_multiline_content() {
    let multiline_text = "This is line 1\nThis is line 2\nThis is line 3";
    let element = create_test_element_data(None, None, None, Some(multiline_text), None);

    let result = element.get_text_content(TextContentField::Content);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), multiline_text);
}

#[test]
fn test_special_characters() {
    let special_text = "Text with §, ©, and other symbols: <>&\"'";
    let element = create_test_element_data(Some(special_text), None, None, None, None);

    let result = element.get_text_content(TextContentField::Heading);
    assert!(result.is_some());
    assert_eq!(result.unwrap(), special_text);
}
