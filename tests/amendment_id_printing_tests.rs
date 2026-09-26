//! An amendment id must be a fact about the words of the amendment.
//!
//! An amendment id is `sha256("{bill_id}:{amending_text}")`, and `amending_text`
//! is gathered from every text node under the instruction. Two printings of one
//! act say the same thing, so they must mint the same ids: a dataset built from
//! the second printing has to hold the links minted against the first.
//!
//! This corpus holds the two printings of `119-hr-1`. They carry the same 603
//! `role="instruction"` elements under the same element ids, and what they
//! differ in is how each publisher laid the file out and how each wrote its
//! quotes and its dashes.
//!
//! #216 collapsed runs of whitespace, which made one file stable against being
//! re-indented. It did not make two printings agree: indentation between
//! elements is itself a text node, so collapsing a run leaves the laid-out file
//! with one space the compact file never had.

use roxmltree::{Document, Node};
use std::collections::{BTreeMap, BTreeSet};
use words_to_data::uslm::bill_parser::get_amendments;

/// The printing `build-dataset` fetches, under the name a dataset gives it.
const COMPACT_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";

/// The same act as the Government Publishing Office lays it out, over many
/// lines and with its em dashes written `--`.
const LAID_OUT_XML: &str = "tests/test_data/bills/hr-119-21.xml";

const BILL_ID: &str = "119-hr-1";

/// Every instruction of one printing, keyed by the element id both printings
/// share, so that the two can be compared amendment by amendment.
///
/// An instruction never holds another instruction in either printing, so
/// [`get_amendments`] over one instruction node answers with that one
/// amendment.
fn ids_by_element(root: &Node) -> BTreeMap<String, String> {
    let mut by_element = BTreeMap::new();
    for node in root
        .descendants()
        .filter(|n| n.attribute("role") == Some("instruction"))
    {
        let element_id = node
            .attribute("id")
            .expect("every instruction of 119-hr-1 carries an element id")
            .to_string();
        let amendments = get_amendments(&node, BILL_ID);
        assert_eq!(
            amendments.len(),
            1,
            "instruction {element_id} should hold one amendment"
        );
        let amendment = amendments.into_values().next().expect("one amendment");
        by_element.insert(element_id, amendment.id);
    }
    by_element
}

/// Every instruction of one printing as the publisher wrote it, keyed by the
/// element id both printings share.
///
/// The markup itself, not the gathered words, so that a test can say which
/// spelling each printing used before it says what the two mint.
fn markup_by_element<'a>(xml: &'a str, root: &Node) -> BTreeMap<String, &'a str> {
    root.descendants()
        .filter(|n| n.attribute("role") == Some("instruction"))
        .map(|node| {
            let element_id = node
                .attribute("id")
                .expect("every instruction of 119-hr-1 carries an element id");
            (element_id.to_string(), &xml[node.range()])
        })
        .collect()
}

/// The same printing with every whitespace character taken out of its words, and
/// nothing else touched.
///
/// Deleting whitespace outright makes the two printings agree too, so something
/// has to say why that fold is wrong. This is that something: it is the real
/// printing, rewritten in the one way the fold must not be blind to. Only the
/// source of a text node is rewritten, so every tag and every attribute of the
/// real file is left as the publisher wrote it and the rewrite still parses.
fn words_run_together(xml: &str, document: &Document) -> String {
    let mut rewritten = String::with_capacity(xml.len());
    let mut copied = 0;
    for text in document.root().descendants().filter(|n| n.is_text()) {
        let range = text.range();
        rewritten.push_str(&xml[copied..range.start]);
        rewritten.extend(xml[range.clone()].chars().filter(|c| !c.is_whitespace()));
        copied = range.end;
    }
    rewritten.push_str(&xml[copied..]);
    rewritten
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path} should be in the corpus: {e}"))
}

