//! A USLM `<footnote>` is a footnote, as `<note type="footnote">` is (#486).
//!
//! Pub. L. 119-60 (`119-s-1071`) prints the classification notes of the OLRC as
//! USLM `<footnote>` elements, in the `<num>` of the provision they classify:
//!
//! ```xml
//! <num value="1">  <ref idref="fn-d2717e98748">*</ref> <footnote
//!   id="fn-d2717e98748"><num>* </num>Note: Classifications for SEC.
//!   3111(b)(1): …</footnote> (1) </num>
//! ```
//!
//! The footnote reader knew only `<note type="footnote">` and
//! `class="footnoteRef"`. So the mark and the footnote text went into the
//! number: § 3111(b)(1) read "* * Note: Classifications for SEC. 3111(b)(1): 50
//! USC 2501…" as its `number_display`.

use std::sync::OnceLock;

use words_to_data::document::{DocumentNode, TextContentField};
use words_to_data::uslm::UslmFacts;
use words_to_data::uslm::bill_parser::bill_expression;

const LAW: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";

const S3111_B_1: &str = "division_C/title_XXXI/subtitle_B/section_3111/subsection_b/paragraph_1";

fn law() -> &'static DocumentNode {
    static LAW_ROOT: OnceLock<DocumentNode> = OnceLock::new();
    LAW_ROOT.get_or_init(|| {
        let xml = std::fs::read_to_string(LAW).expect("the law should read");
        let document = roxmltree::Document::parse(&xml).expect("the law should parse");
        let (law, _) = bill_expression(&document, "119-s-1071").expect("the law should read");
        law.root
    })
}

/// The node whose path ends in `suffix`, wherever it is in the tree.
fn node_at<'a>(node: &'a DocumentNode, suffix: &str) -> Option<&'a DocumentNode> {
    if node.data.path.ends_with(&format!("/{suffix}")) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| node_at(child, suffix))
}

#[test]
fn should_read_only_the_number_when_a_uslm_footnote_is_in_the_num() {
    let paragraph = node_at(law(), S3111_B_1).expect("§ 3111(b)(1) should be held");
    let facts = UslmFacts::of(&paragraph.data).expect("a paragraph carries USLM facts");

    assert_eq!(facts.number_display, "(1) ");
}

#[test]
fn should_hold_the_text_of_a_uslm_footnote_in_the_footnotes_field() {
    let paragraph = node_at(law(), S3111_B_1).expect("§ 3111(b)(1) should be held");
    let footnotes = paragraph
        .data
        .get_text_content(TextContentField::Footnotes)
        .map(|text| text.to_string())
        .unwrap_or_default();

    assert!(
        footnotes.starts_with("Note: Classifications for SEC. 3111(b)(1): 50 USC 2501 and note"),
        "got {footnotes:?}"
    );
}
