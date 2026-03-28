from words_to_data import TreeDiff, compute_diff, parse_uslm_xml, USLMElement, parse_bill_amendments, AmendmentData, BillAmendment, UscReference, FieldChangeEvent, TextChange

def test_uslm_elements():
    element = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
    s174a = element.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
    assert isinstance(element,USLMElement)
    assert isinstance(s174a,USLMElement)

def test_diffs():
    old = parse_uslm_xml('tests/test_data/usc/2025-07-18/usc26.xml', '2025-07-18')
    new = parse_uslm_xml('tests/test_data/usc/2025-07-30/usc26.xml', '2025-07-30')
    # Compute diff
    diff = compute_diff(old, new)
    s174a = diff.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
    assert isinstance(s174a, TreeDiff)
    field_change = s174a.changes[0]
    assert len(field_change.changes) == 2
    assert field_change.field_name == "chapeau"
    assert field_change.old_value == "In the case of a taxpayer’s specified research or experimental expenditures for any taxable year—"
    assert field_change.new_value == "In the case of a taxpayer’s foreign research or experimental expenditures for any taxable year—"

def test_bill_parsing():
    # Parse bill amendments
    data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml")

    # Validate AmendmentData
    assert isinstance(data, AmendmentData)
    assert data.bill_id == "119-21"
    assert len(data.amendments) > 0

    # Validate BillAmendment
    amendment = data.amendments[0]
    assert isinstance(amendment, BillAmendment)
    assert amendment.source_path.startswith("/us/pl/")
    assert len(amendment.target_paths) > 0
    assert len(amendment.action_types) > 0

    # Validate UscReference
    ref = amendment.target_paths[0]
    assert isinstance(ref, UscReference)
    assert ref.path.startswith("/us/usc/")
    assert len(ref.display_text) > 0

    # Validate action types are valid strings
    valid_actions = ["amend", "add", "delete", "insert", "redesignate", "repeal"]
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
    old = parse_uslm_xml('tests/test_data/usc/2025-07-18/usc26.xml', '2025-07-18')
    new = parse_uslm_xml('tests/test_data/usc/2025-07-30/usc26.xml', '2025-07-30')
    diff = compute_diff(old, new)
    diff_json = diff.to_json()
    assert isinstance(diff_json, str)
    parsed_diff = json.loads(diff_json)
    assert "root_path" in parsed_diff

    # Test FieldChangeEvent.to_json()
    s174a = diff.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
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
    data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml")
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
    assert "source_path" in parsed_amendment

    # Test UscReference.to_json()
    ref = amendment.target_paths[0]
    ref_json = ref.to_json()
    assert isinstance(ref_json, str)
    parsed_ref = json.loads(ref_json)
    assert "path" in parsed_ref
    assert "display_text" in parsed_ref


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
    old = parse_uslm_xml('tests/test_data/usc/2025-07-18/usc26.xml', '2025-07-18')
    new = parse_uslm_xml('tests/test_data/usc/2025-07-30/usc26.xml', '2025-07-30')
    diff = compute_diff(old, new)
    diff_json = diff.to_json()
    restored_diff = TreeDiff.from_json(diff_json)
    assert isinstance(restored_diff, TreeDiff)
    assert restored_diff.root_path == diff.root_path
    assert len(restored_diff.child_diffs) == len(diff.child_diffs)

    # Test FieldChangeEvent round-trip
    s174a = diff.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a")
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
    data = parse_bill_amendments("tests/test_data/bills/hr-119-21.xml")
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
    assert restored_amendment.source_path == amendment.source_path
    assert restored_amendment.action_types == amendment.action_types

    # Test UscReference round-trip
    ref = amendment.target_paths[0]
    ref_json = ref.to_json()
    restored_ref = UscReference.from_json(ref_json)
    assert isinstance(restored_ref, UscReference)
    assert restored_ref.path == ref.path
    assert restored_ref.display_text == ref.display_text
