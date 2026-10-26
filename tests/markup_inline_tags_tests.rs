//! The words of a marked-up opinion where an inline tag stands inside a line
//! (#412).
//!
//! A block tag, such as `<p>`, ends a line. An inline tag, such as
//! `<extracted-citation>` or `<em>`, marks some words inside a line and ends
//! nothing. Each record here is a real CourtListener response committed under
//! `tests/test_data/courtlistener`.

use words_to_data::courtlistener::markup::text_of_markup;
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::Expression;
use words_to_data::document::DocumentNode;

const CACHE: &str = "tests/test_data/courtlistener";

/// One committed record, as the API returned it.
fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{CACHE}/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

/// A case, built from its cluster and the opinion records given.
fn case(cluster_id: u64, opinion_ids: &[u64]) -> Expression {
    let cluster = ClusterRecord::from_json(&read(&format!("cluster_{cluster_id}.json")))
        .expect("the cluster should read");
    let opinions: Vec<OpinionRecord> = opinion_ids
        .iter()
        .map(|id| {
            OpinionRecord::from_json(&read(&format!("opinion_{id}.json")))
                .expect("the opinion should read")
        })
        .collect();
    case_expression(&cluster, &opinions)
        .expect("the case should build")
        .expression
}

/// *United States v. Windsor*, 570 U.S. 744 (2013), from its two split records.
fn windsor() -> Expression {
    case(945737, &[9515913, 9515914])
}

/// The words of the one passage that holds `needle`.
fn passage_holding(expression: &Expression, needle: &str) -> String {
    fn walk<'a>(node: &'a DocumentNode, needle: &str, into: &mut Vec<&'a str>) {
        if let Some(words) = node.data.content.as_deref()
            && words.contains(needle)
        {
            into.push(words);
        }
        for child in &node.children {
            walk(child, needle, into);
        }
    }
    let mut found = Vec::new();
    walk(&expression.root, needle, &mut found);
    match found.as_slice() {
        [words] => words.to_string(),
        other => panic!("one passage should hold {needle:?}, {} do", other.len()),
    }
}

#[test]
fn should_keep_a_citation_and_its_subdivision_together_when_a_citation_tag_ends_between_them() {
    // Harvard writes `5 U.S.C. §§ 8901</extracted-citation>(5), 8905`. The
    // tag ends inside the line, so the subdivision `(5)` follows the section
    // number with no line break between them.
    let case = windsor();
    let words = passage_holding(&case, "5 U.S.C. §§ 8901");

    assert!(
        words.contains("5 U.S.C. §§ 8901(5), 8905."),
        "the citation keeps its subdivision on its line, got {words:?}"
    );
}

#[test]
fn should_keep_italic_words_in_their_line_when_harvard_marks_them_with_em() {
    // Harvard writes `Brief for Federalism Scholars as <em>Amici Curiae</em> 34`.
    // A search for the phrase must find it on one line.
    let case = windsor();
    let words = passage_holding(&case, "Brief for Federalism Scholars");

    assert!(
        words.contains("Brief for Federalism Scholars as Amici Curiae 34."),
        "the italic words stay in their line, got {words:?}"
    );
}

#[test]
fn should_keep_two_words_apart_when_an_inline_tag_is_the_only_thing_between_them() {
    // Justice Scalia's dissent quotes the Court and strikes two words through:
    // `"And it humiliates <s>tens of</s>thousands of children`. The source has
    // no space between `of` and `thousands`. Joined, they would be a word that
    // is not in the opinion.
    let case = windsor();
    let words = passage_holding(&case, "\"And it humiliates");

    assert!(
        words.contains("humiliates tens of thousands of children"),
        "the two words stay apart, got {words:?}"
    );
}

#[test]
fn should_keep_a_page_mark_in_its_line_when_the_page_breaks_inside_a_sentence() {
    // Harvard writes `§§ 101</extracted-citation>(14A), <a class="page-label"
    // data-label="773">*773</a>507(a) (1)(A)`. Page 773 starts inside the
    // sentence: the mark stays where the page breaks, as a word of its own,
    // and the sentence stays on one line.
    let case = windsor();
    let words = passage_holding(&case, "11 U.S.C. §§ 101");

    assert!(
        words.contains("11 U.S.C. §§ 101(14A), *773 507(a) (1)(A), 523(a)(5)"),
        "the page mark stands in the line, got {words:?}"
    );
}

