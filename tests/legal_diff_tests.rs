use words_to_data::{
    diff::TreeDiff,
    legal_diff::{
        AnnotationMetadata, AnnotationStatus, BillReference, ChangeAnnotation, LegalDiff,
    },
    uslm::{AmendingAction, BillAmendment, bill_parser::parse_bill_amendments, parser::parse},
};

/// Get the amendment that modifies Section 174 from the bill
/// This is the amendment that strikes "specified research" and inserts "foreign research"
fn get_section_174_amendment() -> BillAmendment {
    let data =
        parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").expect("Failed to parse bill");

    data.amendments
        .into_iter()
        .find(|a| {
            a.amending_text.contains("Section 174 is amended")
                && a.amending_text.contains("foreign research")
        })
        .expect("Should find amendment for section 174")
}

/// Helper to create the real annotation for Section 174(a) change
/// This is the instruction that changes "specified research" to "foreign research" in 26 USC 174(a)
fn make_section_174a_annotation(annotator: &str) -> ChangeAnnotation {
    let data =
        parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").expect("Failed to parse bill");
    let amendment = get_section_174_amendment();

    ChangeAnnotation {
        operation: AmendingAction::StrikeAndInsert,
        source_bill: BillReference {
            bill_id: data.bill_id.clone(),
            causative_text: amendment.amending_text,
        },
        related_paths: vec![],
        metadata: AnnotationMetadata {
            status: AnnotationStatus::Pending,
            confidence: None,
            annotator: annotator.to_string(),
            timestamp: time::OffsetDateTime::now_utc(),
            notes: None,
            reasoning: Some(
                "Bill text explicitly strikes 'specified' and inserts 'foreign'".to_string(),
            ),
        },
    }
}

/// Helper to create a generic test annotation using real bill data
fn make_test_annotation(operation: AmendingAction, annotator: &str) -> ChangeAnnotation {
    let data =
        parse_bill_amendments("tests/test_data/bills/hr-119-21.xml").expect("Failed to parse bill");
    let amendment = get_section_174_amendment();

    ChangeAnnotation {
        operation,
        source_bill: BillReference {
            bill_id: data.bill_id.clone(),
            causative_text: amendment.amending_text,
        },
        related_paths: vec![],
        metadata: AnnotationMetadata {
            status: AnnotationStatus::Pending,
            confidence: None,
            annotator: annotator.to_string(),
            timestamp: time::OffsetDateTime::now_utc(),
            notes: None,
            reasoning: None,
        },
    }
}

/// Helper to create a TreeDiff from real test data
fn make_test_tree_diff() -> TreeDiff {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Failed to parse old doc");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Failed to parse new doc");
    TreeDiff::from_elements(&doc_old, &doc_new)
}

// =============================================================================
// LegalDiff::new tests
// =============================================================================

#[test]
fn should_create_legal_diff_with_empty_annotations() {
    let tree_diff = make_test_tree_diff();
    let legal_diff = LegalDiff::new(&tree_diff);

    assert!(legal_diff.annotations.is_empty());
    assert_eq!(legal_diff.tree_diff.root_path, tree_diff.root_path);
}

// =============================================================================
// LegalDiff::add_annotation tests
// =============================================================================

#[test]
fn should_add_annotation_to_path() {
    let tree_diff = make_test_tree_diff();
    let mut legal_diff = LegalDiff::new(&tree_diff);
    let path = "uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a".to_string();
    let annotation = make_section_174a_annotation("human:test");

    legal_diff.add_annotation(path.clone(), annotation);

    assert!(legal_diff.annotations.contains_key(&path));
    assert_eq!(legal_diff.annotations.get(&path).unwrap().len(), 1);
}

#[test]
fn should_add_multiple_annotations_to_same_path() {
    let tree_diff = make_test_tree_diff();
    let mut legal_diff = LegalDiff::new(&tree_diff);
    let path = "uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a".to_string();

    let annotation1 = make_test_annotation(AmendingAction::Strike, "human:test");
    let annotation2 = make_test_annotation(AmendingAction::Insert, "human:test");

    legal_diff.add_annotation(path.clone(), annotation1);
    legal_diff.add_annotation(path.clone(), annotation2);

    assert_eq!(legal_diff.annotations.get(&path).unwrap().len(), 2);
}

// =============================================================================
// LegalDiff::get_annotations tests
// =============================================================================

#[test]
fn should_get_annotations_for_existing_path() {
    let tree_diff = make_test_tree_diff();
    let mut legal_diff = LegalDiff::new(&tree_diff);
    let path = "uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a".to_string();
    let annotation = make_section_174a_annotation("human:test");

    legal_diff.add_annotation(path.clone(), annotation);

    let retrieved = legal_diff.get_annotations(&path);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().len(), 1);
    assert_eq!(
        retrieved.unwrap()[0].operation,
        AmendingAction::StrikeAndInsert
    );
}

#[test]
fn should_return_none_for_unannotated_path() {
    let tree_diff = make_test_tree_diff();
    let legal_diff = LegalDiff::new(&tree_diff);

    let retrieved = legal_diff.get_annotations("nonexistent/path");
    assert!(retrieved.is_none());
}

// =============================================================================
// LegalDiff::get_diff_node tests
// =============================================================================

