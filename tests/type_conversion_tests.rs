use words_to_data::uslm::{BillType, DocumentType, USCType};

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
    let result = DocumentType::from_str("publiclaw", Some("119-hr-1"));
    assert!(result.is_ok());

    match result.unwrap() {
        DocumentType::Bill { bill_type, bill_id } => {
            assert_eq!(bill_type, BillType::PublicLaw);
            assert_eq!(bill_id, "119-hr-1");
        }
        _ => panic!("Expected Bill variant"),
    }
}
