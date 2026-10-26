//! What the plain-text rule keeps out of the words of an opinion, and what it
//! keeps in them (#413).
//!
//! Every record here is a real CourtListener response committed under
//! `tests/test_data/courtlistener`, and each one is held only as `plain_text`.

use words_to_data::citation::{citations_by_passage, default_code, usc::UscCitation};
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::Expression;
use words_to_data::document::DocumentNode;

const CACHE: &str = "tests/test_data/courtlistener";

/// One committed record, as the API returned it.
fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{CACHE}/{name}"))
        .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
}

fn opinion(id: u64) -> OpinionRecord {
    OpinionRecord::from_json(&read(&format!("opinion_{id}.json"))).expect("the opinion should read")
}

fn cluster(id: u64) -> ClusterRecord {
    ClusterRecord::from_json(&read(&format!("cluster_{id}.json"))).expect("the cluster should read")
}

/// One case built from its cluster and the records given.
fn case_from(cluster_id: u64, opinions: &[OpinionRecord]) -> Expression {
    case_expression(&cluster(cluster_id), opinions)
        .expect("the case should build")
        .expression
}

/// Every node of a case that carries words, as `(path, words)`, in order.
fn passages(case: &Expression) -> Vec<(String, String)> {
    fn collect(node: &DocumentNode, into: &mut Vec<(String, String)>) {
        if let Some(words) = &node.data.content {
            into.push((node.data.path.to_string(), words.to_string()));
        }
        for child in &node.children {
            collect(child, into);
        }
    }
    let mut every = Vec::new();
    collect(&case.root, &mut every);
    every
}

// --- The watermark of a preliminary print ------------------------------------

/// The preliminary print of *Polselli v. IRS* (598 U. S. 432) prints "Page
/// Proof Pending Publication" across each page, and pdftotext sets it on a
/// line of its own in the middle of the words: 17 times in the record. In
/// Justice Jackson's concurrence it falls inside a citation, "26 U. S. C. /
/// Page Proof Pending Publication / § 7609(c)(2)(D)(i)". The watermark is no
/// word of the Court, so it comes out, and the citation reads whole.
#[test]
fn should_leave_out_the_watermark_when_a_preliminary_print_prints_it_on_a_line_of_its_own() {
    let case = case_from(10049688, &[opinion(10516288)]);

    let every = passages(&case);
    for (path, words) in &every {
        assert!(
            !words.contains("Page Proof Pending Publication"),
            "{path} should not hold the watermark: {words}"
        );
    }
    assert!(
        every.iter().any(|(path, words)| path
            .starts_with("judicial/case_10049688/opinion_10516288-2/")
            && words.contains("without notice under 26 U. S. C. § 7609(c)(2)(D)(i). I agree.")),
        "Justice Jackson's citation should read whole"
    );
}

/// Where the watermark crosses the first lines of a paragraph, pdftotext sets
/// its words at the ends of those lines and pushes the words of the paragraph
/// to the right: in *Polselli*, "Page     Proof", " To pursue         Pending",
/// "           unpaid taxes            Publication", "                        and
/// the people who owe them,". The watermark comes out there too, and the
/// pushed lines go on with the paragraph they start, so the paragraph reads
/// whole. In *Moore v. United States* (602 U. S. 572) the watermark crosses
/// the heading "IV", and the heading stays.
#[test]
fn should_leave_out_the_watermark_and_keep_the_paragraph_whole_when_the_watermark_crosses_its_lines()
 {
    let polselli = passages(&case_from(10049688, &[opinion(10516288)]));
    let moore = passages(&case_from(10600069, &[opinion(11066657)]));

    for (path, words) in polselli.iter().chain(&moore) {
        for word in ["Page Proof", "Pending", "Publication"] {
            assert!(
                !words.contains(word),
                "{path} should not hold {word:?}: {words}"
            );
        }
    }
    let starts = |every: &[(String, String)], opening: &str| {
        every.iter().any(|(_, words)| words.starts_with(opening))
    };
    assert!(starts(
        &polselli,
        "To pursue unpaid taxes and the people who owe them, “Congress"
    ));
    assert!(starts(
        &polselli,
        "The statute sets forth three conditions to exempt the IRS from providing notice"
    ));
    assert!(starts(
        &moore,
        "The majority separately concludes that the Moores' claim fails"
    ));
    assert!(
        moore.iter().any(|(_, words)| words == "IV"),
        "the heading IV stays"
    );
}

