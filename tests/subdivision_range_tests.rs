//! A range of subdivisions after one section, as in `26 U. S. C.
//! §§994(a)(1)-(3)`, is a citation of each subdivision in the range (#366).
//!
//! Every text here is real: the committed CourtListener records in
//! `tests/test_data/courtlistener`, added by `add-opinions` offline, and the
//! Code's own text in the committed corpus.

use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::citation::resolve::{Reached, SectionPaths, resolve};
use words_to_data::citation::usc;
use words_to_data::dataset::{Dataset, DatasetMetadata, ExpressionId, WorkId};
use words_to_data::document::DocumentNode;
use words_to_data::link::{Link, LinkKind};
use words_to_data::storage::{InMemoryStorage, LinkReader, SqliteStorage};
use words_to_data::utils::parse_uslm_xml;

const TITLE_26: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const RELEASE: &str = "2025-07-30";

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

const SECTION_994: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_IV/subpart_A/section_994";

/// The Court's opinion in *Boeing Co. v. United States*, 537 U.S. 437 (2003).
const BOEING_COURT: &str = "judicial/case_122262/opinion_9434364";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// Title 26 as it read on 2025-07-30, and the opinions `opinions` added by
/// `add-opinions` offline, in a new SQLite file named `name`.
fn build(name: &str, titles: &[&str], opinions: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let mut dataset = Dataset::new(DatasetMetadata {
        name: name.to_string(),
        description: "Ranges of subdivisions".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    for title in titles {
        dataset
            .add_uslm_xml(title, RELEASE, None)
            .unwrap_or_else(|error| panic!("{title} should parse: {error}"));
    }
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    let added = run(&[
        "add-opinions",
        &path,
        "--offline",
        "--cache-dir",
        CACHE,
        "--opinions",
        opinions,
    ]);
    assert!(
        added.status.success(),
        "add-opinions should store {opinions}, stderr: {}",
        String::from_utf8_lossy(&added.stderr)
    );
    path
}

fn boeing() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| build("subdivision_range_boeing", &[TITLE_26], "9434364"))
}

fn citation_links(path: &str) -> Vec<Link> {
    Dataset::<SqliteStorage>::open_sqlite(path)
        .expect("the fixture should open")
        .links_by_kind(LinkKind::CITES)
        .expect("the links should read")
}

/// Every place in a link's payload, as (written form, words, unresolved part).
fn occurrences(link: &Link) -> Vec<(String, String, Option<String>)> {
    let payload = &link
        .payload
        .as_ref()
        .expect("a citation has a payload")
        .value;
    payload["occurrences"]
        .as_array()
        .expect("the payload lists every place the court wrote it")
        .iter()
        .map(|place| {
            (
                place["written"]
                    .as_str()
                    .expect("a written form")
                    .to_string(),
                place["words"].as_str().expect("the words").to_string(),
                place["unresolved"].as_str().map(str::to_string),
            )
        })
        .collect()
}

#[test]
fn should_link_each_paragraph_of_the_range_when_boeing_cites_994_a_1_to_3() {
    // *Boeing*, 537 U.S. at 441: "the statute provides three alternative ways
    // in which the parties may divert a limited portion of taxable income from
    // the parent to the DISC. See 26 U. S. C. §§994(a)(1)-(3)."
    let links = citation_links(boeing());
    let range: Vec<&Link> = links
        .iter()
        .filter(|link| {
            occurrences(link)
                .iter()
                .any(|(written, _, _)| written == "994(a)(1)-(3)")
        })
        .collect();

    let mut objects: Vec<&str> = range.iter().filter_map(|link| link.object.path()).collect();
    objects.sort();
    assert_eq!(
        objects,
        [
            format!("{SECTION_994}/subsection_a/paragraph_1"),
            format!("{SECTION_994}/subsection_a/paragraph_2"),
            format!("{SECTION_994}/subsection_a/paragraph_3"),
        ],
        "the range is a link to each of its three paragraphs"
    );
    for link in &range {
        let subject = link.subject.path().expect("the subject is a path");
        assert!(
            subject.starts_with(&format!("{BOEING_COURT}/page_441/paragraph_")),
            "the subject is the paragraph on page 441: {subject}"
        );
        assert_eq!(
            occurrences(link),
            [(
                "994(a)(1)-(3)".to_string(),
                "26 U. S. C. \u{a7}\u{a7}994(a)(1)-(3)".to_string(),
                None
            )],
            "each link holds the whole written form"
        );
    }
}

const TITLE_7: &str = "tests/test_data/usc/2025-07-30/usc07.xml";

const SECTION_2015_O_6: &str = "uscode/title_7/chapter_51/section_2015/subsection_o/paragraph_6";

fn district_of_columbia() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| build("subdivision_range_dc_usda", &[TITLE_7], "4516443"))
}

