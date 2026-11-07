//! A reader shows the links of one citation range as one range, as the court
//! wrote it (#383).
//!
//! `§§ 991-997` is a link to each of its two ends (#309), and `§§994(a)(1)-(3)`
//! is a link to each member (#366). Each of those links holds the place the
//! court wrote the range: the same passage, the same `start` and the same
//! `written` form. `cases-citing` and `annotations` group the links by that
//! place when they read them. Nothing new is stored.
//!
//! Every text here is real: the committed CourtListener records in
//! `tests/test_data/courtlistener`, added by `add-opinions` offline, and the
//! Code's own text in the committed corpus.

use std::process::{Command, Output};
use std::sync::OnceLock;

use serde_json::Value;
use words_to_data::dataset::{Dataset, DatasetMetadata};

const TITLE_26: &str = "tests/test_data/usc/2025-07-30/usc26.xml";
const RELEASE: &str = "2025-07-30";

/// Where the committed CourtListener records are, as a cache directory.
const CACHE: &str = "tests/test_data";

/// The part of title 26 that holds §§ 991 to 997, the DISC provisions.
const PART_IV: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_IV";
const SECTION_991: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_IV/subpart_A/section_991";
const SECTION_997: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_IV/subpart_B/section_997";

/// The Court's opinion in *Boeing Co. v. United States*, 537 U.S. 437 (2003).
const BOEING_COURT: &str = "judicial/case_122262/opinion_9434364";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

/// One title of the Code as it read on 2025-07-30, and the opinions
/// `opinions` added by `add-opinions` offline, in a new SQLite file.
fn build(name: &str, title: &str, opinions: &str) -> String {
    let path = format!("{}/{name}.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    let mut dataset = Dataset::new(DatasetMetadata {
        name: name.to_string(),
        description: "Citation ranges".to_string(),
        author: "words_to_data tests".to_string(),
        license: "Public Domain".to_string(),
        version: "1.0".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(title, RELEASE, None)
        .unwrap_or_else(|error| panic!("{title} should parse: {error}"));
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
    PATH.get_or_init(|| build("citation_range_boeing", TITLE_26, "9434364"))
}

/// The JSON a command prints. The command must succeed.
fn json_of(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?} should answer, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("the answer is JSON")
}

/// The ranges `cases-citing --path <path> --json` gives for the writing
/// `writing`.
fn ranges_in_cases_citing(dataset: &str, path: &str, writing: &str) -> Vec<Value> {
    let answers = json_of(&["cases-citing", dataset, "--path", path, "--json"]);
    let [answer] = answers.as_array().expect("one entry per path").as_slice() else {
        panic!("one path was asked about: {answers:#}");
    };
    let case = answer["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["citing"]["held"]["path"] == writing)
        .unwrap_or_else(|| panic!("{writing} cites the path: {answer:#}"));
    case["ranges"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("the case lists its ranges: {case:#}"))
}

/// The paths a range links, in its order.
fn members(range: &Value) -> Vec<&str> {
    range["members"]
        .as_array()
        .expect("a range lists its members")
        .iter()
        .map(|member| member["path"].as_str().expect("each member names a path"))
        .collect()
}

#[test]
fn should_give_one_range_for_each_passage_when_boeing_writes_sections_991_to_997() {
    // *Boeing* writes "26 U. S. C. §§ 991-997" on pages 440, 442 and 446. Each
    // is one range with its two ends, and the three are not one group of six.
    let ranges = ranges_in_cases_citing(boeing(), PART_IV, BOEING_COURT);
    let sections: Vec<&Value> = ranges
        .iter()
        .filter(|range| range["written"] == "991-997")
        .collect();
    let passages: Vec<&str> = sections
        .iter()
        .map(|range| range["passage"].as_str().expect("a passage"))
        .collect();
    assert_eq!(
        passages,
        [
            format!("{BOEING_COURT}/page_440/paragraph_4"),
            format!("{BOEING_COURT}/page_442/paragraph_2"),
            format!("{BOEING_COURT}/page_446/paragraph_2"),
        ]
    );
    for range in sections {
        assert_eq!(members(range), [SECTION_991, SECTION_997]);
        // The words are as each passage wrote them: page 440 puts a space
        // after the marks, and page 442 does not.
        let words = range["words"].as_str().expect("the words");
        assert!(
            words.starts_with("26 U. S. C. \u{a7}\u{a7}") && words.ends_with("991-997"),
            "the words the rule matched: {words}"
        );
        for member in range["members"].as_array().expect("members") {
            assert!(
                member["link"].as_str().is_some_and(|id| !id.is_empty()),
                "each member names its link: {member}"
            );
        }
    }
}

const SECTION_994: &str =
    "uscode/title_26/subtitle_A/chapter_1/subchapter_N/part_IV/subpart_A/section_994";

/// What a command prints for a person. The command must succeed.
fn text_of(args: &[&str]) -> String {
    let output = run(args);
    assert!(
        output.status.success(),
        "{args:?} should answer, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("the answer is UTF-8")
}

/// The lines that follow the first line holding `header`, up to `count`.
fn lines_after<'a>(text: &'a str, header: &str, count: usize) -> Vec<&'a str> {
    let mut lines = text.lines();
    lines
        .by_ref()
        .find(|line| line.contains(header))
        .unwrap_or_else(|| panic!("no line holds {header:?}:\n{text}"));
    lines.take(count).collect()
}

