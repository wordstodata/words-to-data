//! A subdivision whose designation is more than one character: `(ii)`, `(IV)`,
//! `(cc)`, `(14A)` (#410).
//!
//! The reader took a designation of one letter or one or two digits only, so
//! `7 U.S.C. § 2015(o)(4)(A)(ii)` was read as `2015(o)(4)(A)` and its link
//! stopped one level too high, with no report. Every text here is real: a
//! committed CourtListener record in `tests/test_data/courtlistener`, or a note
//! of the Code in `tests/test_data/usc`.

use words_to_data::citation::default_code;
use words_to_data::citation::resolve::{Reached, SectionPaths, resolve};
use words_to_data::citation::usc::{self, UscCitation};
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};

const CACHE: &str = "tests/test_data/courtlistener";
const RELEASE: &str = "2025-07-30";
const TITLE_7: &str = "tests/test_data/usc/2025-07-30/usc07.xml";
const TITLE_11: &str = "tests/test_data/usc/2025-07-30/usc11.xml";
const TITLE_26: &str = "tests/test_data/usc/2025-07-30/usc26.xml";

/// One committed opinion record, as the API returned it.
fn read_record(id: u64) -> String {
    std::fs::read_to_string(format!("{CACHE}/opinion_{id}.json"))
        .unwrap_or_else(|error| panic!("opinion {id} should be committed: {error}"))
}

/// The text the dataset stores for one committed opinion record.
fn stored_text(id: u64) -> String {
    let record =
        OpinionRecord::from_json(&read_record(id)).expect("the opinion record should read");
    let (text, _, _) = record.text().expect("the record carries text");
    text
}

/// The citation in `text` whose words are `written`.
fn citation_written(text: &str, written: &str) -> UscCitation {
    assert!(text.contains(written), "the text writes {written}");
    let found = usc::find(text);
    found
        .iter()
        .find(|citation| citation.text == written)
        .cloned()
        .unwrap_or_else(|| panic!("{written} is read as one citation, got {found:?}"))
}

/// Where each section of `citation` leads in one title as it read on
/// 2025-07-30.
fn reached_in(title_file: &str, work: &str, citation: &UscCitation) -> Vec<Reached> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "One title".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(title_file, RELEASE, None)
        .expect("the title should parse");
    let title = dataset
        .get_expression(&ExpressionId::new(WorkId::new(work), RELEASE))
        .expect("storage should answer")
        .expect("the title should be held");
    let mut index = SectionPaths::new();
    index.add_work(&title.root);
    let scope = dataset.scope().expect("the scope should derive");

    resolve(citation, &scope, &index)
        .into_iter()
        .flat_map(|cited| cited.reached)
        .collect()
}

#[test]
fn should_resolve_to_clause_ii_when_dc_v_usda_cites_2015_o_4_a_ii() {
    // *District of Columbia v. USDA* (D.D.C. 2020), opinion 4578098, writes
    // "under 7 U.S.C. § 2015(o)(4)(A)(ii), other than the 24-month average".
    let text = stored_text(4578098);

    let citation = citation_written(&text, "7 U.S.C. \u{a7} 2015(o)(4)(A)(ii)");

    assert_eq!(citation.sections, ["2015(o)(4)(A)(ii)"]);
    let reached = reached_in(TITLE_7, "uscode/title_7", &citation);
    assert_eq!(reached.len(), 1, "one section, got {reached:?}");
    assert!(
        reached[0]
            .path
            .ends_with("/section_2015/subsection_o/paragraph_4/subparagraph_A/clause_ii"),
        "the link ends at clause (ii), got {reached:?}"
    );
    assert_eq!(reached[0].unresolved, None);
}