#[test]
fn should_link_each_subparagraph_of_the_range_when_a_range_of_letters_is_written() {
    // *District of Columbia v. USDA* (D.D.C. 2020), opinion 4516443: "states
    // to exempt ... up to 15% of all “covered individuals in the State.” 7
    // U.S.C. § 2015(o)(6)(C)–(D)", and "(citing 7 U.S.C. § 2015(o)(6)(C)–(E))".
    let links = citation_links(district_of_columbia());
    let objects_of = |written: &str| {
        let mut objects: Vec<String> = links
            .iter()
            .filter(|link| occurrences(link).iter().any(|(form, _, _)| form == written))
            .filter_map(|link| link.object.path().map(str::to_string))
            .collect();
        objects.sort();
        objects.dedup();
        objects
    };

    assert_eq!(
        objects_of("2015(o)(6)(C)\u{2013}(D)"),
        [
            format!("{SECTION_2015_O_6}/subparagraph_C"),
            format!("{SECTION_2015_O_6}/subparagraph_D"),
        ]
    );
    assert_eq!(
        objects_of("2015(o)(6)(C)\u{2013}(E)"),
        [
            format!("{SECTION_2015_O_6}/subparagraph_C"),
            format!("{SECTION_2015_O_6}/subparagraph_D"),
            format!("{SECTION_2015_O_6}/subparagraph_E"),
        ]
    );
    let words: Vec<String> = links
        .iter()
        .flat_map(occurrences)
        .filter(|(form, _, _)| form == "2015(o)(6)(C)\u{2013}(E)")
        .map(|(_, words, unresolved)| {
            assert_eq!(unresolved, None, "every member is held");
            words
        })
        .collect();
    assert_eq!(
        words, ["7 U.S.C. \u{a7} 2015(o)(6)(C)\u{2013}(E)"; 3],
        "each link holds the whole written form"
    );
}

const TITLE_28: &str = "tests/test_data/usc/2025-07-30/usc28.xml";
const TITLE_11_APPENDIX: &str = "tests/test_data/usc/2025-07-30/usc11a.xml";

const SECTION_158_D_2_A: &str =
    "uscode/title_28/part_I/chapter_6/section_158/subsection_d/paragraph_2/subparagraph_A";

/// The words of the first node below `node` whose words hold `needle`.
fn words_holding(node: &DocumentNode, needle: &str) -> Option<String> {
    if let Some(content) = node.data.content.as_deref()
        && content.contains(needle)
    {
        return Some(content.to_string());
    }
    node.children
        .iter()
        .find_map(|child| words_holding(child, needle))
}

/// One title of the Code as it read on 2025-07-30, held in memory, and an index
/// of its sections.
fn title(file: &str, work: &str) -> (Dataset<InMemoryStorage>, SectionPaths) {
    let mut dataset = Dataset::new(DatasetMetadata::default());
    dataset
        .add_uslm_xml(file, RELEASE, None)
        .unwrap_or_else(|error| panic!("{file} should parse: {error}"));
    let printing = dataset
        .get_expression(&ExpressionId::new(WorkId::new(work), RELEASE))
        .expect("storage should answer")
        .unwrap_or_else(|| panic!("{work} should be held"));
    let mut index = SectionPaths::new();
    index.add_work(&printing.root);
    (dataset, index)
}

#[test]
fn should_reach_each_clause_of_the_range_when_a_range_of_roman_numerals_is_written() {
    // Bankruptcy Rule 8006(f)(2)(D), as the title 11 appendix of the
    // 2025-07-30 release prints it: "the reasons why a direct appeal should be
    // allowed, including which circumstance specified in 28 U.S.C.
    // § 158(d)(2)(A)(i)–(iii) applies".
    let rules = parse_uslm_xml(TITLE_11_APPENDIX, RELEASE).expect("the appendix should parse");
    let text = words_holding(&rules, "158(d)(2)(A)(i)").expect("the rule writes the range");
    let (dataset, index) = title(TITLE_28, "uscode/title_28");
    let scope = dataset.scope().expect("the scope should derive");

    let found = usc::find(&text);
    let [citation] = found.as_slice() else {
        panic!("the rule writes one citation: {found:#?}");
    };
    assert_eq!(citation.sections, ["158(d)(2)(A)(i)\u{2013}(iii)"]);
    let [cited] = resolve(citation, &scope, &index)
        .try_into()
        .expect("one section");

    let reached = |clause: &str| Reached {
        path: format!("{SECTION_158_D_2_A}/clause_{clause}"),
        unresolved: None,
    };
    assert_eq!(
        cited.reached,
        [reached("i"), reached("ii"), reached("iii")],
        "(i)–(iii) of a subparagraph are three clauses, not nine letters"
    );
}

#[test]
fn should_link_only_the_two_ends_when_boeing_cites_the_section_range_991_to_997() {
    // "§§ 991-997" is a range of sections, and #309 decided that its link names
    // the two ends the court wrote and not the sections between them. A range
    // of subdivisions does not change that.
    let links = citation_links(boeing());
    let mut objects: Vec<&str> = links
        .iter()
        .filter(|link| {
            occurrences(link)
                .iter()
                .any(|(written, _, _)| written == "991-997")
        })
        .filter_map(|link| link.object.path())
        .collect();
    objects.sort();
    objects.dedup();
    assert_eq!(
        objects,
        [
            "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_IV/subpart_A/section_991",
            "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_IV/subpart_B/section_997",
        ]
    );
}

