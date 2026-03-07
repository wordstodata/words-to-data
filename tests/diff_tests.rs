use words_to_data::{
    diff::TreeDiff,
    uslm::{TextContentField, parser::parse},
};

#[test]
fn test_diff_generation() {
    let doc_old = parse("tests/test_data/usc/2025-07-18/usc26.xml", "2025-07-18")
        .expect("Error running parser");
    let doc_new = parse("tests/test_data/usc/2025-07-30/usc26.xml", "2025-07-30")
        .expect("Error running parser");

    let diff = TreeDiff::from_elements(&doc_old, &doc_new);

    let s174a_diff = diff.find("uscodedocument_26/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174/subsection_a").expect("Section 174A has no changes, nor does its children!");
    let change = s174a_diff
        .changes
        .first()
        .expect("Change should be detected on Section 174(A)");
    assert_eq!(change.field_name, TextContentField::Chapeau);
    assert_eq!(change.changes.len(), 2);
    assert_eq!(
        change.old_value,
        "In the case of a taxpayer’s specified research or experimental expenditures for any taxable year—"
    );
    assert_eq!(
        change.new_value,
        "In the case of a taxpayer’s foreign research or experimental expenditures for any taxable year—"
    );
}
