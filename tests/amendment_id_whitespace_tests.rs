//! An amendment's identity must not depend on how its source file is laid out.
//!
//! An amendment id is `sha256("{bill_id}:{amending_text}")`, and `amending_text`
//! is gathered from every text node under the instruction. A pretty-printed
//! source puts its indentation into those text nodes, so the same law, saved
//! with different line breaks, mints different ids and every
//! `legislature.amended_by` link pointing at the old one stops resolving.
//!
//! Quotes are already folded at this point (`normalize_quotes`), and dashes are
//! folded elsewhere for the same class of reason (`citation::usc::fold_dashes`,
//! #141). Whitespace was not, and nothing decided that.

use words_to_data::uslm::bill_parser::bill_of_document;

/// The printing `build-dataset` fetches, under the name a dataset gives it.
const BILL_XML: &str = "tests/test_data/congress_client_cache/bill/119/hr/1/public_law.xml";
const BILL_ID: &str = "119-hr-1";

fn amending_texts() -> Vec<String> {
    let xml = std::fs::read_to_string(BILL_XML).expect("the public law should be in the corpus");
    let document = roxmltree::Document::parse(&xml).expect("the public law should parse");
    let (bill, _) = bill_of_document(&document, BILL_ID);
    bill.amendments
        .values()
        .map(|amendment| amendment.amending_text.clone())
        .collect()
}

#[test]
fn should_hold_no_run_of_whitespace_in_an_amending_text_when_the_source_is_laid_out() {
    let texts = amending_texts();
    assert!(!texts.is_empty(), "the corpus bill should hold amendments");

    let with_a_run: Vec<&String> = texts
        .iter()
        .filter(|text| text.contains("  ") || text.contains('\n') || text.contains('\t'))
        .collect();

    assert!(
        with_a_run.is_empty(),
        "{} of {} amending texts carry a run of whitespace, so their ids follow the \
         layout of the file rather than the words of the law. First: {:?}",
        with_a_run.len(),
        texts.len(),
        with_a_run[0].chars().take(120).collect::<String>()
    );
}

#[test]
fn should_hold_no_leading_or_trailing_whitespace_in_an_amending_text() {
    let texts = amending_texts();
    let padded: Vec<&String> = texts
        .iter()
        .filter(|text| text.trim() != text.as_str())
        .collect();

    assert!(
        padded.is_empty(),
        "{} of {} amending texts are padded with whitespace",
        padded.len(),
        texts.len()
    );
}
