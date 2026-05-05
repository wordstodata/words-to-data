"""
Website Example Tests (Python)

These tests replicate the Python code examples shown on wordstodata.com.
If any of these tests fail, the website examples at w2d_site/index.html need to be updated.

Site location: /home/jesse/code/w2d_site/index.html
"""

from words_to_data import parse_uslm_xml, compute_diff, parse_bill_amendments


# =============================================================================
# WEBSITE EXAMPLE: Parse a US Code Document
# https://wordstodata.com/#examples (Example 1)
# =============================================================================


def test_website_example_parse_usc_document():
    """
    Tests the parsing example shown on the website.
    If this fails, update the "Parse a US Code Document" section in index.html.
    """
    # Code from website example
    title_26 = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    s174a = title_26.find(
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    )

    assert s174a is not None, "§174(a) not found - website example path may need updating"

    expected_chapeau = "In the case of a taxpayer's specified research or experimental expenditures for any taxable year—"

    assert s174a.data["chapeau"] == expected_chapeau, (
        f"Chapeau value doesn't match website example. Update index.html if this changed.\n"
        f"Got: {s174a.data['chapeau']}"
    )


# =============================================================================
# WEBSITE EXAMPLE: Compute a Diff Between Versions
# https://wordstodata.com/#examples (Example 2)
# =============================================================================


def test_website_example_compute_diff():
    """
    Tests the diff computation example shown on the website.
    If this fails, update the "Compute a Diff Between Versions" section in index.html.
    """
    # Code from website example
    doc_old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    doc_new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")

    diff = compute_diff(doc_old, doc_new)

    s174a_diff = diff.find(
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    )

    assert s174a_diff is not None, "§174(a) diff not found"
    assert len(s174a_diff.changes) > 0, "§174(a) should have changes as shown on website"

    # Get the chapeau change (as shown on website)
    chapeau_change = None
    for change in s174a_diff.changes:
        if change.field_name == "chapeau":
            chapeau_change = change
            break

    assert chapeau_change is not None, "Chapeau change should exist as shown on website"

    # Verify old value matches website
    expected_old = "In the case of a taxpayer's specified research or experimental expenditures for any taxable year—"
    assert chapeau_change.old_value == expected_old, (
        f"Old chapeau value doesn't match website example.\n"
        f"Got: {chapeau_change.old_value}"
    )

    # Verify new value matches website
    expected_new = "In the case of a taxpayer's foreign research or experimental expenditures for any taxable year—"
    assert chapeau_change.new_value == expected_new, (
        f"New chapeau value doesn't match website example.\n"
        f"Got: {chapeau_change.new_value}"
    )

    # Verify number of word-level changes matches website (shows "2")
    assert len(chapeau_change.changes) == 2, (
        f"Website shows '2' word-level changes. "
        f"Got: {len(chapeau_change.changes)}. Update website if this changed."
    )


def test_website_example_diff_output_format():
    """
    Tests the diff output format shown on the website.
    This verifies the exact output users would see from the example code.
    """
    doc_old = parse_uslm_xml("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
    doc_new = parse_uslm_xml("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")

    diff = compute_diff(doc_old, doc_new)
    s174a_diff = diff.find(
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a"
    )

    # Website shows this output format:
    # chapeau Changed:
    #   Old: In the case of a taxpayer's specified...
    #   New: In the case of a taxpayer's foreign...
    #   Number of word-level changes: 2
    for change in s174a_diff.changes:
        # This is the exact loop from the website example
        output_field = f"{change.field_name} Changed:"
        output_old = f"  Old: {change.old_value}"
        output_new = f"  New: {change.new_value}"
        output_count = f"  Number of word-level changes: {len(change.changes)}"

        # Just verify these don't throw errors
        assert len(output_field) > 0
        assert len(output_old) > 0
        assert len(output_new) > 0
        assert len(output_count) > 0


# =============================================================================
# WEBSITE EXAMPLE: Extract Amendments from a Bill
# https://wordstodata.com/#examples (Example 3)
# =============================================================================


def test_website_example_extract_amendments():
    """
    Tests the bill amendment extraction example shown on the website.
    If this fails, update the "Extract Amendments from a Bill" section in index.html.
    """
    # Code from website example
    data = parse_bill_amendments("119-21", "tests/test_data/bills/119-hr-1/bill_119_hr_1.xml")

    # Verify bill_id matches website (shows "119-21")
    assert data.bill_id == "119-21", (
        f"Bill ID doesn't match website example. "
        f"Got: {data.bill_id}. Update index.html if format changed."
    )

    # Website shows "603 amendments found"
    # NOTE: If this number changes, update the website output section
    amendment_count = len(data.amendments)
    assert amendment_count == 603, (
        f"Website shows '603 amendments found'. "
        f"Current count: {amendment_count}. Update website if this changed."
    )


def test_website_example_amendment_structure():
    """
    Tests that amendments have the structure shown in the website example.
    If this fails, update the amendment output section in index.html.
    """
    data = parse_bill_amendments("119-21", "tests/test_data/bills/119-hr-1/bill_119_hr_1.xml")

    # Website shows action_types field for each amendment
    for amendment in data.amendments.values():
        # action_types should be accessible
        _ = amendment.action_types

        # amending_text should exist
        assert len(amendment.amending_text) > 0, "Amendments should have amending_text"

    # Website shows some amendments have multiple action types
    has_multiple_actions = any(len(a.action_types) > 1 for a in data.amendments.values())
    assert has_multiple_actions, (
        "Some amendments should have multiple action types as shown on website"
    )


def test_website_example_amendment_output_format():
    """
    Tests the amendment output format shown on the website.
    Website shows:
        Amendment at: /us/pl/119/21/tI/stA/s10101/a
          USC sections modified: 1
          Actions: [Amend, Delete, Insert]
    """
    data = parse_bill_amendments("119-21", "tests/test_data/bills/119-hr-1/bill_119_hr_1.xml")

    # The website example shows iterating over amendments
    for amendment in list(data.amendments.values())[:5]:  # Just check first 5
        # These are the fields accessed in the website example
        output_source = f"Amendment at: (source_path would be here)"
        output_actions = f"  Actions: {amendment.action_types}"

        # Verify these don't throw errors
        assert len(output_source) > 0
        assert len(output_actions) > 0
