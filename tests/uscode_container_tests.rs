//! Tests for the USCode container structure
//!
//! The parser produces a `uscode` root element with titles as direct children,
//! enabling cross-title diffs from a single snapshot date.

use words_to_data::uslm::parser::parse;
use words_to_data::uslm::{DocumentType, USCType, UslmFacts};

/// Test: parser should produce uscode root with title as child
#[test]
fn should_parse_title_with_uscode_root() {
    let element =
        parse("tests/test_data/usc/2025-07-18/usc07.xml", "2025-07-18").expect("Failed to parse");

    // Root should be uscode container
    assert_eq!(element.data.node_type.as_str(), "uscode.document");
    assert_eq!(element.data.path.as_ref(), "uscode");

    // The container's own USLM facts say it is the whole Code, not one title
    let facts = UslmFacts::of(&element.data).expect("the container carries USLM facts");
    match facts.document_type {
        DocumentType::USCode { usc_type } => {
            assert_eq!(usc_type, USCType::USCode);
        }
        other => panic!("Expected USCode document type, got {other:?}"),
    }

    // First child should be the title
    assert!(!element.children.is_empty());
    let title = &element.children[0];
    assert_eq!(title.data.node_type.as_str(), "uscode.title");
    assert_eq!(title.data.path.as_ref(), "uscode/title_7");
}
