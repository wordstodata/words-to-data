from words_to_data import (
    TreeDiff,
    compute_diff,
    parse_uslm_xml,
    USLMElement,
    parse_bill_amendments,
    load_uslm_folder,
    AmendmentData,
    BillAmendment,
    FieldChangeEvent,
    TextChange,
)


def test_load_uslm_folder():
    result = load_uslm_folder("tests/test_data/usc/2025-07-30", "2025-07-30")
    assert len(result.children) == 57

def test_uslm_elements():
    element = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    s174a = element.find(
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    )
    assert isinstance(element, USLMElement)
    assert isinstance(s174a, USLMElement)

def test_merge_elements():
    title_9 = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
    title_26 = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    assert len(title_9.children) == 1
    assert len(title_26.children) == 1
    title_26.merge_children(title_9)
    assert len(title_26.children) == 2

def test_diffs():
    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    # Compute diff
    diff = compute_diff(old, new)
    s174a = diff.find(
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    )
    assert isinstance(s174a, TreeDiff)
    field_change = s174a.changes[0]
    assert len(field_change.changes) == 2
    assert field_change.field_name == "chapeau"
    assert (
        field_change.old_value
        == "In the case of a taxpayer’s specified research or experimental expenditures for any taxable year—"
    )
    assert (
        field_change.new_value
        == "In the case of a taxpayer’s foreign research or experimental expenditures for any taxable year—"
    )


def test_bill_parsing():
    # Parse bill amendments
    data = parse_bill_amendments("tests/test_data/bills/pl-119-21.xml")

    # Validate AmendmentData
    assert isinstance(data, AmendmentData)
    assert data.bill_id == "119-21"
    assert len(data.amendments) > 0

    # Validate BillAmendment
    amendment = data.amendments[0]
    assert isinstance(amendment, BillAmendment)
    assert len(amendment.amending_text) > 0
    assert len(amendment.action_types) > 0

    # Validate action types are valid strings
    valid_actions = [
        "amend",
        "add",
        "delete",
        "insert",
        "redesignate",
        "repeal",
        "move",
        "strike",
        "strikeandinsert",
    ]
    assert all(action in valid_actions for action in amendment.action_types)


def test_to_json_methods():
    import json

    # Test USLMElement.to_json()
    element = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    element_json = element.to_json()
    assert isinstance(element_json, str)
    parsed = json.loads(element_json)
    assert "path" in parsed or "data" in parsed  # should have structure

    # Test TreeDiff.to_json() and nested types
    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    diff = compute_diff(old, new)
    diff_json = diff.to_json()
    assert isinstance(diff_json, str)
    parsed_diff = json.loads(diff_json)
    assert "root_path" in parsed_diff

    # Test FieldChangeEvent.to_json()
    s174a = diff.find(
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    )
    assert s174a is not None
    field_change = s174a.changes[0]
    field_json = field_change.to_json()
    assert isinstance(field_json, str)
    parsed_field = json.loads(field_json)
    assert "field_name" in parsed_field

    # Test TextChange.to_json()
    text_change = field_change.changes[0]
    text_json = text_change.to_json()
    assert isinstance(text_json, str)
    parsed_text = json.loads(text_json)
    assert "value" in parsed_text

    # Test AmendmentData.to_json() and nested types
    data = parse_bill_amendments("tests/test_data/bills/pl-119-21.xml")
    data_json = data.to_json()
    assert isinstance(data_json, str)
    parsed_data = json.loads(data_json)
    assert "bill_id" in parsed_data
    assert "amendments" in parsed_data

    # Test BillAmendment.to_json()
    amendment = data.amendments[0]
    amendment_json = amendment.to_json()
    assert isinstance(amendment_json, str)
    parsed_amendment = json.loads(amendment_json)
    assert "amending_text" in parsed_amendment
    assert "action_types" in parsed_amendment


def test_to_dict_methods():
    """Test that to_dict() returns Python dicts compatible with json.dumps"""
    import json
    from words_to_data import BillDiff

    # Test BillDiff.to_dict()
    bill_diff = BillDiff(["added", "words"], ["removed"])
    d = bill_diff.to_dict()
    assert isinstance(d, dict)
    assert d["added"] == ["added", "words"]
    assert d["removed"] == ["removed"]

    # Test it works with json.dumps
    json_str = json.dumps(d, indent=2)
    assert isinstance(json_str, str)
    parsed = json.loads(json_str)
    assert parsed == d

    # Test BillAmendment.to_dict()
    data = parse_bill_amendments("tests/test_data/bills/pl-119-21.xml")
    amendment = data.amendments[0]
    amendment_dict = amendment.to_dict()
    assert isinstance(amendment_dict, dict)
    assert "id" in amendment_dict
    assert "amending_text" in amendment_dict
    assert "action_types" in amendment_dict

    # Test list of amendments with json.dumps
    amendments_list = [a.to_dict() for a in data.amendments]
    json_str = json.dumps(amendments_list, indent=2)
    assert isinstance(json_str, str)

    # Test TreeDiff.to_dict()
    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    diff = compute_diff(old, new)
    diff_dict = diff.to_dict()
    assert isinstance(diff_dict, dict)
    assert "root_path" in diff_dict

    # Test USLMElement.to_dict()
    element_dict = new.to_dict()
    assert isinstance(element_dict, dict)


