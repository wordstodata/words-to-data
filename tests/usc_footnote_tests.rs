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
//!
//! Now the marks and the footnote text go out of the five fields of words. The
//! footnote text goes in a sixth field, `footnotes`, that search reads and the
//! diff does not compare.

use std::sync::OnceLock;

use rstest::rstest;
use words_to_data::dataset::{Dataset, DatasetMetadata, Format};
use words_to_data::diff::TreeDiff;

use words_to_data::document::{DocumentNode, TextContentField};
use words_to_data::uslm::UslmFacts;
use words_to_data::uslm::parser::parse;

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

/// The node whose path ends in `segment`, wherever it is in the tree.
fn section_named<'a>(node: &'a DocumentNode, segment: &str) -> Option<&'a DocumentNode> {
    if node.data.path.ends_with(&format!("/{segment}")) {
        return Some(node);
    }
    node.children
        .iter()
        .find_map(|child| section_named(child, segment))
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

/// The footnote is not lost: it is in the provision's `footnotes` field, so a
/// search for "So in original" still finds it, and names the provision.
#[test]
fn should_hold_the_footnote_text_in_the_footnotes_field_when_the_code_prints_one() {
    let subclause = title_26("2025-07-18")
        .find(S7701_B_5_A_IV_II)
        .expect("7701(b)(5)(A)(iv)(II) should be held");

    assert_eq!(
        text(subclause, TextContentField::Footnotes),
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

/// A footnote that the OLRC adds is not a change to the law either. 26 U.S.C.
/// 151(d)(3)(A) has no footnote on 2025-07-18, and on 2025-07-30 the OLRC
/// adds "See References in Text note below." after "section 68(b),". The
/// `footnotes` field changes, and the diff does not compare it.
#[test]
fn should_report_no_change_when_a_printing_only_adds_a_footnote() {
    const S151_D_3_A: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_V/section_151/subsection_d/paragraph_3/subparagraph_A";
    let before = title_26("2025-07-18")
        .find(S151_D_3_A)
        .expect("held before");
    let after = title_26("2025-07-30").find(S151_D_3_A).expect("held after");
    assert_eq!(text(before, TextContentField::Footnotes), "");
    assert_eq!(
        text(after, TextContentField::Footnotes),
        "See References in Text note below."
    );

    let diff = TreeDiff::from_nodes(title_26("2025-07-18"), title_26("2025-07-30"));

    assert!(
        diff.find(S151_D_3_A).is_none(),
        "{S151_D_3_A} should not be changed, got {:#?}",
        diff.find(S151_D_3_A).map(|d| &d.changes)
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
    assert_eq!(text(subclause, TextContentField::Footnotes), "");
}

/// A second mark for a footnote has no footnote text beside it: the OLRC
/// prints the text once, at the first mark, earlier in the section. The
/// provision with the mark holds that text, so each provision the footnote is
/// about says so, and a search finds each of them.
///
/// Section 1016 prints footnote 2, "So in original. The comma probably should
/// be a semicolon.", at (a)(22), and marks (a)(23) to (a)(37) with a bare
/// `<sup>2</sup>`.
#[test]
fn should_give_a_mark_the_footnote_text_printed_at_the_first_mark_when_the_mark_has_none_beside_it()
{
    let paragraph = title_26("2025-07-18")
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_O/part_II/section_1016/subsection_a/paragraph_26")
        .expect("1016(a)(26) should be held");

    assert_eq!(
        text(paragraph, TextContentField::Content),
        " to the extent provided in sections 23(g) and 137(e),"
    );
    assert_eq!(
        text(paragraph, TextContentField::Footnotes),
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

/// The issue's example, 26 U.S.C. 164(b)(6): the footnote is in the chapeau,
/// and the chapeau reads as the law does.
#[test]
fn should_take_a_footnote_out_of_a_chapeau_when_the_code_prints_one_there() {
    let paragraph = title_26("2025-07-30")
        .find("uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_164/subsection_b/paragraph_6")
        .expect("164(b)(6) should be held");

    assert_eq!(
        text(paragraph, TextContentField::Chapeau),
        "In the case of an individual and a taxable year beginning after December 31, 2017,\u{2014}"
    );
    assert_eq!(
        text(paragraph, TextContentField::Footnotes),
        "So in original. The comma probably should not appear."
    );
}

/// A footnote can sit in the number of a section: "§ 9815.¹ So in original. No
/// section 9814 has been enacted." The number reads "§ 9815.", with the narrow
/// space the Code prints after "§", and the footnote is the section's.
#[test]
fn should_take_a_footnote_out_of_the_number_and_give_it_to_the_element() {
    let section =
        section_named(title_26("2025-07-18"), "section_9815").expect("§ 9815 should be held");
    let facts = UslmFacts::of(&section.data).expect("a section carries USLM facts");

    assert_eq!(facts.number_display, "§\u{202f}9815.");
    assert!(
        text(section, TextContentField::Footnotes).starts_with("So in original. No section 9814"),
        "got {:?}",
        text(section, TextContentField::Footnotes)
    );
}

/// Both stored forms keep the field: a dataset written to SQLite or to the
/// compact JSON and read again holds the footnote where the parser put it.
#[test]
fn should_keep_the_footnotes_field_when_a_dataset_is_written_and_read_again() {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "Footnotes".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-18/usc26.xml",
            "2025-07-18",
            None,
        )
        .expect("title 26 should parse");
    let dir = tempfile::tempdir().expect("a temporary directory");
    let sqlite_path = dir.path().join("footnotes.sqlite");
    let json_path = dir.path().join("footnotes.json");
    dataset
        .save_to_sqlite(&sqlite_path)
        .expect("save to SQLite");
    dataset
        .save(json_path.to_str().unwrap(), Format::Compact)
        .expect("save to compact JSON");

    let from_sqlite = Dataset::open_sqlite(&sqlite_path).expect("open SQLite");
    let from_json =
        Dataset::load(json_path.to_str().unwrap(), Format::Compact).expect("load compact JSON");

    for (form, nodes) in [
        ("SQLite", from_sqlite.find_nodes(S7701_B_5_A_IV_II)),
        ("compact JSON", from_json.find_nodes(S7701_B_5_A_IV_II)),
    ] {
        let nodes = nodes.expect("the path should be read");
        let (_, subclause) = nodes.first().expect("the subclause should be held");
        assert_eq!(
            text(subclause, TextContentField::Footnotes),
            "So in original. The comma probably should not appear.",
            "the {form} form should keep the footnote"
        );
    }
}