/// What `26 U.S.C. § <section>` reaches in the 2025-07-30 printing of
/// title 26: the section written as the reader keeps it, resolved as
/// `add-opinions` resolves it.
fn reached_in_title_26(section: &str) -> Vec<Reached> {
    static TITLE_26_INDEX: OnceLock<(Dataset<InMemoryStorage>, SectionPaths)> = OnceLock::new();
    let (dataset, index) = TITLE_26_INDEX.get_or_init(|| title(TITLE_26, "uscode/title_26"));
    let scope = dataset.scope().expect("the scope should derive");
    let citation = usc::UscCitation {
        title: "26".to_string(),
        sections: vec![section.to_string()],
        text: format!("26 U.S.C. \u{a7} {section}"),
        start: 0,
        repair: None,
        default_code: None,
        edition: None,
    };
    let [cited] = resolve(&citation, &scope, index)
        .try_into()
        .expect("one section");
    cited.reached
}

#[test]
fn should_name_the_range_and_guess_no_member_when_the_two_ends_are_at_different_levels() {
    // The shapes are the Code's own: its notes write `§ 701(b)(1)–(3)(A)` and
    // `§ 4001(a)(4)(A)–(C)(i)`, where the second end goes one level deeper than
    // the first. What lies between the ends is not a list of siblings, so the
    // range leads to the provision that holds both ends and is named whole.
    let paragraph_a = Reached {
        path: format!("{SECTION_994}/subsection_a"),
        unresolved: Some("(1)\u{2013}(3)(A)".to_string()),
    };
    assert_eq!(
        reached_in_title_26("994(a)(1)\u{2013}(3)(A)"),
        [paragraph_a]
    );
    // A paragraph to a subparagraph: one level to another.
    assert_eq!(
        reached_in_title_26("994(a)(1)\u{2013}(B)"),
        [Reached {
            path: format!("{SECTION_994}/subsection_a"),
            unresolved: Some("(1)\u{2013}(B)".to_string()),
        }]
    );
    // A subsection and a paragraph of another subsection.
    assert_eq!(
        reached_in_title_26("994(a)(1)-(b)(2)"),
        [Reached {
            path: SECTION_994.to_string(),
            unresolved: Some("(a)(1)-(b)(2)".to_string()),
        }]
    );
}

#[test]
fn should_name_each_member_that_is_not_held_when_the_range_runs_past_the_printing() {
    // § 994(a) has three paragraphs. A range to (5) is three links to what is
    // held, and the members (4) and (5) lead to § 994(a) and are named.
    let paragraph = |n: &str| Reached {
        path: format!("{SECTION_994}/subsection_a/paragraph_{n}"),
        unresolved: None,
    };
    let not_held = |n: &str| Reached {
        path: format!("{SECTION_994}/subsection_a"),
        unresolved: Some(format!("({n})")),
    };
    assert_eq!(
        reached_in_title_26("994(a)(1)-(5)"),
        [
            paragraph("1"),
            paragraph("2"),
            paragraph("3"),
            not_held("4"),
            not_held("5")
        ]
    );
    // A parent that is not held: no member can be counted below it.
    assert_eq!(
        reached_in_title_26("994(z)(1)-(3)"),
        [Reached {
            path: SECTION_994.to_string(),
            unresolved: Some("(z)(1)-(3)".to_string()),
        }]
    );
}

#[test]
fn should_reach_each_member_when_the_ends_are_joined_by_through() {
    // The Code's notes write `section 838i(b)(2) through (b)(7)`, with the
    // parent written again in the second end, and `section 1017(c) through (i)`.
    let paragraph = |n: &str| Reached {
        path: format!("{SECTION_994}/subsection_a/paragraph_{n}"),
        unresolved: None,
    };
    let all_three = [paragraph("1"), paragraph("2"), paragraph("3")];
    assert_eq!(reached_in_title_26("994(a)(1) through (3)"), all_three);
    assert_eq!(reached_in_title_26("994(a)(1) through (a)(3)"), all_three);
}

#[test]
fn should_ask_about_each_member_when_cases_citing_is_given_a_range_of_subdivisions() {
    // The question can be written the way *Boeing* wrote the citation. Each
    // member of the range is a path asked about, and no part of the range is
    // reported as a designation that does not resolve.
    let output = run(&[
        "cases-citing",
        boeing(),
        "--cites",
        "26 U. S. C. \u{a7}\u{a7}994(a)(1)-(3)",
        "--json",
    ]);
    assert!(
        output.status.success(),
        "cases-citing should answer, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let answers: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("the answer is JSON");
    let asked: Vec<&str> = answers
        .as_array()
        .expect("one entry per path asked about")
        .iter()
        .map(|answer| answer["path"].as_str().expect("each names its path"))
        .collect();
    assert_eq!(
        asked,
        [
            format!("{SECTION_994}/subsection_a/paragraph_1"),
            format!("{SECTION_994}/subsection_a/paragraph_2"),
            format!("{SECTION_994}/subsection_a/paragraph_3"),
        ]
    );
}
