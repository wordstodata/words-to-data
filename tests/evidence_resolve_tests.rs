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
        .record_redesignations_over(BILL_ID, &bill, &windows)
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

/// What the matcher says about every amendment that acts in title 42.
fn title_42_matches() -> &'static [AmendmentMatch] {
    static MATCHES: OnceLock<Vec<AmendmentMatch>> = OnceLock::new();
    MATCHES.get_or_init(|| {
        match_by_evidence(&dataset_with_title("usc42.xml"))
            .expect("the matcher should run")
            .matches
    })
}

/// 42 U.S.C. 1396a, section 1902 of the Social Security Act, which several
/// amendments of the law address as a whole.
const SECTION_1396A: &str = "uscode/title_42/chapter_7/subchapter_XIX/section_1396a";

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

#[test]
fn should_not_let_one_common_word_decide_a_change_when_the_amendment_quotes_nothing_else_there() {
    // 42 U.S.C. 1396a(e)(14)(D)(iv). Section 71108(b) of the law strikes
    // "Subparagraphs" there and inserts "(I) In general.—Subparagraphs", so
    // the clause's words move down into a new subclause (I).
    //
    // Section 71103(a)(1) amends the whole of section 1396a, and one of the
    // things it does is strike "and" at the end of paragraph (a)(86). The
    // clause's words hold an "and", and they moved away, so the clause shows
    // "and" struck. One common word is not evidence of which change an
    // amendment made.
    let clause_iv = format!("{SECTION_1396A}/subsection_e/paragraph_14/subparagraph_D/clause_iv");
    let matches = title_42_matches();

    assert!(
        !linked_paths(matches, "c8020929f28e").contains(&clause_iv),
        "a struck \"and\" does not link the whole-section amendment to the clause"
    );
    assert!(linked_paths(matches, "016d2c93ae1d").contains(&clause_iv));
}

#[test]
fn should_let_common_words_decide_a_change_when_they_are_all_the_change_struck_or_inserted() {
    // 42 U.S.C. 1396a(a)(86). Section 71103(a)(1) of the law strikes "and" at
    // the end of it, and that is the whole of the change: the paragraph lost
    // an "and" and nothing else. Words that small still say which change an
    // amendment made when they are all the change there is.
    let paragraph_86 = format!("{SECTION_1396A}/subsection_a/paragraph_86");

    assert!(linked_paths(title_42_matches(), "c8020929f28e").contains(&paragraph_86));
}

#[test]
fn should_link_an_enacted_provision_when_a_later_amendment_of_the_law_inserted_words_at_its_end() {
    // 26 U.S.C. 6041(d)(3). Section 70201(f)(1)(B) of the law enacts a new
    // paragraph (3) that ends "...receiving such tips.". Section
    // 70202(c)(2)(B), later in the law, strikes that period and inserts
    // ", and". So the Code prints the paragraph with words its enacting
    // amendment never stated, and with those words taken out it prints
    // exactly what the amendment enacted.
    let paragraph_3 = "uscode/title_26/subtitle_F/chapter_61/subchapter_A/part_III/subpart_B/section_6041/subsection_d/paragraph_3";

    assert!(linked_paths(title_26_matches(), "6357bb071b53").contains(&paragraph_3.to_string()));
}

#[test]
fn should_not_give_a_change_to_an_amendment_that_only_renumbers_by_elimination() {
    // 42 U.S.C. 1397gg(e)(1). Section 71109(b)(1) of the law (by the
    // markup: "(1) by redesignating subparagraphs (R) through (V) as
    // paragraphs (S) through (W)") only renumbers. It quotes no words, so
    // nothing speaks against any change under its address. Other amendments
    // edited the words of subparagraph (G) and added a new (H) there.
    //
    // An amendment that only renumbers made moves, and the dataset's
    // redesignation links say which. It is given nothing by elimination.
    //
    // Before #262 the markup gives this amendment no address, and it stops
    // there, so on its own this test only guards the rule.
    let paragraph_1 =
        "uscode/title_42/chapter_7/subchapter_XXI/section_1397gg/subsection_e/paragraph_1";
    let found = match_of(title_42_matches(), "6a32ab182ca2");
    let paths = match &found.outcome {
        Outcome::Linked(linked) => linked.paths(),
        Outcome::Residue(_) => Vec::new(),
    };

    for subparagraph in ["subparagraph_G", "subparagraph_H"] {
        let path = format!("{paragraph_1}/{subparagraph}");
        assert!(
            !paths.contains(&path),
            "the renumbering amendment did not make {path}: {paths:?}"
        );
    }
}

