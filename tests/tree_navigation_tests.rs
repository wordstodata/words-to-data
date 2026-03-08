use time::Date;
use words_to_data::uslm::{DocumentType, ElementData, ElementType, USCType, USLMElement};

/// Helper function to create a minimal ElementData
fn create_element_data(path: &str, element_type: ElementType, number: &str) -> ElementData {
    ElementData {
        path: path.to_string(),
        element_type,
        document_type: DocumentType::USCode {
            usc_type: USCType::Title,
        },
        date: Date::from_calendar_date(2025, time::Month::July, 18).unwrap(),
        number_value: number.to_string(),
        number_display: number.to_string(),
        verbose_name: format!("{:?} {}", element_type, number),
        heading: None,
        chapeau: None,
        proviso: None,
        content: None,
        continuation: None,
        uslm_id: None,
        uslm_uuid: None,
        source_credits: vec![],
    }
}

/// Create a test tree structure:
/// uscodedocument_7
///   └── title_7
///       └── chapter_1
///           └── section_1
///               └── subsection_a
fn create_test_tree() -> USLMElement {
    let subsection = USLMElement {
        data: create_element_data(
            "uscodedocument_7/title_7/chapter_1/section_1/subsection_a",
            ElementType::Subsection,
            "a",
        ),
        children: vec![],
    };

    let section = USLMElement {
        data: create_element_data(
            "uscodedocument_7/title_7/chapter_1/section_1",
            ElementType::Section,
            "1",
        ),
        children: vec![subsection],
    };

    let chapter = USLMElement {
        data: create_element_data(
            "uscodedocument_7/title_7/chapter_1",
            ElementType::Chapter,
            "1",
        ),
        children: vec![section],
    };

    let title = USLMElement {
        data: create_element_data("uscodedocument_7/title_7", ElementType::Title, "7"),
        children: vec![chapter],
    };

    USLMElement {
        data: create_element_data("uscodedocument_7", ElementType::USCodeDocument, "7"),
        children: vec![title],
    }
}

#[test]
fn test_find_root_element() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_7");
    assert!(result.is_some());

    let found = result.unwrap();
    assert_eq!(found.data.path, "uscodedocument_7");
    assert_eq!(found.data.element_type, ElementType::USCodeDocument);
}

#[test]
fn test_find_direct_child() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_7/title_7");
    assert!(result.is_some());

    let found = result.unwrap();
    assert_eq!(found.data.path, "uscodedocument_7/title_7");
    assert_eq!(found.data.element_type, ElementType::Title);
}

#[test]
fn test_find_nested_element() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_7/title_7/chapter_1");
    assert!(result.is_some());

    let found = result.unwrap();
    assert_eq!(found.data.path, "uscodedocument_7/title_7/chapter_1");
    assert_eq!(found.data.element_type, ElementType::Chapter);
}

#[test]
fn test_find_deep_nested_element() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_7/title_7/chapter_1/section_1");
    assert!(result.is_some());

    let found = result.unwrap();
    assert_eq!(
        found.data.path,
        "uscodedocument_7/title_7/chapter_1/section_1"
    );
    assert_eq!(found.data.element_type, ElementType::Section);
}

#[test]
fn test_find_leaf_node() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_7/title_7/chapter_1/section_1/subsection_a");
    assert!(result.is_some());

    let found = result.unwrap();
    assert_eq!(
        found.data.path,
        "uscodedocument_7/title_7/chapter_1/section_1/subsection_a"
    );
    assert_eq!(found.data.element_type, ElementType::Subsection);
}

#[test]
fn test_find_nonexistent_path() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_7/title_7/chapter_99");
    assert!(result.is_none());
}

#[test]
fn test_find_partial_path_nonexistent() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_7/title_7/chapter_1/section_2");
    assert!(result.is_none());
}

#[test]
fn test_find_wrong_prefix() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_26/title_26");
    assert!(result.is_none());
}

#[test]
fn test_find_path_too_deep() {
    let tree = create_test_tree();

    // Path extends beyond what exists
    let result = tree.find("uscodedocument_7/title_7/chapter_1/section_1/subsection_a/paragraph_1");
    assert!(result.is_none());
}

#[test]
fn test_find_empty_string() {
    let tree = create_test_tree();

    let result = tree.find("");
    assert!(result.is_none());
}

#[test]
fn test_tree_with_multiple_children() {
    // Create a tree with multiple siblings
    let subsection_a = USLMElement {
        data: create_element_data(
            "uscodedocument_7/title_7/section_1/subsection_a",
            ElementType::Subsection,
            "a",
        ),
        children: vec![],
    };

    let subsection_b = USLMElement {
        data: create_element_data(
            "uscodedocument_7/title_7/section_1/subsection_b",
            ElementType::Subsection,
            "b",
        ),
        children: vec![],
    };

    let subsection_c = USLMElement {
        data: create_element_data(
            "uscodedocument_7/title_7/section_1/subsection_c",
            ElementType::Subsection,
            "c",
        ),
        children: vec![],
    };

    let section = USLMElement {
        data: create_element_data(
            "uscodedocument_7/title_7/section_1",
            ElementType::Section,
            "1",
        ),
        children: vec![subsection_a, subsection_b, subsection_c],
    };

    let title = USLMElement {
        data: create_element_data("uscodedocument_7/title_7", ElementType::Title, "7"),
        children: vec![section],
    };

    let tree = USLMElement {
        data: create_element_data("uscodedocument_7", ElementType::USCodeDocument, "7"),
        children: vec![title],
    };

    // Test finding each subsection
    let result_a = tree.find("uscodedocument_7/title_7/section_1/subsection_a");
    assert!(result_a.is_some());
    assert_eq!(result_a.unwrap().data.number_value, "a");

    let result_b = tree.find("uscodedocument_7/title_7/section_1/subsection_b");
    assert!(result_b.is_some());
    assert_eq!(result_b.unwrap().data.number_value, "b");

    let result_c = tree.find("uscodedocument_7/title_7/section_1/subsection_c");
    assert!(result_c.is_some());
    assert_eq!(result_c.unwrap().data.number_value, "c");
}

#[test]
fn test_find_preserves_children() {
    let tree = create_test_tree();

    let result = tree.find("uscodedocument_7/title_7/chapter_1/section_1");
    assert!(result.is_some());

    let found = result.unwrap();
    // Section should have one child (subsection_a)
    assert_eq!(found.children.len(), 1);
    assert_eq!(found.children[0].data.element_type, ElementType::Subsection);
}
