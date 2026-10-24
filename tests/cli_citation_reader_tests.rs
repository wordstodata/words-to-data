//! `path` and `annotations` read `judicial.cites` links from both ends (#301).
//!
//! A dataset with opinions holds one `judicial.cites` link for each U.S. Code
//! section an opinion cites. Before #301 only `cases-citing` read them, so no
//! command said what an opinion cites, and `path` on a cited section showed none
//! of the opinions that cite it.
//!
//! The fixture is title 26 as it read on 2025-07-30, and the ten real
//! CourtListener records committed under `tests/test_data/courtlistener`, added
//! by the real binary with `--offline`. Nine of the ten cite § 174. Nothing here
//! is written by hand.

use std::collections::BTreeSet;
use std::process::{Command, Output};
use std::sync::OnceLock;

use words_to_data::dataset::{Dataset, DatasetMetadata};
use words_to_data::storage::InMemoryStorage;

/// The ten opinions, as the command takes them.
const TEN: &str = "2812209,109019,122262,9434365,2651100,6248,6931314,8991218,1527901,406879";

/// The committed CourtListener records, as a cache directory.
const CACHE: &str = "tests/test_data";

const SECTION_174: &str = "uscode/title_26/subtitle_A/chapter_1/subchapter_B/part_VI/section_174";

/// *Shami v. Commissioner* (5th Cir. 2014). It cites § 174 as `26 U.S.C. § 174(e)`.
const SHAMI: &str = "judicial/case_2651100/opinion_2651100";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_words_to_data"))
        .args(args)
        .output()
        .expect("the binary should run")
}

fn stdout_of(output: &Output) -> String {
    assert!(
        output.status.success(),
        "the command should exit zero, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Title 26 and the ten opinions, in one SQLite file that every test reads.
///
/// Built once per test binary: title 26 is large, and no test here writes.
fn dataset() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = format!(
            "{}/cli_citation_readers.sqlite",
            env!("CARGO_TARGET_TMPDIR")
        );
        let _ = std::fs::remove_file(&path);
        let mut title_26: Dataset<InMemoryStorage> = Dataset::new(DatasetMetadata {
            name: "Title 26 and ten opinions".to_string(),
            description: "Opinions that cite 26 U.S.C. § 174".to_string(),
            author: "words_to_data tests".to_string(),
            license: "Public Domain".to_string(),
            version: "1.0".to_string(),
            ..Default::default()
        });
        title_26
            .add_uslm_xml(
                "tests/test_data/usc/2025-07-30/usc26.xml",
                "2025-07-30",
                None,
            )
            .expect("title 26 should parse");
        title_26
            .save_to_sqlite(&path)
            .expect("the fixture should save");
        stdout_of(&run(&[
            "add-opinions",
            &path,
            "--offline",
            "--cache-dir",
            CACHE,
            "--opinions",
            TEN,
        ]));
        path
    })
}

#[test]
fn should_list_the_cited_section_and_the_matched_words_when_path_names_an_opinion() {
    let printed = stdout_of(&run(&["path", dataset(), SHAMI]));

    let line = printed
        .lines()
        .find(|line| line.contains(SECTION_174))
        .unwrap_or_else(|| panic!("path on Shami should name § 174, got:\n{printed}"));
    assert!(
        line.contains("26 U.S.C. § 174(e)"),
        "the citation should show the words the rule matched, got: {line}"
    );
}

/// A case is a work, so `path` on it reports the citations of every writing
/// in it, and each line names the writing that cites. *Boeing*'s majority and
/// Justice Thomas's dissent both write § 174(a)(1), and a reader must be able to
/// tell the dissent's citation from the Court's (#313).
#[test]
fn should_name_the_citing_writing_on_each_line_when_path_names_a_case() {
    let printed = stdout_of(&run(&["path", dataset(), "judicial/case_122262"]));

    for writing in [
        "judicial/case_122262/opinion_9434364",
        "judicial/case_122262/opinion_9434365",
    ] {
        // Since #316 the line names the paragraph, and the paragraph's path
        // starts with the writing's.
        assert!(
            printed
                .lines()
                .any(|line| line.contains(&format!("{writing}/"))
                    && line.contains(&format!(" cites {SECTION_174}"))),
            "path on the case should say {writing} cites § 174, got:\n{printed}"
        );
    }
}