#[test]
fn should_resolve_to_the_item_when_a_note_cites_a_subclause_and_an_item_of_7_usc_2() {
    // A note to 7 U.S.C. § 2 in the 2025-07-30 release writes "The provisions
    // of section 2(c)(2)(B)(i)(II)(cc) [7 U.S.C. 2(c)(2)(B)(i)(II)(cc)] that set
    // forth adjusted net capital requirements". The Code designates a subclause
    // with a capital roman numeral and an item with a doubled letter.
    let text = "The provisions of section 2(c)(2)(B)(i)(II)(cc) \
                [7 U.S.C. 2(c)(2)(B)(i)(II)(cc)] that set forth adjusted net \
                capital requirements";

    let citation = citation_written(text, "7 U.S.C. 2(c)(2)(B)(i)(II)(cc)");

    assert_eq!(citation.sections, ["2(c)(2)(B)(i)(II)(cc)"]);
    let reached = reached_in(TITLE_7, "uscode/title_7", &citation);
    assert_eq!(reached.len(), 1, "one section, got {reached:?}");
    assert!(
        reached[0].path.ends_with(
            "/section_2/subsection_c/paragraph_2/subparagraph_B/clause_i/subclause_II/item_cc"
        ),
        "the link ends at item (cc), got {reached:?}"
    );
    assert_eq!(reached[0].unresolved, None);
}

#[test]
fn should_resolve_to_paragraph_14a_when_an_opinion_cites_11_usc_101_14a() {
    // Opinion 9515913 writes "See 11 U.S.C. §§ 101(14A), 507(a)(1)(A),
    // 523(a)(5)", the definition of a domestic support obligation. The Code
    // numbers a paragraph put in between (14) and (15) with a number and a
    // capital letter. The sentence is taken from the record's `xml_harvard`,
    // without its markup: the stored text puts a line break inside the
    // citation, where the markup ends a tag.
    let text = "It deprives them of the Bankruptcy Code's special protections for \
                domestic-support obligations. See 11 U.S.C. \u{a7}\u{a7} 101(14A), \
                507(a)(1)(A), 523(a)(5), 523(a)(15).";
    assert!(
        read_record(9515913).contains("11 U.S.C. \u{a7}\u{a7} 101</extracted-citation>(14A)"),
        "the record writes the citation"
    );

    let citation = citation_written(
        text,
        "11 U.S.C. \u{a7}\u{a7} 101(14A), 507(a)(1)(A), 523(a)(5), 523(a)(15)",
    );

    assert_eq!(
        citation.sections,
        ["101(14A)", "507(a)(1)(A)", "523(a)(5)", "523(a)(15)"]
    );
    let reached = reached_in(TITLE_11, "uscode/title_11", &citation);
    assert_eq!(reached.len(), 4, "four sections, got {reached:?}");
    assert!(
        reached[0].path.ends_with("/section_101/paragraph_14A"),
        "the link ends at paragraph (14A), got {reached:?}"
    );
    assert_eq!(reached[0].unresolved, None);
}

#[test]
fn should_resolve_to_clause_ii_when_torres_cites_199a_c_3_a_ii_under_its_default_code() {
    // *Patricia A. Torres* (T.C. 2025), opinion 11136635, cluster 10670048,
    // names the Internal Revenue Code as its default code and writes "see
    // I.R.C. § 199A(c)(3)(A)(ii)". The default-code reader reads the section
    // number with the same pattern as the U.S.C. reader.
    let cluster = std::fs::read_to_string(format!("{CACHE}/cluster_10670048.json"))
        .expect("the cluster should be committed");
    let cluster = ClusterRecord::from_json(&cluster).expect("the cluster should read");
    let opinion =
        OpinionRecord::from_json(&read_record(11136635)).expect("the opinion should read");
    let writings = case_expression(&cluster, &[opinion])
        .expect("the case should build")
        .writings;
    let default = default_code::of_case(
        writings
            .iter()
            .map(|writing| (writing.path.as_str(), writing.text.as_str())),
    )
    .expect("Torres names its default code");

    let found: Vec<UscCitation> = writings
        .iter()
        .flat_map(|writing| {
            default_code::find_bare(&writing.text, &default, &usc::find(&writing.text))
        })
        .collect();

    let citation = found
        .iter()
        .find(|citation| citation.text == "I.R.C. \u{a7} 199A(c)(3)(A)(ii)")
        .unwrap_or_else(|| panic!("the reference is read whole, got {found:?}"));
    assert_eq!(citation.sections, ["199A(c)(3)(A)(ii)"]);
    let reached = reached_in(TITLE_26, "uscode/title_26", citation);
    assert_eq!(reached.len(), 1, "one section, got {reached:?}");
    assert!(
        reached[0]
            .path
            .ends_with("/section_199A/subsection_c/paragraph_3/subparagraph_A/clause_ii"),
        "the link ends at clause (ii), got {reached:?}"
    );
    assert_eq!(reached[0].unresolved, None);
}