// --- Stars: a section break, an ellipsis, a footnote mark --------------------

/// The labels of the footnotes of a case, in order.
fn footnote_labels(every: &[(String, String)]) -> Vec<String> {
    every
        .iter()
        .filter_map(|(path, _)| path.rsplit_once("/footnote_"))
        .map(|(_, label)| label.to_string())
        .collect()
}

/// A court sets a line of stars in the middle of the page, between two
/// paragraphs, to break one part of an opinion from the next: "***" in
/// *Mayor and City Council of Baltimore v. USDA* (case 4797751), "*  *  *" in
/// *District of Columbia v. USDA* (case 4736096) and at the end of Justice
/// Jackson's concurrence in *Moore*. The stars are no footnote mark, so the
/// words after them stay in the body. The line is kept as a paragraph of its
/// own, as Harvard's XML keeps "* * *".
#[test]
fn should_keep_the_words_in_the_body_when_a_centred_line_of_stars_breaks_two_paragraphs() {
    let cases = [
        (
            case_from(4797751, &[opinion(4578098)]),
            "***",
            "The procedural flaws in USDA’s notice-and-comment process require vacatur",
        ),
        (
            case_from(4736096, &[opinion(4516443)]),
            "* * *",
            "USDA’s conclusory reasoning and summary dismissal of extensive evidence",
        ),
        (
            case_from(4736096, &[opinion(4516443)]),
            "* * *",
            "As to the waiver-related provisions of the Final Rule, the plaintiffs",
        ),
        (
            case_from(10600069, &[opinion(11066657)]),
            "* * *",
            "I have no doubt that future Congresses will pass, and fu-ture Presidents",
        ),
    ];
    for (case, stars, opening) in cases {
        let every = passages(&case);
        let after = every
            .iter()
            .position(|(_, words)| words.starts_with(opening))
            .unwrap_or_else(|| panic!("a passage should open {opening:?}"));
        let (path, _) = &every[after];
        assert!(path.contains("/paragraph_"), "{path} should be a paragraph");
        assert_eq!(every[after - 1].1, stars, "the stars before {path}");
        assert!(
            footnote_labels(&every)
                .iter()
                .all(|label| !label.starts_with('*')),
            "no footnote of {path}'s case is a line of stars"
        );
    }
}

/// In *Khalaf v. Department of Revenue* (Oregon Tax Court, case 10140651), a
/// quotation in footnote 11 ends with an ellipsis of stars at the start of a
/// line: "for a week or less / * * *.”). Taxpayer misunderstands the law."
/// Stars followed by more stars are an ellipsis, not a footnote mark, so the
/// line goes on with footnote 11.
#[test]
fn should_keep_the_line_in_its_footnote_when_it_opens_with_an_ellipsis_of_stars() {
    let every = passages(&case_from(10140651, &[opinion(10607252)]));

    assert!(
        footnote_labels(&every)
            .iter()
            .all(|label| !label.starts_with('*')),
        "no footnote is labelled with stars: {:?}",
        footnote_labels(&every)
    );
    let (_, eleven) = every
        .iter()
        .find(|(path, _)| path.ends_with("/footnote_11"))
        .expect("footnote 11 should be a node");
    assert!(
        eleven.contains("for a week or less * * *.”). Taxpayer misunderstands the law."),
        "footnote 11 should hold the ellipsis: {eleven}"
    );
}

