//! Citation forms the U.S.C. reader dropped with no link and no report (#309).
//!
//! Every text here is a real sentence from a committed CourtListener record in
//! `tests/test_data/courtlistener`. A measurement of 25 opinions found three
//! forms that disappeared:
//!
//! - a space before each bracket of a subdivision, `§ 174 (a) (1)` (*Snow*);
//! - a range of sections, `§§ 991-997` (*Boeing*);
//! - a reporter name that OCR misread, `26 V.S.C.` (*Shami*).

use words_to_data::citation::resolve::{
    CitedSection, Resolution, SectionPaths, Subdivision, resolve,
};
use words_to_data::citation::usc::SkipReason;
use words_to_data::citation::{Opinion, cites_links, usc};
use words_to_data::courtlistener::markup::text_of_markup;
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, case_expression};
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::judicial::reliance;
use words_to_data::link::LinkKind;
use words_to_data::storage::{InMemoryStorage, LinkReader};

const CACHE: &str = "tests/test_data/courtlistener";
const TITLE_26: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const RELEASE: &str = "2025-07-30";

/// Section 174 as the 2025-07-30 release point publishes it.
const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

/// One committed opinion record, as the API returned it.
fn opinion(id: u64) -> OpinionRecord {
    let json = std::fs::read_to_string(format!("{CACHE}/opinion_{id}.json"))
        .unwrap_or_else(|error| panic!("opinion {id} should be committed: {error}"));
    OpinionRecord::from_json(&json).expect("the opinion record should read")
}

/// The text the dataset stores for one opinion: the field the reader chooses.
fn stored_text(id: u64) -> String {
    let (text, _, _) = opinion(id).text().expect("the record carries text");
    text
}

