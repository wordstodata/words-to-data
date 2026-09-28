//! Stage 3 of the evidence matcher, resolve, on the sections where the
//! amendments of one law meet (#259).
//!
//! `docs/adr/0013-matching-is-evidence-first-and-the-batch-calls-no-model.md`
//! and [`words_to_data::legislature::evidence_matching`] say what the stage
//! does. Every case is read out of the committed corpus: the public law
//! `119-hr-1` and the titles of the Code it amends, at the two release points
//! around its enactment.

use std::sync::OnceLock;

use words_to_data::congress::BillDownload;
use words_to_data::dataset::{Dataset, DatasetMetadata, adjacent_expressions};
use words_to_data::legislature::evidence_matching::{AmendmentMatch, Outcome, match_by_evidence};
use words_to_data::storage::InMemoryStorage;

/// The committed public law, as the Congress client leaves it in the cache.
const BILL_DIR: &str = "tests/test_data/congress_client_cache/bill/119/hr/1";
const BILL_ID: &str = "119-hr-1";
/// The release point before the law, and the first one after it.
const RELEASE_POINTS: [&str; 2] = ["2025-07-18", "2025-07-30"];

/// 26 U.S.C. 6041(a), where five amendments of the law meet in one paragraph.
const SECTION_6041_A: &str = "uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041/subsection_a";

fn committed_bill_download() -> BillDownload {
    let read = |name: &str| {
        std::fs::read_to_string(format!("{BILL_DIR}/{name}"))
            .unwrap_or_else(|e| panic!("{name} should be committed: {e}"))
    };
    BillDownload {
        bill_id: BILL_ID.to_string(),
        bill_xml: read("public_law.xml"),
        bill_metadata_json: read("metadata.json"),
        cosponsors_json: read("cosponsors.json"),
        votes_json: None,
        member_jsons: std::collections::HashMap::new(),
    }
}

/// The bill and one title of the Code at each release point, built the way
/// `build-dataset` builds one: the renumberings the bill states are recorded
/// after everything is loaded.
fn dataset_with_title(file: &str) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    for date in RELEASE_POINTS {
        dataset
            .add_uslm_xml(&format!("tests/test_data/usc/{date}/{file}"), date, None)
            .expect("the title should parse");
    }
    let windows = adjacent_expressions(&dataset).expect("the windows should list");
    let bill = dataset
        .bill_document(BILL_ID)
        .expect("the dataset should answer for the bill")
        .expect("the bill is held as a document");
    dataset
        .record_redesignations_over(BILL_ID, &bill.root, &windows)
        .expect("the renumberings should record");
    dataset
}

/// What the matcher says about every amendment that acts in title 26.
fn title_26_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        match_by_evidence(&dataset_with_title("usc26.xml"))
            .expect("the matcher should run")
            .matches
    })
}

/// The matcher's answer for one amendment, by the start of its id.
fn match_of<'a>(matches: &'a [AmendmentMatch], id_start: &str) -> &'a AmendmentMatch {
    let found: Vec<&AmendmentMatch> = matches
        .iter()
        .filter(|found| found.amendment_id.starts_with(id_start))
        .collect();
    assert_eq!(found.len(), 1, "one amendment starts with {id_start}");
    found[0]
}

/// The paths an amendment was linked to, or a failure that says why not.
fn linked_paths(matches: &[AmendmentMatch], id_start: &str) -> Vec<String> {
    match &match_of(matches, id_start).outcome {
        Outcome::Linked(linked) => linked.paths(),
        other => panic!("{id_start} should be linked: {other:?}"),
    }
}

#[test]
fn should_give_one_change_to_each_amendment_whose_own_words_it_shows_when_several_edit_one_provision()
 {
    // 26 U.S.C. 6041(a) is one paragraph, so the diff reports it as one change.
    // Two amendments of the law edit it, each with words of its own:
    //
    // - § 70433(e)(1): the heading, striking "of $600 or More" and inserting
    //   "Exceeding Threshold".
    // - § 70433(a): striking "$600" and inserting "$2,000".
    //
    // The change shows both. Each amendment made part of it, so each is
    // linked to it.
    let matches = title_26_matches();

    assert_eq!(
        linked_paths(matches, "a90c665c7c34"),
        vec![SECTION_6041_A.to_string()]
    );
    assert_eq!(
        linked_paths(matches, "de3c1b86fed8"),
        vec![SECTION_6041_A.to_string()]
    );
}

#[test]
fn should_not_count_words_as_an_amendments_own_when_another_amendment_quotes_them_inside_longer_words()
 {
    // 26 U.S.C. 250(a)(2)(A). Section 70323(b)(2)(A) of the law strikes
    // "foreign-derived intangible income" there each place it appears. Section
    // 70323(b)(2)(C)(i) strikes "intangible" in the heading of section 250, and
    // its address is the whole section.
    //
    // The paragraph shows "intangible" struck, but only because the longer
    // words were struck. The heading amendment has no words of its own in it.
    let paragraph_2_a = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VIII/section_250/subsection_a/paragraph_2/subparagraph_A".to_string();
    let matches = title_26_matches();

    assert!(linked_paths(matches, "e7a0ad02e344").contains(&paragraph_2_a));
    assert!(
        !linked_paths(matches, "1d50b55e6a2b").contains(&paragraph_2_a),
        "the heading amendment is not a cause of the paragraph"
    );
}

#[test]
fn should_link_an_amendment_whose_inserted_words_a_later_amendment_of_the_law_inserted_into() {
    // 26 U.S.C. 6041(a). Section 70201(f)(1)(A) of the law inserts
    // "(including a separate accounting of any such amounts reasonably
    // designated as cash tips and the occupation described in section
    // 224(d)(1) of the person receiving such tips)". Section 70202(c)(2)(A),
    // later in the same law, inserts "and a separate accounting of any amount
    // of qualified overtime compensation (as defined in section 225(c))"
    // inside those words, before the closing parenthesis.
    //
    // So the Code never prints the first amendment's words as it quotes them.
    // With the later amendment's words taken out, it does.
    let matches = title_26_matches();

    assert_eq!(
        linked_paths(matches, "b5ef6d4dfa56"),
        vec![SECTION_6041_A.to_string()]
    );
    assert_eq!(
        linked_paths(matches, "fe5357110b54"),
        vec![SECTION_6041_A.to_string()]
    );
}