#[test]
fn should_keep_a_footnote_mark_apart_from_the_section_number_before_it() {
    // *Hildebrand v. Commissioner* (10th Cir. 1994): Harvard writes
    // `26 U.S.C. § 183<footnotemark>1</footnotemark> of deductions`. The mark
    // is a word of its own. Joined to the number, it would cite § 1831, which
    // the court did not cite.
    let case = case(7029371, &[6931314]);
    let words = passage_holding(&case, "disallowance under 26 U.S.C.");

    assert!(
        words.contains("disallowance under 26 U.S.C. § 183 1 of deductions"),
        "the footnote mark is a word of its own in the line, got {words:?}"
    );
}

#[test]
fn should_keep_a_page_number_in_its_line_when_cap_marks_the_page_inside_a_sentence() {
    // CAP's own XML writes `appeal the <page-number label="1026">*1026</page-number>Tax
    // Court’s`. The mark is a word of its own, in its line.
    let case = case(7029371, &[6931314]);
    let words = passage_holding(&case, "disallowance under 26 U.S.C.");

    assert!(
        words.contains("appeal the *1026 Tax Court’s disallowance"),
        "the page mark stands in the line, got {words:?}"
    );
}

/// *Wallace v. City of Fresno* (Cal. 1954), from the Columbia archive.
fn wallace() -> Expression {
    case(1197312, &[9943788, 9943789])
}

#[test]
fn should_keep_a_footnote_mark_in_its_line_when_columbia_writes_it_as_sup() {
    // Columbia writes `changes in the law.<sup><a href="#op0-fn**">**</a></sup>
    // (<em>Kern</em>`. The mark `**` is a word of its own, in its line.
    let case = wallace();
    let words = passage_holding(&case, "subsequent changes in the law.");

    assert!(
        words.contains("subsequent changes in the law. ** (Kern"),
        "the footnote mark stands in the line, got {words:?}"
    );
}

#[test]
fn should_keep_a_citation_in_its_line_when_columbia_marks_it_as_a_cross_reference() {
    // Columbia writes `<cross_reference>29 Cal.2d 848</cross_reference>, 852`:
    // the pin cite `852` follows the citation in the same line.
    let case = wallace();
    let words = passage_holding(&case, "subsequent changes in the law.");

    assert!(
        words.contains("(Kern v. City of Long Beach, 29 Cal.2d 848, 852 et seq. [179 P.2d 799].)"),
        "the citation and its pin cite stay in one line, got {words:?}"
    );
}

#[test]
fn should_keep_a_star_page_mark_in_its_line_when_lawbox_writes_it_as_a_span() {
    // *Boeing*'s combined record reads `html_lawbox`, which writes
    // `currently. See <span class="star-pagination">*443</span> 26 U.S.C. § 174.`
    let record =
        OpinionRecord::from_json(&read("opinion_122262.json")).expect("the opinion should read");
    let (text, source, _) = record.text().expect("the record carries text");

    assert_eq!(source.field, "html_lawbox");
    assert!(
        text.contains("currently. See *443 26 U.S.C. § 174."),
        "the page mark stands in the line"
    );
}

#[test]
fn should_keep_a_footnote_mark_apart_from_the_number_before_it_when_it_is_a_footnote_link() {
    // *Boeing*'s combined record, in its `html` field, writes
    // `such as the 737 or 767.<a class="footnote" href="#fn11">11</a>`.
    // Joined, the mark would make the number `767.11`.
    let record =
        OpinionRecord::from_json(&read("opinion_122262.json")).expect("the opinion should read");
    let raw = record.html.as_deref().expect("the record carries `html`");
    let (text, _) = text_of_markup(raw);

    assert!(
        text.contains("such as the 737 or 767. 11"),
        "the footnote mark is a word of its own"
    );
}