/// The nine writings that cite § 174, in path order. *Obergefell* (2812209) is
/// the tenth, and it cites no part of title 26. *Boeing*'s two are its
/// majority, 9434364, and Justice Thomas's dissent, 9434365.
const CITE_174: [&str; 9] = [
    "judicial/case_109019/opinion_109019",
    "judicial/case_122262/opinion_9434364",
    "judicial/case_122262/opinion_9434365",
    "judicial/case_1527901/opinion_1527901",
    "judicial/case_2651100/opinion_2651100",
    "judicial/case_406879/opinion_406879",
    "judicial/case_6248/opinion_6248",
    "judicial/case_7029371/opinion_6931314",
    "judicial/case_8998794/opinion_8991218",
];

#[test]
fn should_list_the_nine_citing_opinions_when_path_names_the_cited_section() {
    let printed = stdout_of(&run(&["path", dataset(), SECTION_174]));

    for opinion in CITE_174 {
        assert!(
            printed
                .lines()
                .any(|line| line.contains("cited by") && line.contains(opinion)),
            "path on § 174 should say {opinion} cites it, got:\n{printed}"
        );
    }
    assert!(
        !printed.contains("judicial/case_2812209"),
        "Obergefell does not cite § 174, got:\n{printed}"
    );
}

/// The five writings that write § 174(a) or § 174(a)(1), and the six that write
/// § 174 alone or § 174(e), which no held printing has. *Boeing*'s majority and
/// dissent write both.
const WRITE_174_A: [&str; 5] = [
    "judicial/case_109019/opinion_109019",
    "judicial/case_122262/opinion_9434364",
    "judicial/case_122262/opinion_9434365",
    "judicial/case_406879/opinion_406879",
    "judicial/case_6248/opinion_6248",
];
const WRITE_174_ALONE: [&str; 6] = [
    "judicial/case_122262/opinion_9434364",
    "judicial/case_122262/opinion_9434365",
    "judicial/case_1527901/opinion_1527901",
    "judicial/case_2651100/opinion_2651100",
    "judicial/case_7029371/opinion_6931314",
    "judicial/case_8998794/opinion_8991218",
];

/// A citation ends at the subdivision the court wrote (#316). So a reader
/// asking about § 174(a) learns which writings cite (a) or a provision in it,
/// and which cite only the section that holds (a), and is never told that a
/// citation of the section is a citation of (a).
#[test]
fn should_tell_a_citation_of_the_subsection_from_one_of_the_section_when_path_names_a_subsection() {
    let subsection = format!("{SECTION_174}/subsection_a");
    let printed = stdout_of(&run(&["path", dataset(), &subsection]));
    let lines_of = |writing: &str| -> Vec<&str> {
        printed
            .lines()
            .filter(|line| line.contains(&format!("{writing}/")))
            .collect()
    };

    for writing in WRITE_174_A {
        assert!(
            // "cited by" alone at (a) itself, and with the path below it.
            lines_of(writing)
                .iter()
                .any(|line| line.trim_start().starts_with("cited by")),
            "{writing} cites § 174(a) or a provision in it, got:\n{printed}"
        );
    }
    for writing in WRITE_174_ALONE {
        assert!(
            lines_of(writing)
                .iter()
                .any(|line| line.contains("cites the section that holds this path")),
            "{writing} cites § 174 alone, and the line should say so, got:\n{printed}"
        );
    }
    let shami = lines_of("judicial/case_2651100/opinion_2651100");
    assert!(
        shami
            .iter()
            .all(|line| line.contains("cites the section that holds this path")),
        "Shami's (e) is not (a), got: {shami:?}"
    );
}

/// The citations `path --json` reports on `opinion`, as parsed JSON.
fn citations_in_json(dataset: &str, path: &str) -> Vec<serde_json::Value> {
    let printed = stdout_of(&run(&["path", dataset, path, "--json"]));
    let report: serde_json::Value =
        serde_json::from_str(&printed).expect("path --json should print JSON");
    report["citations"]
        .as_array()
        .unwrap_or_else(|| panic!("path --json should carry `citations`, got:\n{printed}"))
        .clone()
}

/// The row for Shami's citation of § 174.
fn shami_cites_174(dataset: &str) -> serde_json::Value {
    citations_in_json(dataset, SHAMI)
        .into_iter()
        .find(|row| row["cited_path"] == SECTION_174)
        .expect("Shami's citations should include § 174")
}

