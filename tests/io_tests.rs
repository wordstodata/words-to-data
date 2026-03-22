use words_to_data::io::load_xml_file;

#[test]
fn test_load_xml_file_success() {
    let result = load_xml_file("tests/test_data/usc/2025-07-18/usc07.xml");
    assert!(
        result.is_ok(),
        "Failed to load XML file: {:?}",
        result.err()
    );

    let content = result.unwrap();
    assert!(
        content.contains("uscDoc"),
        "XML content should contain uscDoc element"
    );
}

#[test]
fn test_load_xml_file_nonexistent() {
    let result = load_xml_file("nonexistent_file.xml");
    assert!(result.is_err(), "Should fail for nonexistent file");
}