/// Title 26 as it read on 2025-07-30, the case of one opinion record with that
/// record as its one writing, and the links its text makes, the way
/// `add-opinions` writes them.
fn dataset_with_opinion(opinion_id: u64, cluster_id: u64) -> Dataset<InMemoryStorage> {
    let mut dataset = Dataset::new(DatasetMetadata {
        name: "One opinion and title 26".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(TITLE_26, RELEASE, None)
        .expect("title 26 should parse");
    let title = dataset
        .get_expression(&ExpressionId::new(WorkId::new("uscode/title_26"), RELEASE))
        .expect("storage should answer")
        .expect("title 26 should be held");
    let mut index = SectionPaths::new();
    index.add_work(&title.root);

    let cluster_json = std::fs::read_to_string(format!("{CACHE}/cluster_{cluster_id}.json"))
        .expect("the cluster should be committed");
    let cluster = ClusterRecord::from_json(&cluster_json).expect("the cluster should read");
    let case = case_expression(&cluster, &[opinion(opinion_id)]).expect("the case should build");
    let writing = case.writings[0].path.clone();
    // The whole writing, as `add-opinions` reads it: its words are in its
    // paragraphs and footnotes.
    let text = case.writings[0].text.clone();
    dataset
        .add_expression(case.expression)
        .expect("the case should store");

    let scope = dataset.scope().expect("the scope should derive");
    let citing = Opinion::held(writing, opinion_id.to_string(), cluster.display());
    for citation in usc::find_with_report(&text).0 {
        let cited = resolve(&citation, &scope, &index);
        for link in cites_links(&citing, &citation, &cited) {
            dataset.add_link(link).expect("a link should store");
        }
    }
    dataset
}

#[test]
fn should_link_the_written_subdivision_when_snow_sets_a_space_before_each_bracket() {
    let dataset = dataset_with_opinion(109019, 109019);

    let cases = reliance::cases_citing(&dataset, SECTION_174).expect("the query should run");

    assert_eq!(cases.len(), 1, "Snow cites section 174, got {cases:?}");
    let cited = &cases[0].cites;
    assert_eq!(cited.len(), 1, "one link, got {cited:?}");
    assert_eq!(
        cited[0].subdivision,
        Subdivision::Resolved {
            resolved_path: format!("{SECTION_174}/subsection_a/paragraph_1")
        },
        "the link keeps the (a)(1) Snow wrote"
    );
}

#[test]
fn should_read_the_subdivision_when_a_space_stands_before_each_bracket() {
    // *Snow v. Commissioner*, 416 U.S. 500 (1974), opinion 109019. Its
    // `html_lawbox` sets a space before each bracket. The text stored for Snow
    // is Harvard's since ADR 0014, so Lawbox's field is read here on its own.
    let lawbox = opinion(109019)
        .html_lawbox
        .expect("Snow carries Lawbox's HTML");
    let (text, _) = text_of_markup(&lawbox);
    assert!(
        text.contains("26 U. S. C. \u{a7} 174 (a) (1)"),
        "the committed text writes the spaced form"
    );

    let found = usc::find(&text);

    let snow: Vec<_> = found
        .iter()
        .filter(|citation| citation.title == "26")
        .collect();
    assert_eq!(snow.len(), 1, "one citation, got {found:?}");
    assert_eq!(
        snow[0].sections,
        ["174(a)(1)"],
        "the subdivision is read, the same as `\u{a7} 174(a)(1)`"
    );
    assert_eq!(
        snow[0].text, "26 U. S. C. \u{a7} 174 (a) (1)",
        "the evidence stays as the opinion wrote it"
    );
}

/// The structural path of every `judicial.cites` link the dataset holds.
fn cited_paths(dataset: &Dataset<InMemoryStorage>) -> Vec<String> {
    dataset
        .links_by_kind(LinkKind::CITES)
        .expect("storage should answer")
        .into_iter()
        .filter_map(|link| link.object.path().map(str::to_string))
        .collect()
}

#[test]
fn should_link_each_end_of_a_range_when_boeing_cites_sections_991_to_997() {
    // *Boeing Co. v. United States*, 537 U.S. 437 (2003): the Court's opinion,
    // record 9434364, writes `26 U. S. C. §§ 991-997` three times. Title 26 has no section
    // numbered `991-997`, so the hyphen is a range, and both ends are sections
    // of the title.
    let text = stored_text(9434364);
    assert!(text.contains("26 U. S. C. \u{a7}\u{a7} 991-997"));

    let dataset = dataset_with_opinion(9434364, 122262);

    let paths = cited_paths(&dataset);
    for end in ["section_991", "section_997"] {
        assert!(
            paths.iter().any(|path| path.ends_with(&format!("/{end}"))),
            "the range links {end}, got {paths:?}"
        );
    }
}

/// Every section the citations in `text` name, resolved in one title of the
/// 2025-07-30 release.
fn resolved(title_file: &str, work: &str, text: &str) -> Vec<CitedSection> {
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

    usc::find(text)
        .iter()
        .flat_map(|citation| resolve(citation, &scope, &index))
        .collect()
}

/// Every path the citations in `text` resolve to. A section that does not
/// resolve fails the test.
fn resolved_paths(title_file: &str, work: &str, text: &str) -> Vec<String> {
    resolved(title_file, work, text)
        .into_iter()
        .flat_map(|cited| match cited.resolution {
            Resolution::Provision { paths } => paths,
            other => panic!("{} should resolve, got {other:?}", cited.uslm_id),
        })
        .collect()
}

#[test]
fn should_resolve_each_end_of_a_range_when_the_dash_is_an_en_dash() {
    // Real prose from the notes to the Federal Rules of Criminal Procedure, in
    // `tests/test_data/usc/2025-07-30/usc18a.xml`, which writes a range with an
    // en dash and a narrow no-break space after the section sign.
    let text = "pretrial release under the Bail Reform Act of 1966 \
                (18 U.S.C. \u{a7}\u{a7}\u{202f}3141\u{2013}3152).";

    let paths = resolved_paths(
        "tests/test_data/usc/2025-07-30/usc18.xml",
        "uscode/title_18",
        text,
    );

    assert_eq!(paths.len(), 2, "one path for each end, got {paths:?}");
    assert!(paths[0].ends_with("/section_3141"), "got {paths:?}");
    assert!(paths[1].ends_with("/section_3152"), "got {paths:?}");
}

#[test]
fn should_resolve_each_end_of_a_range_when_the_second_end_is_written_short() {
    // Real prose from the notes to the Federal Rules of Civil Procedure, in
    // `tests/test_data/usc/2025-07-30/usc28a.xml`. `1740–42` is 1740 to 1742,
    // written the way a range in prose often is.
    let text = "See also 28 U.S.C. \u{a7}\u{a7}\u{202f}1740\u{2013}42, 1745;";

    let cited = resolved(
        "tests/test_data/usc/2025-07-30/usc28.xml",
        "uscode/title_28",
        text,
    );

    let named: Vec<&str> = cited.iter().map(|cited| cited.uslm_id.as_str()).collect();
    assert_eq!(
        named,
        [
            "/us/usc/t28/s1740",
            "/us/usc/t28/s1742",
            "/us/usc/t28/s1745"
        ],
        "both ends of the range and the section after it"
    );
    // Section 1742 is repealed, and the parser does not hold a repealed
    // section, so that end is Absent. It is named, and not dropped.
    assert_eq!(cited[1].resolution, Resolution::Absent);
}

#[test]
fn should_link_sections_1361_and_1363_and_name_the_repair_when_ocr_reads_the_reporter_as_v_s_c() {
    // *Shami v. Commissioner* (5th Cir. 2014), opinion 2651100. Its Harvard
    // text is OCR, and footnote 6 reads `26 V.S.C. §§ 1361(a)(1), 1363(a)` where
    // the court wrote `26 U.S.C.`. Until #341 the citation was reported and not
    // read, and both links were lost. "V.S.C." is a known OCR form of "U.S.C.",
    // so it is read, and the evidence of each link says that it was repaired.
    let text = stored_text(2651100);
    assert!(
        text.contains("26 V.S.C."),
        "the committed text writes the OCR form"
    );

    let dataset = dataset_with_opinion(2651100, 2651100);

    let links = dataset
        .links_by_kind(LinkKind::CITES)
        .expect("storage should answer");
    for section in ["section_1361", "section_1363"] {
        let link = links
            .iter()
            .find(|link| {
                link.object
                    .path()
                    .is_some_and(|path| path.ends_with(&format!("/{section}")))
            })
            .unwrap_or_else(|| panic!("Shami cites {section}, got {links:?}"));
        let evidence = link
            .provenance
            .evidence
            .as_ref()
            .and_then(|evidence| evidence.reasoning.as_deref())
            .expect("a citation link carries the words it read");
        assert!(
            evidence.starts_with("26 V.S.C."),
            "the evidence keeps the words as the text wrote them, got {evidence:?}"
        );
        assert!(
            evidence.contains("OCR repair: \"V.S.C.\" read as \"U.S.C.\""),
            "the evidence names the repair, got {evidence:?}"
        );
    }

    let (_, report) = usc::find_with_report(&text);
    assert!(
        report
            .skipped
            .iter()
            .all(|skipped| skipped.reason != SkipReason::NearMissReporter),
        "a citation that is read is not reported as a skip, got {report:?}"
    );
}

#[test]
fn should_read_the_citation_when_the_reporter_is_written_u_s_c_with_no_last_point() {
    // *District of Columbia v. USDA* (D.D.C. 2020), opinion 4578098, paragraph
    // 97 writes `7 U.S.C § 2015(o)(4)(A)`, with no point after the C. The marker
    // and the number make it a citation, and no character is changed to read it,
    // so it is read as written and is no repair (#341).
    let text = stored_text(4578098);
    assert!(
        text.contains("7 U.S.C \u{a7} 2015(o)(4)(A)"),
        "the committed text writes the form"
    );

    let (found, report) = usc::find_with_report(&text);

    let citation = found
        .iter()
        .find(|citation| citation.text == "7 U.S.C \u{a7} 2015(o)(4)(A)")
        .unwrap_or_else(|| panic!("the citation is read, got {report:?}"));
    assert_eq!(citation.title, "7");
    assert_eq!(citation.sections, ["2015(o)(4)(A)"]);
    assert_eq!(citation.repair, None, "nothing was repaired");
    assert!(
        report
            .skipped
            .iter()
            .all(|skipped| skipped.reason != SkipReason::NearMissReporter),
        "no near miss is left, got {report:?}"
    );
}

#[test]
fn should_read_a_citation_with_no_section_marker_when_a_subdivision_is_written_onto_the_number() {
    // With no marker, a bare number after the reporter can be a page or an
    // edition year ("26 U.S.C. 1958 ed."), so it stays unread. A page or a year
    // never carries a bracketed subdivision, so `2015(o)(6)(G)` is a section
    // (#341). *District of Columbia v. USDA*, opinion 4578098, writes
    // `7 U.S.C. 2015(o)(6)(G)` and `7 U.S.C. 2015(o)(4)(A)`; *BMW (US) Holding
    // Corp.*, opinion 11409814, writes `26 U.S.C. 7508A(a)`.
    for (opinion_id, written, section) in [
        (4578098, "7 U.S.C. 2015(o)(6)(G)", "2015(o)(6)(G)"),
        (4578098, "7 U.S.C. 2015(o)(4)(A)", "2015(o)(4)(A)"),
        (11409814, "26 U.S.C. 7508A(a)", "7508A(a)"),
    ] {
        let text = stored_text(opinion_id);
        assert!(
            text.contains(written),
            "the committed text writes {written}"
        );

        let (found, report) = usc::find_with_report(&text);

        let citation = found
            .iter()
            .find(|citation| citation.text == written)
            .unwrap_or_else(|| panic!("{written} is read, got {report:?}"));
        assert_eq!(citation.sections, [section]);
        assert!(
            report.skipped.iter().all(|skipped| skipped.text != written),
            "{written} is not also reported, got {report:?}"
        );
    }
}