#[test]
fn should_give_a_rewritten_provision_only_to_the_amendment_that_enacted_its_words() {
    // 26 U.S.C. 168(k)(10)(A). Section 70301(b)(3) of the law strikes the
    // subparagraph and enacts a new one, and the Code prints exactly its
    // words. The old words said "the applicable percentage", so the change
    // also shows that string struck, and section 70301(b)(1) strikes that
    // string, in paragraph (1)(A).
    //
    // An amendment that enacted every word a provision now holds rewrote it.
    // Another amendment's string struck with the old words is not a second
    // edit.
    let subparagraph_a =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_168/subsection_k/paragraph_10/subparagraph_A".to_string();
    let matches = title_26_matches();

    assert!(linked_paths(matches, "3becff5cdac9").contains(&subparagraph_a));
    assert!(
        !linked_paths(matches, "1be2c21a3613").contains(&subparagraph_a),
        "the amendment of paragraph (1)(A) did not edit the rewritten subparagraph"
    );
}

/// 26 U.S.C. 36B(f)(2), which section 71305 of the law amends twice.
const SECTION_36B_F_2: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_C/section_36B/subsection_f/paragraph_2";

#[test]
fn should_give_an_amendment_that_strikes_a_named_unit_only_changes_in_that_unit_when_its_address_holds_others()
 {
    // Section 71305(a) of the law: "Section 36B(f)(2) is amended by striking
    // subparagraph (B)." It quotes no words, and its address is the whole of
    // paragraph (2). The window holds three changes there:
    //
    // - subparagraph (B) is removed. This is the amendment's change.
    // - subparagraph (A) is removed, and
    // - paragraph (2) gains the words of (A). Section 71305(b)(1) strikes
    //   "advance payments.-" and all that follows through "If the advance
    //   payments", and so moves the words of (A) up into the paragraph. Its
    //   quoted words give it (A). They do not show in the paragraph, so the
    //   paragraph is left for elimination.
    //
    // "Striking subparagraph (B)" names the one unit it acts on. The
    // paragraph's new words are not in that unit, so elimination does not give
    // them to this amendment.
    assert_eq!(
        linked_paths(title_26_matches(), "5fe0631d2a2d"),
        vec![format!("{SECTION_36B_F_2}/subparagraph_B")]
    );
}

#[test]
fn should_give_an_amendment_that_only_strikes_the_note_the_code_prints_where_the_struck_unit_was() {
    // Section 71302(a) of the law: "Section 36B(c)(1) is amended by striking
    // subparagraph (B)." The Code does not drop subparagraph (B). Its editors
    // leave a note in its place, "Repealed. Pub. L. 119–21, § 71302(a), July
    // 4, 2025, 139 Stat. 322]", and remove its clauses (i) and (ii).
    //
    // The note holds words the subparagraph did not hold, and they are the
    // editors' record of the strike and not words of the law. So the change
    // is the strike's, with the removals below it.
    let subparagraph_b = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_C/section_36B/subsection_c/paragraph_1/subparagraph_B";

    assert_eq!(
        linked_paths(title_26_matches(), "bdd2f00cce42"),
        vec![
            subparagraph_b.to_string(),
            format!("{subparagraph_b}/clause_i"),
            format!("{subparagraph_b}/clause_ii"),
        ]
    );
}

/// The matcher's answer for every amendment that acts in one title, with the
/// title's two committed release points read in the wrong order: the text of
/// 2025-07-30 dated 2025-07-18, and the text of 2025-07-18 dated 2025-07-30.
///
/// The window then undoes the law. Every provision the law removed is added,
/// and every provision it added is removed. The committed corpus holds one law,
/// and none of its amendments meets a change of the wrong kind that elimination
/// could give it. Read this way, the same real text puts one there.
fn matches_read_backwards(file: &str) -> Vec<AmendmentMatch> {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .load_bill_download(&committed_bill_download())
        .expect("the committed bill should load");
    for (text_of, dated) in [("2025-07-30", "2025-07-18"), ("2025-07-18", "2025-07-30")] {
        dataset
            .add_uslm_xml(
                &format!("tests/test_data/usc/{text_of}/{file}"),
                dated,
                None,
            )
            .expect("the title should parse");
    }
    match_by_evidence(&dataset)
        .expect("the matcher should run")
        .matches
}

