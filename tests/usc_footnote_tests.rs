//! The OLRC's footnotes are not the words of the law (#416).
//!
//! The Office of the Law Revision Counsel prints editorial footnotes in the
//! Code: "So in original. The comma probably should not appear.", "See
//! References in Text note below.". USLM puts the mark and the footnote text
//! inside the provision's own text elements, so the parser stored them in the
//! provision's words: 26 U.S.C. 164(b)(6) read "…December 31, 2017,22 So in
//! original. The comma probably should not appear.—".
//!
//! The OLRC numbers its footnotes again in each printing. A footnote that only
//! got a new number then made `diff` report a change to the law, and `coverage`
//! put it in the work queue.

use std::sync::OnceLock;

use rstest::rstest;
use words_to_data::diff::TreeDiff;
use words_to_data::document::{DocumentNode, TextContentField};
use words_to_data::uslm::parser::parse;
use words_to_data::uslm::{FootnoteMark, UslmFacts};

/// 26 U.S.C. 7701(b)(5)(A)(iv)(II), which ends in a comma that the OLRC
/// footnotes: "So in original. The comma probably should not appear."
const S7701_B_5_A_IV_II: &str = "uscode/title_26/subtitle_F/chapter_79/section_7701/subsection_b/paragraph_5/subparagraph_A/clause_iv/subclause_II";

fn title_26(date: &'static str) -> &'static DocumentNode {
    static JULY_18: OnceLock<DocumentNode> = OnceLock::new();
    static JULY_30: OnceLock<DocumentNode> = OnceLock::new();
    let cell = match date {
        "2025-07-18" => &JULY_18,
        "2025-07-30" => &JULY_30,
        other => panic!("no title 26 fixture for {other}"),
    };
    cell.get_or_init(|| {
        parse(&format!("tests/test_data/usc/{date}/usc26.xml"), date)
            .expect("title 26 should parse")
    })
}

fn text(node: &DocumentNode, field: TextContentField) -> String {
    node.data
        .get_text_content(field)
        .map(|t| t.to_string())
        .unwrap_or_default()
}

#[test]
fn should_keep_a_footnote_out_of_the_words_of_a_provision_when_the_code_prints_one() {
    let subclause = title_26("2025-07-18")
        .find(S7701_B_5_A_IV_II)
        .expect("7701(b)(5)(A)(iv)(II) should be held");

    assert_eq!(
        text(subclause, TextContentField::Content),
        " all of the net proceeds of which are contributed to such organization, and,"
    );
}

/// The footnote is not lost: it is a node of its own under the provision it
/// annotates, so a search for "So in original" still finds it, and names the
/// provision in its path.
#[test]
fn should_hold_the_footnote_text_in_a_footnote_node_under_the_provision_when_the_code_prints_one() {
    let footnote = title_26("2025-07-18")
        .find(&format!("{S7701_B_5_A_IV_II}/footnote_1"))
        .expect("the footnote should be a node under the subclause");

    assert_eq!(footnote.data.node_type.as_str(), "uscode.footnote");
    assert_eq!(
        text(footnote, TextContentField::Content),
        "So in original. The comma probably should not appear."
    );
}

/// Two printings that differ only in the OLRC's number for a footnote say the
/// same thing, so the diff reports no change there.
///
/// 7701(b)(5)(A)(iv)(II) has the footnote itself, numbered 1 and then 2.
/// 1016(a)(26) has only a mark, `<sup>2</sup>` and then `<sup>1</sup>`, that
/// points at a footnote printed earlier in the section.
#[rstest]
#[case::footnote_text_renumbered(S7701_B_5_A_IV_II)]
#[case::footnote_mark_renumbered(
    "uscode/title_26/subtitle_A/chapter_1/subchapter_O/part_II/section_1016/subsection_a/paragraph_26"
)]
fn should_report_no_change_when_a_printing_only_renumbers_a_footnote(#[case] path: &str) {
    let diff = TreeDiff::from_nodes(title_26("2025-07-18"), title_26("2025-07-30"));

    assert!(
        diff.find(path).is_none(),
        "{path} should not be changed, got {:#?}",
        diff.find(path).map(|d| &d.changes)
    );
}

/// A `<sup>` also holds the words of the law: the top half of a fraction. It
/// is not a footnote mark, so it stays in the words.
#[test]
fn should_keep_the_top_of_a_fraction_in_the_words_when_it_is_raised() {
    let subclause = title_26("2025-07-18")
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_C/section_36B/subsection_c/paragraph_4/subparagraph_C/clause_i/subclause_II")
        .expect("36B(c)(4)(C)(i)(II) should be held");

    assert!(
        text(subclause, TextContentField::Content).starts_with(" 1\u{2044}12 of the employee's"),
        "got {:?}",
        text(subclause, TextContentField::Content)
    );
}

/// A second mark for a footnote has no footnote text beside it: the OLRC
/// prints the text once, at the first mark, earlier in the section. The node
/// for the mark holds that text, so each provision the footnote is about says
/// so, and a search finds each of them.
///
/// Section 1016 prints footnote 2, "So in original. The comma probably should
/// be a semicolon.", at (a)(22), and marks (a)(23) to (a)(37) with a bare
/// `<sup>2</sup>`.
#[test]
fn should_give_a_mark_the_footnote_text_printed_at_the_first_mark_when_the_mark_has_none_beside_it()
{
    let footnote = title_26("2025-07-18")
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_O/part_II/section_1016/subsection_a/paragraph_26/footnote_1")
        .expect("the mark in 1016(a)(26) should be a footnote node");

    assert_eq!(
        text(footnote, TextContentField::Content),
        "So in original. The comma probably should be a semicolon."
    );
}

/// The OLRC sets a mark apart from the word before it with a narrow no-break
/// space (U+202F). The space belongs to the mark and goes with it, so the
/// words read "for whom a American", with the one space the Code prints after
/// the footnote. The footnote says "So in original. Probably should be \"an\"."
#[test]
fn should_take_the_narrow_space_before_a_mark_out_with_the_mark() {
    let subparagraph = title_26("2025-07-18")
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_A/section_25A/subsection_c/paragraph_2/subparagraph_A")
        .expect("25A(c)(2)(A) should be held");

    let content = text(subparagraph, TextContentField::Content);
    assert!(
        content.contains("an eligible student for whom a American Opportunity Tax Credit"),
        "got {content:?}"
    );
}

/// The node for a footnote says where its mark was, so a reader can tell which
/// words it is about: "The comma probably should not appear" is about the comma
/// just before the mark. This is the issue's example, 26 U.S.C. 164(b)(6).
#[test]
fn should_say_where_the_mark_was_in_the_words_when_a_footnote_is_taken_out() {
    let paragraph = title_26("2025-07-30")
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_164/subsection_b/paragraph_6")
        .expect("164(b)(6) should be held");
    let before_the_mark =
        "In the case of an individual and a taxable year beginning after December 31, 2017,";
    assert_eq!(
        text(paragraph, TextContentField::Chapeau),
        format!("{before_the_mark}\u{2014}")
    );

    let footnote = paragraph
        .children
        .iter()
        .find(|child| child.data.path.ends_with("/footnote_1"))
        .expect("164(b)(6) should hold its footnote");
    let facts = UslmFacts::of(&footnote.data).expect("a footnote node carries USLM facts");
    assert_eq!(
        facts.footnote_mark,
        Some(FootnoteMark {
            field: "chapeau".to_string(),
            at: before_the_mark.chars().count(),
        })
    );
    assert_eq!(
        text(footnote, TextContentField::Content),
        "So in original. The comma probably should not appear."
    );
}