/// A rule made the link and no person has looked at it. After a reviewer
/// confirms it, both readings say so: the maker's state stays, and the review
/// is reported beside it.
#[test]
fn should_report_the_review_state_of_a_citation_before_and_after_a_review() {
    let copy = format!(
        "{}/cli_citation_reviewed.sqlite",
        env!("CARGO_TARGET_TMPDIR")
    );
    std::fs::copy(dataset(), &copy).expect("the fixture should copy");

    let before = shami_cites_174(&copy);
    assert_eq!(before["verification"], "machine_suggested", "{before}");
    assert_eq!(before["review_state"], "unreviewed", "{before}");

    let id = before["id"]
        .as_str()
        .expect("the row should carry the link id");
    stdout_of(&run(&[
        "settle",
        &copy,
        "--link",
        id,
        "--verdict",
        "confirmed",
        "--reviewer",
        "human:tester",
        "--reason",
        "Shami cites 26 U.S.C. § 174(e) in its text.",
    ]));

    let after = shami_cites_174(&copy);
    assert_eq!(after["verification"], "machine_suggested", "{after}");
    assert_eq!(after["review_state"], "confirmed", "{after}");
    let printed = stdout_of(&run(&["path", &copy, SHAMI]));
    let line = printed
        .lines()
        .find(|line| line.contains(SECTION_174))
        .unwrap_or_else(|| panic!("path on Shami should name § 174, got:\n{printed}"));
    assert!(
        line.contains("confirmed by human:tester"),
        "the line should name the review, got: {line}"
    );
}

#[test]
fn should_include_citation_links_when_annotations_is_asked_about_an_opinion_or_a_cited_section() {
    let on_shami = stdout_of(&run(&["annotations", dataset(), "--path", SHAMI]));
    assert!(
        on_shami
            .lines()
            .any(|line| line.contains(SECTION_174) && line.contains("26 U.S.C. § 174(e)")),
        "annotations on Shami should name its citation of § 174, got:\n{on_shami}"
    );

    let on_174 = stdout_of(&run(&["annotations", dataset(), "--path", SECTION_174]));
    for opinion in CITE_174 {
        assert!(
            on_174.contains(opinion),
            "annotations on § 174 should name {opinion}, got:\n{on_174}"
        );
    }
}

/// `--json` is an interface for agents: the citations are added as named fields,
/// and no field an agent already reads is renamed or dropped.
#[test]
fn should_add_citations_and_keep_every_existing_field_when_json_is_asked() {
    let printed = stdout_of(&run(&[
        "annotations",
        dataset(),
        "--path",
        SECTION_174,
        "--json",
    ]));
    let listing: serde_json::Value =
        serde_json::from_str(&printed).expect("annotations --json should print JSON");
    assert!(listing["annotations"].is_array(), "{listing}");
    assert!(listing["total"].is_number(), "{listing}");
    let citations = listing["citations"]
        .as_array()
        .unwrap_or_else(|| panic!("annotations --json should carry `citations`, got:\n{listing}"));
    // One row for each passage that cites § 174 or a provision in it, and each
    // names its writing (#316).
    let writings: BTreeSet<&str> = citations
        .iter()
        .map(|row| row["writing"].as_str().expect("each row names its writing"))
        .collect();
    assert_eq!(writings, BTreeSet::from(CITE_174), "{listing}");
    for row in citations {
        let passage = row["opinion"].as_str().expect("each row names its passage");
        let writing = row["writing"].as_str().expect("each row names its writing");
        assert!(passage.starts_with(&format!("{writing}/")), "{row}");
    }
    assert_eq!(listing["citations_total"], 14, "{listing}");

    let printed = stdout_of(&run(&["path", dataset(), SECTION_174, "--json"]));
    let report: serde_json::Value =
        serde_json::from_str(&printed).expect("path --json should print JSON");
    for field in [
        "path",
        "present_in",
        "provisions",
        "unfollowed_redesignations",
        "annotations",
        "olrc_classifications",
        "citations",
    ] {
        assert!(
            report.get(field).is_some(),
            "path --json should carry `{field}`, got:\n{report}"
        );
    }
}