#[test]
fn should_not_give_an_amendment_that_only_strikes_a_change_that_brings_new_words() {
    // Section 71302(a) of the law: "Section 36B(c)(1) is amended by striking
    // subparagraph (B)." It quotes no words. Read in the right order, the
    // window removes subparagraph (B) and its clauses (i) and (ii), and the
    // amendment is given those removals. Read backwards, the window adds
    // them, in the unit the amendment names.
    //
    // An amendment that only strikes removes words. It cannot add a
    // provision, so elimination does not give it one, and it is left as
    // residue.
    let matches = matches_read_backwards("usc26.xml");

    match &match_of(&matches, "bdd2f00cce42").outcome {
        Outcome::Residue(_) => {}
        Outcome::Linked(linked) => {
            panic!("a strike made none of these changes: {:?}", linked.paths())
        }
    }
}

#[test]
fn should_not_give_an_amendment_that_only_adds_a_change_that_removes_a_provision() {
    // Section 10604(b) of the law: "Section 7601(g)(1)(A) of the Agricultural
    // Act of 2014 (7 U.S.C. 5939(g)(1)(A)) is amended by adding at the end the
    // following: (iv) …". Read in the right order, the window adds clause (iv),
    // the one change under the address. Read backwards, the window removes it.
    // The enacted words show in no removal, so the removal is the one change
    // left to the amendment for elimination.
    //
    // An amendment that only adds or inserts brings words. It cannot remove a
    // provision whole, so elimination does not give it one, and it is left as
    // residue.
    let matches = matches_read_backwards("usc07.xml");

    match &match_of(&matches, "63c608ae5b45").outcome {
        Outcome::Residue(_) => {}
        Outcome::Linked(linked) => panic!(
            "an addition made none of these changes: {:?}",
            linked.paths()
        ),
    }
}

/// 26 U.S.C. 24(h), where two amendments of the law meet.
const SECTION_24_H: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_A/section_24/subsection_h";

#[test]
fn should_not_give_a_change_inside_a_provision_to_its_editor_when_another_amendments_address_is_the_change()
 {
    // Pub. L. 119-21 § 70104:
    //
    // - (a) "Section 24(h) is amended—(1) in paragraph (1), by striking …,
    //   (2) in paragraph (2), by striking "$2,000" and inserting "$2,200", and
    //   (3) by striking "2018 Through 2025" in the heading …". Its words place
    //   the edit of the heading of (h).
    // - (b) "Section 24(h)(7) is amended to read as follows: …".
    //
    // (h)(7) changed. It is inside (h), but (a) says nothing of (h)(7), and
    // (b) is addressed to it. The change is not (a)'s (#339).
    let matches = title_26_matches();
    let paragraph_7 = format!("{SECTION_24_H}/paragraph_7");

    let of_a = linked_paths(matches, "9d4f2ae0b2be");
    assert!(
        !of_a.contains(&paragraph_7),
        "§ 70104(a) should not hold (h)(7): {of_a:?}"
    );
    let of_b = linked_paths(matches, "6c0a0ef049ab");
    assert!(
        of_b.contains(&paragraph_7),
        "§ 70104(b) should hold (h)(7): {of_b:?}"
    );
}

#[test]
fn should_give_a_change_inside_a_rewritten_provision_to_its_amendment_when_a_wider_amendment_edits_the_provision_around_it()
 {
    // Pub. L. 119-21 § 70104(d): "Section 24(h)(5) is amended to read as
    // follows". Its new subparagraphs (A) and (B) are inside (h)(5), which it
    // rewrote. § 70104(a) edits the heading of (h), which holds (h)(5), but
    // (h)(5) is the provision nearest to the change.
    let matches = title_26_matches();
    let paragraph_5 = format!("{SECTION_24_H}/paragraph_5");

    let of_d = linked_paths(matches, "000b7e5b4e86");
    for below in ["subparagraph_A", "subparagraph_B"] {
        let path = format!("{paragraph_5}/{below}");
        assert!(
            of_d.contains(&path),
            "§ 70104(d) should hold {path}: {of_d:?}"
        );
    }
}