#[test]
fn should_get_diff_node_for_existing_path() {
    let tree_diff = make_test_tree_diff();
    let legal_diff = LegalDiff::new(&tree_diff);
    let path = "uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";

    let node = legal_diff.get_diff_node(path);
    assert!(node.is_some());
    assert_eq!(node.unwrap().root_path, path);
}

#[test]
fn should_return_none_for_nonexistent_diff_node() {
    let tree_diff = make_test_tree_diff();
    let legal_diff = LegalDiff::new(&tree_diff);

    let node = legal_diff.get_diff_node("nonexistent/path");
    assert!(node.is_none());
}

// =============================================================================
// LegalDiff::find_related_annotations tests
// =============================================================================

#[test]
fn should_find_annotations_that_reference_path_in_related_paths() {
    let tree_diff = make_test_tree_diff();
    let mut legal_diff = LegalDiff::new(&tree_diff);

    let source_path = "uscodedocument_26/title_26/section_1/paragraph_2".to_string();
    let target_path = "uscodedocument_26/title_26/section_1/paragraph_3".to_string();

    // Create annotation on source that references target
    let mut annotation = make_test_annotation(AmendingAction::Redesignate, "human:test");
    annotation.related_paths = vec![target_path.clone()];

    legal_diff.add_annotation(source_path.clone(), annotation);

    // Find annotations that reference target_path
    let related = legal_diff.find_related_annotations(&target_path);
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].0, &source_path);
    assert_eq!(related[0].1.operation, AmendingAction::Redesignate);
}

#[test]
fn should_return_empty_when_no_annotations_reference_path() {
    let tree_diff = make_test_tree_diff();
    let legal_diff = LegalDiff::new(&tree_diff);

    let related = legal_diff.find_related_annotations("some/path");
    assert!(related.is_empty());
}

// =============================================================================
// LegalDiff::annotated_paths tests
// =============================================================================

#[test]
fn should_return_all_annotated_paths() {
    let tree_diff = make_test_tree_diff();
    let mut legal_diff = LegalDiff::new(&tree_diff);

    let path1 = "uscodedocument_26/title_26/section_174".to_string();
    let path2 = "uscodedocument_26/title_26/section_175".to_string();

    legal_diff.add_annotation(
        path1.clone(),
        make_test_annotation(AmendingAction::Amend, "human:a"),
    );
    legal_diff.add_annotation(
        path2.clone(),
        make_test_annotation(AmendingAction::Add, "human:b"),
    );

    let paths: Vec<&String> = legal_diff.annotated_paths().collect();
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&&path1));
    assert!(paths.contains(&&path2));
}

#[test]
fn should_return_empty_iterator_when_no_annotations() {
    let tree_diff = make_test_tree_diff();
    let legal_diff = LegalDiff::new(&tree_diff);

    let paths: Vec<&String> = legal_diff.annotated_paths().collect();
    assert!(paths.is_empty());
}

// =============================================================================
// LegalDiff::unannotated_paths tests
// =============================================================================

#[test]
fn should_return_paths_with_changes_but_no_annotations() {
    let tree_diff = make_test_tree_diff();
    let legal_diff = LegalDiff::new(&tree_diff);

    // With no annotations, all paths with changes should be unannotated
    let unannotated = legal_diff.unannotated_paths();

    // We know section 174(a) has changes from the diff_tests
    let s174a_path = "uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a";
    assert!(unannotated.contains(&s174a_path.to_string()));
}

#[test]
fn should_exclude_annotated_paths_from_unannotated_list() {
    let tree_diff = make_test_tree_diff();
    let mut legal_diff = LegalDiff::new(&tree_diff);

    let s174a_path = "uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a".to_string();

    // Annotate section 174(a) with real annotation
    legal_diff.add_annotation(
        s174a_path.clone(),
        make_section_174a_annotation("human:test"),
    );

    let unannotated = legal_diff.unannotated_paths();

    // Section 174(a) should no longer be in unannotated list
    assert!(!unannotated.contains(&s174a_path));
}

// =============================================================================
// Serialization tests
// =============================================================================

#[test]
fn should_roundtrip_legal_diff_through_json() {
    let tree_diff = make_test_tree_diff();
    let mut legal_diff = LegalDiff::new(&tree_diff);

    let path = "uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a".to_string();
    legal_diff.add_annotation(path.clone(), make_section_174a_annotation("human:test"));

    // Serialize to JSON
    let json = serde_json::to_string(&legal_diff).expect("Failed to serialize");

    // Deserialize back
    let restored: LegalDiff = serde_json::from_str(&json).expect("Failed to deserialize");

    assert_eq!(restored.tree_diff.root_path, legal_diff.tree_diff.root_path);
    assert_eq!(restored.annotations.len(), 1);
    assert!(restored.annotations.contains_key(&path));

    // Verify annotation survived serialization
    let restored_annotation = &restored.annotations.get(&path).unwrap()[0];
    assert_eq!(
        restored_annotation.operation,
        AmendingAction::StrikeAndInsert
    );
    // Verify the amending text contains the expected content
    assert!(
        restored_annotation
            .source_bill
            .causative_text
            .contains("Section 174 is amended")
    );
}