def test_from_json_roundtrip():
    """Test that from_json() can deserialize JSON produced by to_json()"""

    # Test USLMElement round-trip
    element = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    element_json = element.to_json()
    restored_element = USLMElement.from_json(element_json)
    assert isinstance(restored_element, USLMElement)
    assert restored_element.data["path"] == element.data["path"]
    assert len(restored_element.children) == len(element.children)

    # Test TreeDiff round-trip
    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    diff = compute_diff(old, new)
    diff_json = diff.to_json()
    restored_diff = TreeDiff.from_json(diff_json)
    assert isinstance(restored_diff, TreeDiff)
    assert restored_diff.root_path == diff.root_path
    assert len(restored_diff.child_diffs) == len(diff.child_diffs)

    # Test FieldChangeEvent round-trip
    s174a = diff.find(
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    )
    assert s174a is not None
    field_change = s174a.changes[0]
    field_json = field_change.to_json()
    restored_field = FieldChangeEvent.from_json(field_json)
    assert isinstance(restored_field, FieldChangeEvent)
    assert restored_field.field_name == field_change.field_name
    assert restored_field.old_value == field_change.old_value
    assert restored_field.new_value == field_change.new_value

    # Test TextChange round-trip
    text_change = field_change.changes[0]
    text_json = text_change.to_json()
    restored_text = TextChange.from_json(text_json)
    assert isinstance(restored_text, TextChange)
    assert restored_text.value == text_change.value
    assert restored_text.tag == text_change.tag

    # Test AmendmentData round-trip
    data = parse_bill_amendments("tests/test_data/bills/pl-119-21.xml")
    data_json = data.to_json()
    restored_data = AmendmentData.from_json(data_json)
    assert isinstance(restored_data, AmendmentData)
    assert restored_data.bill_id == data.bill_id
    assert len(restored_data.amendments) == len(data.amendments)

    # Test BillAmendment round-trip
    amendment = data.amendments[0]
    amendment_json = amendment.to_json()
    restored_amendment = BillAmendment.from_json(amendment_json)
    assert isinstance(restored_amendment, BillAmendment)
    assert restored_amendment.amending_text == amendment.amending_text
    assert restored_amendment.action_types == amendment.action_types


# ============================================================================
# legal_diff module tests
# ============================================================================


def test_legal_diff_creation():
    """Test creating a LegalDiff from a TreeDiff"""
    from words_to_data import LegalDiff

    # Create a TreeDiff first
    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    diff = compute_diff(old, new)

    # Create a LegalDiff from the TreeDiff
    legal_diff = LegalDiff(diff)

    assert isinstance(legal_diff, LegalDiff)
    # tree_diff property should return a TreeDiff
    assert isinstance(legal_diff.tree_diff, TreeDiff)
    assert legal_diff.tree_diff.root_path == diff.root_path


def test_change_annotation_creation():
    """Test creating a ChangeAnnotation and adding it to a LegalDiff"""
    from words_to_data import LegalDiff, ChangeAnnotation

    # Create a LegalDiff
    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    diff = compute_diff(old, new)
    legal_diff = LegalDiff(diff)
    path = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    # Create a ChangeAnnotation
    annotation = ChangeAnnotation(
        operation="amend",
        bill_id="119-21",
        causative_text="Section 2 of Public Law 119-21 amends section 174(a)...",
        annotator="human:test_user",
        amendment_id="test",
        paths = [path]
    )

    assert isinstance(annotation, ChangeAnnotation)
    assert annotation.operation == "amend"
    assert annotation.source_bill.bill_id == "119-21"
    assert annotation.source_bill.causative_text == "Section 2 of Public Law 119-21 amends section 174(a)..."
    assert annotation.metadata.annotator == "human:test_user"
    assert annotation.metadata.status == "pending"  # default status

    # Add the annotation to the legal diff
    legal_diff.add_annotation(annotation)

    # Retrieve it back
    retrieved = legal_diff.get_annotations(path)
    assert retrieved is not None
    assert len(retrieved) == 1
    assert retrieved[0].operation == "amend"


def test_legal_diff_methods():
    """Test various LegalDiff methods"""
    from words_to_data import LegalDiff, ChangeAnnotation

    # Create a LegalDiff with changes
    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    diff = compute_diff(old, new)
    legal_diff = LegalDiff(diff)

    # Test unannotated_paths - should have paths since we haven't added annotations yet
    unannotated = legal_diff.unannotated_paths()
    assert len(unannotated) > 0
    assert isinstance(unannotated, list)

    # Add an annotation to one path
    path_with_change = unannotated[0]
    annotation = ChangeAnnotation(
        operation="amend",
        bill_id="119-21",
        causative_text="Test amendment text",
        annotator="human:test",
        amendment_id="test",
        paths = [path_with_change]
    )
    legal_diff.add_annotation(annotation)

    # Test annotated_paths
    annotated = legal_diff.annotated_paths()
    assert path_with_change in annotated

    # Test get_diff_node
    diff_node = legal_diff.get_diff_node(path_with_change)
    assert diff_node is not None
    assert isinstance(diff_node, TreeDiff)

    # Test that unannotated_paths now excludes the annotated path
    new_unannotated = legal_diff.unannotated_paths()
    assert path_with_change not in new_unannotated