#[test]
fn should_show_the_range_as_one_item_with_its_members_when_cases_citing_reads_994_a_1_to_3() {
    // *Boeing*, 537 U.S. at 441: "See 26 U. S. C. §§994(a)(1)-(3)." It is three
    // links (#366), and a person reads one citation with three provisions.
    let text = text_of(&["cases-citing", boeing(), "--path", SECTION_994]);
    let passage = format!("{BOEING_COURT}/page_441/paragraph_1");
    let header = format!(
        "cites one range in {passage}, written \"26 U. S. C. \u{a7}\u{a7}994(a)(1)-(3)\": \
         3 links, one to each provision below"
    );
    let members = lines_after(&text, &header, 3);
    for (line, paragraph) in members.iter().zip(["1", "2", "3"]) {
        let path = format!("{SECTION_994}/subsection_a/paragraph_{paragraph}");
        assert!(
            line.trim_start().starts_with(&format!("{path} — link ")),
            "each member is a line under the range: {line:?}\n{text}"
        );
    }
    assert_eq!(
        text.matches(&header).count(),
        1,
        "the range is one item:\n{text}"
    );
}

#[test]
fn should_name_the_range_on_each_end_when_annotations_lists_the_citations_of_991_to_997() {
    let answer = json_of(&["annotations", boeing(), "--path", PART_IV, "--json"]);
    let passage = format!("{BOEING_COURT}/page_440/paragraph_4");
    let rows: Vec<&Value> = answer["citations"]
        .as_array()
        .expect("citations")
        .iter()
        .filter(|row| row["opinion"] == passage.as_str())
        .collect();
    let cited: Vec<&str> = rows
        .iter()
        .map(|row| row["cited_path"].as_str().expect("a cited path"))
        .collect();
    assert_eq!(
        cited,
        [SECTION_991, SECTION_997],
        "page 440 cites the two ends"
    );

    let ids: Vec<&str> = rows
        .iter()
        .map(|row| row["id"].as_str().expect("an id"))
        .collect();
    for row in &rows {
        let [range] = row["ranges"]
            .as_array()
            .expect("each end names its range")
            .as_slice()
        else {
            panic!("each end is in one range: {row:#}");
        };
        assert_eq!(range["written"], "991-997");
        assert_eq!(range["passage"], passage.as_str());
        let links: Vec<&str> = range["members"]
            .as_array()
            .expect("members")
            .iter()
            .map(|member| member["link"].as_str().expect("a link"))
            .collect();
        assert_eq!(links, ids, "the range names the link of each end");
    }
}

#[test]
fn should_show_the_range_as_one_item_with_its_members_when_annotations_reads_994_a_1_to_3() {
    let text = text_of(&["annotations", boeing(), "--path", SECTION_994, "--all"]);
    let passage = format!("{BOEING_COURT}/page_441/paragraph_1");
    let header = format!(
        "cites one range in {passage}, written \"26 U. S. C. \u{a7}\u{a7}994(a)(1)-(3)\": \
         3 links, one to each provision below"
    );
    assert_eq!(
        text.matches(&header).count(),
        1,
        "the range is one item:\n{text}"
    );
    let members = lines_after(&text, &header, 3);
    for (line, paragraph) in members.iter().zip(["1", "2", "3"]) {
        let path = format!("{SECTION_994}/subsection_a/paragraph_{paragraph}");
        assert!(
            line.starts_with("    cited by ") && line.contains(&format!(", at {path} ")),
            "each member is a line under the range: {line:?}\n{text}"
        );
    }
    assert_eq!(
        text.matches(&format!("cited by {passage} ")).count(),
        3,
        "each member is told once:\n{text}"
    );
}