#[test]
fn should_give_the_provision_at_its_address_by_elimination_only_to_an_amendment_that_rewrites_it() {
    // Pub. L. 119-21 § 70434:
    //
    // - (e) "Section 181 is amended by redesignating subsections (f) and (g)
    //   as subsections (g) and (h), respectively, and by inserting after
    //   subsection (e) the following new subsection: …". Its words place the
    //   new subsection (f).
    // - (g)(1) "The heading for section 181 is amended to read as follows:
    //   …". The change to the words of § 181 itself is its own.
    //
    // (e) does not rewrite § 181, so elimination does not give it the
    // section's own change (#339).
    let matches = title_26_matches();
    let section_181 = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_181";

    let of_e = linked_paths(matches, "6a37a668013d");
    assert!(
        !of_e.iter().any(|path| path == section_181),
        "§ 70434(e) should not hold § 181 itself: {of_e:?}"
    );
    let of_heading = linked_paths(matches, "f3ee50d2b145");
    assert!(
        of_heading.iter().any(|path| path == section_181),
        "the heading amendment should hold § 181: {of_heading:?}"
    );
}

/// 26 U.S.C. 274(o), which TCJA § 13304 added for taxable years after 2025.
const SECTION_274_O: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_IX/section_274/subsection_o";

#[test]
fn should_not_give_the_provisions_inside_a_provision_to_an_amendment_that_only_edited_its_words() {
    // Pub. L. 119-21 § 70305(a): "Section 274(o), as added by section 13304 of
    // Public Law 115-97, is amended by striking "No deduction" and inserting
    // "Except in the case of an expense described in subsection (e)(8) or
    // (n)(2)(C), no deduction"." Its quoted strings place the edit of the
    // words of (o).
    //
    // In the same window the OLRC executed § 13304, and (o)(1) and (o)(2)
    // came into the Code. § 70305(a) edited the words of (o) in place. It did
    // not strike (o) and enact it anew, so the paragraphs inside (o) are not
    // its changes (#390).
    let of_a = linked_paths(title_26_matches(), "cee3304a1b3d");

    assert!(
        of_a.contains(&SECTION_274_O.to_string()),
        "§ 70305(a) should hold (o): {of_a:?}"
    );
    for paragraph in ["paragraph_1", "paragraph_2"] {
        let path = format!("{SECTION_274_O}/{paragraph}");
        assert!(
            !of_a.contains(&path),
            "§ 70305(a) did not make {path}: {of_a:?}"
        );
    }
}

/// 26 U.S.C. 163(j)(12), the special rule for taxable years beginning in 2019
/// and 2020, which the law renumbered twice.
const SECTION_163_J_12: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_163/subsection_j/paragraph_12";

#[test]
fn should_not_let_an_enacted_block_take_a_change_of_a_few_words_that_a_renumbered_provision_carries()
 {
    // Pub. L. 119-21 § 70303:
    //
    // - (a) renumbers paragraphs (10) and (11) of § 163(j) and enacts a new
    //   paragraph (10). One of its subparagraphs is headed "In general".
    // - (c) renumbers again, and the dataset's redesignation link says that
    //   it moved the old special rule to (12).
    //
    // (12)(A) of the special rule is headed "In general" and holds nothing
    // else. Those two words are in the block (a) enacts, and in many other
    // provisions too. A change of a few words inside a longer block is not
    // evidence that the block made it, when the renumbering of the provision
    // around it explains it (#389).
    let matches = title_26_matches();
    let subparagraph_a = format!("{SECTION_163_J_12}/subparagraph_A");

    let of_a = linked_paths(matches, "b2f12a7edfa5");
    assert!(
        !of_a.iter().any(|path| path.starts_with(&subparagraph_a)),
        "§ 70303(a) did not make (12)(A): {of_a:?}"
    );
    let of_c = linked_paths(matches, "21c6fe30594a");
    assert!(
        of_c.contains(&subparagraph_a),
        "the renumbering of (12) carries (12)(A): {of_c:?}"
    );
}