#[test]
fn should_mint_the_same_amendment_ids_when_two_printings_of_one_act_are_read() {
    let compact_xml = read(COMPACT_XML);
    let laid_out_xml = read(LAID_OUT_XML);
    let compact = Document::parse(&compact_xml).expect("the compact printing should parse");
    let laid_out = Document::parse(&laid_out_xml).expect("the laid-out printing should parse");

    let compact_ids = ids_by_element(&compact.root());
    let laid_out_ids = ids_by_element(&laid_out.root());

    assert_eq!(
        compact_ids.keys().collect::<Vec<_>>(),
        laid_out_ids.keys().collect::<Vec<_>>(),
        "both printings should carry the same instruction elements"
    );

    let agreeing = compact_ids
        .iter()
        .filter(|(element_id, id)| laid_out_ids.get(*element_id) == Some(*id))
        .count();

    assert_eq!(
        agreeing,
        compact_ids.len(),
        "{agreeing} of {} amendment ids agree between the two printings, so an id \
         follows the printing rather than the words of the law",
        compact_ids.len()
    );
}

/// A fold that makes two printings agree must not make two amendments agree.
///
/// Deleting whitespace outright closes the gap between the printings as well,
/// and this is the guard that says why it is the wrong fold: an id is only worth
/// having while one id names one amendment.
#[test]
fn should_keep_every_amendment_distinct_when_a_printing_is_read() {
    for path in [COMPACT_XML, LAID_OUT_XML] {
        let xml = read(path);
        let document = Document::parse(&xml).expect("the printing should parse");
        let by_element = ids_by_element(&document.root());

        let distinct: BTreeSet<&String> = by_element.values().collect();

        assert_eq!(
            distinct.len(),
            by_element.len(),
            "{path} holds {} instructions under {} ids, so the fold has made one \
             amendment out of two",
            by_element.len(),
            distinct.len()
        );
    }
}

/// A space between two words is part of what the law says.
///
/// `two words` and `twowords` are two texts, so they must be two ids. This is
/// the line between the fold this issue asks for and the one that would also
/// have closed the gap: a whitespace-only text node is layout and goes, and the
/// whitespace between words in a text node that carries words stays.
#[test]
fn should_mint_a_different_id_when_the_words_of_an_amendment_run_together() {
    let xml = read(COMPACT_XML);
    let document = Document::parse(&xml).expect("the compact printing should parse");
    let run_together_xml = words_run_together(&xml, &document);
    let run_together =
        Document::parse(&run_together_xml).expect("the rewritten printing should parse");

    let spaced = ids_by_element(&document.root());
    let unspaced = ids_by_element(&run_together.root());

    let agreeing = spaced
        .iter()
        .filter(|(element_id, id)| unspaced.get(*element_id) == Some(*id))
        .count();

    assert_eq!(
        agreeing,
        0,
        "{agreeing} of {} amendments keep their id when their words are run \
         together, so the fold has deleted spaces the law says",
        spaced.len()
    );
}

/// An em dash and its ASCII transliteration are one dash.
///
/// The cached printing writes the em dash of `.—` as one character and the
/// Government Publishing Office writes it `--`. A fold that writes one dash as
/// one hyphen cannot reverse that, because the transliteration is two
/// characters: the run is what has to fold.
#[test]
fn should_mint_the_same_id_when_one_printing_transliterates_an_em_dash() {
    let compact_xml = read(COMPACT_XML);
    let laid_out_xml = read(LAID_OUT_XML);
    let compact = Document::parse(&compact_xml).expect("the compact printing should parse");
    let laid_out = Document::parse(&laid_out_xml).expect("the laid-out printing should parse");

    let compact_markup = markup_by_element(&compact_xml, &compact.root());
    let laid_out_markup = markup_by_element(&laid_out_xml, &laid_out.root());
    let compact_ids = ids_by_element(&compact.root());
    let laid_out_ids = ids_by_element(&laid_out.root());

    let transliterated: Vec<&String> = compact_markup
        .iter()
        .filter(|(element_id, markup)| {
            markup.contains('\u{2014}')
                && laid_out_markup
                    .get(*element_id)
                    .is_some_and(|other| other.contains("--"))
        })
        .map(|(element_id, _)| element_id)
        .collect();

    assert!(
        !transliterated.is_empty(),
        "the two printings should hold an em dash written both ways, or this \
         proves nothing"
    );

    for element_id in &transliterated {
        assert_eq!(
            compact_ids.get(*element_id),
            laid_out_ids.get(*element_id),
            "instruction {element_id} writes an em dash in one printing and `--` \
             in the other, and the two must mint one id"
        );
    }
}
