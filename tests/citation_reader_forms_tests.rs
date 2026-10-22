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
use words_to_data::courtlistener::{ClusterRecord, OpinionRecord, opinion_expression, work_id};
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

/// Title 26 as it read on 2025-07-30, one opinion, and the links its text
/// makes, the way `add-opinions` writes them.
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
    let (expression, _, _) =
        opinion_expression(&opinion(opinion_id), &cluster).expect("the opinion should build");
    let text = expression
        .root
        .data
        .content
        .clone()
        .expect("an opinion has text");
    dataset
        .add_expression(expression)
        .expect("the opinion should store");

    let scope = dataset.scope().expect("the scope should derive");
    let citing = Opinion::held(
        work_id(opinion_id),
        opinion_id.to_string(),
        cluster.display(),
    );
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
    // *Snow v. Commissioner*, 416 U.S. 500 (1974), opinion 109019. Its text is
    // from `html_lawbox`, which sets a space before each bracket.
    let text = stored_text(109019);
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
    // *Boeing Co. v. United States*, 537 U.S. 437 (2003), opinion 122262,
    // writes `26 U. S. C. §§ 991-997` three times. Title 26 has no section
    // numbered `991-997`, so the hyphen is a range, and both ends are sections
    // of the title.
    let text = stored_text(122262);
    assert!(text.contains("26 U. S. C. \u{a7}\u{a7} 991-997"));

    let dataset = dataset_with_opinion(122262, 122262);

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
fn should_report_a_skip_when_ocr_reads_the_reporter_as_v_s_c() {
    // *Shami v. Commissioner* (5th Cir. 2014), opinion 2651100. Its Harvard
    // text is OCR, and footnote 6 reads `26 V.S.C.` where the court wrote
    // `26 U.S.C.`. The citation is not read, because the reporter is not one the
    // reader knows, and it is reported, so nobody takes the silence for "this
    // footnote cites no statute".
    let harvard = opinion(2651100)
        .xml_harvard
        .expect("the record carries Harvard text");
    let (text, _) = text_of_markup(&harvard);

    let (found, report) = usc::find_with_report(&text);

    assert!(
        found
            .iter()
            .all(|citation| !citation.text.contains("V.S.C.")),
        "a near-miss reporter is not linked, got {found:?}"
    );
    let skipped: Vec<_> = report
        .skipped
        .iter()
        .filter(|skipped| skipped.reason == SkipReason::NearMissReporter)
        .collect();
    assert_eq!(skipped.len(), 1, "one skip, got {report:?}");
    assert!(
        skipped[0].text.starts_with("26 V.S.C.") && skipped[0].text.ends_with("1361(a)(1)"),
        "the skip carries the text, got {:?}",
        skipped[0].text
    );
    assert!(
        report
            .summary()
            .iter()
            .any(|line| line.contains("26 V.S.C.")),
        "and the report a command prints names it, got {:?}",
        report.summary()
    );
}