#[test]
fn should_give_the_provisions_inside_a_provision_to_an_amendment_whose_inserted_words_open_a_new_one()
 {
    // 42 U.S.C. 1396a(e)(14)(D)(iv). Section 71108(b) of the law strikes
    // "Subparagraphs" there and inserts "(I) In general.—Subparagraphs", and
    // adds a new subclause (II) at the end. The words of the clause move down
    // into the new subclause (I).
    //
    // The amendment edits the words of (iv) by quoted strings, but the string
    // it inserts opens a new subclause. So (iv) carries the subclauses inside
    // it, unlike a provision whose words an amendment only edited (#390).
    let clause_iv = format!("{SECTION_1396A}/subsection_e/paragraph_14/subparagraph_D/clause_iv");
    let of_b = linked_paths(title_42_matches(), "016d2c93ae1d");

    for subclause in ["subclause_I", "subclause_II"] {
        let path = format!("{clause_iv}/{subclause}");
        assert!(of_b.contains(&path), "§ 71108(b) made {path}: {of_b:?}");
    }
}

#[test]
fn should_give_the_removal_of_a_provision_inside_a_provision_to_the_amendment_whose_strike_edited_it()
 {
    // 26 U.S.C. 460(c)(6)(B). Pub. L. 119-21: "Section 460(c)(6)(B) is
    // amended by striking "which" and all that follows through
    // the period and inserting "which has a recovery period of 7 years or
    // less."." The words it strikes run on into clauses (i) and (ii), so the
    // clauses are removed, and their words are struck with them.
    //
    // The amendment edits (B) by quoted strings and enacts nothing anew. A
    // strike can still remove the provisions inside the provision it edits, so
    // (B) carries their removal (#390).
    let subparagraph_b = "uscode/title_26/subtitle_A/chapter_1/subchapter_E/part_II/subpart_B/section_460/subsection_c/paragraph_6/subparagraph_B";
    let of_d = linked_paths(title_26_matches(), "4057af6072f0");

    for clause in ["clause_i", "clause_ii"] {
        let path = format!("{subparagraph_b}/{clause}");
        assert!(of_d.contains(&path), "the strike removed {path}: {of_d:?}");
    }
}

#[test]
fn should_give_the_provisions_inside_a_provision_to_an_amendment_whose_inserted_string_holds_a_new_designation()
 {
    // 26 U.S.C. 55(d)(4)(B)(i). Pub. L. 119-21, in "Modification of
    // Inflation Adjustment", strikes the end of the clause and inserts
    // "determined by substituting for 'calendar year 2016' in subparagraph
    // (A)(ii) thereof—"(1) 'calendar year 2017', …". The string starts with
    // words of the old clause, and a new provision opens inside it, after a
    // quotation mark. The Code prints the new provisions as subclauses (I)
    // and (II), so the string does not show in them.
    let clause_i = "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_VI/section_55/subsection_d/paragraph_4/subparagraph_B/clause_i";
    let of_b = linked_paths(title_26_matches(), "f4da38548dd3");

    let subclause_ii = format!("{clause_i}/subclause_II");
    assert!(
        of_b.contains(&subclause_ii),
        "the inserted string opened {subclause_ii}: {of_b:?}"
    );
}

/// 26 U.S.C. 904(b), where Pub. L. 119-21 adds two paragraphs at the end.
const SECTION_904_B: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_III/subpart_A/section_904/subsection_b";

#[test]
fn should_link_an_added_provision_by_the_words_below_it_when_its_subparagraphs_split_its_own_words()
{
    // Pub. L. 119-21 § 70311(a): "Section 904(b) is amended by adding at the
    // end the following new paragraph: (5) Deductions treated as allocable to
    // foreign source net CFC tested income.—Solely for purposes of … (A) …
    // (B) … (C) … Any amount or deduction which would …". The Code prints
    // subparagraphs (A) to (C) between the chapeau of (5) and its
    // continuation, so the own words of (5) are not one run of the block.
    //
    // An added provision holds every provision below it (#336), so the words
    // of (5) and of its subparagraphs, in the order the Code prints them,
    // are what the block is compared with (#432).
    let paragraph_5 = format!("{SECTION_904_B}/paragraph_5");

    let of_a = linked_paths(title_26_matches(), "9f5277a3626a");

    assert_eq!(of_a, vec![paragraph_5]);
}