def test_change_annotation_with_optional_fields():
    """Test creating ChangeAnnotation with optional fields"""
    from words_to_data import ChangeAnnotation

    # Create with all optional fields
    annotation = ChangeAnnotation(
        operation="strikeandinsert",
        bill_id="119-21",
        causative_text="Section 2(a) strikes 'specified' and inserts 'foreign'",
        annotator="model:gpt-4",
        confidence=0.95,
        notes="High confidence match based on text similarity",
        reasoning="The bill text directly matches the change observed in the diff",
        paths=["uscode/title_26/section_175"],
        amendment_id="test"
    )

    assert annotation.operation == "strikeandinsert"
    assert annotation.metadata.confidence is not None
    assert abs(annotation.metadata.confidence - 0.95) < 0.001  # f32 precision
    assert annotation.metadata.notes == "High confidence match based on text similarity"
    assert annotation.metadata.reasoning == "The bill text directly matches the change observed in the diff"


def test_legal_diff_json_roundtrip():
    """Test JSON serialization and deserialization of LegalDiff"""
    import json
    from words_to_data import LegalDiff, ChangeAnnotation

    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    diff = compute_diff(old, new)
    legal_diff = LegalDiff(diff)

    # Add an annotation
    path = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    annotation = ChangeAnnotation(
        operation="amend",
        bill_id="119-21",
        causative_text="Test text",
        annotator="human:test",
        confidence=0.9,
        amendment_id="test",
        paths=[path]
    )
    legal_diff.add_annotation(annotation)

    # Serialize to JSON
    json_str = legal_diff.to_json()
    assert isinstance(json_str, str)
    parsed = json.loads(json_str)
    assert "tree_diff" in parsed
    assert "annotations" in parsed

    # Deserialize back
    restored = LegalDiff.from_json(json_str)
    assert isinstance(restored, LegalDiff)
    assert restored.tree_diff.root_path == legal_diff.tree_diff.root_path

    # Check annotations survived roundtrip
    restored_anns = restored.get_annotations(path)
    assert restored_anns is not None
    assert len(restored_anns) == 1
    assert restored_anns[0].operation == "amend"
    assert restored_anns[0].source_bill.bill_id == "119-21"


def test_tree_diff_shallow():
    """Test that shallow() returns a TreeDiff without children"""
    old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    diff = compute_diff(old, new)

    # Find a node with children
    s174 = diff.find(
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174"
    )
    assert s174 is not None
    assert len(s174.child_diffs) > 0, "Section 174 should have child diffs"

    # Get shallow copy
    shallow = s174.shallow()

    # Verify shallow copy has same data but no children
    assert shallow.root_path == s174.root_path
    assert len(shallow.changes) == len(s174.changes)
    assert len(shallow.child_diffs) == 0, "Shallow copy should have no children"


def test_annotation_types_json_roundtrip():
    """Test JSON roundtrip for ChangeAnnotation and nested types"""
    import json
    from words_to_data import ChangeAnnotation, BillReference, AnnotationMetadata

    # Test ChangeAnnotation roundtrip
    annotation = ChangeAnnotation(
        operation="add",
        bill_id="119-21",
        causative_text="Adding new subsection (c)",
        annotator="model:claude-3",
        confidence=0.85,
        notes="AI-generated annotation",
        amendment_id="test",
        paths=["test"]
    )

    ann_json = annotation.to_json()
    parsed = json.loads(ann_json)
    assert "operation" in parsed
    assert "source_bill" in parsed
    assert "metadata" in parsed

    restored_ann = ChangeAnnotation.from_json(ann_json)
    assert restored_ann.operation == annotation.operation
    assert restored_ann.source_bill.bill_id == annotation.source_bill.bill_id
    assert restored_ann.metadata.confidence == annotation.metadata.confidence

    # Test BillReference roundtrip

    bill_ref = BillReference(bill_id="119-21", amendment_id="test", causative_text="Section 2(a)(1)")
    bill_json = bill_ref.to_json()
    restored_bill = BillReference.from_json(bill_json)
    assert restored_bill.bill_id == bill_ref.bill_id
    assert restored_bill.causative_text == bill_ref.causative_text

    # Test AnnotationMetadata roundtrip
    meta_json = annotation.metadata.to_json()
    restored_meta = AnnotationMetadata.from_json(meta_json)
    assert restored_meta.status == annotation.metadata.status
    assert restored_meta.annotator == annotation.metadata.annotator
    assert restored_meta.confidence == annotation.metadata.confidence
