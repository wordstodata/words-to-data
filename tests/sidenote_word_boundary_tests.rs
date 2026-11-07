//! A marginal note keeps a word boundary at each of its edges (#484).
//!
//! The Public Law print puts a marginal note (`<sidenote>`) between two words
//! of an amending line:
//!
//! ```xml
//! chapter 61<sidenote><p>26 USC</p><p>prec. 6041.</p></sidenote> <amendingAction>
//! ```
//!
//! The parser drops each text node that holds only white space, so the words
//! on the two sides of the note joined: "chapter 6126 USCprec. 6041.is
//! amended". The number of the chapter changed, and the address reader read a
//! wrong number. These tests read the real public laws. Nothing is mocked.

use words_to_data::uslm::bill_parser::bill_of_document;

const HR_1: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const S_1071: &str = "tests/test_data/congress_client_cache/bill/119/s/1071/public_law.xml";

fn amending_texts(path: &str, bill_id: &str) -> Vec<String> {
    let xml = std::fs::read_to_string(path).expect("the public law should be in the corpus");
    let document = roxmltree::Document::parse(&xml).expect("the public law should parse");
    let (bill, _) = bill_of_document(&document, bill_id);
    bill.amendments
        .values()
        .map(|amendment| amendment.amending_text.clone())
        .collect()
}

/// The amending texts that hold `marker`, a word of one marginal note.
fn texts_with(path: &str, bill_id: &str, marker: &str) -> Vec<String> {
    let texts: Vec<String> = amending_texts(path, bill_id)
        .into_iter()
        .filter(|text| text.contains(marker))
        .collect();
    assert!(
        !texts.is_empty(),
        "an amending text of {bill_id} should hold {marker:?}"
    );
    texts
}

#[test]
fn should_keep_the_chapter_number_when_a_sidenote_follows_it() {
    for text in texts_with(HR_1, "119-hr-1", "prec. 6041.") {
        assert!(
            text.contains("chapter 61 "),
            "the § 6041 table of sections amendment should name chapter 61: {text:?}"
        );
        assert!(
            !text.contains("6126"),
            "the chapter number should not join the sidenote words: {text:?}"
        );
        assert!(
            !text.contains("USCprec."),
            "the two lines of the sidenote should not join: {text:?}"
        );
        assert!(
            !text.contains("6041.is"),
            "the word after the sidenote should not join it: {text:?}"
        );
    }
}

/// Whether `word` is one whole word of `text`.
fn has_word(text: &str, word: &str) -> bool {
    text.split_whitespace().any(|each| each == word)
}

/// The words of the note stay in the text (#484 keeps them, and their removal
/// is a different issue), so "section 50 USC 3227b. 1033" is the line. The
/// number after the note must be a word of its own, and not a part of the
/// cite in the note.
#[test]
fn should_keep_the_section_number_a_word_of_its_own_when_a_sidenote_comes_before_it() {
    for text in texts_with(S_1071, "119-s-1071", "3227b.") {
        assert!(
            has_word(&text, "1033;"),
            "the struck section should be 1033: {text:?}"
        );
        assert!(
            !text.contains("3227b.1033"),
            "the section number should not join the sidenote: {text:?}"
        );
    }
    for text in texts_with(S_1071, "119-s-1071", "14 USC 551.") {
        assert!(
            has_word(&text, "548"),
            "the redesignated section should be 548: {text:?}"
        );
        assert!(
            !text.contains("551.548"),
            "the section number should not join the sidenote: {text:?}"
        );
    }
}