/// The slip opinion of *Bittner v. United States* (598 U. S. 85) sets the star
/// of the dissent's one footnote against its first word: "*In 2007, for
/// example, he held over $10 million across 61 foreign bank accounts." The
/// star is the footnote's label, and the words after it are its words.
#[test]
fn should_label_the_footnote_with_its_star_when_the_star_touches_the_first_word() {
    let every = passages(&case_from(9380169, &[opinion(9375645)]));

    let (_, words) = every
        .iter()
        .find(|(path, _)| path == "judicial/case_9380169/opinion_9375645-2/footnote_*")
        .unwrap_or_else(|| {
            panic!(
                "the dissent's footnote should be labelled \"*\": {:?}",
                footnote_labels(&every)
            )
        });
    assert!(
        words.starts_with("In 2007, for example, he held over $10 million"),
        "{words}"
    );
}

// --- A footnote mark against a section number --------------------------------

/// The passages of one writing, as `add-opinions` gives them to the citation
/// reader: each paragraph and footnote, by its path, with its words.
fn writing_passages<'a>(every: &'a [(String, String)], writing: &str) -> Vec<(&'a str, &'a str)> {
    every
        .iter()
        .filter(|(path, _)| path.starts_with(&format!("{writing}/")))
        .map(|(path, words)| (path.as_str(), words.as_str()))
        .collect()
}

/// The citations the reader gives to the passage at `path`.
fn citations_in<'a>(passages: &[(&'a str, &'a str)], path: &str) -> Vec<UscCitation> {
    citations_by_passage(passages, None)
        .by_passage
        .into_iter()
        .find(|(passage, _)| *passage == path)
        .map(|(_, citations)| citations)
        .unwrap_or_else(|| panic!("{path} should be a passage"))
}

/// *Bittner* ends a sentence with a citation and sets footnote mark 6 against
/// its full stop: "31 U. S. C. §5311.6 Here we see further evidence". No
/// section of the Code has a full stop in its number, and the opinion of the
/// Court has a footnote 6, so the citation is of § 5311, and the 6 is the mark.
#[test]
fn should_read_the_section_without_the_footnote_mark_when_the_mark_follows_its_full_stop() {
    let every = passages(&case_from(9380169, &[opinion(9375645)]));
    let writing = writing_passages(&every, "judicial/case_9380169/opinion_9375645-1");

    let found = citations_in(
        &writing,
        "judicial/case_9380169/opinion_9375645-1/paragraph_38",
    );
    let cited: Vec<(&str, &[String], &str)> = found
        .iter()
        .map(|citation| {
            (
                citation.title.as_str(),
                citation.sections.as_slice(),
                citation.text.as_str(),
            )
        })
        .collect();
    assert!(
        cited.contains(&("31", &["5311".to_string()][..], "31 U. S. C. §5311")),
        "§ 5311 should be read, got {cited:?}"
    );
}

/// *Khalaf* states that its references to the Internal Revenue Code are to
/// the Code, and ends a paragraph with footnote mark 17 against a reference:
/// "to be expensed under IRC section 179.17". Read under the default code,
/// the reference is to § 179 of title 26, and the 17 is the mark.
#[test]
fn should_read_the_bare_section_without_the_footnote_mark_when_the_mark_follows_its_full_stop() {
    let case =
        case_expression(&cluster(10140651), &[opinion(10607252)]).expect("the case should build");
    let default = default_code::of_case(
        case.writings
            .iter()
            .map(|writing| (writing.path.as_str(), writing.text.as_str())),
    );
    let every = passages(&case.expression);
    let writing = writing_passages(&every, "judicial/case_10140651/opinion_10607252");

    let found = citations_by_passage(&writing, default.as_ref())
        .by_passage
        .into_iter()
        .find(|(path, _)| *path == "judicial/case_10140651/opinion_10607252/paragraph_51")
        .map(|(_, citations)| citations)
        .expect("paragraph 51 should be a passage");
    let cited: Vec<(&str, &[String], &str)> = found
        .iter()
        .map(|citation| {
            (
                citation.title.as_str(),
                citation.sections.as_slice(),
                citation.text.as_str(),
            )
        })
        .collect();
    assert!(
        cited.contains(&("26", &["179".to_string()][..], "IRC section 179")),
        "§ 179 should be read, got {cited:?}"
    );
}

// --- The rule's version ------------------------------------------------------