#[test]
fn should_not_name_a_range_when_a_passage_writes_a_single_citation() {
    // Page 446 writes "26 U. S. C. § 994", and page 447 writes the OCR form
    // "§§994(a)(1)(3)", which is no range: (1)(3) is one paragraph below
    // another, and no dash joins two ends. Each is one link and no range, and
    // its row is as it was before #383.
    let answer = json_of(&["annotations", boeing(), "--path", SECTION_994, "--json"]);
    let singles: Vec<&Value> = answer["citations"]
        .as_array()
        .expect("citations")
        .iter()
        .filter(|row| {
            [
                format!("{BOEING_COURT}/page_446/paragraph_3"),
                format!("{BOEING_COURT}/page_447/paragraph_9"),
            ]
            .iter()
            .any(|passage| row["opinion"] == passage.as_str())
        })
        .collect();
    assert_eq!(singles.len(), 2, "two single citations: {answer:#}");
    for row in singles {
        assert!(row.get("ranges").is_none(), "no range: {row:#}");
    }

    // The dissent cites § 994 once, and its case names no range.
    let answers = json_of(&["cases-citing", boeing(), "--path", SECTION_994, "--json"]);
    let dissent = answers[0]["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["citing"]["held"]["path"] == "judicial/case_122262/opinion_9434365")
        .expect("the dissent cites § 994");
    assert!(dissent.get("ranges").is_none(), "no range: {dissent:#}");

    let text = text_of(&["annotations", boeing(), "--path", SECTION_994, "--all"]);
    assert!(
        text.lines().any(|line| line.starts_with(&format!(
            "  cited by {BOEING_COURT}/page_446/paragraph_3 as \"26 U. S. C. \u{a7} 994\" ("
        ))),
        "a single citation is one line, as before:\n{text}"
    );
}

#[test]
fn should_name_each_link_of_the_range_when_the_path_asked_about_holds_one_end() {
    // Asked about § 991, the listing holds the link to § 991 alone. The range
    // still names its other end, and says that the listing does not hold it.
    let text = text_of(&["annotations", boeing(), "--path", SECTION_991, "--all"]);
    let header = format!(
        "cites one range in {BOEING_COURT}/page_440/paragraph_4, written \
         \"26 U. S. C. \u{a7}\u{a7} 991-997\": 2 links, one to each provision below"
    );
    let lines = lines_after(&text, &header, 2);
    assert!(
        lines[0].starts_with(&format!(
            "    cited by {BOEING_COURT}/page_440/paragraph_4 "
        )),
        "{text}"
    );
    assert!(
        lines[1].starts_with(&format!("    {SECTION_997} [link "))
            && lines[1].ends_with("], which this listing does not hold"),
        "{text}"
    );

    let ranges = ranges_in_cases_citing(boeing(), SECTION_991, BOEING_COURT);
    assert_eq!(ranges.len(), 3, "a range on each of three pages");
    for range in &ranges {
        assert_eq!(members(range), [SECTION_991, SECTION_997]);
    }
}

const TITLE_7: &str = "tests/test_data/usc/2025-07-30/usc07.xml";
const SECTION_2015_O_6: &str = "uscode/title_7/chapter_51/section_2015/subsection_o/paragraph_6";

#[test]
fn should_give_each_range_of_letters_its_own_members_when_two_ranges_share_their_first_end() {
    // *District of Columbia v. USDA* (D.D.C. 2020), opinion 4516443, writes
    // "7 U.S.C. § 2015(o)(6)(C)–(D)" in three paragraphs and
    // "§ 2015(o)(6)(C)–(E)" in paragraph 73 (#366).
    static PATH: OnceLock<String> = OnceLock::new();
    let dataset = PATH.get_or_init(|| build("citation_range_dc_usda", TITLE_7, "4516443"));
    let ranges = ranges_in_cases_citing(
        dataset,
        SECTION_2015_O_6,
        "judicial/case_4736096/opinion_4516443",
    );
    let written: Vec<(&str, Vec<&str>)> = ranges
        .iter()
        .map(|range| (range["written"].as_str().expect("written"), members(range)))
        .collect();
    let letters = |last: &[&str]| -> Vec<String> {
        last.iter()
            .map(|letter| format!("{SECTION_2015_O_6}/subparagraph_{letter}"))
            .collect()
    };
    let c_to_d = letters(&["C", "D"]);
    let c_to_e = letters(&["C", "D", "E"]);
    assert_eq!(
        written,
        [
            (
                "2015(o)(6)(C)\u{2013}(D)",
                c_to_d.iter().map(String::as_str).collect()
            ),
            (
                "2015(o)(6)(C)\u{2013}(D)",
                c_to_d.iter().map(String::as_str).collect()
            ),
            (
                "2015(o)(6)(C)\u{2013}(D)",
                c_to_d.iter().map(String::as_str).collect()
            ),
            (
                "2015(o)(6)(C)\u{2013}(E)",
                c_to_e.iter().map(String::as_str).collect()
            ),
        ]
    );
}
