use words_to_data::uslm::{TextContentField, parser::parse};

// Test 1: Parse heading field from real USC title
#[test]
fn test_parse_heading_field() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Navigate to title element
    let title = root
        .find("uscodedocument_9/title_9")
        .expect("Failed to find title element");

    // Verify heading exists and matches expected value
    assert!(title.data.heading.is_some(), "Title should have heading");
    assert_eq!(
        title
            .data
            .get_text_content(TextContentField::Heading)
            .unwrap(),
        "ARBITRATION"
    );
}

// Test 2: Parse content field from real USC paragraph
#[test]
fn test_parse_content_field() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Navigate to a paragraph which has actual content text
    let paragraph = root
        .find("uscodedocument_9/title_9/chapter_1/section_10/subsection_a/paragraph_1")
        .expect("Failed to find paragraph element");

    // Verify content field exists and contains text
    assert!(
        paragraph.data.content.is_some(),
        "Paragraph should have content"
    );
    let content_text = paragraph
        .data
        .get_text_content(TextContentField::Content)
        .unwrap();
    assert!(
        !content_text.trim().is_empty(),
        "Content should not be empty"
    );
    assert!(
        content_text.contains("award"),
        "Content should contain expected text"
    );
}

// Test 3: Parse chapeau field from real USC subsection
#[test]
fn test_parse_chapeau_field() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Navigate to section 10 subsection a which has chapeau
    let subsection = root
        .find("uscodedocument_9/title_9/chapter_1/section_10/subsection_a")
        .expect("Failed to find subsection with chapeau");

    // Verify chapeau field exists
    assert!(
        subsection.data.chapeau.is_some(),
        "Subsection should have chapeau"
    );
    let chapeau_text = subsection
        .data
        .get_text_content(TextContentField::Chapeau)
        .unwrap();
    assert!(
        chapeau_text.contains("United States court"),
        "Chapeau should contain expected text"
    );
}

// Test 4: Parse mixed text fields (heading and content)
#[test]
fn test_parse_mixed_text_fields() {
    let root = parse("tests/test_data/usc/2025-07-18/usc01.xml", "2025-07-18")
        .expect("Failed to parse usc01.xml");

    // Section 1 has both heading and content
    let section = root
        .find("uscodedocument_1/title_1/chapter_1/section_1")
        .expect("Failed to find section element");

    // Verify both fields exist
    assert!(
        section.data.heading.is_some(),
        "Section should have heading"
    );
    assert!(
        section.data.content.is_some(),
        "Section should have content"
    );

    // Verify we can retrieve both
    assert!(
        section
            .data
            .get_text_content(TextContentField::Heading)
            .is_some()
    );
    assert!(
        section
            .data
            .get_text_content(TextContentField::Content)
            .is_some()
    );

    // Verify proviso and continuation are None
    assert!(
        section
            .data
            .get_text_content(TextContentField::Proviso)
            .is_none()
    );
    assert!(
        section
            .data
            .get_text_content(TextContentField::Continuation)
            .is_none()
    );
}

// Test 5: Parse element with minimal text fields
#[test]
fn test_parse_no_optional_fields() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Find a paragraph element which typically has only content
    let paragraph = root
        .find("uscodedocument_9/title_9/chapter_1/section_10/subsection_a/paragraph_1")
        .expect("Failed to find paragraph element");

    // Paragraph should have content but not chapeau, proviso, or continuation
    assert!(
        paragraph.data.content.is_some(),
        "Paragraph should have content"
    );
    assert!(
        paragraph.data.chapeau.is_none(),
        "Paragraph should not have chapeau"
    );
    assert!(
        paragraph.data.proviso.is_none(),
        "Paragraph should not have proviso"
    );
    assert!(
        paragraph.data.continuation.is_none(),
        "Paragraph should not have continuation"
    );
}

// Test 6: Verify special characters (§) are preserved in heading
#[test]
fn test_parse_special_characters_in_text() {
    let root = parse("tests/test_data/usc/2025-07-18/usc01.xml", "2025-07-18")
        .expect("Failed to parse usc01.xml");

    // Section 1 has "§ 1." in its number display
    let section = root
        .find("uscodedocument_1/title_1/chapter_1/section_1")
        .expect("Failed to find section element");

    // The number_display field should contain the § symbol
    assert!(
        section.data.number_display.contains("§"),
        "Number display should contain § symbol"
    );
}

// Test 7: Parse heading from chapter element
#[test]
fn test_parse_chapter_heading() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Navigate to chapter 1
    let chapter = root
        .find("uscodedocument_9/title_9/chapter_1")
        .expect("Failed to find chapter element");

    // Verify chapter has heading
    assert!(
        chapter.data.heading.is_some(),
        "Chapter should have heading"
    );
    assert_eq!(
        chapter
            .data
            .get_text_content(TextContentField::Heading)
            .unwrap(),
        "GENERAL PROVISIONS"
    );
}

// Test 8: Verify all text field accessors work
#[test]
fn test_all_text_field_accessors() {
    let root = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");

    // Get subsection with chapeau
    let subsection = root
        .find("uscodedocument_9/title_9/chapter_1/section_10/subsection_a")
        .expect("Failed to find subsection");

    // Test all accessor methods exist and work
    let _heading = subsection.data.get_text_content(TextContentField::Heading);
    let _chapeau = subsection.data.get_text_content(TextContentField::Chapeau);
    let _proviso = subsection.data.get_text_content(TextContentField::Proviso);
    let _content = subsection.data.get_text_content(TextContentField::Content);
    let _continuation = subsection
        .data
        .get_text_content(TextContentField::Continuation);

    // Verify chapeau is present (we know this one should be)
    assert!(_chapeau.is_some(), "Subsection a should have chapeau");
}