/// The plain-text rule now finds other paragraphs and footnotes than it found
/// at version 2: the watermark is out of the words, a centred line of stars
/// is no footnote mark, and a star against a word is one. So each passage it
/// reads names version 3.
#[test]
fn should_name_plain_text_paragraphs_at_version_3_when_a_passage_is_read_from_plain_text() {
    let case = case_from(4797751, &[opinion(4578098)]);

    let paragraph = case
        .root
        .find("judicial/case_4797751/opinion_4578098/paragraph_1")
        .expect("the first paragraph should be a node");
    let reasoning = paragraph
        .data
        .provenance
        .as_ref()
        .and_then(|provenance| provenance.evidence.as_ref())
        .and_then(|evidence| evidence.reasoning.as_deref())
        .expect("the paragraph should say what it was based on");
    assert!(
        reasoning.contains(
            "plain-text-paragraphs@3: paragraphs and footnotes read from the layout of the text"
        ),
        "{reasoning}"
    );
}

/// The same paragraph of *Bittner*, read without the footnotes of its
/// writing: no footnote 6 is there, so nothing shows that the 6 is a mark,
/// and "5311.6" is read as it is written. It names no section, so it makes
/// no link.
#[test]
fn should_keep_the_number_as_written_when_the_writing_has_no_footnote_with_that_label() {
    let every = passages(&case_from(9380169, &[opinion(9375645)]));
    let paragraph = "judicial/case_9380169/opinion_9375645-1/paragraph_38";
    let alone: Vec<(&str, &str)> =
        writing_passages(&every, "judicial/case_9380169/opinion_9375645-1")
            .into_iter()
            .filter(|(path, _)| *path == paragraph)
            .collect();

    let found = citations_in(&alone, paragraph);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].sections, ["5311.6"]);
}

// --- Two blank lines on the first page ----------------------------------------

/// *District of Columbia v. USDA* is double spaced, and its caption has two
/// blank lines under "FOR THE DISTRICT OF COLUMBIA". In a double-spaced text
/// two blank lines start the rest of a footnote carried over from the page
/// before. No footnote has started before the caption, so nothing is carried
/// over, and the caption stays in the body.
#[test]
fn should_keep_the_caption_in_the_body_when_two_blank_lines_stand_before_any_footnote() {
    let every = passages(&case_from(4736096, &[opinion(4516443)]));

    assert!(
        !footnote_labels(&every).iter().any(String::is_empty),
        "no footnote has an empty label: {:?}",
        footnote_labels(&every)
    );
    let (path, _) = every
        .iter()
        .find(|(_, words)| words == "DISTRICT OF COLUMBIA, et al.,")
        .expect("the first party of the caption should be a passage");
    assert!(path.contains("/paragraph_"), "{path} should be a paragraph");
}

// --- A list after its lead-in -----------------------------------------------

/// In *Polselli* the Court quotes 26 U. S. C. § 7609(c)(2)(D): the lead-in
/// "“issued in aid of the collection of—" and then the items "“(i) an
/// assessment made…" and "“(ii) the liability…", each with its wrapped
/// lines below it. The watermark stood between the lead-in and item (i).
/// Without it, an item that opens with its designation after a lead-in that
/// ends with a dash starts a paragraph of its own, and its wrapped lines stay
/// with it (#430).
#[test]
fn should_start_a_paragraph_at_a_quoted_list_item_when_the_line_before_ends_with_a_dash() {
    let every = passages(&case_from(10049688, &[opinion(10516288)]));
    let majority: Vec<&str> = every
        .iter()
        .filter(|(path, _)| path.starts_with("judicial/case_10049688/opinion_10516288-1/"))
        .map(|(_, words)| words.as_str())
        .collect();

    let lead_in = majority
        .iter()
        .position(|words| words.starts_with("“issued in aid of the collection of—"))
        .expect("the lead-in should open a paragraph");
    assert_eq!(majority[lead_in], "“issued in aid of the collection of—");
    assert_eq!(
        majority[lead_in + 1],
        "“(i) an assessment made or judgment rendered against the person with respect to whose liability the summons is issued; or"
    );
    assert!(
        majority[lead_in + 2].starts_with("“(ii) the liability at law or in equity"),
        "{}",
        majority[lead_in + 2]
    );
}