/// Shami's citations, recorded as a dataset records them for an opinion it does
/// **not** hold: the subject is `judicial.opinion:2651100`, not a node. The text
/// is the committed record's, and the links are made by the real rule.
fn title_26_with_shami_cited_from_outside() -> String {
    use words_to_data::citation::resolve::{SectionPaths, resolve};
    use words_to_data::citation::{Opinion, cites_links, usc};
    use words_to_data::courtlistener::{ClusterRecord, OpinionRecord};
    use words_to_data::dataset::{ExpressionId, WorkId};

    let mut dataset: Dataset<InMemoryStorage> = Dataset::new(DatasetMetadata {
        name: "Title 26, and Shami's citations without Shami".to_string(),
        author: "words_to_data tests".to_string(),
        ..Default::default()
    });
    dataset
        .add_uslm_xml(
            "tests/test_data/usc/2025-07-30/usc26.xml",
            "2025-07-30",
            None,
        )
        .expect("title 26 should parse");
    let title_26 = dataset
        .get_expression(&ExpressionId::new(
            WorkId::new("uscode/title_26"),
            "2025-07-30",
        ))
        .expect("storage should answer")
        .expect("title 26 should be held");
    let mut index = SectionPaths::new();
    index.add_work(&title_26.root);
    let scope = dataset.scope().expect("the scope should derive");

    let read = |name: &str| {
        std::fs::read_to_string(format!("{CACHE}/courtlistener/{name}"))
            .unwrap_or_else(|error| panic!("{name} should be committed: {error}"))
    };
    let record = OpinionRecord::from_json(&read("opinion_2651100.json")).expect("record reads");
    let cluster = ClusterRecord::from_json(&read("cluster_2651100.json")).expect("cluster reads");
    let (text, _, _) = record.text().expect("an opinion carries text");

    let outside = Opinion::new("2651100", cluster.display());
    for citation in usc::find_with_report(&text).0 {
        let cited = resolve(&citation, &scope, &index);
        for link in cites_links(&outside, &citation, &cited) {
            dataset.add_link(link).expect("a link should store");
        }
    }

    let path = format!("{}/cli_citation_unheld.sqlite", env!("CARGO_TARGET_TMPDIR"));
    let _ = std::fs::remove_file(&path);
    dataset
        .save_to_sqlite(&path)
        .expect("the fixture should save");
    path
}

/// A citation from an opinion outside the dataset is still a citation. It is
/// shown with the opinion's id, and the reader is told the dataset does not
/// hold the opinion, so nothing about its text can be checked here.
#[test]
fn should_show_an_opinion_the_dataset_does_not_hold_by_its_id_when_it_cites_the_path() {
    let dataset = title_26_with_shami_cited_from_outside();

    let printed = stdout_of(&run(&["path", &dataset, SECTION_174]));
    let line = printed
        .lines()
        .find(|line| line.contains("judicial.opinion:2651100"))
        .unwrap_or_else(|| panic!("path on § 174 should name the opinion by id, got:\n{printed}"));
    assert!(
        line.contains("not held in this dataset"),
        "the line should say the dataset does not hold the opinion, got: {line}"
    );

    let rows = citations_in_json(&dataset, SECTION_174);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["opinion"], "judicial.opinion:2651100");
    assert_eq!(rows[0]["opinion_held"], false);
}

/// `annotations` stops at its limit and says what it left out. A list of
/// citations that stopped in silence would read as every opinion that cites
/// the section.
#[test]
fn should_stop_at_the_limit_and_say_how_many_citations_it_did_not_show() {
    let printed = stdout_of(&run(&[
        "annotations",
        dataset(),
        "--path",
        SECTION_174,
        "--limit",
        "3",
    ]));
    let shown = printed
        .lines()
        .filter(|line| line.contains("cited by"))
        .count();
    assert_eq!(shown, 3, "three citations are shown, got:\n{printed}");
    assert!(
        printed.contains("11 more not shown"),
        "the run should say eleven were left out, got:\n{printed}"
    );

    let printed = stdout_of(&run(&[
        "annotations",
        dataset(),
        "--path",
        SECTION_174,
        "--limit",
        "3",
        "--json",
    ]));
    let listing: serde_json::Value = serde_json::from_str(&printed).expect("JSON");
    assert_eq!(listing["citations"].as_array().map(Vec::len), Some(3));
    assert_eq!(listing["citations_total"], 14, "{listing}");
}