#[test]
fn should_link_an_added_provision_by_its_own_words_when_a_marginal_note_splits_the_words_below_it()
{
    // Pub. L. 119-21 § 70307 adds subsection (n), "Special allowance for
    // qualified production property", to 26 U.S.C. 168. The bill prints the
    // marginal note "Applicability." inside the words of a paragraph below
    // (n), so that paragraph is not one run of the block.
    //
    // The own words of (n) are one run of the block, and they are more than
    // a few words. They are evidence for (n), as they were before the words
    // below it were compared (#432).
    let subsection_n =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_168/subsection_n"
            .to_string();

    let of_70307 = linked_paths(title_26_matches(), "b7565236f4ad");

    assert!(
        of_70307.contains(&subsection_n),
        "§ 70307 added {subsection_n}: {of_70307:?}"
    );
}

#[test]
fn should_give_a_change_to_the_amendment_with_the_narrower_address_when_two_quote_the_same_words() {
    // 26 U.S.C. 168(k)(5)(A)(i). Two amendments of the law strike "the
    // applicable percentage" and insert "100 percent":
    //
    // - § 70301(b)(1), addressed to the whole of 168(k): "(A) in paragraph
    //   (1)(A), by striking …";
    // - § 70301(b)(2), addressed to 168(k)(5)(A)(i) itself.
    //
    // The clause shows the words of both, as much and with the same overlap.
    // The amendment addressed to the clause is nearer to it than the one
    // addressed to the whole subsection around it, as in step 2 (#339), so
    // the clause is its change (#434).
    let clause_i = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_168/subsection_k/paragraph_5/subparagraph_A/clause_i";
    let matches = title_26_matches();

    assert_eq!(
        linked_paths(matches, "2852a8dd94f5"),
        vec![clause_i.to_string()]
    );
    assert!(
        !linked_paths(matches, "1be2c21a3613").contains(&clause_i.to_string()),
        "the amendment to the whole subsection is not a cause of the clause"
    );
}

#[test]
fn should_read_an_address_through_a_renumbering_the_law_makes_after_it_when_both_act_in_one_window()
{
    // 26 U.S.C. 67. Section 70110 of the law says, in this order:
    //
    // - (a): "Section 67(g) is amended—(1) by striking ', and before January
    //   1, 2026', and (2) by striking '2018 Through 2025' in the heading …";
    // - (b)(2): "Section 67 is amended by redesignating subsection (g), as
    //   amended by this section, as subsection (h), and by inserting after
    //   subsection (f) the following new section: '(g) Educator Expenses …'".
    //
    // § 70110(a) acts first, on the old (g). The Code after the window prints
    // that provision as (h), and a new (g) where it was. The address is read
    // through the renumbering, so (h) is § 70110(a)'s change (#434).
    let subsection_h =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_I/section_67/subsection_h";
    let matches = title_26_matches();

    assert_eq!(
        linked_paths(matches, "a54c7b4eef28"),
        vec![subsection_h.to_string()]
    );
}

#[test]
fn should_read_an_address_at_the_provision_that_shows_its_inserted_words_when_another_law_moved_it_in_the_window()
 {
    // 26 U.S.C. 25C. § 70505(a) of the law says: "Section 25C(h) is amended by
    // striking 'placed in service' and all that follows through 'December 31,
    // 2032' and inserting 'placed in service after December 31, 2025'".
    //
    // In the same window the Code's editors also executed another law, which
    // redesignated the termination rule (h) as (i) and inserted a new (h), the
    // product identification rule. So the Code after the window prints the
    // words this amendment inserted at (i), and the provision now at (h) is
    // another law's.
    //
    // The quoted words decide (#391). The old (h) held the words it strikes.
    // No change under (h) shows the words it inserts, and (i), added beside
    // (h) in the window, shows them all. So (i) is its change, and nothing
    // under (h) is.
    let section_25c =
        "uscode/title_26/subtitle_A/chapter_1/subchapter_A/part_IV/subpart_A/section_25C";
    let matches = title_26_matches();

    assert_eq!(
        linked_paths(matches, "f1e4088d276d"),
        vec![format!("{section_25c}/subsection_i")]
    );
}
