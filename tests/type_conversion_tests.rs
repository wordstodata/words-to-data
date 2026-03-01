use std::str::FromStr;
use words_to_data::uslm::{AmendingAction, BillType, DocumentType, ElementType, USCType};

// ===== DocumentType Tests =====

#[test]
fn test_document_type_usc_title() {
    let result = DocumentType::from_str("uscode", Some("usctitle"));
    assert!(result.is_ok());

    match result.unwrap() {
        DocumentType::USCode { usc_type } => {
            assert_eq!(usc_type, USCType::Title);
        }
        _ => panic!("Expected USCode variant"),
    }
}

#[test]
fn test_document_type_usc_title_appendix() {
    let result = DocumentType::from_str("uscode", Some("usctitleappendix"));
    assert!(result.is_ok());

    match result.unwrap() {
        DocumentType::USCode { usc_type } => {
            assert_eq!(usc_type, USCType::TitleAppendix);
        }
        _ => panic!("Expected USCode variant"),
    }
}

#[test]
fn test_document_type_public_law() {
    let result = DocumentType::from_str("publiclaw", Some("119-21"));
    assert!(result.is_ok());

    match result.unwrap() {
        DocumentType::Bill { bill_type, bill_id } => {
            assert_eq!(bill_type, BillType::PublicLaw);
            assert_eq!(bill_id, "119-21");
        }
        _ => panic!("Expected Bill variant"),
    }
}

#[test]
fn test_document_type_case_insensitive() {
    // Test various capitalizations
    let result1 = DocumentType::from_str("PublicLaw", Some("119-21"));
    assert!(result1.is_ok());

    let result2 = DocumentType::from_str("PUBLICLAW", Some("119-21"));
    assert!(result2.is_ok());

    let result3 = DocumentType::from_str("USCode", Some("usctitle"));
    assert!(result3.is_ok());
}

#[test]
fn test_document_type_alternative_names() {
    // Test alternative names for public law
    let result = DocumentType::from_str("plaw", Some("119-21"));
    assert!(result.is_ok());

    // Test alternative names for USC
    let result = DocumentType::from_str("us_code", Some("usctitle"));
    assert!(result.is_ok());

    let result = DocumentType::from_str("uscdoc", Some("usctitle"));
    assert!(result.is_ok());
}

#[test]
fn test_document_type_unknown() {
    let result = DocumentType::from_str("unknown_type", Some("meta"));
    assert!(result.is_err());
}

#[test]
fn test_document_type_missing_meta_str_usc() {
    let result = DocumentType::from_str("uscode", None);
    assert!(result.is_err());
}

#[test]
fn test_document_type_missing_meta_str_bill() {
    let result = DocumentType::from_str("publiclaw", None);
    assert!(result.is_err());
}

#[test]
fn test_document_type_invalid_usc_type() {
    let result = DocumentType::from_str("uscode", Some("invalid_type"));
    assert!(result.is_err());
}

// ===== ElementType Tests =====

#[test]
fn test_element_type_basic_elements() {
    assert_eq!(ElementType::from_str("title").unwrap(), ElementType::Title);
    assert_eq!(
        ElementType::from_str("subtitle").unwrap(),
        ElementType::Subtitle
    );
    assert_eq!(
        ElementType::from_str("chapter").unwrap(),
        ElementType::Chapter
    );
    assert_eq!(
        ElementType::from_str("subchapter").unwrap(),
        ElementType::Subchapter
    );
    assert_eq!(
        ElementType::from_str("section").unwrap(),
        ElementType::Section
    );
    assert_eq!(
        ElementType::from_str("subsection").unwrap(),
        ElementType::Subsection
    );
}

#[test]
fn test_element_type_case_insensitive() {
    assert_eq!(ElementType::from_str("TITLE").unwrap(), ElementType::Title);
    assert_eq!(
        ElementType::from_str("Section").unwrap(),
        ElementType::Section
    );
    assert_eq!(
        ElementType::from_str("SUBSECTION").unwrap(),
        ElementType::Subsection
    );
}

#[test]
fn test_element_type_unknown() {
    // Unknown strings should return Unknown variant (not an error)
    assert_eq!(
        ElementType::from_str("unknown_element").unwrap(),
        ElementType::Unknown
    );
    assert_eq!(
        ElementType::from_str("random").unwrap(),
        ElementType::Unknown
    );
    assert_eq!(ElementType::from_str("").unwrap(), ElementType::Unknown);
}

// ===== AmendingAction Tests =====

#[test]
fn test_amending_action_case_insensitive() {
    assert_eq!(
        AmendingAction::from_str("AMEND").unwrap(),
        AmendingAction::Amend
    );
    assert_eq!(
        AmendingAction::from_str("Add").unwrap(),
        AmendingAction::Add
    );
    assert_eq!(
        AmendingAction::from_str("DELETE").unwrap(),
        AmendingAction::Delete
    );
    assert_eq!(
        AmendingAction::from_str("Insert").unwrap(),
        AmendingAction::Insert
    );
}

#[test]
fn test_amending_action_invalid() {
    let result = AmendingAction::from_str("invalid_action");
    assert!(result.is_err());

    let result = AmendingAction::from_str("modify");
    assert!(result.is_err());

    let result = AmendingAction::from_str("");
    assert!(result.is_err());
}
