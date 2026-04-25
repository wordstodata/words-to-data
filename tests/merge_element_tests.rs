use words_to_data::uslm::parser::parse;

#[test]
fn test_merge_elements() {
    let title_9 = parse("tests/test_data/usc/2025-07-18/usc09.xml", "2025-07-18")
        .expect("Failed to parse usc09.xml");
    let mut title_26 = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Failed to parse usc26.xml");

    assert_eq!(title_9.children.len(), 1);
    assert_eq!(title_26.children.len(), 1);

    title_26.merge_children(&mut title_9.clone());
    assert_eq!(title_26.children.len(), 2);
}
